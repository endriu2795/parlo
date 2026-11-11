//! Ollama through Claude Code's Anthropic-compatible endpoint.
//! Models are hosted by the selected Ollama instance, with no cloud fallback.
use crate::{
    auth::AuthStatus,
    catalog::{Catalog, ModelInfo},
    model::Settings,
    Result,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io::Read, process::Command, time::Duration};

pub const BASE_URL: &str = "http://127.0.0.1:11434";
const RESPONSE_LIMIT: u64 = 4 * 1024 * 1024;

pub(crate) fn uses_full_tools(settings: &Settings) -> bool {
    settings.ollama_full_tools || !settings.ollama_server_url.trim().is_empty()
}

pub(crate) fn chat_error(diagnostic: &str, remote: bool) -> String {
    use crate::locale::tr;
    let error = diagnostic.to_ascii_lowercase();
    if ["econnrefused", "connection refused", "econnreset", "connection reset", "failed to connect"].iter().any(|text| error.contains(text)) {
        return if remote {
            tr("Connessione al server Ollama persa. Verifica il server e riprova.", "The connection to the Ollama server was lost. Check the server and try again.")
        } else {
            tr("PARLO non riesce a collegarsi a Ollama. Avvia Ollama sul computer e riprova, oppure cambia fornitore.", "PARLO could not connect to Ollama. Start Ollama on this computer and try again, or switch providers.")
        }.into();
    }
    if ["timeout", "timed out", "etimedout"].iter().any(|text| error.contains(text)) {
        return tr("Ollama non ha risposto in tempo. Riprova con un modello più leggero o un contesto più breve.", "Ollama did not respond in time. Try a smaller model or a shorter context.").into();
    }
    if ["out of memory", "requires more system memory", "not enough memory"].iter().any(|text| error.contains(text)) {
        return tr("Ollama ha esaurito la memoria. Prova un modello più leggero o riduci il contesto.", "Ollama ran out of memory. Try a smaller model or reduce the context.").into();
    }
    if error.contains("unauthorized") || error.contains("forbidden") {
        return tr("Il server Ollama ha rifiutato l’accesso. Verifica la chiave e i permessi.", "The Ollama server denied access. Check the API key and permissions.").into();
    }
    tr("Ollama non ha completato la risposta. Controlla che sia in esecuzione e riprova.", "Ollama did not complete the response. Check that it is running and try again.").into()
}

pub fn valid_model(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && name.as_bytes()[0].is_ascii_alphanumeric()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.:/".contains(&c))
}
fn remote(value: &Value) -> bool {
    ["remote_host", "remote_model"].iter().any(|key| {
        value
            .get(key)
            .is_some_and(|field| !field.is_null() && field.as_str() != Some(""))
    })
}
fn cloud_name(name: &str) -> bool {
    let tag = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
    tag == "cloud" || tag.ends_with("-cloud") || tag.starts_with("cloud-")
}
pub fn server_base(input: &str) -> Result<String> {
    let endpoint = crate::custom_endpoint::Endpoint { url: input.into(), ..Default::default() };
    let base = endpoint.url("")?.trim_end_matches('/').to_owned();
    let url = reqwest::Url::parse(&base).map_err(|_| "Indirizzo Ollama non valido.")?;
    if url.path().trim_end_matches('/').ends_with("/v1") {
        return Err("Usa l’indirizzo base di Ollama senza /v1.".into());
    }
    if url.scheme() == "http" {
        let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
        let private = match host.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private(),
            Ok(std::net::IpAddr::V6(ip)) => ip.is_loopback() || ip.is_unique_local(),
            Err(_) => host == "localhost",
        };
        if !private { return Err("Usa HTTPS per un server Ollama fuori dalla rete privata.".into()); }
    }
    Ok(base)
}

struct Server {
    base: String,
    key: String,
    local: bool,
}

