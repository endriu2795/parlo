use crate::{db, model::Settings, LockExt, Result, Shared};
use std::{path::Path, process::{Command, Stdio}};
use tauri::{AppHandle, State, WebviewWindow};

pub(crate) const RECORDING_BUSY: &str = "Termina la registrazione prima di importare un file.";

pub(crate) fn supported_extension(extension: &str) -> bool {
    ["wav", "mp3", "m4a", "aac", "flac", "ogg", "opus", "wma", "aiff", "mp4", "mov", "mkv", "webm", "avi", "m4v"].contains(&extension)
}

pub(crate) fn import_file(
    app: &AppHandle, state: &Shared, source: &Path, id: &str, title: &str,
    origin: &str,
    on_created: impl FnOnce(&rusqlite::Connection) -> Result<()>,
) -> Result<String> {
    let extension = source.extension().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase();
    if !supported_extension(&extension) {
        return Err("Formato non supportato. Scegli un file audio o video comune.".into());
    }
    if !source.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Il file selezionato non è leggibile.".into());
    }
    let settings: Settings = db::get_settings(&state.db.guard())?;
    crate::stt::ensure_ready(&settings)?;
    let _power = crate::power::ProcessingPower::new();
    let python = if settings.remote_stt() { crate::provision::media_python(app)? }
        else { std::path::PathBuf::from(&settings.python_bin) };
    let directory = crate::temp_dir(state, id)?;
    std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let wav = directory.join("system.wav");
    let converted = (|| {
        let mut command = Command::new(python);
        command.arg("-I").arg(state.data_dir.join("import_media.py")).arg(source).arg(&wav)
            .env_remove("PYTHONHOME").env_remove("PYTHONPATH")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        #[cfg(windows)] {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let output = command.output().map_err(|e| format!("Conversione audio non avviata: {e}"))?;
        if !output.status.success() {
            let reason = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Impossibile estrarre l’audio: {}", reason.lines().last().unwrap_or("formato non leggibile").chars().take(300).collect::<String>()));
        }
        let reader = hound::WavReader::open(&wav).map_err(|e| e.to_string())?;
        let duration = i64::from(reader.duration()) * 1000 / 16000;
        if duration == 0 { return Err("Il file non contiene audio da trascrivere.".into()); }
        Ok(duration)
    })();
    let duration = match converted {
        Ok(value) => value,
        Err(error) => { let _ = crate::remove_audio(state, id); return Err(error); }
    };
    let result = (|| {
        let _guard = state.operation.guard();
        if state.status.guard().capture.is_some() { return Err(RECORDING_BUSY.into()); }
        crate::updates::ensure_not_installing(app)?;
        if settings.keep_audio { std::fs::write(directory.join("keep-audio"), b"").map_err(|e| e.to_string())?; }
        let title = title.chars().take(120).collect::<String>();
        let now = chrono::Utc::now().to_rfc3339();
        {
            let mut db = state.db.guard();
            let transaction = db.transaction().map_err(|e| e.to_string())?;
            db::create_with_origin(&transaction, id, &title, &now, origin)?;
            db::status(&transaction, id, "pending", Some(duration), None)?;
            on_created(&transaction)?;
            transaction.commit().map_err(|e| e.to_string())?;
        }
        if let Err(error) = crate::processing::enqueue(app, state, id) {
            let _ = db::status(&state.db.guard(), id, "failed", Some(duration), Some(&error));
            crate::archive_changed(app, Some(id));
            return Err(error);
        }
        crate::changed(app, state);
        crate::archive_changed(app, Some(id));
        Ok(id.to_owned())
    })();
    if result.is_err() && db::detail(&state.db.guard(), id).is_err() { let _ = crate::remove_audio(state, id); }
    result
}

#[tauri::command]
pub async fn import_media(app: AppHandle, window: WebviewWindow, state: State<'_, Shared>) -> Result<Option<String>> {
    if window.label() != "main" { return Err("Importazione disponibile dalla finestra principale.".into()); }
    let paths = crate::sources::pick(app.clone(), window, false, true).await?;
    let Some(source) = paths.into_iter().next() else { return Ok(None); };
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let title = source.file_stem().and_then(|s| s.to_str()).unwrap_or("File importato");
        import_file(&app, &state, &source, &uuid::Uuid::new_v4().to_string(), title, "file", |_| Ok(())).map(Some)
    })
        .await.map_err(|e| e.to_string())?
}
