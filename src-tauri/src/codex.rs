//! Codex's official stdio app-server. Tokens remain owned by Codex, never by SQLite.
use crate::LockExt;
use crate::{
    auth::AuthStatus,
    catalog::{Catalog, ModelInfo},
    connectors, db,
    engine_child::EngineChild,
    model::Settings,
    process, AppState,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
type Result<T> = std::result::Result<T, String>;

pub struct Rpc {
    pub child: Arc<EngineChild>,
    input: ChildStdin,
    receive: mpsc::Receiver<Value>,
    queued: VecDeque<Value>,
    serial: u64,
}
impl Drop for Rpc {
    fn drop(&mut self) {
        self.child.cancel();
    }
}
pub fn command(settings: &Settings, directory: &Path) -> Result<Command> {
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        if !Path::new(&home).is_dir() {
            return Err("La cartella CODEX_HOME configurata sul PC non esiste. Correggi questa variabile o rimuovila per usare la cartella standard di Codex.".into());
        }
    }
    let mut cmd = Command::new(process::executable(&settings.codex_bin)?);
    crate::document_runtime::configure(&mut cmd);
    cmd.args([
        "app-server",
        "--listen",
        "stdio://",
        "-c",
        "features.shell_tool=true",
        "-c",
        "features.multi_agent=true",
        "-c",
        "features.code_mode.enabled=true",
        "-c",
        "features.apply_patch_freeform=true",
        "-c",
        "features.js_repl=true",
        "-c",
        "web_search=\"live\"",
        "-c",
        "model_provider=\"openai\"",
        "-c",
        "approval_policy=\"on-request\"",
        "-c",
        "approvals_reviewer=\"user\"",
    ])
    .current_dir(directory)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    process::cli_environment(&mut cmd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    Ok(cmd)
}
impl Rpc {
    pub fn start(settings: &Settings, directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let mut child = command(settings, directory)?
            .spawn()
            .map_err(|e| format!("Avvio Codex fallito: {e}"))?;
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let child = Arc::new(EngineChild::new(child)?);
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if let Ok(value) = serde_json::from_str(&line) {
                    if send.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        // No account data, OAuth links or tool results are written to logs.
        std::thread::spawn(move || for _ in BufReader::new(stderr).lines() {});
        let mut rpc = Self {
            child,
            input,
            receive,
            queued: VecDeque::new(),
            serial: 0,
        };
        rpc.call("initialize",json!({"clientInfo":{"name":"parlo","title":"PARLO","version":"0.1.0"},"capabilities":{"experimentalApi":true}}))?;
        rpc.send(json!({"method":"initialized"}))?;
        Ok(rpc)
    }
    fn send(&mut self, value: Value) -> Result<()> {
        writeln!(self.input, "{value}")
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())
    }
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.serial += 1;
        let id = self.serial;
        self.send(json!({"id":id,"method":method,"params":params}))?;
        let started = Instant::now();
        loop {
            let value=self.receive.recv_timeout(Duration::from_secs(45).saturating_sub(started.elapsed())).map_err(|e|match e {mpsc::RecvTimeoutError::Disconnected=>"Codex si è chiuso durante l’avvio. Riprova l’accesso da Impostazioni → Assistente; se persiste, ripristina la configurazione automatica in Avanzate.".to_string(),mpsc::RecvTimeoutError::Timeout=>"Codex non risponde. Verifica installazione, account e connessione.".to_string()})?;
            if value.get("method").is_none() && value["id"] == id {
                if let Some(error) = value.get("error") {
                    return Err(error["message"]
                        .as_str()
                        .unwrap_or("Richiesta Codex non riuscita")
                        .into());
                }
                return Ok(value["result"].clone());
            }
            if value.get("id").is_some() && value.get("method").is_some() {
                self.reject(&value)?;
            } else {
                if self.queued.len() >= 1000 {
                    return Err("Troppi eventi Codex durante l’inizializzazione.".into());
                }
                self.queued.push_back(value);
            }
        }
    }
    fn next(&mut self, timeout: Duration) -> Result<Value> {
        if let Some(value) = self.queued.pop_front() {
            return Ok(value);
        }
        self.receive
            .recv_timeout(timeout)
            .map_err(|_| "Codex ha interrotto la connessione o non ha risposto in tempo.".into())
    }
    fn reject(&mut self, value: &Value) -> Result<()> {
        let result=match value["method"].as_str(){
            Some("item/commandExecution/requestApproval"|"item/fileChange/requestApproval")=>json!({"decision":"decline"}),
            Some("execCommandApproval"|"applyPatchApproval")=>json!({"decision":"denied"}),
            Some("item/permissions/requestApproval")=>json!({"permissions":{},"scope":"turn"}),
            Some("mcpServer/elicitation/request")=>json!({"action":"decline","content":null}),
            Some("item/tool/requestUserInput")=>decline_questions(&value["params"]),
            _=>return self.send(json!({"id":value["id"],"error":{"code":-32601,"message":"Interazione non supportata da PARLO"}}))
        };
        self.send(json!({"id":value["id"],"result":result}))
    }
}
pub fn check(settings: &Settings, directory: &Path) -> Result<AuthStatus> {
    if process::executable(&settings.codex_bin).is_err() {
        return Ok(AuthStatus::default());
    }
    let mut rpc = Rpc::start(settings, directory)?;
    let value = rpc.call("account/read", json!({"refreshToken":false}))?;
    Ok(AuthStatus {
        installed: true,
        logged_in: !value["account"].is_null(),
        ..Default::default()
    })
}
#[derive(Default)]
pub struct AuthManager {
    pub status: Arc<Mutex<AuthStatus>>,
    pub guard: Mutex<()>,
    child: Mutex<Option<Arc<EngineChild>>>,
    generation: Arc<AtomicU64>,
}
impl AuthManager {
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        if let Some(child) = self.child.guard().take() {
            child.cancel();
        }
        *self.status.guard() = AuthStatus::default();
    }
    pub fn login(
        &self,
        app: tauri::AppHandle,
        settings: &Settings,
        directory: &Path,
        force: bool,
    ) -> Result<AuthStatus> {
        let current = self.status.guard().clone();
        if current.pending {
            return Ok(current);
        }
        let mut rpc = Rpc::start(settings, directory)?;
        let account = rpc.call("account/read", json!({"refreshToken":false}))?;
        if !account["account"].is_null() && !force {
            return Ok(AuthStatus {
                installed: true,
                logged_in: true,
                ..Default::default()
            });
        }
        let result = rpc.call("account/login/start", json!({"type":"chatgpt"}))?;
        let url = result["authUrl"]
            .as_str()
            .ok_or("Codex non ha restituito la pagina di accesso")?;
        if !login_url_allowed(url) {
            return Err("Pagina di accesso Codex non valida.".into());
        }
        let login_id = result["loginId"].clone();
        let status = AuthStatus {
            installed: true,
            pending: true,
            url: Some(url.into()),
            ..Default::default()
        };
        *self.status.guard() = status.clone();
        *self.child.guard() = Some(rpc.child.clone());
        let generation = self.generation.clone();
        let version = generation.fetch_add(1, Ordering::SeqCst) + 1;
        let state = self.status.clone();
        let _ = app.emit("codex-auth-changed", &status);
        if let Err(error) = open_login(url) {
            let mut pending = state.guard();
            pending.error = Some(error);
            let _ = app.emit("codex-auth-changed", &*pending);
        }
        std::thread::spawn(move || {
            let result = (|| {
                let deadline = Instant::now() + Duration::from_secs(480);
                loop {
                    if generation.load(Ordering::SeqCst) != version {
                        return Err("Accesso annullato".into());
                    }
                    let value = rpc.next(deadline.saturating_duration_since(Instant::now()))?;
                    if value["method"] == "account/login/completed"
                        && value["params"]["loginId"] == login_id
                    {
                        if value["params"]["success"] == true {
                            return Ok(());
                        }
                        return Err(value["params"]["error"]
                            .as_str()
                            .unwrap_or("Accesso Codex non completato")
                            .to_string());
                    }
                    if value.get("id").is_some() && value.get("method").is_some() {
                        rpc.reject(&value)?;
                    }
                }
            })();
            if generation.load(Ordering::SeqCst) != version {
                return;
            }
            let mut state = state.guard();
            *state = AuthStatus {
                installed: true,
                logged_in: result.is_ok(),
                error: result.err(),
                ..Default::default()
            };
            let _ = app.emit("codex-auth-changed", &*state);
        });
        Ok(status)
    }
}
pub fn login_url_allowed(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.username().is_empty()
            && u.password().is_none()
            && matches!(
                u.host_str(),
                Some("auth.openai.com" | "auth0.openai.com" | "chatgpt.com")
            )
    })
}
pub fn open_login(url: &str) -> Result<()> {
    if !login_url_allowed(url) {
        return Err("Pagina di accesso Codex non valida".into());
    }
    Command::new(if cfg!(windows) {
        "explorer.exe"
    } else {
        "open"
    })
    .arg(url)
    .spawn()
    .map(|_| ())
    .map_err(|e| e.to_string())
}
pub fn models(settings: &Settings, directory: &Path) -> Result<Catalog> {
    let mut rpc = Rpc::start(settings, directory)?;
    let mut models = vec![];
    let mut cursor = Value::Null;
    loop {
        let result = rpc.call(
            "model/list",
            json!({"limit":100,"cursor":cursor,"includeHidden":false}),
        )?;
        for value in result["data"]
            .as_array()
            .ok_or("Catalogo Codex non disponibile")?
        {
            if value["hidden"] == true {
                continue;
            }
            let Some(model) = value["model"].as_str() else {
                continue;
            };
            models.push(ModelInfo {
                value: model.into(),
                display_name: value["displayName"].as_str().unwrap_or(model).into(),
                description: value["description"].as_str().unwrap_or_default().into(),
                resolved_model: Some(model.into()),
            });
        }
        cursor = result["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
        if models.len() > 500 {
            return Err("Catalogo Codex troppo grande".into());
        }
    }
    if models.is_empty() {
        return Err("Codex non ha restituito modelli. Verifica l’account.".into());
    }
    Ok(Catalog {
        models,
        checked_at: chrono::Utc::now().to_rfc3339(),
        cli_version: None,
    })
}
pub fn connectors(settings: &Settings, directory: &Path) -> Result<Vec<connectors::Connector>> {
    if !settings.codex_connectors {
        return Ok(vec![]);
    }
    let mut rpc = Rpc::start(settings, directory)?;
    let mut servers = vec![];
    let mut cursor = Value::Null;
    loop {
        let result = rpc.call("mcpServerStatus/list", json!({"cursor":cursor,"limit":100}))?;
        for value in result["data"]
            .as_array()
            .ok_or("Inventario MCP Codex non disponibile")?
        {
            servers.push(connectors::Connector {
                name: value["name"].as_str().unwrap_or_default().into(),
                status: value["runtimeStatus"]
                    .as_str()
                    .unwrap_or(if value["toolsError"].is_null() {
                        "connected"
                    } else {
                        "failed"
                    })
                    .into(),
                tools: value["tools"].as_object().map_or(0, |v| v.len()),
            });
        }
        cursor = result["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
        if servers.len() > 500 {
            return Err("Inventario MCP Codex troppo grande".into());
        }
    }
    Ok(servers)
}
fn scoped_config(settings: &Settings, value: &Value, servers: &Value) -> Value {
    let effective = settings.connector_settings();
    let mut config = json!({"features.shell_tool":true,"features.multi_agent":true,"features.code_mode.enabled":true,"features.apply_patch_freeform":true,"features.js_repl":true,"web_search":"live","sandbox_workspace_write.network_access":true,"mcp_servers":{},"plugins":{},"apps":{"_default":{"default_tools_approval_mode":"prompt","approvals_reviewer":"user"}}});
    let apps_disabled = !settings.codex_connectors
        || settings
            .codex_disabled_connectors
            .iter()
            .any(|s| s == "codex_apps");
    if apps_disabled {
        config["features.apps"] = json!(false);
        config["apps"]["_default"]["enabled"] = json!(false);
    }
    // Only policy fields are sent back to Codex. Native credentials, transport
    // commands and environment variables stay in its own configuration.
    let policy = |name: &str, tools: &Value| {
        let mut result = json!({"default_tools_approval_mode":"prompt","tools":{}});
        if !effective.claude_connectors || effective.disabled_connectors.iter().any(|s| s == name) {
            result["enabled"] = json!(false);
        }
        if let Some(tools) = tools.as_object() {
            for tool in tools.keys() {
                result["tools"][tool] = json!({"approval_mode":"prompt"});
            }
        }
        result
    };
    if let Some(servers) = value["config"]["mcp_servers"].as_object() {
        for (name, server) in servers {
            config["mcp_servers"][name] = policy(name, &server["tools"]);
        }
    }
    if let Some(apps) = value["config"]["apps"].as_object() {
        for (name, app) in apps {
            let mut policy = policy(name, &app["tools"]);
            policy["approvals_reviewer"] = json!("user");
            if apps_disabled {
                policy["enabled"] = json!(false);
            }
            config["apps"][name] = policy;
        }
    }
    if let Some(servers) = servers.as_array() {
        for server in servers {
            let Some(name) = server["name"].as_str() else {
                continue;
            };
            if let Some(plugin) = server["pluginId"].as_str() {
                config["plugins"][plugin]["mcp_servers"][name] = policy(name, &server["tools"]);
            }
        }
    }
    config
}
fn decline_questions(params: &Value) -> Value {
    let mut answers = serde_json::Map::new();
    if let Some(questions) = params["questions"].as_array() {
        for q in questions {
            if let Some(id) = q["id"].as_str() {
                let label = q["options"]
                    .as_array()
                    .and_then(|opts| {
                        opts.iter().find_map(|o| {
                            o["label"].as_str().filter(|v| {
                                v.eq_ignore_ascii_case("decline")
                                    || v.eq_ignore_ascii_case("cancel")
                            })
                        })
                    })
                    .unwrap_or("Decline");
                answers.insert(id.into(), json!({"answers":[label]}));
            }
        }
    }
    json!({"answers":answers})
}
fn approve_questions(params: &Value) -> Option<Value> {
    let mut answers = serde_json::Map::new();
    for q in params["questions"].as_array()? {
        let id = q["id"].as_str()?;
        let options = q["options"].as_array()?;
        if !options.iter().any(|o| {
            o["label"]
                .as_str()
                .is_some_and(|l| l.eq_ignore_ascii_case("decline"))
        }) {
            return None;
        }
        let label = options.iter().find_map(|o| {
            o["label"].as_str().filter(|v| {
                v.eq_ignore_ascii_case("accept")
                    || v.eq_ignore_ascii_case("allow once")
                    || v.eq_ignore_ascii_case("allow")
            })
        })?;
        answers.insert(id.into(), json!({"answers":[label]}));
    }
    Some(json!({"answers":answers}))
}
fn elicitation_tool<'a>(params: &Value, items: &'a HashMap<String, Value>) -> Option<&'a Value> {
    if params["mode"] != "form" || params["_meta"]["codex_approval_kind"] != "mcp_tool_call" {
        return None;
    }
    let schema = &params["requestedSchema"];
    if schema["type"] != "object" || !schema["properties"].as_object()?.is_empty() {
        return None;
    }
    if schema
        .get("required")
        .is_some_and(|v| !v.as_array().is_some_and(|a| a.is_empty()))
    {
        return None;
    }
    if schema
        .as_object()?
        .keys()
        .any(|k| !matches!(k.as_str(), "type" | "properties" | "required"))
    {
        return None;
    }
    let mut matching = items.values().filter(|item| {
        item["type"] == "mcpToolCall"
            && item["status"] == "inProgress"
            && item["server"] == params["serverName"]
            && item["arguments"] == params["_meta"]["tool_params"]
            && item["tool"].as_str().is_some_and(|tool| {
                params["message"]
                    .as_str()
                    .is_some_and(|m| m.contains(&format!("\"{tool}\"")))
            })
    });
    let item = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(item)
}
fn native_approval(value: &Value, items: &HashMap<String, Value>, thread: &str, turn: &str) -> Option<Value> {
    let params = &value["params"];
    if params["threadId"] != thread || params["turnId"] != turn { return None; }
    let (tool, title) = match value["method"].as_str()? {
        "item/commandExecution/requestApproval" => ("exec_command", "Esegui comando"),
        "item/fileChange/requestApproval" => ("apply_patch", "Modifica file"),
        "item/permissions/requestApproval" => ("request_permissions", "Accesso a file o rete"),
        _ => return None,
    };
    let mut input = params.clone();
    if let Some(item) = params["itemId"].as_str().and_then(|id|items.get(id)) {
        for field in ["command", "cwd", "changes"] {
            if !item[field].is_null() { input[field] = item[field].clone(); }
        }
    }
    Some(json!({"request_id":uuid::Uuid::new_v4().to_string(),"request":{"tool_name":tool,"tool_use_id":format!("codex:{}",params["itemId"].as_str().unwrap_or_default()),"title":title,"input":input,"suppress_always_allow_rule":true}}))
}
fn native_approval_result(value: &Value, allowed: bool) -> Value {
    if value["method"] == "item/permissions/requestApproval" {
        json!({"permissions":if allowed {value["params"]["permissions"].clone()} else {json!({})},"scope":"turn"})
    } else {
        json!({"decision":if allowed {"accept"} else {"decline"}})
    }
}
fn question_result(response: &Value) -> Value {
    let mut answers=serde_json::Map::new();
    if response["response"]["response"]["behavior"]=="allow" {
        if let Some(values)=response["response"]["response"]["updatedInput"]["answers"].as_object() {
            for (id,answer) in values { if let Some(text)=answer.as_str() { answers.insert(id.clone(),json!({"answers":[text]})); } }
        }
    }
    json!({"answers":answers})
}
pub fn chat(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    id: &str,
    prompt: &str,
    settings: &Settings,
    context: &str,
    user_index: usize,
) -> Result<()> {
    state.chat_control.start_progress(app, id, user_index, "Codex");
    let directory = crate::chat_files::directory(state, id, true)?;
    let mut rpc = Rpc::start(settings, &directory)?;
    *state.chat_control.process.guard() = Some(rpc.child.clone());
    let value = rpc.call("config/read", json!({"includeLayers":false}))?;
    let mut servers = vec![];
    let mut cursor = Value::Null;
    loop {
        let inventory = rpc.call("mcpServerStatus/list", json!({"cursor":cursor,"limit":100}))?;
        if let Some(data) = inventory["data"].as_array() {
            servers.extend(data.iter().cloned());
        }
        cursor = inventory["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
        if servers.len() > 500 {
            return Err("Inventario MCP troppo grande.".into());
        }
    }
    let mut config = scoped_config(settings, &value, &json!(servers));
    if let Some(mut server) = crate::tts::server_config(settings, &directory)? {
        server["enabled"] = json!(true);
        server["tool_timeout_sec"] = json!(600);
        server["default_tools_approval_mode"] = json!("auto");
        config["mcp_servers"][crate::tts::SERVER] = server;
    }
    let model = if settings.codex_model.is_empty() {
        Value::Null
    } else {
        json!(settings.codex_model)
    };
    let thread=rpc.call("thread/start",json!({"model":model,"cwd":directory,"approvalPolicy":"on-request","approvalsReviewer":"user","sandbox":"workspace-write","ephemeral":true,"baseInstructions":process::system_prompt(settings),"config":config}))?;
    let thread_id = thread["thread"]["id"]
        .as_str()
        .ok_or("Codex non ha creato la chat")?;
    let resolved = thread["model"]
        .as_str()
        .unwrap_or(&settings.codex_model)
        .to_owned();
    let turn=rpc.call("turn/start",json!({"threadId":thread_id,"input":[{"type":"text","text":context,"text_elements":[]}],"summary":"auto"}))?;
    let turn_id=turn["turn"]["id"].as_str().ok_or("Codex non ha creato il turno")?.to_owned();
    db::append_message(&state.db.guard(), id, "user", prompt.trim())?;
    let _ = app.emit("calls-changed", ());
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut items = HashMap::<String, Value>::new();
    loop {
        if state.chat_control.cancelled.load(Ordering::SeqCst) {
            return Err("Risposta interrotta.".into());
        }
        let value = rpc.next(deadline.saturating_duration_since(Instant::now()))?;
        if matches!(
            value["method"].as_str(),
            Some("item/started" | "item/completed")
        ) {
            if let Some(item_id) = value["params"]["item"]["id"].as_str() {
                items.insert(item_id.into(), value["params"]["item"].clone());
            }
        }
        if let Some(t) = state.chat_control.progress.guard().as_mut() {
            t.observe_codex(&value);
        }
        let _ = app.emit("chat-progress", state.chat_control.progress());
        if value["method"] == "item/agentMessage/delta" || value["method"] == "item/completed" {
            let answer = state
                .chat_control
                .progress()
                .map(|r| {
                    r.entries
                        .iter()
                        .filter(|e| e.kind == "text")
                        .map(|e| e.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n")
                })
                .unwrap_or_default();
            let _ = app.emit("chat-delta", json!({"id":id,"text":answer}));
        }
        if value.get("id").is_some() && value.get("method").is_some() {
            if let Some(request) = native_approval(&value, &items, thread_id, &turn_id) {
                let latest = db::get_settings(&state.db.guard())?.connector_settings();
                let response = state.chat_control.request(app, id, &latest, &request);
                let allowed = response["response"]["response"]["behavior"] == "allow";
                rpc.send(json!({"id":value["id"],"result":native_approval_result(&value,allowed)}))?;
            } else if value["method"] == "item/tool/requestUserInput" {
                let params = &value["params"];
                let item = items.get(params["itemId"].as_str().unwrap_or_default());
                if let (Some(item), Some(approved)) = (
                    item.filter(|i| i["type"] == "mcpToolCall"),
                    approve_questions(params),
                ) {
                    let tool = format!(
                        "mcp__{}__{}",
                        item["server"].as_str().unwrap_or_default(),
                        item["tool"].as_str().unwrap_or_default()
                    );
                    let latest = db::get_settings(&state.db.guard())?.connector_settings();
                    let response=state.chat_control.request(app,id,&latest,&json!({"request_id":uuid::Uuid::new_v4().to_string(),"request":{"tool_name":tool,"input":item["arguments"],"mcp_server":{"name":item["server"]},"title":params["questions"][0]["question"],"suppress_always_allow_rule":item["readOnlyHint"]!=true}}));
                    let allowed = response["response"]["response"]["behavior"] == "allow";
                    rpc.send(json!({"id":value["id"],"result":if allowed{approved}else{decline_questions(params)}}))?;
                } else {
                    if params["threadId"]==thread_id && params["turnId"]==turn_id && params["questions"].as_array().is_some_and(|q|!q.is_empty()) && item.is_none_or(|i|i["type"]!="mcpToolCall") {
                        let latest=db::get_settings(&state.db.guard())?.connector_settings();
                        let response=state.chat_control.request(app,id,&latest,&json!({"request_id":uuid::Uuid::new_v4().to_string(),"request":{"tool_name":"request_user_input","title":"Domande dell’assistente","input":params,"suppress_always_allow_rule":true}}));
                        rpc.send(json!({"id":value["id"],"result":question_result(&response)}))?;
                    } else { rpc.reject(&value)?; }
                }
            } else if value["method"] == "mcpServer/elicitation/request" {
                if let Some(item) = elicitation_tool(&value["params"], &items) {
                    let tool = format!(
                        "mcp__{}__{}",
                        item["server"].as_str().unwrap_or_default(),
                        item["tool"].as_str().unwrap_or_default()
                    );
                    let latest = db::get_settings(&state.db.guard())?.connector_settings();
                    let response=state.chat_control.request(app,id,&latest,&json!({"request_id":uuid::Uuid::new_v4().to_string(),"request":{"tool_name":tool,"input":item["arguments"],"tool_use_id":format!("codex:{}",item["id"].as_str().unwrap_or_default()),"mcp_server":{"name":item["server"]},"title":value["params"]["message"],"suppress_always_allow_rule":item["readOnlyHint"]!=true}}));
                    let allowed = response["response"]["response"]["behavior"] == "allow";
                    rpc.send(json!({"id":value["id"],"result":{"action":if allowed{"accept"}else{"decline"},"content":if allowed{json!({})}else{Value::Null},"_meta":null}}))?;
                } else {
                    rpc.reject(&value)?;
                }
            } else {
                rpc.reject(&value)?;
            }
        }
        if value["method"] == "turn/completed" {
            match value["params"]["turn"]["status"].as_str() {
                Some("completed") => break,
                Some("interrupted") => return Err("Risposta interrotta.".into()),
                _ => {
                    return Err(value["params"]["turn"]["error"]["message"]
                        .as_str()
                        .unwrap_or("Codex non ha completato la risposta")
                        .into())
                }
            }
        }
        if value["method"] == "error" && value["params"]["willRetry"] != true {
            return Err(value["params"]["error"]["message"]
                .as_str()
                .unwrap_or("Errore Codex")
                .into());
        }
    }
    let answer = state
        .chat_control
        .progress()
        .map(|r| {
            r.entries
                .iter()
                .filter(|e| e.kind == "text")
                .map(|e| e.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default();
    if answer.trim().is_empty() {
        return Err("Codex non ha restituito una risposta.".into());
    }
    db::append_response(&state.db.guard(), id, &answer, &resolved)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_mcp_confirmation_requires_matching_active_tool_and_empty_form() {
        let mut items = HashMap::new();
        items.insert("item".into(),json!({"id":"item","type":"mcpToolCall","status":"inProgress","server":"test","tool":"read","arguments":{}}));
        let mut params = json!({"serverName":"test","mode":"form","_meta":{"codex_approval_kind":"mcp_tool_call","tool_params":{}},"message":"Allow tool \"read\"?","requestedSchema":{"type":"object","properties":{}}});
        assert!(elicitation_tool(&params, &items).is_some());
        params["requestedSchema"]["properties"] = json!({"password":{"type":"string"}});
        assert!(elicitation_tool(&params, &items).is_none());
        params["requestedSchema"]["properties"] = json!({});
        params["_meta"]["codex_approval_kind"] = json!("server_form");
        assert!(elicitation_tool(&params, &items).is_none());
        params["_meta"]["codex_approval_kind"] = json!("mcp_tool_call");
        items.get_mut("item").unwrap()["status"] = json!("completed");
        assert!(elicitation_tool(&params, &items).is_none());
    }
    #[test]
    fn login_url_rejects_injection_and_other_domains() {
        assert!(login_url_allowed(
            "https://auth.openai.com/oauth/authorize?state=abc"
        ));
        for url in [
            "https://auth.openai.com.evil.test/",
            "http://auth.openai.com/",
            "https://secret@auth.openai.com/",
        ] {
            assert!(!login_url_allowed(url));
        }
    }
    #[test]
    fn approvals_accept_only_explicit_consent_questions() {
        let q =
            json!({"questions":[{"id":"q","options":[{"label":"Accept"},{"label":"Decline"}]}]});
        assert_eq!(
            approve_questions(&q).unwrap()["answers"]["q"]["answers"][0],
            "Accept"
        );
        assert_eq!(
            decline_questions(&q)["answers"]["q"]["answers"][0],
            "Decline"
        );
        assert!(
            approve_questions(&json!({"questions":[{"id":"q","options":[{"label":"Yes"}]}]}))
                .is_none()
        );
    }
    #[test]
    fn native_tools_are_enabled_while_disabled_connectors_stay_disabled() {
        let settings = Settings {
            assistant_provider: "codex".into(),
            codex_disabled_connectors: vec!["drive".into(), "native".into(), "codex_apps".into()],
            ..Default::default()
        };
        let config = scoped_config(
            &settings,
            &json!({"config":{"mcp_servers":{"drive":{"url":"https://secret.test/mcp","env":{"TOKEN":"secret"},"tools":{"read":{"approval_mode":"auto"}}}}}}),
            &json!([{"name":"native","pluginId":"native@official","tools":{"files/read":{}}}]),
        );
        assert_eq!(config["mcp_servers"]["drive"]["enabled"], false);
        assert_eq!(
            config["mcp_servers"]["drive"]["tools"]["read"]["approval_mode"],
            "prompt"
        );
        assert_eq!(
            config["plugins"]["native@official"]["mcp_servers"]["native"]["enabled"],
            false
        );
        assert_eq!(
            config["plugins"]["native@official"]["mcp_servers"]["native"]["tools"]["files/read"]
                ["approval_mode"],
            "prompt"
        );
        for feature in ["features.shell_tool","features.multi_agent","features.code_mode.enabled","features.apply_patch_freeform","features.js_repl"] {
            assert_eq!(config[feature], true);
        }
        assert_eq!(config["web_search"], "live");
        assert_eq!(config["features.apps"], false);
        assert!(!config.to_string().contains("secret"));
    }
    #[test]
    fn native_approvals_are_scoped_and_expose_command_and_file_changes() {
        let mut items=HashMap::new();
        items.insert("edit".into(),json!({"changes":[{"path":"/work/test.txt","diff":"+hello"}]}));
        for (method,tool) in [("item/commandExecution/requestApproval","exec_command"),("item/fileChange/requestApproval","apply_patch"),("item/permissions/requestApproval","request_permissions")] {
            let value=json!({"method":method,"params":{"threadId":"thread","turnId":"turn","itemId":"edit","command":"echo hello","permissions":{"network":{"enabled":true}}}});
            let request=native_approval(&value,&items,"thread","turn").unwrap();
            assert_eq!(request["request"]["tool_name"],tool);
            assert_eq!(request["request"]["input"]["changes"][0]["path"],"/work/test.txt");
            assert_eq!(request["request"]["suppress_always_allow_rule"],true);
            assert!(native_approval(&value,&items,"other","turn").is_none());
            assert!(native_approval(&value,&items,"thread","old").is_none());
            if tool=="request_permissions" {
                assert_eq!(native_approval_result(&value,true)["permissions"],value["params"]["permissions"]);
                assert_eq!(native_approval_result(&value,false)["permissions"],json!({}));
                assert_eq!(native_approval_result(&value,true)["scope"],"turn");
            } else {
                assert_eq!(native_approval_result(&value,true)["decision"],"accept");
                assert_eq!(native_approval_result(&value,false)["decision"],"decline");
            }
        }
        assert!(native_approval(&json!({"method":"item/tool/call","params":{"threadId":"thread","turnId":"turn"}}),&items,"thread","turn").is_none());
    }
    #[test]
    fn user_questions_return_only_explicit_answers() {
        let response=connectors::permission_response("q",true,&json!({"answers":{"format":"PDF"}}));
        assert_eq!(question_result(&response),json!({"answers":{"format":{"answers":["PDF"]}}}));
        assert_eq!(question_result(&connectors::permission_response("q",false,&json!({"answers":{"format":"PDF"}}))),json!({"answers":{}}));
    }
}
