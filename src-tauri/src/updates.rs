use crate::{LockExt, Shared};
use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Clone, Default, Serialize)]
pub struct Status {
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub phase: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub notice: Option<String>,
}

#[derive(Default)]
pub struct Updates(Mutex<Pending>);

#[derive(Default)]
struct Pending {
    status: Status,
    update: Option<Update>,
    bytes: Option<Vec<u8>>,
}

pub fn enabled(config: &tauri::Config) -> bool {
    !cfg!(debug_assertions) && config.plugins.0.get("updater")
        .and_then(|updater| updater.get("endpoints"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|endpoints| !endpoints.is_empty())
}

fn snapshot(app: &tauri::AppHandle, pending: &Pending) -> Status {
    let mut status = pending.status.clone();
    status.current_version = app.package_info().version.to_string();
    if !enabled(app.config()) {
        status.phase = "disabled".into();
    } else if status.phase.is_empty() {
        status.phase = "idle".into();
    }
    status
}

fn emit(app: &tauri::AppHandle, pending: &Pending) {
    let _ = app.emit("update-status", snapshot(app, pending));
}

fn fresh_manifest_endpoints(config: Option<&serde_json::Value>, request_id: &str) -> tauri_plugin_updater::Result<Vec<tauri::Url>> {
    let mut endpoints: Vec<tauri::Url> = serde_json::from_value(
        config.and_then(|value| value.get("endpoints")).cloned().unwrap_or(serde_json::Value::Null)
    )?;
    // GitHub can cache the /latest redirect separately from the release asset.
    // Ask for a fresh redirect when another platform/version was just published.
    for endpoint in &mut endpoints {
        endpoint.query_pairs_mut().append_pair("parlo_check", request_id);
    }
    Ok(endpoints)
}

fn wait_for_platform(pending: &mut Pending) {
    pending.status.phase = "waiting_platform".into();
    pending.status.version = None;
    pending.status.notes = None;
    pending.status.downloaded = 0;
    pending.status.total = None;
    pending.bytes = None;
    pending.update = None;
    pending.status.notice = Some("La nuova versione non è ancora disponibile per questo sistema.".into());
}

fn temporary_check_error(error: &tauri_plugin_updater::Error) -> bool {
    match error {
        tauri_plugin_updater::Error::Reqwest(error) => error.is_timeout() || error.is_connect()
            || error.is_request() || error.is_body()
            || error.status().is_some_and(|status| status.is_server_error() || matches!(status.as_u16(), 408 | 429)),
        // The updater also uses ReleaseNotFound for non-success HTTP replies,
        // including a temporarily unavailable release server.
        tauri_plugin_updater::Error::Network(_) | tauri_plugin_updater::Error::ReleaseNotFound => true,
        _ => false,
    }
}

#[tauri::command]
pub fn update_status(app: tauri::AppHandle, updates: State<Updates>) -> Status {
    snapshot(&app, &updates.0.guard())
}

#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<Status, String> {
    {
        let updates = app.state::<Updates>();
        let mut pending = updates.0.guard();
        if !enabled(app.config()) { return Ok(snapshot(&app, &pending)); }
        if matches!(pending.status.phase.as_str(), "checking" | "downloading" | "installing" | "ready") {
            return Ok(snapshot(&app, &pending));
        }
        pending.status.phase = "checking".into();
        pending.status.error = None;
        pending.status.notice = None;
        emit(&app, &pending);
    }
    let result = async {
        let cleanup_app = app.clone();
        app.updater_builder()
            .endpoints(fresh_manifest_endpoints(app.config().plugins.0.get("updater"), &uuid::Uuid::new_v4().to_string())?)?
            .header("Cache-Control", "no-cache")?
            .timeout(Duration::from_secs(20))
            .on_before_exit(move || {
                let state = cleanup_app.state::<Shared>();
                state.auth.cancel();
                state.engine.guard().take();
                state.live_engine.guard().take();
            })
            .build()?
            .check().await
    }.await;
    let updates = app.state::<Updates>();
    let mut pending = updates.0.guard();
    match result {
        Ok(update) => {
            pending.status.version = update.as_ref().map(|u| u.version.clone());
            pending.status.notes = update.as_ref().and_then(|u| u.body.clone());
            pending.status.phase = if update.is_some() { "available" } else { "current" }.into();
            pending.status.downloaded = 0;
            pending.status.total = None;
            pending.bytes = None;
            pending.update = update;
        }
        Err(tauri_plugin_updater::Error::TargetNotFound(_)) | Err(tauri_plugin_updater::Error::TargetsNotFound(_)) => {
            wait_for_platform(&mut pending);
        }
        Err(error) => {
            eprintln!("Verifica aggiornamenti: {error}");
            let temporary = temporary_check_error(&error);
            pending.status.phase = if pending.update.is_some() { "available" }
                else if temporary { "unavailable" } else { "error" }.into();
            if temporary {
                pending.status.notice = Some("Controllo aggiornamenti momentaneamente non disponibile. Riproveremo automaticamente; puoi anche riprovare ora.".into());
            } else {
                pending.status.error = Some("Impossibile verificare gli aggiornamenti. Riprova tra poco.".into());
            }
        }
    }
    emit(&app, &pending);
    Ok(snapshot(&app, &pending))
}

// Called under the same lock used to start recording, inference, chat and provisioning.
pub(crate) fn ensure_idle(state: &Shared) -> Result<(), String> {
    let status = state.status.guard();
    if status.capture.is_some() || status.processing.is_some() || !status.queued.is_empty() || status.chat_busy {
        return Err("Termina la registrazione e attendi elaborazione e chat prima di aggiornare.".into());
    }
    if state.provision.0.guard().running {
        return Err("Attendi il completamento della preparazione del motore prima di aggiornare.".into());
    }
    Ok(())
}

