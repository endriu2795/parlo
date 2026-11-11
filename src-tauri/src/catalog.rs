//! Read Claude Code's initialize response, the same control-plane source used by
//! Agent SDK Query.supportedModels(). Never sends a user message or model request.
use crate::LockExt;
use crate::{model::Settings, process};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::Stdio,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub value: String,
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub resolved_model: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct Catalog {
    pub models: Vec<ModelInfo>,
    pub checked_at: String,
    pub cli_version: Option<String>,
}
pub struct Cache {
    pub key: String,
    pub fetched: Instant,
    pub catalog: Catalog,
}
pub fn parse(value: &serde_json::Value, id: &str) -> Result<Option<Catalog>, String> {
    if value["type"] != "control_response" || value["response"]["request_id"] != id {
        return Ok(None);
    }
    let response = &value["response"];
    if response["subtype"] == "error" {
        return Err(response["error"]
            .as_str()
            .unwrap_or("Catalogo modelli non disponibile")
            .into());
    }
    let data = &response["response"];
    let models:Vec<ModelInfo>=serde_json::from_value(data["models"].clone()).map_err(|_|"Claude Code non ha restituito il catalogo dei modelli. Aggiorna il binario configurato.".to_string())?;
    let mut cleaned = Vec::new();
    for model in models {
        if !model.value.trim().is_empty()
            && !model.display_name.trim().is_empty()
            && !cleaned.iter().any(|m: &ModelInfo| m.value == model.value)
        {
            cleaned.push(model);
        }
    }
    if cleaned.is_empty() {
        return Err("Claude Code ha restituito un catalogo vuoto.".into());
    }
    Ok(Some(Catalog {
        models: cleaned,
        checked_at: chrono::Utc::now().to_rfc3339(),
        cli_version: data["claude_code_version"].as_str().map(String::from),
    }))
}
pub fn fetch(settings: &Settings, directory: &Path) -> Result<Catalog, String> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let mut command = process::claude_command(settings, directory)?;
    command.args(["--input-format", "stream-json"]);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|e| format!("Avvio catalogo Claude fallito: {e}"))?;
    let mut input = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let child = Arc::new(Mutex::new(child));
    let result = (|| {
        writeln!(input,"{}",serde_json::json!({"type":"control_request","request_id":id,"request":{"subtype":"initialize","hooks":null}})).map_err(|e|e.to_string())?;
        input.flush().map_err(|e| e.to_string())?;
        let (send, receive) = mpsc::channel();
        let request_id = id.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                match parse(&value, &request_id) {
                    Ok(None) => {}
                    other => {
                        let _ = send.send(other.and_then(|c| c.ok_or("Catalogo mancante".into())));
                        return;
                    }
                }
            }
        });
        receive.recv_timeout(Duration::from_secs(60)).map_err(|_|"Catalogo Claude non disponibile entro un minuto. Verifica account e connessione e riprova.".to_string())?
    })();
    drop(input);
    let mut child = child.guard();
    let _ = child.kill();
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_uses_provider_names_versions_and_new_families() {
        let value = serde_json::json!({"type":"control_response","response":{"subtype":"success","request_id":"test","response":{"models":[{"value":"new-family","displayName":"Future 9.2","resolvedModel":"claude-future-9-2","description":"Provider supplied"},{"value":"new-family","displayName":"Duplicate"}]}}});
        let catalog = parse(&value, "test").unwrap().unwrap();
        assert_eq!(catalog.models.len(), 1);
        assert_eq!(catalog.models[0].display_name, "Future 9.2");
        assert_eq!(
            catalog.models[0].resolved_model.as_deref(),
            Some("claude-future-9-2")
        );
        assert!(parse(&value, "other-session").unwrap().is_none());
    }
    #[test]
    fn malformed_or_failed_catalogs_are_not_replaced_with_invented_models() {
        for value in [
            serde_json::json!({"type":"control_response","response":{"request_id":"test","subtype":"error","error":"Unavailable"}}),
            serde_json::json!({"type":"control_response","response":{"request_id":"test","response":{"models":[]}}}),
        ] {
            assert!(parse(&value, "test").is_err());
        }
    }
}
