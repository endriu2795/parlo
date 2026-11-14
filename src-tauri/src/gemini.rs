//! Official Antigravity ACP. Legacy `gemini_*` settings and events stay compatible.
use crate::{
    auth::AuthStatus,
    catalog::{Catalog, ModelInfo},
    connectors, db,
    engine_child::EngineChild,
    model::Settings,
    process, AppState, LockExt,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{ChildStdin, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::Emitter;
type Result<T> = std::result::Result<T, String>;
const CHAT_IDLE_TIMEOUT: Duration = Duration::from_secs(120);
struct WarmChat {
    rpc: Rpc,
    session: Value,
    key: String,
    call_id: String,
    next_user_index: usize,
    last_used: Instant,
    context_hash: String,
    snapshot: crate::sources::SourceSnapshot,
}
struct Rpc {
    child: Arc<EngineChild>,
    input: ChildStdin,
    receive: mpsc::Receiver<Value>,
    serial: u64,
}
impl Drop for Rpc {
    fn drop(&mut self) {
        self.child.cancel();
    }
}
impl Rpc {
    fn start(
        settings: &Settings,
        directory: &Path,
        home: &Path,
        url: Option<Arc<dyn Fn(String) + Send + Sync>>,
        background: bool,
    ) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let mut command = crate::gemini_runtime::command(&settings.gemini_bin)?;
        std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        command
            .env("GEMINI_HOME", home)
            .env("AGY_ACP_FORCE_FILE_STORAGE", "1")
            .env_remove("GEMINI_SYSTEM_MD")
            .env_remove("GEMINI_CLI_HOME")
            .current_dir(directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if url.is_some() {
            // Open the browser outside the supervised runtime job. Python invokes
            // this helper without a shell; inherited stdout returns the login URL.
            let executable = std::env::current_exe().map_err(|e| e.to_string())?;
            let executable = executable.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
            command.env("BROWSER", format!("\"{executable}\" --antigravity-login-url %s"));
        }
        if background {
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                use windows::Win32::System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, CREATE_NO_WINDOW};
                command.creation_flags(CREATE_NO_WINDOW.0 | BELOW_NORMAL_PRIORITY_CLASS.0);
            }
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                unsafe {
                    command.pre_exec(|| {
                        libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                        Ok(())
                    });
                }
            }
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Avvio Antigravity fallito: {e}"))?;
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let child = Arc::new(EngineChild::new(child)?);
        let (send, receive) = mpsc::sync_channel(128);
        let output_url = url.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut bytes = Vec::new();
                // Bound malformed protocol frames before allocating unbounded memory.
                let mut limited = (&mut reader).take(2_100_000);
                use std::io::Read;
                let Ok(n) = limited.read_until(b'\n', &mut bytes) else {
                    break;
                };
                if n == 0 || n >= 2_100_000 {
                    break;
                }
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    if let Some(callback) = &output_url {
                        report_login_urls(&value, callback.as_ref());
                    }
                    if send.send(value).is_err() {
                        break;
                    }
                } else if let Some(callback) = &output_url {
                    report_login_text(&String::from_utf8_lossy(&bytes), callback.as_ref());
                }
            }
        });
        std::thread::spawn(move || {
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if let Some(callback) = &url {
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        report_login_urls(&value, callback.as_ref());
                    } else {
                        report_login_text(&line, callback.as_ref());
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            receive,
            serial: 0,
        })
    }
    fn send(&mut self, value: Value) -> Result<()> {
        writeln!(self.input, "{value}")
            .and_then(|_| self.input.flush())
            .map_err(|e| e.to_string())
    }
    fn begin(&mut self, method: &str, params: Value) -> Result<u64> {
        self.serial += 1;
        let id = self.serial;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        Ok(id)
    }
    fn next(&self, deadline: Instant) -> Result<Value> {
        self.receive.recv_timeout(deadline.saturating_duration_since(Instant::now())).map_err(|e| match e { mpsc::RecvTimeoutError::Timeout => "Antigravity non ha risposto in tempo. Verifica account e connessione.".into(), _ => "Antigravity ha interrotto la connessione. Verifica il percorso del CLI nelle impostazioni avanzate.".into() })
    }
    fn reject(&mut self, value: &Value) -> Result<()> {
        if value["method"] == "session/request_permission" {
            self.send(json!({"jsonrpc":"2.0","id":value["id"],"result":{"outcome":{"outcome":"cancelled"}}}))
        } else {
            self.send(json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32601,"message":"Interazione non supportata da PARLO"}}))
        }
    }
    fn call(&mut self, method: &str, params: Value, timeout: u64) -> Result<Value> {
        let id = self.begin(method, params)?;
        let deadline = Instant::now() + Duration::from_secs(timeout);
        loop {
            let value = self.next(deadline)?;
            if value.get("method").is_none() && value["id"] == id {
                return response(&value);
            }
            if value.get("method").is_some() && value.get("id").is_some() {
                self.reject(&value)?;
            }
        }
    }
    fn initialize(&mut self, timeout: u64) -> Result<Value> {
        let value = self.call("initialize", json!({"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"parlo","version":env!("CARGO_PKG_VERSION")}}), timeout)?;
        if value["agentInfo"]["name"] != "antigravity-acp" || value["protocolVersion"] != 1 {
            return Err("Il programma configurato non è Antigravity ACP compatibile. Ripristina il percorso nelle impostazioni avanzate e ricollega l’account.".into());
        }
        if value["agentCapabilities"]["_meta"]["antigravity"]["systemPrompt"]["append"] != true {
            return Err("Questa versione di Antigravity non supporta le istruzioni personalizzate di PARLO. Usa il runtime incluso.".into());
        }
        Ok(value)
    }
    fn session(&mut self, directory: &Path, settings: &Settings, tools: bool, timeout: u64) -> Result<Value> {
        self.call("session/new", session_params(directory, settings, tools)?, timeout)
    }
}
fn session_params(directory: &Path, settings: &Settings, tools: bool) -> Result<Value> {
        let servers = if tools && settings.gemini_connectors {
            configured_servers(directory)?
                .into_iter()
                .filter(|s| !settings.gemini_disabled_connectors.contains(&s.name))
                .filter_map(|s| s.wire)
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let mut params = json!({"cwd":directory,"mcpServers":servers});
        if tools {
            params["_meta"] =
                json!({"antigravity":{"systemPrompt":{"append":process::system_prompt(settings)}}});
        }
        Ok(params)
}
fn response(value: &Value) -> Result<Value> {
    if let Some(error) = value.get("error") {
        return Err(error["message"]
            .as_str()
            .unwrap_or("Richiesta Antigravity non riuscita")
            .chars()
            .take(2000)
            .collect());
    }
    Ok(value["result"].clone())
}
fn home_for(directory: &Path) -> PathBuf {
    directory
        .parent()
        .unwrap_or(directory)
        .join("antigravity-home")
}
fn has_saved_login(directory: &Path) -> bool {
    home_for(directory)
        .join("antigravity-acp/acp_token.json")
        .is_file()
}
pub fn check(settings: &Settings, directory: &Path) -> Result<AuthStatus> {
    // Never launch an auth-capable server when merely reopening the app. Antigravity
    // can open a browser from session/new if a refresh token is no longer usable.
    let installed = crate::gemini_runtime::command(&settings.gemini_bin).is_ok();
    Ok(AuthStatus {
        installed,
        logged_in: installed && has_saved_login(directory),
        ..Default::default()
    })
}
#[derive(Default)]
pub struct AuthManager {
    pub status: Arc<Mutex<AuthStatus>>,
    child: Mutex<Option<Arc<EngineChild>>>,
    generation: AtomicU64,
    guard: Mutex<()>,
    runtime_guard: Mutex<()>,
    warm_chat: Mutex<Option<WarmChat>>,
}
impl AuthManager {
    pub fn discard_chat(&self) {
        self.warm_chat.guard().take();
    }
    pub fn discard_call(&self, id: &str) {
        let mut cached = self.warm_chat.guard();
        if cached.as_ref().is_some_and(|chat| chat.call_id == id) { cached.take(); }
    }
    pub fn cancel(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.discard_chat();
        if let Some(child) = self.child.guard().take() {
            child.cancel();
        }
        *self.status.guard() = AuthStatus::default();
    }
    pub fn login(app: tauri::AppHandle, state: Arc<AppState>, force: bool) -> Result<AuthStatus> {
        if !crate::gemini_runtime::SUPPORTED { return Err(crate::gemini_runtime::unavailable()); }
        let _guard = state.gemini_auth.guard.guard();
        let current = state.gemini_auth.status.guard().clone();
        if current.pending {
            return Ok(current);
        }
        let generation = state.gemini_auth.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let pending = AuthStatus {
            pending: true,
            ..Default::default()
        };
        *state.gemini_auth.status.guard() = pending.clone();
        let worker = state.clone();
        std::thread::spawn(move || {
            let manager = &worker.gemini_auth;
            let publish = |status: AuthStatus| {
                if manager.generation.load(Ordering::SeqCst) == generation {
                    *manager.status.guard() = status.clone();
                    let _ = app.emit("gemini-auth-changed", status);
                }
            };
            let result = (|| {
                let _runtime = manager.runtime_guard.try_lock()
                    .map_err(|_| "Antigravity è occupato. Attendi la fine della chat o dell’aggiornamento modelli.")?;
                manager.discard_chat();
                let mut settings = db::get_settings(&worker.db.guard())?;
                if crate::gemini_runtime::command(&settings.gemini_bin).is_err() {
                    let bin = crate::gemini_runtime::ensure(
                        &crate::model::engine_dir(),
                        &settings,
                        |message| {
                            publish(AuthStatus {
                                pending: true,
                                installing: true,
                                install_message: Some(message.into()),
                                ..Default::default()
                            })
                        },
                    )?;
                    settings.gemini_bin = bin.to_string_lossy().into_owned();
                    let conn = worker.db.guard();
                    let mut latest = db::get_settings(&conn)?;
                    latest.gemini_bin = settings.gemini_bin.clone();
                    db::save_settings(&conn, &latest)?;
                }
                if manager.generation.load(Ordering::SeqCst) != generation {
                    return Err("Accesso annullato.".into());
                }
                publish(AuthStatus {
                    installed: true,
                    pending: true,
                    ..Default::default()
                });
                let owner = Arc::downgrade(&worker);
                let handle = app.clone();
                let callback = Arc::new(move |url: String| {
                    if let Some(owner) = owner.upgrade() {
                        if owner.gemini_auth.generation.load(Ordering::SeqCst) != generation {
                            return;
                        }
                        {
                            let mut current = owner.gemini_auth.status.guard();
                            if !current.pending || current.installing || current.url.as_deref() == Some(url.as_str()) {
                                return;
                            }
                            current.url = Some(url.clone());
                            let _ = handle.emit("gemini-auth-changed", current.clone());
                        }
                        if let Err(error) = open_login(&url) {
                            let mut current = owner.gemini_auth.status.guard();
                            if owner.gemini_auth.generation.load(Ordering::SeqCst) == generation && current.pending {
                                current.error = Some(format!("Apertura del browser non riuscita: {error}. Premi Apri la pagina di accesso per riprovare."));
                                let _ = handle.emit("gemini-auth-changed", current.clone());
                            }
                        }
                    }
                });
                let directory = worker.data_dir.join("gemini-session");
                let mut rpc =
                    Rpc::start(&settings, &directory, &home_for(&directory), Some(callback), false)?;
                *manager.child.guard() = Some(rpc.child.clone());
                if manager.generation.load(Ordering::SeqCst) != generation {
                    rpc.child.cancel();
                    return Err("Accesso annullato.".into());
                }
                rpc.initialize(90)?;
                if force && has_saved_login(&directory) {
                    rpc.call("logout", json!({}), 20)?;
                }
                rpc.call("authenticate", json!({"methodId":"oauth-personal"}), 330)?;
                let session = rpc.session(&directory, &settings, false, 330)?;
                save_catalog(&directory, &session)?;
                Ok(())
            })();
            if manager.generation.load(Ordering::SeqCst) == generation {
                manager.child.guard().take();
                publish(match result {
                    Ok(()) => AuthStatus {
                        installed: true,
                        logged_in: true,
                        ..Default::default()
                    },
                    Err(error) => AuthStatus {
                        installed: crate::gemini_runtime::command(
                            &db::get_settings(&worker.db.guard())
                                .unwrap_or_default()
                                .gemini_bin,
                        )
                        .is_ok(),
                        error: Some(error),
                        ..Default::default()
                    },
                });
            }
        });
        Ok(pending)
    }
}
fn login_url_allowed(url: &str) -> bool {
    !url.chars().any(|c| c.is_control() || c.is_whitespace())
        && reqwest::Url::parse(url).ok().is_some_and(|u| {
            u.scheme() == "https"
                && u.host_str() == Some("accounts.google.com")
                && u.username().is_empty()
                && u.password().is_none()
                && u.port().is_none()
        })
}
fn report_login_text(text: &str, callback: &dyn Fn(String)) {
    // Logs can surround the link with quotes, brackets or ANSI escapes.
    for (start, _) in text.match_indices("https://accounts.google.com/") {
        let url = text[start..].split(|c: char| c.is_whitespace() || c.is_control() || matches!(c, '\"' | '\'' | '<' | '>'))
            .next().unwrap_or_default().trim_end_matches([')', ']', ',', '.']);
        if login_url_allowed(url) { callback(url.into()); }
    }
}
fn report_login_urls(value: &Value, callback: &dyn Fn(String)) {
    match value {
        Value::String(text) => report_login_text(text, callback),
        Value::Array(values) => values.iter().for_each(|value| report_login_urls(value, callback)),
        Value::Object(values) => values.values().for_each(|value| report_login_urls(value, callback)),
        _ => {}
    }
}
pub fn open_login(url: &str) -> Result<()> {
    if !login_url_allowed(url) {
        return Err("URL di accesso Google non valido.".into());
    }
    #[cfg(windows)]
    {
        use windows::{core::{w, PCWSTR}, Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL}};
        let url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let result = unsafe { ShellExecuteW(None, w!("open"), PCWSTR(url.as_ptr()), PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL) };
        if result.0 as isize <= 32 {
            return Err(format!("Windows non ha aperto il browser (codice {}).", result.0 as isize));
        }
    }
    #[cfg(not(windows))]
    if !std::process::Command::new("/usr/bin/open").arg(url).status().map_err(|e| e.to_string())?.success() {
        return Err("macOS non ha aperto il browser predefinito.".into());
    }
    Ok(())
}
pub fn forward_login_url(url: &str) -> Result<()> {
    if !login_url_allowed(url) { return Err("URL di accesso Google non valido.".into()); }
    println!("{url}");
    std::io::stdout().flush().map_err(|e| e.to_string())
}
fn parse_models(session: &Value) -> Result<Vec<ModelInfo>> {
    let values = session["models"]["availableModels"]
        .as_array()
        .ok_or("Antigravity non ha restituito il catalogo modelli. Ricollega l’account.")?;
    let models: Vec<_> = values
        .iter()
        .filter_map(|entry| {
            Some(ModelInfo {
                value: entry["modelId"].as_str()?.into(),
                display_name: entry["name"].as_str()?.into(),
                description: entry["description"].as_str().unwrap_or_default().into(),
                resolved_model: None,
            })
        })
        .collect();
    if models.is_empty() {
        return Err("Nessun modello disponibile per questo account Antigravity.".into());
    }
    Ok(models)
}
fn save_catalog(directory: &Path, session: &Value) -> Result<()> {
    let models = parse_models(session)?;
    let path = home_for(directory).join("presentio-models.json");
    std::fs::write(
        path,
        serde_json::to_vec(&models).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
pub fn models(manager: &AuthManager, settings: &Settings, directory: &Path, refresh: bool) -> Result<Catalog> {
    let path = home_for(directory).join("presentio-models.json");
    let modified = std::fs::metadata(&path).ok().and_then(|meta| meta.modified().ok());
    let cached = std::fs::read(&path).ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<ModelInfo>>(&bytes).ok())
        .filter(|models| !models.is_empty())
        .map(|models| Catalog {
            models,
            checked_at: modified.map(chrono::DateTime::<chrono::Utc>::from)
                .unwrap_or_else(chrono::Utc::now).to_rfc3339(),
            cli_version: None,
        });
    if !refresh {
        return cached.ok_or_else(|| "Collega l’account Antigravity per caricare i modelli disponibili.".into());
    }
    if !has_saved_login(directory) {
        return Err("Collega prima l’account Antigravity.".into());
    }
    // Login and chats already return the live catalog. Repeated clicks must not
    // start another full agent just to obtain that same, very recent response.
    if let Some(catalog) = &cached {
        if modified.and_then(|stamp| stamp.elapsed().ok()).is_some_and(|age| age < Duration::from_secs(30)) {
            return Ok(catalog.clone());
        }
    }
    let Ok(_runtime) = manager.runtime_guard.try_lock() else {
        return cached.ok_or_else(|| "Antigravity è occupato. Riprova al termine dell’operazione in corso.".into());
    };
    manager.discard_chat();
    let mut rpc = Rpc::start(settings, directory, &home_for(directory), None, true)?;
    // Catalog requests get a short budget; the login and chat budgets stay separate.
    let init = rpc.initialize(10)?;
    let session = rpc.session(directory, settings, false, 20)?;
    save_catalog(directory, &session)?;
    Ok(Catalog {
        models: parse_models(&session)?,
        checked_at: chrono::Utc::now().to_rfc3339(),
        cli_version: init["agentInfo"]["version"].as_str().map(str::to_owned),
    })
}
fn settings_file(path: &Path) -> Result<Value> {
    if !path.is_file() {
        return Ok(json!({}));
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if text.len() > 2_000_000 {
        return Err("Configurazione Gemini troppo grande.".into());
    }
    if text.trim_start_matches('\u{feff}').trim().is_empty() {
        return Ok(json!({}));
    }
    // JSONC: strip comments and trailing commas without touching string literals.
    let chars: Vec<char> = text.chars().collect();
    let mut clean = String::new();
    let (mut i, mut quoted, mut escape) = (0, false, false);
    while i < chars.len() {
        let c = chars[i];
        if quoted {
            clean.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            quoted = true;
            clean.push(c);
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 >= chars.len() {
                return Err(format!("Il file MCP {} contiene un commento /* non chiuso.", path.display()));
            }
            i += 2;
            clean.push(' ');
            continue;
        }
        clean.push(c);
        i += 1;
    }
    let chars: Vec<char> = clean.chars().collect();
    let mut json = String::new();
    quoted = false;
    escape = false;
    for (i, &c) in chars.iter().enumerate() {
        if quoted {
            json.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        if c == '"' {
            quoted = true;
        }
        if c == ','
            && matches!(
                chars[i + 1..].iter().find(|c| !c.is_whitespace()),
                Some('}' | ']')
            )
        {
            continue;
        }
        json.push(c);
    }
    let json = json.trim_start_matches('\u{feff}').trim();
    if json.is_empty() { return Ok(json!({})); }
    serde_json::from_str(json)
        .map_err(|e| format!("Il file MCP {} non è JSON/JSONC valido (riga {}, colonna {}). Correggilo oppure disattiva i connettori Antigravity nelle impostazioni per continuare senza MCP.", path.display(), e.line(), e.column()))
}
struct Server {
    name: String,
    status: String,
    wire: Option<Value>,
}
fn configured_servers(directory: &Path) -> Result<Vec<Server>> {
    let home = std::env::var_os("GEMINI_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|p| PathBuf::from(p).join(".gemini"))
        })
        .ok_or("Cartella utente Antigravity non disponibile")?;
    let mut all = BTreeMap::new();
    for path in [
        home.join("config/mcp_config.json"),
        directory.join(".agents/mcp_config.json"),
    ] {
        let config = settings_file(&path)?;
        if let Some(servers) = config["mcpServers"].as_object() {
            for (name, value) in servers {
                all.insert(name.clone(), value.clone());
            }
        }
    }
    Ok(all
        .into_iter()
        .map(|(name, value)| {
            let disabled = value["disabled"] == true;
            let wire = server_wire(&name, &value);
            Server {
                name,
                status: if disabled {
                    "disabled"
                } else if wire.is_none() {
                    "unsupported"
                } else {
                    "configured"
                }
                .into(),
                wire: if disabled { None } else { wire },
            }
        })
        .collect())
}
fn named_values(value: &Value) -> Option<Value> {
    if value.is_null() {
        return Some(json!([]));
    }
    let mut result = vec![];
    for (name, value) in value.as_object()? {
        result.push(json!({"name":name,"value":value.as_str()?}));
    }
    Some(json!(result))
}
fn server_wire(name: &str, value: &Value) -> Option<Value> {
    // ACP does not carry the CLI's credential-provider configuration. Never
    // silently strip OAuth or per-tool exclusions and run a broader server.
    if value.to_string().contains("${") {
        return None;
    }
    if name.is_empty()
        || name.chars().any(char::is_control)
        || !value["oauth"].is_null()
        || !value["authProviderType"].is_null()
        || (!value["disabledTools"].is_null()
            && !value["disabledTools"]
                .as_array()
                .is_some_and(|a| a.is_empty()))
        || (!value["disabled"].is_null() && !value["disabled"].is_boolean())
    {
        return None;
    }
    if let Some(command) = value["command"].as_str().filter(|s| !s.is_empty()) {
        if !value["type"].is_null() && value["type"] != "stdio" {
            return None;
        }
        let args = value.get("args").cloned().unwrap_or(json!([]));
        if !args.as_array()?.iter().all(|v| v.as_str().is_some()) || !value["cwd"].is_null() {
            return None;
        }
        Some(json!({"name":name,"command":command,"args":args,"env":named_values(&value["env"])?}))
    } else {
        if !value["type"].is_null()
            && !["http", "sse", "streamable-http"]
                .iter()
                .any(|t| value["type"] == *t)
        {
            return None;
        }
        let url = value["serverUrl"]
            .as_str()
            .or_else(|| value["url"].as_str())?;
        let parsed = reqwest::Url::parse(url).ok()?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return None;
        }
        Some(
            json!({"name":name,"type":if value["type"]=="sse" || parsed.path().ends_with("/sse") {"sse"}else{"http"},"url":url,"headers":named_values(&value["headers"])?}),
        )
    }
}
pub fn connectors(directory: &Path) -> Result<Vec<connectors::Connector>> {
    Ok(configured_servers(directory)?
        .into_iter()
        .map(|server| connectors::Connector {
            name: server.name,
            status: server.status,
            tools: 0,
        })
        .collect())
}
fn connection_stage(app: &tauri::AppHandle, state: &AppState, label: &str) {
    if let Some(tracker) = state.chat_control.progress.guard().as_mut() {
        if let Some(entry) = tracker.run.entries.iter_mut().find(|entry| entry.id == "connection") {
            entry.label = label.into();
        }
    }
    let _ = app.emit("chat-progress", state.chat_control.progress());
}
fn watch_idle_chat(state: &Arc<AppState>, child: &Arc<EngineChild>) {
    let owner = Arc::downgrade(state);
    let child = Arc::downgrade(child);
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(10));
        let (Some(owner), Some(child)) = (owner.upgrade(), child.upgrade()) else { break; };
        let mut cached = owner.gemini_auth.warm_chat.guard();
        let expired = cached.as_ref().is_some_and(|chat| {
            Arc::ptr_eq(&chat.rpc.child, &child) && chat.last_used.elapsed() >= CHAT_IDLE_TIMEOUT
        });
        if expired {
            cached.take();
            break;
        }
        if child.exited() { break; }
    });
}
pub fn chat(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    id: &str,
    prompt: &str,
    settings: &Settings,
    context: &str,
    current_context: &str,
    user_index: usize,
) -> Result<()> {
    state.chat_control.start_progress(app, id, user_index, "Antigravity");
    let manager = &state.gemini_auth;
    let _runtime = manager.runtime_guard.try_lock()
        .map_err(|_| "Antigravity è occupato. Attendi la fine dell’accesso o dell’aggiornamento modelli.")?;
    let generation = manager.generation.load(Ordering::SeqCst);
    let directory = crate::chat_files::directory(state, id, true)?;
    let account_directory = state.data_dir.join("gemini-session");
    if !has_saved_login(&account_directory) {
        return Err("Collega prima l’account Antigravity dalle impostazioni.".into());
    }
    if state.chat_control.cancelled.load(Ordering::SeqCst) || manager.generation.load(Ordering::SeqCst) != generation {
        return Err("Risposta interrotta.".into());
    }
    let params = session_params(&directory, settings, true)?;
    // Compare the actual MCP configuration too: editing a file must invalidate
    // the agent even when the saved PARLO settings have not changed.
    let key = json!({"call":id,"settings":settings,"session":params,"accountGeneration":generation}).to_string();
    let cached = manager.warm_chat.guard().take();
    let reused = cached.as_ref().is_some_and(|chat| {
        chat.key == key && chat.next_user_index == user_index
            && chat.last_used.elapsed() < CHAT_IDLE_TIMEOUT && !chat.rpc.child.exited()
    });
    let context_hash = format!("{:x}", Sha256::digest(current_context.as_bytes()));
    let unchanged = reused && cached.as_ref().is_some_and(|chat| {
        chat.context_hash == context_hash && chat.snapshot.is_current()
    });
    connection_stage(app, state, "Preparazione delle fonti per Antigravity");
    let mut snapshot = if unchanged { None } else {
        Some(crate::sources::stage_for_antigravity(state, id, &directory)?)
    };
    let mut warm = if reused {
        connection_stage(app, state, "Ripresa della sessione Antigravity");
        cached.unwrap()
    } else {
        drop(cached);
        connection_stage(app, state, "Avvio del componente locale Antigravity");
        let mut rpc = Rpc::start(settings, &directory, &home_for(&account_directory), None, true)?;
        *state.chat_control.process.guard() = Some(rpc.child.clone());
        if state.chat_control.cancelled.load(Ordering::SeqCst) { return Err("Risposta interrotta.".into()); }
        rpc.initialize(30).map_err(|error| format!("Avvio del componente Antigravity non riuscito: {error}"))?;
        connection_stage(app, state, "Preparazione della sessione Antigravity e dei connettori");
        let session = rpc.call("session/new", params, 60)
            .map_err(|error| format!("Preparazione della sessione Antigravity non riuscita: {error}"))?;
        watch_idle_chat(state, &rpc.child);
        WarmChat { rpc, session, key, call_id: id.into(), next_user_index: user_index, last_used: Instant::now(),
            context_hash: context_hash.clone(), snapshot: snapshot.take().unwrap() }
    };
    if let Some(snapshot) = snapshot { warm.snapshot = snapshot; }
    *state.chat_control.process.guard() = Some(warm.rpc.child.clone());
    if state.chat_control.cancelled.load(Ordering::SeqCst) || manager.generation.load(Ordering::SeqCst) != generation {
        return Err("Risposta interrotta.".into());
    }
    let context = if unchanged {
        warm.snapshot.context(prompt.trim())
    } else if reused {
        warm.snapshot.context(&format!("<contesto_aggiornato>Il seguente contesto e l’elenco delle fonti sostituiscono quelli dei turni precedenti. Le vecchie copie temporanee non sono più disponibili: usa solo i percorsi attuali. La cronologia della conversazione è già presente nella sessione.</contesto_aggiornato>\n{current_context}\n\nRichiesta attuale: {}", prompt.trim()))
    } else { warm.snapshot.context(context) };
    warm.context_hash = context_hash;
    let WarmChat { rpc, session, snapshot, .. } = &mut warm;
    // Reuse the model list that session/new already fetched for this account.
    if !reused { let _ = save_catalog(&account_directory, session); }
    let session_id = session["sessionId"]
        .as_str()
        .ok_or("Antigravity non ha creato la chat")?;
    if !reused { rpc.call(
        "session/set_mode",
        json!({"sessionId":session_id,"modeId":"default"}),
        30,
    )?; }
    let mut resolved = session["models"]["currentModelId"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if !settings.gemini_model.is_empty() {
        if !parse_models(&session)?
            .iter()
            .any(|m| m.value == settings.gemini_model)
        {
            return Err("Il modello scelto non è più disponibile per questo account. Aggiorna la lista e selezionane un altro.".into());
        }
        if !reused { rpc.call(
            "session/set_model",
            json!({"sessionId":session_id,"modelId":settings.gemini_model}),
            30,
        )?; }
        resolved = settings.gemini_model.clone();
    }
    let request_id = rpc.begin(
        "session/prompt",
        json!({"sessionId":session_id,"prompt":[{"type":"text","text":context}]}),
    )?;
    connection_stage(app, state, "In attesa della risposta di Antigravity");
    db::append_message(&state.db.guard(), id, "user", prompt.trim())?;
    crate::detail_changed(app, id);
    let deadline = Instant::now() + Duration::from_secs(600);
    let mut answer = String::new();
    loop {
        if state.chat_control.cancelled.load(Ordering::SeqCst) || manager.generation.load(Ordering::SeqCst) != generation {
            return Err("Risposta interrotta.".into());
        }
        let value = rpc.next(deadline)?;
        if value.get("method").is_none() && value["id"] == request_id {
            let result = response(&value)?;
            match result["stopReason"].as_str() {
                Some("end_turn") => break,
                Some("cancelled") => return Err("Risposta interrotta.".into()),
                _ => {
                    return Err(
                        "Antigravity ha interrotto la risposta prima del completamento.".into(),
                    )
                }
            }
        }
        if value["method"] == "session/update" && value["params"]["sessionId"] == session_id {
            let update = &value["params"]["update"];
            if update["sessionUpdate"] == "agent_message_chunk"
                && update["content"]["type"] == "text"
            {
                if let Some(text) = update["content"]["text"].as_str() {
                    if answer.len() + text.len() > 1_000_000 {
                        return Err("Risposta Antigravity troppo lunga.".into());
                    }
                    answer.push_str(text);
                    if let Some(tracker) = state.chat_control.progress.guard().as_mut() {
                        tracker.observe_gemini(update);
                    }
                    let _ = app.emit("chat-progress", state.chat_control.progress());
                    let _ = app.emit(
                        "chat-delta",
                        json!({"id":id,"text":snapshot.answer(&answer)}),
                    );
                }
            } else if matches!(
                update["sessionUpdate"].as_str(),
                Some("tool_call" | "tool_call_update")
            ) {
                if let Some(tracker) = state.chat_control.progress.guard().as_mut() {
                    tracker.observe_gemini(update);
                }
                let _ = app.emit("chat-progress", state.chat_control.progress());
            }
        }
        if value.get("method").is_some() && value.get("id").is_some() {
            if value["method"] == "session/request_permission"
                && value["params"]["sessionId"] == session_id
            {
                let params = &value["params"];
                let Some(request) = permission_request(params) else {
                    rpc.reject(&value)?;
                    return Err("Antigravity ha richiesto un’interazione non gestita da PARLO. L’operazione è stata interrotta; specifica la scelta nella domanda e riprova.".into());
                };
                let latest = db::get_settings(&state.db.guard())?.connector_settings();
                let response = state.chat_control.request(app, id, &latest, &request);
                rpc.send(json!({"jsonrpc":"2.0","id":value["id"],"result":permission_result(params, &response)}))?;
            } else {
                rpc.reject(&value)?;
            }
        }
    }
    if answer.trim().is_empty() {
        return Err("Antigravity non ha restituito una risposta.".into());
    }
    let answer = snapshot.answer(&answer);
    if let Some(tracker) = state.chat_control.progress.guard().as_mut() {
        for entry in &mut tracker.run.entries {
            if entry.kind == "text" {
                entry.text = snapshot.answer(&entry.text);
            }
        }
    }
    db::append_response(&state.db.guard(), id, &answer, &resolved)?;
    if !state.chat_control.cancelled.load(Ordering::SeqCst) && manager.generation.load(Ordering::SeqCst) == generation {
        warm.next_user_index = user_index + 2;
        warm.last_used = Instant::now();
        *manager.warm_chat.guard() = Some(warm);
    }
    Ok(())
}

// Antigravity also uses request_permission for fixed-choice questions. Their
// options are answers: choosing the first allow_once would invent a user answer.
fn permission_request(params: &Value) -> Option<Value> {
    let tool = &params["toolCall"];
    let id = tool["toolCallId"].as_str()?;
    let options = params["options"]
        .as_array()?
        .iter()
        .filter(|o| o["kind"] == "allow_once" && o["optionId"].as_str().is_some())
        .collect::<Vec<_>>();
    if options.is_empty() || options.len() > 20 {
        return None;
    }
    let question = id.starts_with("interaction_");
    let input = if question {
        let mut labels = std::collections::HashSet::new();
        let choices: Option<Vec<_>> = options
            .iter()
            .map(|o| {
                let name = o["name"].as_str()?.trim();
                if name.is_empty() || name.len() > 500 || !labels.insert(name) {
                    return None;
                }
                Some(json!({"label":name}))
            })
            .collect();
        json!({"questions":[{"id":"choice","question":tool["title"].as_str().unwrap_or("Domanda di Antigravity"),"options":choices?}]})
    } else {
        json!({"kind":tool["kind"],"content":tool["content"],"locations":tool["locations"],"rawInput":tool["rawInput"]})
    };
    Some(
        json!({"request_id":uuid::Uuid::new_v4().to_string(),"request":{"tool_name":if question {"request_user_input"}else{"antigravity_tool"},"tool_use_id":id,"title":tool["title"],"input":input,"suppress_always_allow_rule":true}}),
    )
}
fn permission_result(params: &Value, response: &Value) -> Value {
    let cancelled = json!({"outcome":{"outcome":"cancelled"}});
    if response["response"]["response"]["behavior"] != "allow" {
        return cancelled;
    }
    let Some(options) = params["options"].as_array() else {
        return cancelled;
    };
    let question = params["toolCall"]["toolCallId"]
        .as_str()
        .is_some_and(|s| s.starts_with("interaction_"));
    let answer = response["response"]["response"]["updatedInput"]["answers"]["choice"].as_str();
    let option = options.iter().find(|o| {
        o["kind"] == "allow_once"
            && (!question
                || answer.is_some_and(|a| o["name"].as_str().is_some_and(|n| n.trim() == a)))
    });
    match option.and_then(|o| o["optionId"].as_str()) {
        Some(id) => json!({"outcome":{"outcome":"selected","optionId":id}}),
        None => cancelled,
    }
}

#[cfg(test)]
mod antigravity_tests {
    use super::*;
    #[test]
    fn question_options_require_the_users_exact_answer() {
        let params = json!({"toolCall":{"toolCallId":"interaction_42","title":"Quale formato?"},"options":[{"kind":"allow_once","optionId":"pdf","name":"PDF"},{"kind":"allow_once","optionId":"word","name":"Word"}]});
        let request = permission_request(&params).unwrap();
        assert_eq!(request["request"]["tool_name"], "request_user_input");
        let response = |answer| {
            connectors::permission_response("id", true, &json!({"answers":{"choice":answer}}))
        };
        assert_eq!(
            permission_result(&params, &response("Word"))["outcome"]["optionId"],
            "word"
        );
        assert_eq!(
            permission_result(&params, &response("Inventato"))["outcome"]["outcome"],
            "cancelled"
        );
        assert_eq!(
            permission_result(
                &params,
                &connectors::permission_response("id", true, &json!({}))
            )["outcome"]["outcome"],
            "cancelled"
        );
    }
    #[test]
    fn tools_never_choose_permanent_approval_or_ignore_denial() {
        let params = json!({"toolCall":{"toolCallId":"tool","title":"Comando"},"options":[{"kind":"allow_always","optionId":"forever","name":"Sempre"},{"kind":"allow_once","optionId":"once","name":"Una volta"}]});
        assert_eq!(
            permission_request(&params).unwrap()["request"]["suppress_always_allow_rule"],
            true
        );
        assert_eq!(
            permission_result(
                &params,
                &connectors::permission_response("id", true, &json!({}))
            )["outcome"]["optionId"],
            "once"
        );
        assert_eq!(
            permission_result(
                &params,
                &connectors::permission_response("id", false, &json!({}))
            )["outcome"]["outcome"],
            "cancelled"
        );
        assert!(permission_request(&json!({"toolCall":{"toolCallId":"tool"},"options":[{"kind":"allow_always","optionId":"forever"}]})).is_none());
    }
    #[test]
    fn unsupported_mcp_configuration_is_not_silently_broadened() {
        for server in [
            json!({"serverUrl":"https://example.test/mcp","oauth":{}}),
            json!({"command":"example","disabledTools":["send"]}),
            json!({"command":"example","env":{"TOKEN":"${SECRET}"}}),
            json!({"command":"example","cwd":"/elsewhere"}),
            json!({"command":"example","disabledTools":"send"}),
            json!({"command":"example","disabled":"true"}),
            json!({"serverUrl":"https://example.test/mcp","type":"unknown"}),
        ] {
            assert!(server_wire("test", &server).is_none());
        }
        assert_eq!(
            server_wire(
                "remote",
                &json!({"serverUrl":"https://example.test/mcp","headers":{"X-Test":"synthetic"}})
            )
            .unwrap()["headers"][0]["value"],
            "synthetic"
        );
        assert_eq!(
            server_wire("local", &json!({"command":"example","args":["--stdio"]})).unwrap()["env"],
            json!([])
        );
    }
}