pub fn ensure_not_installing(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(updates) = app.try_state::<Updates>() {
        if updates.0.guard().status.phase == "installing" {
            return Err("Aggiornamento dell’app in corso. Attendi il riavvio.".into());
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<(), String> {
    if cfg!(debug_assertions) {
        return Err("Gli aggiornamenti si installano dalla versione distribuita dell’app.".into());
    }
    if !enabled(app.config()) {
        return Err(crate::locale::tr("Questa build non ha un canale di aggiornamento.", "This build has no update channel.").into());
    }
    let state = state.inner().clone();
    {
        let _operation = state.operation.guard();
        ensure_idle(&state)?;
    }
    let (update, cached) = {
        let updates = app.state::<Updates>();
        let mut pending = updates.0.guard();
        if !matches!(pending.status.phase.as_str(), "available" | "ready") {
            return Err("Verifica prima la disponibilità di un aggiornamento.".into());
        }
        let update = pending.update.clone().ok_or("Nessun aggiornamento disponibile.")?;
        pending.status.phase = "downloading".into();
        pending.status.error = None;
        pending.status.notice = None;
        pending.status.downloaded = 0;
        pending.status.total = None;
        let cached = pending.bytes.take();
        emit(&app, &pending);
        (update, cached)
    };
    let bytes = match cached {
        Some(bytes) => Ok(bytes),
        None => {
            // Download timeout is separate from the short metadata request timeout.
            let mut download = update.clone();
            download.timeout = Some(Duration::from_secs(1800));
            let mut last_progress = std::time::Instant::now();
            download.download(|chunk, total| {
                let updates = app.state::<Updates>();
                let mut pending = updates.0.guard();
                pending.status.downloaded += chunk as u64;
                pending.status.total = total;
                if last_progress.elapsed() >= Duration::from_millis(250) {
                    emit(&app, &pending);
                    last_progress = std::time::Instant::now();
                }
            }, || {}).await.map_err(|e| format!("Download non riuscito: {e}"))
        }
    };
    let bytes = match bytes {
        Ok(bytes) => bytes,
        Err(error) => {
            let updates = app.state::<Updates>();
            let mut pending = updates.0.guard();
            pending.status.phase = "available".into();
            pending.status.error = Some(error.clone());
            emit(&app, &pending);
            return Err(error);
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        {
            let _operation = state.operation.guard();
            // Work may have started while downloading. Keep the verified bytes
            // until the app is idle, without interrupting the work in progress.
            if let Err(error) = ensure_idle(&state) {
                let updates = app.state::<Updates>();
                let mut pending = updates.0.guard();
                pending.bytes = Some(bytes);
                pending.status.phase = "ready".into();
                pending.status.error = Some(error.clone());
                emit(&app, &pending);
                return Err(error);
            }
            let updates = app.state::<Updates>();
            let mut pending = updates.0.guard();
            pending.status.phase = "installing".into();
            emit(&app, &pending);
        }
        // New work is rejected through ensure_not_installing. Release the operation
        // lock before installation: macOS may need to dispatch to the main thread.
        if let Err(error) = update.install(&bytes) {
            let updates = app.state::<Updates>();
            let mut pending = updates.0.guard();
            pending.bytes = Some(bytes);
            pending.status.phase = "ready".into();
            pending.status.error = Some(format!("Installazione non riuscita: {error}"));
            emit(&app, &pending);
            return Err(error.to_string());
        }
        // Windows exits inside install() and its installer restarts the app.
        // Other desktop platforms need an explicit restart.
        app.restart();
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn update_checks_refresh_the_redirect_without_changing_the_release_endpoint() {
        let config = json!({"endpoints":["https://github.com/example/app/releases/latest/download/latest.json?channel=stable"]});
        let first = fresh_manifest_endpoints(Some(&config), "first").unwrap();
        let second = fresh_manifest_endpoints(Some(&config), "second").unwrap();
        assert_ne!(first, second);
        assert_eq!(first[0].host_str(), Some("github.com"));
        assert_eq!(first[0].path(), "/example/app/releases/latest/download/latest.json");
        assert_eq!(first[0].query_pairs().collect::<Vec<_>>(), vec![("channel".into(),"stable".into()),("parlo_check".into(),"first".into())]);
        assert!(fresh_manifest_endpoints(None, "missing").is_err());
    }

    #[test]
    fn mac_only_release_is_pending_instead_of_reporting_windows_up_to_date() {
        let mut pending = Pending::default();
        pending.status.version = Some("0.2.16".into());
        pending.status.notes = Some("Previous result".into());
        pending.status.downloaded = 100;
        pending.status.total = Some(100);
        pending.bytes = Some(vec![1]);
        wait_for_platform(&mut pending);
        assert_eq!(pending.status.phase, "waiting_platform");
        assert!(pending.status.notice.is_some());
        assert!(pending.status.version.is_none());
        assert!(pending.status.notes.is_none());
        assert_eq!(pending.status.downloaded, 0);
        assert!(pending.status.total.is_none());
        assert!(pending.bytes.is_none());
    }

    #[test]
    fn windows_can_skip_the_mac_only_version() {
        let remote: tauri_plugin_updater::RemoteRelease = serde_json::from_value(json!({
            "version":"0.2.17","platforms":{"windows-x86_64":{
                "url":"https://github.com/example/app/releases/download/v0.2.17/app.exe","signature":"synthetic"
            }}
        })).unwrap();
        assert!(remote.version > "0.2.15".parse().unwrap());
        assert!(remote.version > "0.2.16".parse().unwrap());
    }
}
