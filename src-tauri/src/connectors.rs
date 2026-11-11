//! Claude Code control protocol: connector metadata and explicit per-call tool consent.
use crate::LockExt;
use crate::{
    activity::{ChatRun, Tracker},
    db,
    model::Settings,
    process,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::Stdio,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::Emitter;

pub fn tool_prefix(server: &str) -> String {
    let name: String = server
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("mcp__{name}__")
}
pub fn tool_enabled(settings: &Settings, request: &Value) -> bool {
    let tool = request["tool_name"].as_str().unwrap_or_default();
    if settings.assistant_provider == "ollama" && !settings.ollama_tools_enabled { return false; }
    if settings.assistant_provider == "custom" {
        return settings.custom_chat.tools_enabled && crate::custom_endpoint::is_tool(tool);
    }
    if !tool.starts_with("mcp__") {
        if settings.assistant_provider=="ollama" && tool=="WebSearch" {return false;}
        if settings.assistant_provider=="ollama" && tool=="WebFetch" {return settings.ollama_web_search;}
        return valid_tool(tool);
    }
    if crate::tts::is_tool(tool) { return settings.tts_enabled; }
    if crate::web_tools::is_tool(tool) {return settings.assistant_provider=="ollama" && settings.ollama_web_search;}
    if !settings.claude_connectors {
        return false;
    }
    let server = request["mcp_server"]["name"].as_str();
    !settings
        .disabled_connectors
        .iter()
        .any(|name| server == Some(name.as_str()) || tool.starts_with(&tool_prefix(name)))
}
pub fn eligible_tool(settings: &Settings, request: &Value) -> bool {
    valid_tool(request["tool_name"].as_str().unwrap_or_default())
        && tool_enabled(settings, request)
        && (!request["requires_user_interaction"].as_bool().unwrap_or(false)
            || question_tool(request["tool_name"].as_str().unwrap_or_default()))
}
pub fn question_tool(tool: &str) -> bool { matches!(tool, "AskUserQuestion" | "request_user_input") }
fn wait_for_permission(receive: &mpsc::Receiver<bool>, cancelled: &std::sync::atomic::AtomicBool) -> bool {
    // Reading a question or a permission card is not a refusal. Wait for a real
    // decision; keep checking cancellation, including before pending was set.
    loop {
        if cancelled.load(std::sync::atomic::Ordering::SeqCst) { return false; }
        match receive.recv_timeout(Duration::from_millis(250)) {
            Ok(allow) => return allow,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => return false,
        }
    }
}
fn validated_answers(input: &Value, answers: Option<&Value>) -> Result<Value, String> {
    let answers=answers.and_then(Value::as_object).ok_or("Rispondi alle domande prima di continuare.")?;
    let questions=input["questions"].as_array().filter(|q|!q.is_empty()).ok_or("Domande non valide.")?;
    let mut result=serde_json::Map::new();
    for question in questions {
        let key=question["id"].as_str().or_else(||question["question"].as_str()).ok_or("Domanda non valida.")?;
        let answer=answers.get(key).and_then(Value::as_str).filter(|s|!s.trim().is_empty()).ok_or("Rispondi a tutte le domande prima di continuare.")?;
        result.insert(key.into(),json!(answer));
    }
    Ok(Value::Object(result))
}
pub fn valid_tool(tool: &str) -> bool {
    if !tool.starts_with("mcp__") {
        return !tool.is_empty() && tool.len() <= 128
            && tool.as_bytes()[0].is_ascii_alphabetic()
            && tool.bytes().all(|c|c.is_ascii_alphanumeric() || c == b'_');
    }
    tool.strip_prefix("mcp__")
        .and_then(|s| s.rsplit_once("__"))
        .is_some_and(|(server, name)| !server.is_empty() && !name.is_empty())
        && tool.len() <= 512
        && !tool
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '*' || c == '?')
}
pub fn tool_trusted(settings: &Settings, request: &Value) -> bool {
    if request["tool_name"].as_str().is_some_and(crate::tts::is_tool) {
        // Enabling the integrated voice tool authorizes its use in chat.
        return eligible_tool(settings, request);
    }
    eligible_tool(settings, request)
        && !question_tool(request["tool_name"].as_str().unwrap_or_default())
        && !request["suppress_always_allow_rule"]
            .as_bool()
            .unwrap_or(false)
        && settings
            .trusted_tools
            .iter()
            .any(|tool| request["tool_name"] == tool.as_str())
}