impl Server {
    fn configured(url: &str) -> Result<Self> {
        if url.trim().is_empty() {
            return Ok(Self { base: BASE_URL.into(), key: String::new(), local: true });
        }
        let base = server_base(url)?;
        let endpoint = crate::custom_endpoint::Endpoint { url: base.clone(), ..Default::default() };
        let key = if base.starts_with("https://") {
            crate::custom_endpoint::key("ollama", &endpoint)?
        } else { String::new() };
        Ok(Self { base, key, local: false })
    }

    fn request(&self, path: &str, body: Option<Value>) -> Result<Value> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|_| "Client Ollama non disponibile.".to_string())?;
        let url = format!("{}{path}", self.base);
        let mut request = if let Some(body) = body {
            client
                .post(url)
                .header("Content-Type", "application/json")
                .body(body.to_string())
        } else {
            client.get(url)
        };
        if !self.key.is_empty() { request = request.bearer_auth(&self.key); }
        let response = request.send().map_err(|_| if self.local {
            "Ollama non raggiungibile. Avvia Ollama sul computer (porta 11434) e aggiorna l’elenco.".to_owned()
        } else { "Server Ollama non raggiungibile. Verifica indirizzo e connessione.".to_owned() })?;
        if matches!(response.status().as_u16(), 401 | 403) {
            return Err("Il server Ollama ha rifiutato l’accesso. Verifica la chiave e i permessi.".into());
        }
        if !response.status().is_success() {
            return Err("Ollama non ha restituito i modelli richiesti. Verifica che sia aggiornato e che il modello sia installato.".into());
        }
        let mut bytes = Vec::new();
        response
            .take(RESPONSE_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Risposta Ollama interrotta.".to_string())?;
        if bytes.len() as u64 > RESPONSE_LIMIT {
            return Err("Elenco Ollama troppo grande.".into());
        }
        serde_json::from_slice(&bytes).map_err(|_| "Risposta di Ollama non valida.".into())
    }

    fn models(&self) -> Result<Catalog> {
        parse_catalog(&self.request("/api/tags", None)?)
    }
}
fn parse_catalog(value: &Value) -> Result<Catalog> {
    let entries = value["models"]
        .as_array()
        .ok_or("Ollama non ha restituito un elenco modelli valido.")?;
    let mut models: Vec<ModelInfo> = Vec::new();
    for entry in entries {
        let Some(name) = entry["name"].as_str() else {
            continue;
        };
        if !valid_model(name)
            || is_context_model(name)
            || cloud_name(name)
            || remote(entry)
            || models.iter().any(|model| model.value == name)
        {
            continue;
        }
        if entry["capabilities"]
            .as_array()
            .is_some_and(|caps| !caps.is_empty() && !caps.iter().any(|cap| cap == "completion"))
        {
            continue;
        }
        let details = [
            &entry["details"]["parameter_size"],
            &entry["details"]["quantization_level"],
        ]
        .into_iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        models.push(ModelInfo {
            value: name.into(),
            display_name: name.into(),
            description: if details.is_empty() {
                "Modello installato in Ollama".into()
            } else {
                details
            },
            resolved_model: Some(name.into()),
        });
    }
    models.sort_by(|a, b| a.value.cmp(&b.value));
    Ok(Catalog {
        models,
        checked_at: chrono::Utc::now().to_rfc3339(),
        cli_version: None,
    })
}
pub fn models(server_url: &str) -> Result<Catalog> {
    Server::configured(server_url)?.models()
}

fn validate_details(value: &Value, require_tools: bool) -> Result<()> {
    if remote(value) {
        return Err(
            "Questo modello Ollama usa un servizio remoto. Seleziona un modello locale installato."
                .into(),
        );
    }
    if require_tools && !value["capabilities"]
        .as_array()
        .is_some_and(|caps| caps.iter().any(|cap| cap == "tools"))
    {
        return Err("Questo modello Ollama non dichiara il supporto agli strumenti. Scegli un modello con tool calling e usa una versione aggiornata di Ollama.".into());
    }
    Ok(())
}
fn is_context_model(name: &str) -> bool {
    name.starts_with("parlo-local/") || name.starts_with("presentio-local/")
}