#[derive(Clone, Serialize, Debug)]
pub struct Connector {
    pub name: String,
    pub status: String,
    pub tools: usize,
}
pub fn sanitize_servers(value: &Value) -> Vec<Connector> {
    value
        .as_array()
        .map(|servers| {
            servers
                .iter()
                .filter_map(|server| {
                    Some(Connector {
                        name: server["name"].as_str()?.to_string(),
                        status: server["status"].as_str().unwrap_or("pending").into(),
                        tools: server["tools"].as_array().map_or(0, Vec::len),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
fn discovery_complete(servers: &[Connector], elapsed: Duration) -> bool {
    // Empty startup status is not an authoritative empty account: Claude may
    // retry its network discovery in the background, especially after login.
    // The integrated TTS server connects immediately and says nothing about
    // whether Claude has finished discovering the account's connectors.
    let mut external = servers.iter().filter(|s| s.name != crate::tts::SERVER).peekable();
    let ready = external.peek().is_some() && external.all(|s| s.status != "pending");
    ready || elapsed >= Duration::from_secs(20)
}
fn discovery_complete_for(settings: &Settings, servers: &[Connector], elapsed: Duration) -> bool {
    if settings.claude_connectors { return discovery_complete(servers, elapsed); }
    // The explicitly configured integrated tool needs no cloud-account discovery.
    let mut expected=Vec::new();
    if settings.tts_enabled {expected.push(crate::tts::SERVER);}
    if crate::web_tools::available(settings) {expected.push(crate::web_tools::SERVER);}
    expected.iter().all(|name|servers.iter().any(|server|server.name==*name && server.status!="pending")) || elapsed >= Duration::from_secs(12)
}
pub fn fetch(settings: &Settings, directory: &Path) -> Result<Vec<Connector>, String> {
    let effective = settings.connector_settings();
    let settings = &effective;
    if !settings.claude_connectors && !settings.tts_enabled && !crate::web_tools::available(settings) {
        return Ok(vec![]);
    }
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let mut command = process::chat_command(settings, directory)?;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut input = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let _ = send.send(value);
            }
        }
    });
    let result = (|| {
        let mut servers = vec![];
        let deadline = Instant::now() + Duration::from_secs(70);
        let mut discovery_started = None;
        for attempt in 0..=11 {
            let request = if attempt == 0 { json!({"subtype":"initialize","hooks":null}) }
                else { json!({"subtype":"mcp_status"}) };
            let id = uuid::Uuid::new_v4().to_string();
            writeln!(
                input,
                "{}",
                json!({"type":"control_request","request_id":id,"request":request})
            )
            .map_err(|e| e.to_string())?;
            input.flush().map_err(|e| e.to_string())?;
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now()).min(Duration::from_secs(35));
                let value = receive.recv_timeout(remaining).map_err(|_| {
                    "I connettori non hanno risposto. Verifica account e connessione.".to_string()
                })?;
                if value["type"] == "control_response" && value["response"]["request_id"] == id {
                    if value["response"]["subtype"] == "error" {
                        return Err(
                            "Claude Code non ha completato la verifica dei connettori.".into()
                        );
                    }
                    if request["subtype"] == "mcp_status" {
                        let metadata = &value["response"]["response"]["mcpServers"];
                        if !metadata.is_array() {
                            return Err("Claude non ha restituito una lista MCP valida. Riprova la verifica.".into());
                        }
                        servers = sanitize_servers(metadata);
                        if !settings.claude_connectors { servers.retain(|server| server.name == crate::tts::SERVER || server.name == crate::web_tools::SERVER); }
                    }
                    break;
                }
            }
            if attempt == 0 { discovery_started = Some(Instant::now()); continue; }
            if discovery_complete_for(settings, &servers, discovery_started.unwrap().elapsed()) {
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        if !settings.claude_connectors && settings.tts_enabled && !servers.iter().any(|server|server.name==crate::tts::SERVER) {
            return Err("Il TTS è abilitato ma la CLI non ha restituito il suo collegamento MCP. Aggiorna la verifica; nessuna sintesi è stata avviata.".into());
        }
        Ok(servers)
    })();
    drop(input);
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod discovery_tests {
    use super::*;
    #[test]
    fn empty_or_pending_status_waits_for_background_discovery() {
        assert!(!discovery_complete(&[], Duration::from_secs(6)));
        assert!(discovery_complete(&[], Duration::from_secs(20)));
        let pending = vec![Connector{name:"Account MCP".into(), status:"pending".into(), tools:0}];
        assert!(!discovery_complete(&pending, Duration::from_secs(10)));
        let connected = vec![Connector{name:"Account MCP".into(), status:"connected".into(), tools:2}];
        assert!(discovery_complete(&connected, Duration::ZERO));
    }
    #[test]
    fn integrated_tts_does_not_finish_account_connector_discovery() {
        let tts = || Connector {name:crate::tts::SERVER.into(),status:"connected".into(),tools:3};
        assert!(!discovery_complete(&[tts()], Duration::from_secs(6)));
        assert!(discovery_complete(&[tts()], Duration::from_secs(20)));
        let account = |status:&str| Connector {name:"Account MCP".into(),status:status.into(),tools:2};
        assert!(!discovery_complete(&[tts(),account("pending")], Duration::from_secs(10)));
        assert!(discovery_complete(&[tts(),account("connected")], Duration::from_secs(10)));
    }
    #[test]
    fn ollama_reports_actual_integrated_tts_connection_without_waiting_for_cloud() {
        let settings = Settings {assistant_provider:"ollama".into(),tts_enabled:true,ollama_web_search:false,..Default::default()}.connector_settings();
        assert!(!discovery_complete_for(&settings, &[], Duration::from_secs(2)));
        let server = |status:&str| Connector {name:crate::tts::SERVER.into(),status:status.into(),tools:3};
        assert!(!discovery_complete_for(&settings, &[server("pending")], Duration::from_secs(2)));
        assert!(discovery_complete_for(&settings, &[server("connected")], Duration::from_secs(2)));
        assert!(discovery_complete_for(&settings, &[server("failed")], Duration::from_secs(2)));
        assert!(discovery_complete_for(&settings, &[], Duration::from_secs(12)));
    }
}

#[derive(Clone, Serialize)]
pub struct Permission {
    pub id: String,
    pub call_id: String,
    pub tool: String,
    pub title: String,
    pub input: Value,
    pub server: Option<String>,
    pub can_always: bool,
}
pub struct Pending {
    pub permission: Permission,
    pub answer: mpsc::Sender<bool>,
}
#[derive(Default)]
pub struct ChatControl {
    pub cancelled: std::sync::atomic::AtomicBool,
    pub process: Mutex<Option<Arc<crate::engine_child::EngineChild>>>,
    pub pending: Mutex<Option<Pending>>,
    answers: Mutex<Option<(String, Value)>>,
    pub progress: Mutex<Option<Tracker>>,
}
impl ChatControl {
    pub fn progress(&self) -> Option<ChatRun> {
        self.progress
            .guard()
            .as_ref()
            .map(|t| t.run.clone())
    }
    pub fn start_progress(&self, app: &tauri::AppHandle, call_id: &str, user_index: usize, provider: &str) {
        let mut tracker = Tracker::new(call_id, user_index);
        if let Some(entry) = tracker.run.entries.first_mut() {
            entry.label = if provider == "Ollama" {
                crate::locale::tr("Avvio assistente Ollama", "Starting Ollama assistant").into()
            } else {
                format!("Connessione a {provider}")
            };
        }
        *self.progress.guard() = Some(tracker);
        let _ = app.emit("chat-progress", self.progress());
    }
    pub fn observe(&self, app: &tauri::AppHandle, value: &Value) {
        if let Some(t) = self.progress.guard().as_mut() {
            t.observe(value);
        }
        let _ = app.emit("chat-progress", self.progress());
    }
    pub fn reconcile_answer(&self, app: &tauri::AppHandle, answer: &str) {
        if let Some(t) = self.progress.guard().as_mut() {
            t.reconcile_answer(answer);
        }
        let _ = app.emit("chat-progress", self.progress());
    }
    pub(crate) fn tool_status(&self, app: &tauri::AppHandle, request: &Value, status: &str) {
        if let Some(t) = self.progress.guard().as_mut() {
            t.permission(request, status);
        }
        let _ = app.emit("chat-progress", self.progress());
    }
    pub fn finish_progress(&self, app: &tauri::AppHandle, error: Option<&str>) -> Option<ChatRun> {
        if let Some(t) = self.progress.guard().as_mut() {
            t.finish(
                error,
                self.cancelled.load(std::sync::atomic::Ordering::SeqCst),
            );
        }
        let run = self.progress();
        let _ = app.emit("chat-progress", &run);
        run
    }
    pub fn current(&self) -> Option<Permission> {
        self.pending
            .guard()
            .as_ref()
            .map(|p| p.permission.clone())
    }
    #[cfg(test)]
    pub fn decide(&self, id: &str, allow: bool) -> Result<(), String> {
        let mut pending = self.pending.guard();
        let current = pending
            .as_ref()
            .ok_or("Questa richiesta non è più attiva.")?;
        if current.permission.id != id {
            return Err("La richiesta di autorizzazione è cambiata.".into());
        }
        let current = pending.take().unwrap();
        current
            .answer
            .send(allow)
            .map_err(|_| "Richiesta terminata".into())
    }
    pub fn decide_persisted(
        &self,
        conn: &rusqlite::Connection,
        id: &str,
        allow: bool,
        always: bool,
    ) -> Result<Settings, String> {
        self.decide_with_answers(conn,id,allow,always,None)
    }
    pub fn decide_with_answers(&self, conn:&rusqlite::Connection, id:&str, allow:bool, always:bool, answers:Option<Value>) -> Result<Settings,String> {
        let mut pending = self.pending.guard();
        let current = pending
            .as_ref()
            .ok_or("Questa richiesta non è più attiva.")?;
        if current.permission.id != id || self.cancelled.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("La richiesta di autorizzazione è cambiata.".into());
        }
        let is_question=question_tool(&current.permission.tool);
        let answer=if allow && is_question {Some(validated_answers(&current.permission.input,answers.as_ref())?)} else {None};
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let mut settings = db::get_settings(&tx)?;
        let request = json!({"tool_name":current.permission.tool,"mcp_server":{"name":current.permission.server}});
        if always
            && (!allow || is_question || !current.permission.can_always || !eligible_tool(&settings.connector_settings(), &request))
        {
            return Err("Questo strumento non può essere autorizzato in modo permanente.".into());
        }
        if always && !settings.active_trusted_mut().contains(&current.permission.tool) {
            settings.active_trusted_mut().push(current.permission.tool.clone());
            db::save_settings(&tx, &settings)?;
        }
        let current = pending.take().unwrap();
        *self.answers.guard()=answer.map(|a|(id.into(),a));
        current
            .answer
            .send(allow && eligible_tool(&settings.connector_settings(), &request))
            .map_err(|_| "Richiesta terminata".to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(settings)
    }
    pub fn cancel(&self) {
        self.answers.guard().take();
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(pending) = self.pending.guard().take() {
            let _ = pending.answer.send(false);
        }
        if let Some(child) = self.process.guard().as_ref() { child.cancel(); }
    }
    pub fn finish(&self, app: &tauri::AppHandle) {
        self.answers.guard().take();
        self.pending.guard().take();
        self.process.guard().take();
        let _ = app.emit("chat-permission", Option::<Permission>::None);
    }
    pub fn request(
        &self,
        app: &tauri::AppHandle,
        call_id: &str,
        settings: &Settings,
        value: &Value,
    ) -> Value {
        let id = value["request_id"].as_str().unwrap_or_default();
        let request = &value["request"];
        let tool = request["tool_name"].as_str().unwrap_or_default();
        // ToolSearch discovers tool descriptions; it does not execute an MCP operation.
        if tool == "ToolSearch" {
            return permission_response(id, true, &request["input"]);
        }
        let eligible = eligible_tool(settings, request);
        let allowed = if tool_trusted(settings, request) {
            true
        } else if eligible {
            let (send, receive) = mpsc::channel();
            let permission = Permission {
                id: id.into(),
                call_id: call_id.into(),
                tool: tool.into(),
                title: request["title"]
                    .as_str()
                    .unwrap_or("Uso di uno strumento")
                    .into(),
                input: request["input"].clone(),
                server: request["mcp_server"]["name"].as_str().map(str::to_owned),
                can_always: !request["suppress_always_allow_rule"]
                    .as_bool()
                    .unwrap_or(false) && !question_tool(tool),
            };
            *self.pending.guard() = Some(Pending {
                permission: permission.clone(),
                answer: send,
            });
            self.tool_status(app, request, "waiting");
            let _ = app.emit("chat-permission", Some(permission));
            let result = wait_for_permission(&receive, &self.cancelled);
            self.pending.guard().take();
            let _ = app.emit("chat-permission", Option::<Permission>::None);
            result
        } else {
            false
        };
        self.tool_status(app, request, if allowed { "running" } else { "denied" });
        let mut input=request["input"].clone();
        if let Some((answer_id,answers))=self.answers.guard().take() {
            if allowed && answer_id==id { input["answers"]=answers; }
        }
        permission_response(id, allowed, &input)
    }
}
pub fn permission_response(id: &str, allow: bool, input: &Value) -> Value {
    let answer = if allow {
        json!({"behavior":"allow","updatedInput":input})
    } else {
        json!({"behavior":"deny","message":"Operazione non autorizzata dall’utente in PARLO."})
    };
    json!({"type":"control_response","response":{"subtype":"success","request_id":id,"response":answer}})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn permission_waits_for_a_decision_and_still_handles_cancellation() {
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let (send, receive) = mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(300));
                send.send(true).unwrap();
            });
            assert!(wait_for_permission(&receive, &cancelled));
        });
        let (_send, receive) = mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| cancelled.store(true, std::sync::atomic::Ordering::SeqCst));
            assert!(!wait_for_permission(&receive, &cancelled));
        });
        cancelled.store(false, std::sync::atomic::Ordering::SeqCst);
        let (send, receive) = mpsc::channel();
        drop(send);
        assert!(!wait_for_permission(&receive, &cancelled));
    }
    #[test]
    fn integrated_tts_consent_depends_on_its_setting() {
        let mut settings=Settings {claude_connectors:false,tts_enabled:true,..Default::default()};
        let request=json!({"tool_name":"mcp__parlo_tts__generate_speech","suppress_always_allow_rule":true});
        assert!(tool_enabled(&settings,&request));
        assert!(tool_trusted(&settings,&request));
        assert!(!tool_trusted(&settings,&json!({"tool_name":"mcp__drive__read"})));
        settings.tts_enabled=false;
        assert!(!tool_enabled(&settings,&request));
        assert!(!tool_trusted(&settings,&request));
    }
    #[test]
    fn questions_require_real_answers_and_never_gain_permanent_consent() {
        let input=json!({"questions":[{"id":"format","question":"Quale formato?"}]});
        assert!(validated_answers(&input,None).is_err());
        assert!(validated_answers(&input,Some(&json!({"format":""}))).is_err());
        assert_eq!(validated_answers(&input,Some(&json!({"format":"PDF","unrelated":"ignored"}))).unwrap(),json!({"format":"PDF"}));
        let claude=json!({"questions":[{"question":"Quale formato?"}]});
        assert_eq!(validated_answers(&claude,Some(&json!({"Quale formato?":"Word"}))).unwrap(),json!({"Quale formato?":"Word"}));
        let mut settings=Settings::default();settings.trusted_tools.push("AskUserQuestion".into());
        assert!(!tool_trusted(&settings,&json!({"tool_name":"AskUserQuestion"})));
        assert!(eligible_tool(&settings,&json!({"tool_name":"AskUserQuestion","requires_user_interaction":true})));
        assert!(!eligible_tool(&settings,&json!({"tool_name":"mcp__form__input","requires_user_interaction":true})));
    }
    #[test]
    fn codex_and_claude_trust_are_isolated_even_for_the_same_tool_name() {
        let conn=db::open(Path::new(":memory:")).unwrap();let control=ChatControl::default();
        let mut settings=db::get_settings(&conn).unwrap();settings.assistant_provider="codex".into();db::save_settings(&conn,&settings).unwrap();
        let (send,receive)=mpsc::channel();*control.pending.guard()=Some(Pending{permission:Permission{id:"codex".into(),call_id:"call".into(),tool:"mcp__drive__read".into(),title:"Read".into(),input:json!({}),server:Some("drive".into()),can_always:true},answer:send});
        let mut settings=control.decide_persisted(&conn,"codex",true,true).unwrap();assert!(receive.recv().unwrap());assert_eq!(settings.codex_trusted_tools,vec!["mcp__drive__read"]);assert!(settings.trusted_tools.is_empty());
        let request=json!({"tool_name":"mcp__drive__read","mcp_server":{"name":"drive"}});assert!(tool_trusted(&settings.connector_settings(),&request));settings.assistant_provider="claude".into();assert!(!tool_trusted(&settings.connector_settings(),&request));settings.assistant_provider="codex".into();settings.codex_disabled_connectors.push("drive".into());assert!(!tool_trusted(&settings.connector_settings(),&request));
    }
    #[test]
    fn permanent_consent_is_exact_revocable_and_never_overrides_disabled_connectors() {
        let conn = db::open(Path::new(":memory:")).unwrap();
        let control = ChatControl::default();
        let (send, receive) = mpsc::channel();
        *control.pending.guard() = Some(Pending {
            permission: Permission {
                id: "active".into(),
                call_id: "call".into(),
                tool: "mcp__drive__read".into(),
                title: "Read".into(),
                input: json!({}),
                server: Some("drive".into()),
                can_always: true,
            },
            answer: send,
        });
        assert!(control
            .decide_persisted(&conn, "stale", true, true)
            .is_err());
        assert!(db::get_settings(&conn).unwrap().trusted_tools.is_empty());
        let mut settings = control
            .decide_persisted(&conn, "active", true, true)
            .unwrap();
        assert!(receive.recv().unwrap());
        assert_eq!(
            db::get_settings(&conn).unwrap().trusted_tools,
            vec!["mcp__drive__read"]
        );
        let request = json!({"tool_name":"mcp__drive__read"});
        assert!(tool_trusted(&settings, &request));
        assert!(!tool_trusted(
            &settings,
            &json!({"tool_name":"mcp__drive__read","suppress_always_allow_rule":true})
        ));
        assert!(!tool_trusted(
            &settings,
            &json!({"tool_name":"mcp__drive__write"})
        ));
        settings.disabled_connectors.push("drive".into());
        assert!(!tool_trusted(&settings, &request));
        settings.disabled_connectors.clear();
        settings.claude_connectors = false;
        assert!(!tool_trusted(&settings, &request));
        settings.claude_connectors = true;
        settings.trusted_tools.clear();
        db::save_settings(&conn, &settings).unwrap();
        assert!(!tool_trusted(&db::get_settings(&conn).unwrap(), &request));
        for tool in ["", "Bash(*)", "mcp__drive__*", "mcp__drive__", "mcp____read"] {
            assert!(!valid_tool(tool));
        }
    }
    #[test]
    fn native_tools_remain_eligible_without_mcp_and_can_be_revoked() {
        let mut settings = Settings { claude_connectors:false, disabled_connectors:vec!["drive".into()], ..Default::default() };
        for tool in ["Bash","Read","Write","Edit","Glob","Grep","WebSearch","WebFetch","Agent","exec_command","apply_patch","request_permissions"] {
            let request = json!({"tool_name":tool});
            assert!(eligible_tool(&settings,&request), "{tool}");
            assert!(!tool_trusted(&settings,&request));
            settings.trusted_tools.push(tool.into());
            assert!(tool_trusted(&settings,&request));
            settings.trusted_tools.clear();
            assert!(!tool_trusted(&settings,&request));
        }
        assert!(!eligible_tool(&settings,&json!({"tool_name":"mcp__drive__read"})));
    }
    #[test]
    fn expired_permission_does_not_persist_trust() {
        let conn = db::open(Path::new(":memory:")).unwrap();
        let control = ChatControl::default();
        let (send, receive) = mpsc::channel();
        drop(receive);
        *control.pending.guard() = Some(Pending {
            permission: Permission {
                id: "expired".into(),
                call_id: "call".into(),
                tool: "mcp__drive__read".into(),
                title: "Read".into(),
                input: json!({}),
                server: None,
                can_always: true,
            },
            answer: send,
        });
        assert!(control
            .decide_persisted(&conn, "expired", true, true)
            .is_err());
        assert!(db::get_settings(&conn).unwrap().trusted_tools.is_empty());
    }
    #[test]
    fn deselected_connectors_are_denied_by_provenance_and_normalized_tool_name() {
        let settings = Settings {
            disabled_connectors: vec!["claude.ai Drive".into()],
            ..Settings::default()
        };
        assert!(!tool_enabled(
            &settings,
            &json!({"tool_name":"mcp__claude_ai_Drive__search"})
        ));
        assert!(!tool_enabled(
            &settings,
            &json!({"tool_name":"mcp__other__search","mcp_server":{"name":"claude.ai Drive"}})
        ));
        assert!(tool_enabled(
            &settings,
            &json!({"tool_name":"mcp__claude_ai_Drive_Extra__search"})
        ));
    }
    #[test]
    fn connector_metadata_does_not_expose_config_or_tokens() {
        let servers = sanitize_servers(
            &json!([{"name":"Account connector","status":"connected","config":{"url":"secret-token-url"},"tools":[{"name":"read"}]}]),
        );
        assert_eq!(servers[0].tools, 1);
        assert!(!serde_json::to_string(&servers)
            .unwrap()
            .contains("secret-token"));
    }
    #[test]
    fn permission_is_once_only_and_default_denial_is_explicit() {
        let input = json!({"title":"Draft"});
        let allowed = permission_response("req", true, &input);
        assert_eq!(allowed["response"]["response"]["updatedInput"], input);
        assert!(allowed["response"]["response"]
            .get("updatedPermissions")
            .is_none());
        assert_eq!(
            permission_response("req", false, &input)["response"]["response"]["behavior"],
            "deny"
        );
        let control = ChatControl::default();
        let (send, receive) = mpsc::channel();
        *control.pending.guard() = Some(Pending {
            permission: Permission {
                id: "req".into(),
                call_id: "call".into(),
                tool: "mcp__server__read".into(),
                title: "Read".into(),
                input,
                server: None,
                can_always: true,
            },
            answer: send,
        });
        assert!(control.decide("wrong", true).is_err());
        control.decide("req", false).unwrap();
        assert!(!receive.recv().unwrap());
        assert!(control.decide("req", true).is_err());
    }
}