fn context_model(name: &str, context: u32) -> String {
    let hash = format!("{:x}", Sha256::digest(name.as_bytes()));
    format!("parlo-local/{}:ctx{context}", &hash[..20])
}
fn context_request(settings: &Settings, details: &Value) -> Result<Value> {
    validate_details(details, settings.ollama_tools_enabled)?;
    let context = settings.ollama_context_length;
    if context < 8192 {
        return Err("Dimensione del contesto Ollama non valida.".into());
    }
    if details["model_info"].as_object().and_then(|info| info.iter()
        .filter(|(key,_)| key.ends_with(".context_length")).filter_map(|(_,value)|value.as_u64()).max())
        .is_some_and(|maximum| u64::from(context) > maximum) {
        return Err("Il modello selezionato non supporta questo contesto. Scegli una memoria della conversazione inferiore in Impostazioni > Assistente.".into());
    }
    Ok(json!({"model":context_model(&settings.ollama_model,context),"from":settings.ollama_model,
        "parameters":{"num_ctx":context},"stream":false}))
}
// Text-only preview: tool schemas are counted by the backend at execution time.
pub const ANSWER_RESERVE: usize = 4096;
#[derive(Clone, serde::Serialize)]
pub struct ContextBudget {
    pub input_tokens: usize,
    pub reserved_tokens: usize,
    pub limit: u32,
}
pub fn context_budget(settings: &Settings, context: &str, system: &str) -> ContextBudget {
    ContextBudget {
        input_tokens: context.chars().count().saturating_add(system.chars().count()).div_ceil(2),
        reserved_tokens: ANSWER_RESERVE,
        limit: settings.ollama_context_length,
    }
}
pub fn context_update(id: &str, value: &Value) -> Option<Value> {
    if value["type"]=="system" && value["subtype"]=="compact_boundary" {
        return Some(json!({"id":id,"compacted":true}));
    }
    let usage=match value["type"].as_str() {
        Some("assistant")=>&value["message"]["usage"],
        Some("stream_event") if value["event"]["type"]=="message_start" => &value["event"]["message"]["usage"],
        _=>return None,
    };
    let input=usage["input_tokens"].as_u64()?;
    let input=input.saturating_add(usage["cache_read_input_tokens"].as_u64().unwrap_or(0))
        .saturating_add(usage["cache_creation_input_tokens"].as_u64().unwrap_or(0));
    (input>0).then(||json!({"id":id,"input_tokens":input}))
}
pub fn ensure_model(settings: &Settings) -> Result<String> {
    let server = Server::configured(&settings.ollama_server_url)?;
    ensure_model_on(settings, &server)
}

fn ensure_model_on(settings: &Settings, server: &Server) -> Result<String> {
    let name = &settings.ollama_model;
    if !valid_model(name) || cloud_name(name) {
        return Err("Seleziona un modello Ollama nelle impostazioni o nella chat.".into());
    }
    if !server.models()?.models.iter().any(|model| &model.value == name) {
        return Err("Il modello selezionato non è installato nell’istanza Ollama scelta. Aggiorna l’elenco e scegli un modello disponibile.".into());
    }
    let details = server.request("/api/show", Some(json!({"model": name})))?;
    if !server.local {
        validate_details(&details, settings.ollama_tools_enabled)?;
        return Ok(name.clone());
    }
    let body = context_request(settings, &details)?;
    let alias = body["model"].as_str().unwrap().to_owned();
    // /api/create reuses local layers. No pull, conversion or global setting
    // change; refreshing the alias also picks up updates to the source model.
    let result = server.request("/api/create", Some(body)).map_err(|_| "Preparazione del contesto Ollama non riuscita. Verifica che Ollama sia aggiornato; nessun modello è stato scaricato.".to_string())?;
    if result["status"] != "success" { return Err("Ollama non ha confermato la preparazione del contesto locale.".into()); }
    Ok(alias)
}
pub fn status(settings: &Settings) -> AuthStatus {
    let installed = crate::process::executable(&settings.claude_bin).is_ok();
    let error = if !installed {
        Some("Per usare Ollama con gli strumenti serve la CLI Claude Code installata. Non è richiesto un account Claude.".into())
    } else {
        models(&settings.ollama_server_url).err()
    };
    AuthStatus {
        installed,
        logged_in: error.is_none(),
        error,
        ..Default::default()
    }
}

pub fn configure(command: &mut Command, settings: &Settings, model: &str) -> Result<()> {
    let server = Server::configured(&settings.ollama_server_url)?;
    configure_server(command, &server, model);
    Ok(())
}

fn configure_server(command: &mut Command, server: &Server, model: &str) {
    for key in [
        "CLAUDE_CODE_OAUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN_FILE",
        "ANTHROPIC_CUSTOM_HEADERS",
        "CLAUDE_CODE_API_KEY_HELPER_TTL_MS",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
        "CLAUDE_CODE_SIMPLE",
        "CLAUDE_CODE_BARE",
        "CLAUDE_CODE_MAX_CONTEXT_TOKENS",
    ] {
        command.env_remove(key);
    }
    command
        .env("ANTHROPIC_BASE_URL", &server.base)
        .env("ANTHROPIC_AUTH_TOKEN", if server.key.is_empty() { "ollama" } else { &server.key })
        .env("ANTHROPIC_API_KEY", "")
        .env("CLAUDE_CODE_USE_BEDROCK", "0")
        .env("CLAUDE_CODE_USE_VERTEX", "0")
        .env("CLAUDE_CODE_USE_FOUNDRY", "0")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
        .env("DISABLE_TELEMETRY", "1")
        .env("DISABLE_ERROR_REPORTING", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .env("MAX_THINKING_TOKENS", "0")
        .env("CLAUDE_CODE_EFFORT_LEVEL", "low")
        .env("CLAUDE_CODE_MAX_OUTPUT_TOKENS", "4096")
        .env("CLAUDE_CONFIG_DIR",crate::model::engine_dir().join(if server.local { "ollama-claude-config" } else { "ollama-server-claude-config" }))
        .env("ENABLE_TOOL_SEARCH","false")
        .env("NO_PROXY", "127.0.0.1,localhost,::1")
        .env("no_proxy", "127.0.0.1,localhost,::1");
    #[cfg(windows)]
    command.env("CLAUDE_CODE_USE_POWERSHELL_TOOL","1");
    for key in [
        "ANTHROPIC_MODEL",
        "ANTHROPIC_DEFAULT_SONNET_MODEL",
        "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        "ANTHROPIC_DEFAULT_OPUS_MODEL",
        "ANTHROPIC_SMALL_FAST_MODEL",
        "CLAUDE_CODE_SUBAGENT_MODEL",
    ] {
        command.env(key, model);
    }
}

#[tauri::command]
pub async fn ollama_model_context(window: tauri::WebviewWindow, model: String) -> Result<Option<u64>> {
    if window.label() != "main" {return Err("Gestisci il modello dalla finestra principale.".into());}
    tauri::async_runtime::spawn_blocking(move || {
        if !valid_model(&model) || cloud_name(&model) || is_context_model(&model) {return Err("Scegli un modello locale installato.".into());}
        let details=Server::configured("")?.request("/api/show",Some(json!({"model":model})))?;
        validate_details(&details, false)?;
        Ok(details["model_info"].as_object().and_then(|info| info.iter()
            .filter(|(key,_)| key.ends_with(".context_length")).filter_map(|(_,value)| value.as_u64()).max()))
    }).await.map_err(|e|e.to_string())?
}

#[tauri::command]
pub async fn ollama_models(window: tauri::WebviewWindow, server_url: Option<String>) -> Result<Catalog> {
    if window.label() != "main" { return Err("Gestisci Ollama dalla finestra principale.".into()); }
    tauri::async_runtime::spawn_blocking(move || models(server_url.as_deref().unwrap_or_default()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn ollama_download(window: tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" { return Err("Apri il sito dalle impostazioni della finestra principale.".into()); }
    let program = if cfg!(windows) { "explorer.exe" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    Command::new(program).arg("https://ollama.com/download/").spawn()
        .map_err(|_| "Impossibile aprire il browser. Visita https://ollama.com/download/ per installare Ollama.".to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_server_reuses_tools_without_creating_a_model_on_the_server() {
        use std::{io::{BufRead, BufReader, Write}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let fixture = std::thread::spawn(move || {
            for (expected, response) in [
                ("GET /api/tags HTTP/1.1", r#"{"models":[{"name":"test:latest"}]}"#),
                ("POST /api/show HTTP/1.1", r#"{"capabilities":["completion","tools"]}"#),
            ] {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut reader = BufReader::new(&mut socket);
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                assert_eq!(line.trim_end(), expected);
                let mut length = 0;
                loop {
                    line.clear();
                    assert!(reader.read_line(&mut line).unwrap() > 0);
                    if line == "\r\n" { break; }
                    if let Some((key, value)) = line.split_once(':') {
                        if key.eq_ignore_ascii_case("content-length") { length = value.trim().parse().unwrap(); }
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                if length > 0 {
                    assert_eq!(serde_json::from_slice::<Value>(&body).unwrap()["model"], "test:latest");
                }
                write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
            }
        });
        let settings = Settings {
            assistant_provider: "ollama".into(),
            ollama_model: "test:latest".into(),
            ollama_server_url: base.clone(),
            claude_bin: std::env::current_exe().unwrap().to_string_lossy().into_owned(),
            ollama_trusted_tools: vec!["Read".into()],
            ..Settings::default()
        };
        assert_eq!(ensure_model(&settings).unwrap(), "test:latest");
        fixture.join().unwrap();
        let command = crate::process::chat_command(&settings, std::path::Path::new(".")).unwrap();
        assert!(command.get_args().any(|value| value == "--permission-prompt-tool"));
        assert!(command.get_args().any(|value| value == "--tools"));
        assert!(command.get_envs().any(|(key, value)| key == "ANTHROPIC_BASE_URL" && value == Some(std::ffi::OsStr::new(&base))));
        assert!(settings.connector_settings().trusted_tools.is_empty());
        assert!(server_base("http://192.168.1.2:11434").is_ok());
        assert!(server_base("http://example.com:11434").is_err());
        assert!(server_base("https://example.com/v1").is_err());
    }
    #[test]
    fn context_variant_reuses_the_selected_model_and_honors_its_limit() {
        let mut settings=Settings {ollama_model:"qwen3:0.6b".into(),..Default::default()};
        let details=json!({"capabilities":["completion","tools"],"model_info":{"qwen3.context_length":40960}});
        let body=context_request(&settings,&details).unwrap();
        assert_eq!(body["from"],"qwen3:0.6b");
        assert_eq!(body["parameters"]["num_ctx"],32768);
        assert_eq!(body["stream"],false);
        let first=body["model"].as_str().unwrap();
        assert!(first.starts_with("parlo-local/"));
        settings.ollama_context_length=24576;
        assert_ne!(context_request(&settings,&details).unwrap()["model"],first);
        settings.ollama_context_length=65536;
        assert!(context_request(&settings,&details).is_err());
        settings.ollama_context_length=131072;
        let large=json!({"capabilities":["tools"],"model_info":{"deepseek.context_length":1048576}});
        assert_eq!(context_request(&settings,&large).unwrap()["parameters"]["num_ctx"],131072);
        settings.ollama_context_length=100000;
        assert_eq!(context_request(&settings,&large).unwrap()["parameters"]["num_ctx"],100000);
        assert!(context_request(&settings,&json!({"capabilities":["tools"],"remote_host":"https://ollama.com"})).is_err());
    }
    #[test]
    fn context_preview_has_no_fixed_tool_charge_and_oversized_text_stays_available() {
        let settings=Settings::default();
        let text="Città 🙂";
        let budget=context_budget(&settings,text,"Istruzioni");
        assert_eq!(budget.input_tokens,(text.chars().count()+10).div_ceil(2));
        assert_eq!(budget.reserved_tokens,ANSWER_RESERVE);
        let long="Trascrizione sintetica. ".repeat(10000);
        let original=long.clone();
        let preview=context_budget(&settings,&long,"");
        assert!(preview.input_tokens > preview.limit as usize);
        assert_eq!(long,original);
        assert_eq!(context_budget(&settings,"","").input_tokens,0);
    }
    #[test]
    fn context_updates_report_input_usage_and_cli_compaction_without_tool_content() {
        let value=json!({"type":"assistant","message":{"usage":{"input_tokens":100,"cache_read_input_tokens":20,"cache_creation_input_tokens":30,"output_tokens":50}}});
        assert_eq!(context_update("synthetic",&value).unwrap(),json!({"id":"synthetic","input_tokens":150}));
        let streamed=json!({"type":"stream_event","event":{"type":"message_start","message":{"usage":{"input_tokens":42}}}});
        assert_eq!(context_update("synthetic",&streamed).unwrap()["input_tokens"],42);
        assert_eq!(context_update("synthetic",&json!({"type":"system","subtype":"compact_boundary"})).unwrap()["compacted"],true);
        assert!(context_update("synthetic",&json!({"type":"user","message":{"content":"PRIVATE TOOL DATA"}})).is_none());
    }
    #[test]
    fn ollama_catalog_contains_only_installed_local_generation_models() {
        let catalog = parse_catalog(&json!({"models":[
            {"name":"local:small","details":{"parameter_size":"4B","quantization_level":"Q4_K_M"}},
            {"name":"local:small"}, {"name":"parlo-local/generated:ctx16384"}, {"name":"presentio-local/generated:ctx16384"}, {"name":"other:cloud"}, {"name":"other:123-cloud"},
            {"name":"alias:latest","remote_model":"large","remote_host":"https://ollama.com"},
            {"name":"embedding:latest","capabilities":["embedding"]}, {"name":"--bad"}
        ]}))
        .unwrap();
        assert_eq!(catalog.models.len(), 1);
        assert_eq!(catalog.models[0].value, "local:small");
        assert!(catalog.models[0].description.contains("4B"));
        assert!(parse_catalog(&json!({"models":[]}))
            .unwrap()
            .models
            .is_empty());
        assert!(parse_catalog(&json!({})).is_err());
    }
    #[test]
    fn ollama_preflight_rejects_remote_aliases_and_models_without_tools() {
        assert!(validate_details(&json!({"capabilities":["completion","tools"]}), true).is_ok());
        assert!(validate_details(&json!({"capabilities":["completion"]}), true).is_err());
        assert!(validate_details(&json!({"capabilities":["completion"]}), false).is_ok());
        assert!(validate_details(
            &json!({"capabilities":["tools"],"remote_host":"https://ollama.com"}), true
        )
        .is_err());
        assert!(
            validate_details(&json!({"capabilities":["tools"],"remote_model":"cloud-model"}), true)
                .is_err()
        );
    }
    #[test]
    fn ollama_environment_routes_every_model_role_locally_without_cloud_credentials() {
        let mut command = Command::new("unused");
        configure(&mut command, &Settings::default(), "local:small").unwrap();
        let env: std::collections::HashMap<_, _> = command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        assert_eq!(env["ANTHROPIC_BASE_URL"].as_deref(), Some(BASE_URL));
        assert_eq!(env["ANTHROPIC_API_KEY"].as_deref(), Some(""));
        assert_eq!(env["CLAUDE_CODE_OAUTH_TOKEN"], None);
        assert_eq!(env["CLAUDE_CODE_SIMPLE"], None);
        assert_eq!(env["CLAUDE_CODE_BARE"], None);
        assert!(env["CLAUDE_CONFIG_DIR"].as_deref().unwrap().ends_with("ollama-claude-config"));
        assert_eq!(env["ENABLE_TOOL_SEARCH"].as_deref(), Some("false"));
        #[cfg(windows)]
        assert_eq!(env["CLAUDE_CODE_USE_POWERSHELL_TOOL"].as_deref(), Some("1"));
        assert_eq!(env["MAX_THINKING_TOKENS"].as_deref(),Some("0"));
        assert_eq!(env["CLAUDE_CODE_MAX_OUTPUT_TOKENS"].as_deref(),Some("4096"));
        assert_eq!(
            env["ANTHROPIC_DEFAULT_HAIKU_MODEL"].as_deref(),
            Some("local:small")
        );
        assert_eq!(
            env["CLAUDE_CODE_SUBAGENT_MODEL"].as_deref(),
            Some("local:small")
        );
        assert_eq!(
            env["CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"].as_deref(),
            Some("1")
        );
    }
    #[test]
    fn ollama_is_opt_in_and_tool_permissions_are_separate_from_cloud_accounts() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.assistant_provider, "claude");
        assert!(settings.ollama_model.is_empty());
        settings.assistant_provider = "ollama".into();
        settings.ollama_model = "local:small".into();
        settings.trusted_tools.push("Write".into());
        settings.validate().unwrap();
        let effective = settings.connector_settings();
        assert!(!effective.claude_connectors);
        assert!(!crate::connectors::tool_trusted(
            &effective,
            &json!({"tool_name":"Write"})
        ));
        assert!(!crate::connectors::tool_enabled(
            &effective,
            &json!({"tool_name":"WebSearch"})
        ));
        assert!(!crate::connectors::tool_enabled(
            &effective,
            &json!({"tool_name":"mcp__drive__read"})
        ));
        assert!(crate::connectors::tool_enabled(
            &effective,
            &json!({"tool_name":"Read"})
        ));
        for tool in ["Bash", "PowerShell"] {
            assert!(crate::connectors::tool_enabled(&effective, &json!({"tool_name":tool})));
        }
        assert!(!settings.ollama_web_search);
        for tool in ["WebFetch", "mcp__parlo_web__WebSearch"] {
            assert!(!crate::connectors::tool_enabled(&effective, &json!({"tool_name":tool})));
        }
        let mut enabled=effective.clone(); enabled.ollama_web_search=true;
        for tool in ["WebFetch", "mcp__parlo_web__WebSearch"] {
            assert!(crate::connectors::tool_enabled(&enabled, &json!({"tool_name":tool})));
        }
        let mut no_search=effective.clone(); no_search.ollama_web_search=false;
        assert!(!crate::connectors::tool_enabled(&no_search,&json!({"tool_name":"mcp__parlo_web__WebSearch"})));
        settings.active_trusted_mut().push("Read".into());
        assert_eq!(settings.trusted_tools, vec!["Write"]);
        assert_eq!(settings.ollama_trusted_tools, vec!["Read"]);
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.ollama_model, "local:small");
    }
    #[test]
    fn ollama_harness_keeps_full_tools_stdio_permissions_and_explicit_tts_mcp() {
        let settings = Settings {
            assistant_provider: "ollama".into(),
            ollama_full_tools: true,
            ollama_model: "local:small".into(),
            claude_bin: std::env::current_exe()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            tts_enabled: true,
            tts_provider: "piper".into(),
            ollama_web_search: false,
            ..Default::default()
        };
        let command = crate::process::chat_command(&settings, std::path::Path::new(".")).unwrap();
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(!args.iter().any(|arg| arg == "--bare"));
        assert!(args.iter().any(|arg| arg == "--strict-mcp-config"));
        assert!(args.windows(2).any(|args| args == ["--tools", "default"]));
        assert!(args
            .windows(2)
            .any(|args| args == ["--permission-prompt-tool", "stdio"]));
        assert!(args
            .windows(2)
            .any(|args| args == ["--permission-mode", "default"]));
        let config = args
            .windows(2)
            .find(|args| args[0] == "--mcp-config")
            .unwrap();
        let config: Value = serde_json::from_str(&config[1]).unwrap();
        assert_eq!(config["mcpServers"].as_object().unwrap().len(), 1);
        assert!(config["mcpServers"].get("parlo_tts").is_some());
        let config = args
            .windows(2)
            .find(|args| args[0] == "--settings")
            .unwrap();
        let config: Value = serde_json::from_str(&config[1]).unwrap();
        assert_eq!(config["disableClaudeAiConnectors"], true);
        assert!(args.iter().any(|arg| arg == "WebSearch"));
    }
}
