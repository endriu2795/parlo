#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod video;
mod playback;
mod import_media;
mod telegram;
mod sharing;
mod custom_endpoint;
mod appearance;
mod auth;
mod catalog;
mod ollama;
mod connectors;
mod activity;
mod db;
mod diagnostics;
mod export;
mod model;
mod notifications;
mod locale;
mod process;
mod processing;
mod engine_child;
mod provision;
mod python_runtime;
mod document_runtime;
mod tts;
mod web_tools;
mod piper;
mod kokoro;
mod qwen_tts;
mod stt;
mod power;
mod claude_runtime;
mod codex_runtime;
mod codex;
mod gemini;
mod gemini_runtime;
mod crash;
mod data_dir;
mod transcript_quality;
mod screen;
mod screen_dock;
mod call_notes;
mod drawing_interaction;
mod updates;
mod sources;
mod chat_files;
#[cfg(target_os = "macos")]
mod mac_audio;
mod mac_permissions;

use locale::tr;
use model::*;
use rusqlite::Connection;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub status: Mutex<Status>,
    pub recorder: Mutex<Option<audio::Recording>>,
    pub operation: Mutex<()>,
    pub data_dir: PathBuf,
    pub processing: Mutex<processing::Queue>,
    pub engine: Mutex<Option<process::Engine>>,
    pub live_engine: Mutex<Option<process::Engine>>,
    pub live_process: Mutex<Option<Arc<engine_child::EngineChild>>>,
    pub auth: auth::AuthManager,
    pub codex_auth: codex::AuthManager,
    pub gemini_auth: gemini::AuthManager,
    pub catalog: Mutex<Option<catalog::Cache>>,
    pub chat_control: connectors::ChatControl,
    pub shortcut_capture: std::sync::atomic::AtomicBool,
    pub provision: provision::Provision,
}
type Shared = Arc<AppState>;
/// Bundle identifier, also used as the Windows notification identity (AppUserModelID).
pub const APP_ID: &str = "app.parlo.desktop";
/// A panic on one thread poisons the mutexes it held: later users take the value back
/// instead of panicking in turn and closing the whole app.
pub(crate) trait LockExt<T> {
    fn guard(&self) -> std::sync::MutexGuard<'_, T>;
}
impl<T> LockExt<T> for Mutex<T> {
    fn guard(&self) -> std::sync::MutexGuard<'_, T> {
        self.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
type Result<T> = std::result::Result<T, String>;
pub fn changed(app: &tauri::AppHandle, state: &AppState) {
    let status = state.status.guard().clone();
    if let Some(tray) = app.tray_by_id("callnotes") {
        let tooltip = if status.capture.is_some() {
            "PARLO · REGISTRAZIONE IN CORSO"
        } else if status.processing.is_some() {
            "PARLO · elaborazione locale"
        } else {
            "PARLO · pronto"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
    let _ = app.emit("status-changed", status);
}
fn detail_changed(app: &tauri::AppHandle, id: &str) {
    let _ = app.emit("call-detail-changed", id);
}
fn archive_changed(app: &tauri::AppHandle, id: Option<&str>) {
    if let Some(id) = id { detail_changed(app, id); }
    let _ = app.emit("calls-changed", id);
}
pub fn temp_dir(state: &AppState, id: &str) -> Result<PathBuf> {
    let parsed = uuid::Uuid::parse_str(id).map_err(|_| "Identificatore call non valido")?;
    Ok(state.data_dir.join("temporary").join(parsed.to_string()))
}
pub fn remove_audio(state: &AppState, id: &str) -> Result<()> {
    let dir = temp_dir(state, id)?;
    if !dir.exists() {
        return Ok(());
    }
    let parent = state
        .data_dir
        .join("temporary")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let canonical = dir.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent() != Some(parent.as_path()) {
        return Err("Percorso audio temporaneo non sicuro".into());
    }
    std::fs::remove_dir_all(&dir)
        .map_err(|e| format!("Impossibile eliminare l’audio temporaneo: {e}"))
}
fn retained_audio_directory(state: &AppState, id: &str) -> Result<Option<PathBuf>> {
    let dir=temp_dir(state,id)?;
    if !dir.join("keep-audio").is_file() { return Ok(None); }
    let parent=state.data_dir.join("temporary").canonicalize().map_err(|e|e.to_string())?;
    let canonical=dir.canonicalize().map_err(|e|e.to_string())?;
    if canonical.parent()!=Some(parent.as_path()) { return Err("Percorso audio non sicuro".into()); }
    Ok(["registrazione.wav","microphone.wav","system.wav"].iter().any(|name|canonical.join(name).is_file()).then_some(dir))
}
fn finalize_audio(state: &AppState, id: &str) -> Result<()> {
    if temp_dir(state, id)?.join("keep-video").is_file() && video::retained(state, id)?.is_none() {
        return Err("Video da completare: file temporanei conservati. Riprova l’elaborazione.".into());
    }
    if let Some(dir) = retained_audio_directory(state,id)? {
        playback::consolidate(&dir, true)?;
    } else { remove_audio(state,id)?; }
    Ok(())
}
fn recover_finalized_calls(state: &AppState) -> Result<Vec<String>> {
    let mut ready = vec![];
    let conn = state.db.guard();
    for id in db::finalizing(&conn)? {
        let warning = db::detail(&conn, &id)?.call.error;
        let (status, error) = match finalize_audio(state, &id) {
            Ok(()) => ("ready", warning),
            Err(error) => ("failed", Some(error)),
        };
        db::status(&conn, &id, status, None, error.as_deref())?;
        if status == "ready" { ready.push(id); }
    }
    Ok(ready)
}
fn cleanup_core(state: &AppState) -> Result<usize> {
    let _guard = state.operation.guard();
    let mut conn = state.db.guard();
    let config = db::get_settings(&conn)?;
    let ids = db::expired(&conn, config.retention_months)?;
    let chat = state.status.guard().chat_busy;
    if chat {
        return Ok(0);
    }
    for id in &ids {
        remove_audio(state, id)?;
        screen::remove(state, id)?;
        db::delete(&mut conn, id)?;
    }
    Ok(ids.len())
}
fn cleanup_archive(app: &tauri::AppHandle, state: &Shared) -> Result<usize> {
    let result = cleanup_core(state);
    // Earlier removals may have succeeded before a later one failed.
    if !matches!(result, Ok(0)) {
        archive_changed(app, None);
        let _ = app.emit("archive-tags-changed", ());
    }
    let removed = result?;
    sharing::schedule_expiration(app, state);
    sharing::schedule_title_sync(app, state, None);
    Ok(removed)
}
#[tauri::command]
fn list_calls(
    state: State<Shared>,
    query: String,
    cursor: Option<model::CallCursor>,
    call_id: Option<String>,
    limit: usize,
    date_from: Option<String>,
    date_to: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<CallPage> {
    let slice = match (cursor.as_ref(), call_id.as_deref()) {
        (_, Some(id)) => db::ArchiveSlice::Call(id),
        (Some(cursor), None) => db::ArchiveSlice::After(cursor),
        (None, None) => db::ArchiveSlice::First,
    };
    db::page_filtered(
        &state.db.guard(),
        &query,
        limit.clamp(1, 100),
        date_from.as_deref(),
        date_to.as_deref(),
        tags.as_deref().unwrap_or_default(),
        slice,
    )
}
#[tauri::command]
fn list_tags(state: State<Shared>) -> Result<Vec<TagCount>> {
    db::tags(&state.db.guard())
}
#[tauri::command]
async fn get_call(state: State<'_, Shared>, id: String) -> Result<Detail> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut detail = db::detail(&state.db.guard(), &id)?;
        sharing::hydrate_shared_detail(&state, &mut detail)?;
        detail.audio_directory = retained_audio_directory(&state, &id)?.map(|p| p.to_string_lossy().into_owned());
        detail.video_path = video::retained(&state, &id)?.map(|p| p.to_string_lossy().into_owned());
        Ok(detail)
    }).await.map_err(|_| "Lettura della registrazione interrotta.".to_string())?
}
#[tauri::command]
async fn prepare_call_audio(app: tauri::AppHandle, state: State<'_, Shared>, id: String) -> Result<String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = state.operation.guard();
        let detail = db::detail(&state.db.guard(), &id)?;
        if !["ready", "failed"].contains(&detail.call.status.as_str()) { return Err("Attendi il termine dell’elaborazione per ascoltare l’audio.".into()); }
        let dir = retained_audio_directory(&state, &id)?.ok_or("Audio conservato non disponibile")?;
        let path = playback::consolidate(&dir, detail.call.status == "ready")?.canonicalize().map_err(|e| e.to_string())?;
        app.asset_protocol_scope().allow_file(&path).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().into_owned())
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_call_audio(state: State<Shared>, id: String) -> Result<()> {
    db::detail(&state.db.guard(),&id)?;
    let dir=retained_audio_directory(&state,&id)?.ok_or("Audio conservato non disponibile per questa call.")?;
    let program=if cfg!(windows) {"explorer.exe"} else if cfg!(target_os="macos") {"open"} else {"xdg-open"};
    std::process::Command::new(program).arg(dir).spawn().map_err(|e|e.to_string())?;
    Ok(())
}
#[tauri::command]
fn get_settings(state: State<Shared>) -> Result<Settings> {
    db::get_settings(&state.db.guard())
}
#[tauri::command]
fn get_status(state: State<Shared>) -> Status {
    state.status.guard().clone()
}
#[tauri::command]
fn get_health(app: tauri::AppHandle, state: State<Shared>) -> Result<serde_json::Value> {
    let s = db::get_settings(&state.db.guard())?;
    let root = engine_dir();
    let defaults = Settings::default();
    let acceleration = std::fs::read(root.join("hardware-profile.json")).ok()
        .and_then(|data| serde_json::from_slice::<serde_json::Value>(&data).ok());
    let models: Vec<_> = ["turbo", "tiny", "small"].iter().map(|id| {
        let path = root.join("models").join(format!("whisper-{id}"));
        serde_json::json!({"id":id,"path":path.to_string_lossy(),"ready":model_ready(&path, false),"preview_ready":preview_model_ready(&path, false),"preview_cpu_ready":model_ready(&path, true),"download_size":match *id {"tiny"=>"circa 80–160 MB", "small"=>"circa 0,5–0,8 GB", _=>"circa 0,8–2,5 GB"}})
    }).collect();
    Ok(serde_json::json!({"windows_capture":cfg!(windows),"capture_platform":if cfg!(windows){"windows"}else if cfg!(target_os="macos"){"macos"}else{"unsupported"},"python_available":provision::python(&s).is_some(),"shortcut_ready":app.global_shortcut().is_registered(s.shortcut.as_str()),"whisper_ready":s.whisper_ready(),"transcription_ready":stt::ready(&s),"preview_ready":s.preview_ready(),"diarization_ready":s.diarization_ready(),"claude_ready":process::executable(&s.claude_bin).is_ok(),"codex_ready":process::executable(&s.codex_bin).is_ok(),"gemini_ready":gemini_runtime::command(&s.gemini_bin).is_ok(),"data_dir":state.data_dir.to_string_lossy(),"engine_root":root.to_string_lossy(),"models":models,"acceleration":acceleration,"default_system_prompt":process::default_system_prompt(),"engine_defaults":{"python_bin":defaults.python_bin,"whisper_model":defaults.whisper_model,"live_whisper_model":defaults.live_whisper_model,"segmentation_model":defaults.segmentation_model,"embedding_model":defaults.embedding_model,"claude_bin":defaults.claude_bin,"codex_bin":defaults.codex_bin,"gemini_bin":defaults.gemini_bin}}))
}
fn register_shortcut(app: &tauri::AppHandle, shortcut: &str) -> Result<()> {
    let shortcut: Shortcut = shortcut
        .parse()
        .map_err(|e| format!("Shortcut non valida: {e}"))?;
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let app = app.clone();
            let state = app.state::<Shared>().inner().clone();
            if state.shortcut_capture.load(std::sync::atomic::Ordering::SeqCst) { return; }
            std::thread::spawn(move || {
                let active = state.status.guard().capture.is_some();
                let result = if active {
                    show_recorder(&app, true)
                } else {
                    start_core(&app, &state)
                };
                if let Err(e) = result {
                    let _ = app.emit("app-error", e);
                }
            });
        })
        .map_err(|e| e.to_string())
}
fn apply_theme(app: &tauri::AppHandle, settings: &Settings) {
    let theme = if settings.theme == "light" { tauri::Theme::Light } else { tauri::Theme::Dark };
    for window in app.webview_windows().values() { let _ = window.set_theme(Some(theme)); appearance::apply(window, settings); }
    let _ = app.emit("theme-changed", &settings.theme);
}
fn register_screen_shortcuts(app: &tauri::AppHandle, settings: &Settings) -> Result<()> {
    let result=(|| {
        for (shortcut,action) in [(&settings.drawing_shortcut,"drawing"),(&settings.screenshot_shortcut,"screenshot"),(&settings.text_shortcut,"text"),(&settings.note_shortcut,"note")] {
            let shortcut:Shortcut=shortcut.parse().map_err(|_|"Scorciatoia non valida")?;
            app.global_shortcut().on_shortcut(shortcut,move|app,_,event| {
                if event.state!=ShortcutState::Pressed {return;}
                let app=app.clone();let state=app.state::<Shared>().inner().clone();
                if state.shortcut_capture.load(std::sync::atomic::Ordering::SeqCst) || state.status.guard().capture.is_none() {return;}
                std::thread::spawn(move|| {
                    let result=match action {"note"=>call_notes::open(&app,&state),"drawing"=>screen::toggle(&app,&state),"text"=>screen::open_editor(&app,&state,Some("text")),_=>screen::request(&app,&state)};
                    if let Err(error)=result {let _=app.emit("app-error",error);}
                });
            }).map_err(|e|format!("Scorciatoia {shortcut} non disponibile: {e}"))?;
        }
        Ok(())
    })();
    if result.is_err() {unregister_screen_shortcuts(app,settings);}
    result
}
fn register_all_shortcuts(app: &tauri::AppHandle, settings: &Settings) -> Result<()> {
    register_shortcut(app,&settings.shortcut)?;
    // Reserve editing hotkeys only while recording, leaving other apps’ shortcuts available.
    if app.state::<Shared>().status.guard().capture.is_some() {register_screen_shortcuts(app,settings)?;}
    Ok(())
}
fn unregister_screen_shortcuts(app: &tauri::AppHandle,settings:&Settings) {
    for shortcut in [&settings.drawing_shortcut,&settings.screenshot_shortcut,&settings.text_shortcut,&settings.note_shortcut] {let _=app.global_shortcut().unregister(shortcut.as_str());}
}
fn unregister_all_shortcuts(app: &tauri::AppHandle,settings:&Settings) {
    let _=app.global_shortcut().unregister(settings.shortcut.as_str());
    unregister_screen_shortcuts(app,settings);
}
#[tauri::command]
fn shortcut_capture(app: tauri::AppHandle, state: State<Shared>, active: bool) -> Result<()> {
    let _guard = state.operation.guard();
    if state.shortcut_capture.load(std::sync::atomic::Ordering::SeqCst) == active { return Ok(()); }
    let settings = db::get_settings(&state.db.guard())?;
    state.shortcut_capture.store(active, std::sync::atomic::Ordering::SeqCst);
    if active {
        unregister_all_shortcuts(&app,&settings);
    } else {
        register_all_shortcuts(&app, &settings)?;
    }
    Ok(())
}
#[tauri::command]
fn engine_setup_status(state: State<Shared>) -> provision::Status {
    state.provision.0.guard().clone()
}
#[tauri::command]
fn engine_setup(app: tauri::AppHandle, state: State<Shared>, model: String, preview: Option<bool>, preview_mode: Option<String>) -> Result<()> {
    start_engine_setup(app, state.inner().clone(), model, preview.unwrap_or(false), preview_mode)
}
fn ensure_engine(app: &tauri::AppHandle, state: &Shared) -> Result<()> {
    if state.provision.0.guard().running { return Ok(()); }
    let settings = db::get_settings(&state.db.guard())?;
    if settings.remote_stt() {
        state.engine.guard().take();
        state.live_engine.guard().take();
        return Ok(());
    }
    let Some(model) = bundled_model_id(&settings.whisper_model) else { return Ok(()); };
    if python_runtime::prepared_installed(&engine_dir()).is_none() || !settings.whisper_ready() {
        start_engine_setup(app.clone(), state.clone(), model.into(), false, None)
    } else if !settings.preview_ready() {
        let live = bundled_model_id(&settings.live_whisper_model).ok_or("Il modello personalizzato dell’anteprima non è disponibile.")?;
        start_engine_setup(app.clone(), state.clone(), live.into(), true, None)
    } else {
        let mut setup = state.provision.0.guard();
        if setup.error.is_some() && (setup.model.as_deref() != Some(model) || (setup.preview && !settings.live_transcription)) {
            setup.error = None;
            setup.message = tr("Trascrizione locale pronta.", "Local transcription is ready.").into();
        }
        Ok(())
    }
}
fn start_engine_setup(app: tauri::AppHandle, state: Shared, model: String, preview: bool, preview_mode: Option<String>) -> Result<()> {
    if !["turbo", "small", "tiny"].contains(&model.as_str()) { return Err("Scegli un modello fornito dall’app per installarlo.".into()); }
    let _guard = state.operation.guard();
    updates::ensure_not_installing(&app)?;
    let status = state.status.guard().clone();
    if status.capture.is_some() || status.processing.is_some() || !status.queued.is_empty() || status.chat_busy {
        return Err("Termina registrazione, elaborazione e chat prima di aggiornare il motore.".into());
    }
    let mut settings = db::get_settings(&state.db.guard())?;
    if let Some(mode) = preview_mode {
        if !["auto", "cpu"].contains(&mode.as_str()) { return Err("Accelerazione dell’anteprima non valida.".into()); }
        settings.live_compute_mode = mode;
    }
    let mut setup = state.provision.0.guard();
    if setup.running { return Err("Installazione già in corso.".into()); }
    *setup = provision::Status { running: true, message: if locale::ui_language() == "it" { format!("Preparazione di Whisper {model}…") } else { format!("Preparing Whisper {model}…") }, model: Some(model.clone()), preview, ..Default::default() };
    drop(setup);
    state.engine.guard().take();
    state.live_engine.guard().take();
    let state = state.clone();
    std::thread::spawn(move || {
        let result = provision::run(&app, &state, &model, preview, &settings);
        let success = result.is_ok();
        let mut setup = state.provision.0.guard();
        setup.running = false;
        match result { Ok(()) => { setup.message = tr("Trascrizione locale pronta.", "Local transcription is ready.").into(); }, Err(error) => { setup.message = tr("Download da completare", "Download incomplete").into(); setup.error=Some(error); } }
        drop(setup);
        let _ = app.emit("configuration-changed", ());
        if success && !preview { if let Err(error) = ensure_engine(&app, &state) { let _ = app.emit("app-error", error); } }
    });
    Ok(())
}
#[tauri::command]
fn set_theme(app: tauri::AppHandle, state: State<Shared>, theme: String) -> Result<Settings> {
    let _guard = state.operation.guard();
    let conn = state.db.guard();
    let mut settings = db::get_settings(&conn)?;
    settings.theme = theme;
    settings.validate()?;
    db::save_settings(&conn, &settings)?;
    apply_theme(&app, &settings);
    Ok(settings)
}
#[derive(serde::Serialize)]
struct SavedSettings {
    settings: Settings,
    cleanup_error: Option<String>,
    engine_error: Option<String>,
}

#[tauri::command]
fn save_settings(app: tauri::AppHandle, state: State<Shared>, mut settings: Settings) -> Result<SavedSettings> {
    let _guard = state.operation.guard();
    let status = state.status.guard().clone();
    if status.capture.is_some() {
        return Err("Termina la registrazione prima di modificare le impostazioni.".into());
    }
    settings.validate()?;
    if settings.remote_stt() { stt::ensure_ready(&settings)?; }
    let conn = state.db.guard();
    let old = db::get_settings(&conn)?;
    if (settings.stt_provider != old.stt_provider || settings.whisper_model != old.whisper_model || settings.live_transcription != old.live_transcription || settings.live_whisper_model != old.live_whisper_model || settings.compute_mode != old.compute_mode || settings.live_compute_mode != old.live_compute_mode)
        && (status.processing.is_some() || !status.queued.is_empty() || status.chat_busy) {
        return Err("Attendi il termine dell’elaborazione e della chat prima di cambiare modello.".into());
    }
    if let Some(python) = python_runtime::prepared_installed(&engine_dir()) {
        if std::path::Path::new(&settings.python_bin).starts_with(engine_dir()) { settings.python_bin = python.to_string_lossy().into_owned(); }
    }
    // Trust changes only through an active permission or explicit revocation.
    settings.trusted_tools = old.trusted_tools.clone();
    settings.codex_trusted_tools = old.codex_trusted_tools.clone();
    settings.ollama_trusted_tools = old.ollama_trusted_tools.clone();
    settings.ollama_server_url = settings.ollama_server_url.trim().to_owned();
    settings.ollama_server_trusted_tools = if settings.ollama_server_url == old.ollama_server_url {
        old.ollama_server_trusted_tools.clone()
    } else { vec![] };
    if status.chat_busy && (settings.ollama_model != old.ollama_model || settings.ollama_context_length != old.ollama_context_length || settings.ollama_server_url != old.ollama_server_url || settings.ollama_tools_enabled != old.ollama_tools_enabled || settings.ollama_full_tools != old.ollama_full_tools) { return Err("Attendi la risposta prima di cambiare le impostazioni Ollama.".into()); }
    if status.chat_busy && settings.assistant_provider != old.assistant_provider { return Err("Attendi la risposta prima di cambiare fornitore AI.".into()); }
    if status.chat_busy && (settings.custom_chat.url != old.custom_chat.url
        || settings.custom_chat.model != old.custom_chat.model
        || settings.custom_chat.tools_enabled != old.custom_chat.tools_enabled) {
        return Err("Attendi la risposta prima di modificare l’endpoint della chat.".into());
    }
    if settings.shortcut != old.shortcut || settings.drawing_shortcut != old.drawing_shortcut || settings.screenshot_shortcut != old.screenshot_shortcut || settings.text_shortcut != old.text_shortcut || settings.note_shortcut != old.note_shortcut {
        unregister_all_shortcuts(&app,&old);
        if let Err(error)=register_all_shortcuts(&app,&settings) {
            unregister_all_shortcuts(&app,&settings);
            let _=register_all_shortcuts(&app,&old);
            return Err(error);
        }
        if let Err(e) = db::save_settings(&conn, &settings) {
            unregister_all_shortcuts(&app,&settings);
            let _=register_all_shortcuts(&app,&old);
            return Err(e);
        }
    } else {
        db::save_settings(&conn, &settings)?;
    }
    drop(conn);
    drop(_guard);
    if !status.chat_busy { state.gemini_auth.discard_chat(); }
    apply_theme(&app, &settings);
    let cleanup_error = cleanup_archive(&app, state.inner()).err();
    let engine_error = ensure_engine(&app, state.inner()).err();
    let _ = app.emit("configuration-changed", ());
    Ok(SavedSettings { settings, cleanup_error, engine_error })
}
fn start_core(app: &tauri::AppHandle, state: &Shared) -> Result<()> {
    start_with_region(app, state, None)
}
fn start_with_region(app: &tauri::AppHandle, state: &Shared, region: Option<video::Region>) -> Result<()> {
    let _guard = state.operation.guard();
    updates::ensure_not_installing(app)?;
    let status = state.status.guard().clone();
    if status.capture.is_some() {
        return show_recorder(app, true);
    }
    if state.provision.0.guard().running { return Err("Installazione del motore in corso. Attendi il completamento.".into()); }
    let mut settings = db::get_settings(&state.db.guard())?;
    settings.validate()?;
    stt::ensure_ready(&settings)?;
    if settings.keep_video && region.is_none() { return video::select(app, state, &settings); }
    if region.is_some() && !settings.keep_video { return Err("La registrazione video è stata disattivata. Avvia di nuovo la registrazione.".into()); }
    if region.is_some() && settings.remote_stt() {
        settings.python_bin = provision::media_python(app)?.to_string_lossy().into_owned();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let dir = temp_dir(state, &id)?;
    let prepared_video = if let Some(region) = region.as_ref() {
        match video::prepare(state, &settings, &dir, region) {
            Ok(prepared) => {
                std::fs::write(dir.join("keep-video"), b"").map_err(|e| e.to_string())?;
                Some(prepared)
            }
            Err(error) => { let _ = remove_audio(state, &id); return Err(error); }
        }
    } else { None };
    if region.is_some() {
        video::hide_selection(app)?;
        // Let the selection controls leave the compositor before recording.
        std::thread::sleep(Duration::from_millis(150));
    }
    // Persist the choice with this call before recording: crash recovery must use
    // its original choice even if the global preference changes later.
    if settings.keep_audio {
        std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
        std::fs::write(dir.join("keep-audio"), b"").map_err(|e|e.to_string())?;
    }
    let now = chrono::Utc::now();
    let info = Capture {
        id: id.clone(),
        started_at: now.to_rfc3339(),
        microphone: settings.microphone,
        system_audio: settings.system_audio,
    };
    let mut recording = match audio::start(app.clone(), &settings, info.clone(), dir) {
        Ok(r) => r,
        Err(e) => {
            drop(prepared_video); // Release the video file before removing a failed startup.
            let _ = remove_audio(state, &id);
            return Err(e);
        }
    };
    if let Some(prepared) = prepared_video {
        if let Err(error) = prepared.attach(app.clone(), &mut recording) {
            let _ = recording.finish();
            let _ = remove_audio(state, &id);
            return Err(error);
        }
    }
    let title = format!("Call — {}", chrono::Local::now().format("%d/%m/%Y, %H:%M"));
    if let Err(e) = db::create(&state.db.guard(), &id, &title, &now.to_rfc3339()) {
        let _ = recording.finish();
        let _ = remove_audio(state, &id);
        return Err(e);
    }
    let stopped = recording.stop.clone();
    *state.recorder.guard() = Some(recording);
    let _=app.emit("drawing-reset",&id);
    {
        let mut status = state.status.guard();
        status.capture = Some(info);
        status.live_segments.clear();
        status.live_stage = if settings.live_transcription {
            tr("Preparazione testo provvisorio…", "Preparing draft transcript…").into()
        } else {
            String::new()
        };
    }
    if let Err(error)=register_screen_shortcuts(app,&settings) {let _=app.emit("app-error",error);}
    // Capture is already running: a recorder-window error must not skip the
    // status notification or the thread that finalizes a device failure.
    // Place the compact indicator while the window is still hidden; otherwise
    // the full recorder flashes on screen before it reaches the edge.
    if let Err(error) = prepare_recorder(app, true).and_then(|_| screen_dock::dock(app)) { let _ = app.emit("app-error", error); }
    if settings.live_transcription {
        start_live(app, state, &id, settings.clone(), stopped.clone());
    }
    if let Err(error) = screen_dock::show_capture(app, state, &id) { let _ = app.emit("app-error", error); }
    changed(app, state);
    archive_changed(app, Some(&id));
    // When a device fails, finalize all remaining tracks and leave a recoverable failed call.
    let app = app.clone();
    let state = state.clone();
    std::thread::spawn(move || {
        while !stopped.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(200));
        }
        if state
            .status
            .guard()
            .capture
            .as_ref()
            .map(|c| c.id.as_str())
            == Some(id.as_str())
        {
            if let Err(e) = stop_core(&app, &state) {
                let _ = app.emit("app-error", e);
            }
        }
    });
    Ok(())
}
fn stop_core(app: &tauri::AppHandle, state: &Shared) -> Result<()> {
    let _power = power::ProcessingPower::new();
    let _guard = state.operation.guard();
    let Some(recording) = state.recorder.guard().take() else {
        return Ok(());
    };
    let id = recording.info.id.clone();
    let duration = recording.start.elapsed().as_millis() as i64;
    let result = recording.finish();
    let _ = video::hide_selection(app);
    screen::clear_selection(app);
    if let Ok(dir) = temp_dir(state, &id) {
        if let Err(error) = db::save_capture_diagnostics(&state.db.guard(), &id, &dir) {
            eprintln!("Diagnostica cattura non archiviata: {error}");
        }
    }
    let draft = state.status.guard().live_segments.clone();
    state.status.guard().capture = None;
    if let Ok(settings)=db::get_settings(&state.db.guard()) {unregister_screen_shortcuts(app,&settings);}
    if let Some(window) = app.get_webview_window("recorder") { let _ = window.hide(); }
    screen_dock::reset(app);
    let _=app.emit("drawing-reset",());
    if let Some(window)=app.get_webview_window("drawing") {let _=window.hide();}
    // Do not wait for the preview inference mutex: it can be decoding a long
    // window. Cancel its complete process tree before starting the final model.
    if let Some(preview) = state.live_process.guard().take() {
        preview.cancel();
    }
    if !draft.is_empty() {
        if let Err(e) = db::store_draft(&mut state.db.guard(), &id, &draft) {
            changed(app, state);
            archive_changed(app, Some(&id));
            notifications::recording_issue(app, &state.data_dir, &id, tr("Registrazione interrotta", "Recording interrupted"), tr("La registrazione si è fermata. L’audio e il video acquisiti sono conservati: apri PARLO per riprovare.", "Recording stopped. Your audio and video were saved. Open PARLO to try again."));
            return Err(e);
        }
    }
    match result {
        Ok((_, duration, _, warning)) => {
            db::status(
                &state.db.guard(),
                &id,
                "pending",
                Some(duration),
                warning.as_deref(),
            )?;
            if let Err(error) = processing::enqueue(app, state, &id) {
                db::status(&state.db.guard(), &id, "failed", Some(duration), Some(&error))?;
                changed(app, state);
                archive_changed(app, Some(&id));
                return Err(error);
            }
        }
        Err(e) => {
            db::status(
                &state.db.guard(),
                &id,
                "failed",
                Some(duration),
                Some(&format!("Registrazione incompleta: {e}")),
            )?;
            changed(app, state);
            archive_changed(app, Some(&id));
            notifications::recording_issue(app, &state.data_dir, &id, tr("Registrazione interrotta", "Recording interrupted"), tr("La registrazione si è fermata. I file acquisiti sono conservati: apri PARLO per vedere l’errore e riprovare.", "Recording stopped. Your files were saved. Open PARLO to see the error and try again."));
            return Err(e);
        }
    }
    changed(app, state);
    archive_changed(app, Some(&id));
    Ok(())
}
fn start_live(
    app: &tauri::AppHandle,
    state: &Shared,
    id: &str,
    settings: Settings,
    stopped: Arc<std::sync::atomic::AtomicBool>,
) {
    let app = app.clone();
    let state = state.clone();
    let id = id.to_string();
    std::thread::spawn(move || {
        let mut cursors = serde_json::json!({});
        let mut next_poll = std::time::Instant::now() + Duration::from_secs(3);
        while !stopped.load(std::sync::atomic::Ordering::SeqCst) {
            while std::time::Instant::now() < next_poll {
                if stopped.load(std::sync::atomic::Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            if stopped.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            next_poll = std::time::Instant::now() + Duration::from_secs(2);
            let Ok(dir) = temp_dir(&state, &id) else {
                return;
            };
            match process::inference(&app, &state, &settings, &dir, true, &cursors) {
                Ok(value) => {
                    let mut status = state.status.guard();
                    if status.capture.as_ref().map(|c| c.id.as_str()) != Some(id.as_str()) {
                        return;
                    }
                    cursors = value["cursors"].clone();
                    if !value["streamed"].as_bool().unwrap_or(false) {
                        if let Ok(segments) =
                            serde_json::from_value::<Vec<Segment>>(value["segments"].clone())
                        {
                            status.live_segments.extend(segments);
                            status.live_segments.sort_by_key(|s| s.start_ms);
                        }
                    }
                    status.live_stage = tr("Testo provvisorio · aggiornamento durante la call", "Draft transcript · updates during the call").into();
                    let _ = app.emit("status-changed", status.clone());
                }
                Err(e) => {
                    let mut status = state.status.guard();
                    if status.capture.as_ref().map(|c| c.id.as_str()) == Some(id.as_str()) {
                        status.live_stage = if locale::ui_language() == "it" {
                            format!("Anteprima non disponibile: {e}. Il testo sarà elaborato dopo lo stop.")
                        } else {
                            format!("Preview unavailable: {e}. Transcription will continue when recording stops.")
                        };
                        let _ = app.emit("status-changed", status.clone());
                    }
                    return;
                }
            }
        }
    });
}
fn prepare_recorder(app: &tauri::AppHandle, expanded: bool) -> Result<tauri::WebviewWindow> {
    screen_dock::restore(app)?;
    let window = app
        .get_webview_window("recorder")
        .ok_or("Indicatore non disponibile")?;
    screen_dock::resize(&window, expanded)?;
    Ok(window)
}
fn show_recorder(app: &tauri::AppHandle, expanded: bool) -> Result<()> {
    let window = prepare_recorder(app, expanded)?;
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())
}
#[tauri::command]
async fn recorder_size(app: tauri::AppHandle, state:State<'_,Shared>, expanded: bool) -> Result<()> {
    let state=state.inner().clone();
    tauri::async_runtime::spawn_blocking(move||{let _guard=state.operation.guard();show_recorder(&app,expanded)}).await.map_err(|e|e.to_string())?
}
#[tauri::command]
async fn claude_auth_status(state: State<'_, Shared>, provider: Option<String>) -> Result<auth::AuthStatus> {
    let state = state.inner().clone();
    let settings=db::get_settings(&state.db.guard())?;
    if provider.as_deref().unwrap_or(&settings.assistant_provider)=="custom" { return Ok(custom_endpoint::status(&settings)); }
    if provider.as_deref().unwrap_or(&settings.assistant_provider)=="ollama" {
        return tauri::async_runtime::spawn_blocking(move||Ok(ollama::status(&settings))).await.map_err(|e|e.to_string())?;
    }
    if provider.as_deref().unwrap_or(&settings.assistant_provider)=="gemini" {
        let current=state.gemini_auth.status.guard().clone(); if current.pending { return Ok(current); }
        return tauri::async_runtime::spawn_blocking(move||gemini::check(&settings,&state.data_dir.join("gemini-session"))).await.map_err(|e|e.to_string())?;
    }
    if provider.as_deref().unwrap_or(&settings.assistant_provider)=="codex" {
        let current=state.codex_auth.status.guard().clone();if current.pending{return Ok(current);}
        return tauri::async_runtime::spawn_blocking(move||codex::check(&settings,&state.data_dir.join("codex-session"))).await.map_err(|e|e.to_string())?;
    }
    let current = state.auth.status.guard().clone();
    if current.pending {
        return Ok(current);
    }
    let bin = db::get_settings(&state.db.guard())?.claude_bin;
    if process::executable(&bin).is_err() { return Ok(auth::AuthStatus::default()); }
    tauri::async_runtime::spawn_blocking(move || auth::check(&bin))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn claude_login(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    force: bool,
    provider: Option<String>,
) -> Result<auth::AuthStatus> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings=db::get_settings(&state.db.guard())?;
        if provider.as_deref().unwrap_or(&settings.assistant_provider)=="gemini" { return gemini::AuthManager::login(app, state, force); }
        if provider.as_deref().unwrap_or(&settings.assistant_provider)=="codex" {
            let _guard=state.codex_auth.guard.guard();
            let current=state.codex_auth.status.guard().clone();if current.pending{return Ok(current);}
            let result:Result<auth::AuthStatus>=(||{
                let mut settings=settings;
                if process::executable(&settings.codex_bin).is_err(){
                    *state.codex_auth.status.guard()=auth::AuthStatus{pending:true,installing:true,install_message:Some(tr("Preparazione di Codex…", "Preparing Codex…").into()),..Default::default()};
                    let _=app.emit("codex-auth-changed",state.codex_auth.status.guard().clone());
                    let path=codex_runtime::ensure(&engine_dir(),|message|{state.codex_auth.status.guard().install_message=Some(message.into());let _=app.emit("codex-auth-changed",state.codex_auth.status.guard().clone());})?;
                    settings.codex_bin=path.to_string_lossy().into_owned();
                    {let _operation=state.operation.guard();let conn=state.db.guard();let mut latest=db::get_settings(&conn)?;latest.codex_bin=settings.codex_bin.clone();db::save_settings(&conn,&latest)?;}
                    *state.codex_auth.status.guard()=auth::AuthStatus{installed:true,..Default::default()};
                }
                state.codex_auth.login(app.clone(),&settings,&state.data_dir.join("codex-session"),force)
            })();
            if let Err(error)=&result{*state.codex_auth.status.guard()=auth::AuthStatus{error:Some(error.clone()),..Default::default()};let _=app.emit("codex-auth-changed",state.codex_auth.status.guard().clone());}return result;
        }
        let _guard = state.auth.install_guard.guard();
        let current = state.auth.status.guard().clone();
        if current.pending { return Ok(current); }
        let mut bin = db::get_settings(&state.db.guard())?.claude_bin;
        if process::executable(&bin).is_err() {
            *state.auth.status.guard() = auth::AuthStatus { pending:true, installing:true, install_message:Some(tr("Preparazione di Claude Code…", "Preparing Claude Code…").into()), ..Default::default() };
            let _ = app.emit("claude-auth-changed", state.auth.status.guard().clone());
            let result = claude_runtime::ensure(&engine_dir(), |message| {
                state.auth.status.guard().install_message = Some(message.into());
                let _ = app.emit("claude-auth-changed", state.auth.status.guard().clone());
            });
            match result {
                Ok(path) => {
                    bin = path.to_string_lossy().into_owned();
                    let saved: Result<()> = (|| {
                        let _operation = state.operation.guard();
                        let conn = state.db.guard();
                        let mut settings = db::get_settings(&conn)?;
                        settings.claude_bin = bin.clone();
                        db::save_settings(&conn, &settings)
                    })();
                    if let Err(error) = saved {
                        *state.auth.status.guard() = auth::AuthStatus { error:Some(error.clone()), ..Default::default() };
                        let _ = app.emit("claude-auth-changed", state.auth.status.guard().clone());
                        return Err(error);
                    }
                    state.catalog.guard().take();
                    *state.auth.status.guard() = auth::AuthStatus { installed:true, ..Default::default() };
                },
                Err(error) => {
                    *state.auth.status.guard() = auth::AuthStatus { error:Some(error.clone()), ..Default::default() };
                    let _ = app.emit("claude-auth-changed", state.auth.status.guard().clone());
                    return Err(error);
                }
            }
        }
        let result = state.auth.start(app.clone(), bin, force);
        if let Err(error) = &result {
            *state.auth.status.guard() = auth::AuthStatus { installed:true, error:Some(error.clone()), ..Default::default() };
            let _ = app.emit("claude-auth-changed", state.auth.status.guard().clone());
        }
        result
    })
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn claude_login_code(state: State<Shared>, code: String) -> Result<()> {
    state.auth.code(&code)
}
#[tauri::command]
fn claude_login_cancel(state: State<Shared>, provider: Option<String>) {
    if provider.as_deref()==Some("gemini"){state.gemini_auth.cancel();}else if provider.as_deref()==Some("codex"){state.codex_auth.cancel();}else{state.auth.cancel();}
}
#[tauri::command]
fn claude_login_browser(state: State<Shared>, provider: Option<String>) -> Result<()> {
    if provider.as_deref()==Some("gemini") {let url=state.gemini_auth.status.guard().url.clone().ok_or("Pagina di accesso non ancora disponibile")?;return gemini::open_login(&url);}
    if provider.as_deref()==Some("codex") {let url=state.codex_auth.status.guard().url.clone().ok_or("Pagina di accesso non ancora disponibile")?;return codex::open_login(&url);}
    let url = state
        .auth
        .status
        .guard()
        .url
        .clone()
        .ok_or("Pagina di accesso non ancora disponibile")?;
    auth::open_login(&url)
}
#[tauri::command]
async fn set_chat_model(state: State<'_, Shared>, model: String) -> Result<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let initial = db::get_settings(&state.db.guard())?;
        let provider = initial.assistant_provider;
        if !((provider == "codex" || provider == "gemini") && model.is_empty()) {
            let catalog = read_catalog(&state, false)?;
            if !catalog.models.iter().any(|entry| entry.value == model) {
                return Err("Il modello scelto non compare nel catalogo dell’assistente. Aggiorna la lista.".into());
            }
        }
        let _guard = state.operation.guard();
        if state.status.guard().chat_busy {
            return Err("Attendi la risposta prima di cambiare modello.".into());
        }
        let conn = state.db.guard();
        let mut settings = db::get_settings(&conn)?;
        if settings.assistant_provider != provider || (provider == "ollama" && settings.ollama_server_url != initial.ollama_server_url) { return Err("L’assistente è cambiato. Seleziona nuovamente il modello.".into()); }
        if settings.assistant_provider=="custom" {settings.custom_chat.model=model;}else if settings.assistant_provider=="ollama" {settings.ollama_model=model;}else if settings.assistant_provider=="gemini" {settings.gemini_model=model;}else if settings.assistant_provider=="codex" {settings.codex_model=model;}else{settings.claude_model = model;}
        settings.validate()?;
        db::save_settings(&conn, &settings)
    })
    .await
    .map_err(|e| e.to_string())?
}
fn read_catalog(state: &AppState, refresh: bool) -> Result<catalog::Catalog> {
    let settings = db::get_settings(&state.db.guard())?;
    if settings.assistant_provider=="custom" { return Ok(custom_endpoint::catalog(&settings)); }
    if settings.assistant_provider=="ollama" { return ollama::models(&settings.ollama_server_url); }
    if settings.assistant_provider=="gemini" { return gemini::models(&state.gemini_auth,&settings,&state.data_dir.join("gemini-session"),refresh); }
    if settings.assistant_provider=="codex" { return codex::models(&settings,&state.data_dir.join("codex-session")); }
    let executable = process::executable(&settings.claude_bin)?;
    let stamp = std::fs::metadata(&executable)
        .ok()
        .and_then(|m| m.modified().ok());
    let key = format!("{}:{stamp:?}", executable.display());
    let mut cache = state.catalog.guard();
    if !refresh {
        if let Some(cached) = cache.as_ref() {
            if cached.key == key && cached.fetched.elapsed() < Duration::from_secs(600) {
                return Ok(cached.catalog.clone());
            }
        }
    }
    let catalog = catalog::fetch(&settings, &state.data_dir.join("claude-session"))?;
    *cache = Some(catalog::Cache {
        key,
        fetched: std::time::Instant::now(),
        catalog: catalog.clone(),
    });
    Ok(catalog)
}
#[tauri::command]
async fn claude_models(state: State<'_, Shared>, refresh: bool) -> Result<catalog::Catalog> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || read_catalog(&state, refresh))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn set_assistant_provider(state: State<Shared>, provider: String) -> Result<Settings> {
    let _guard=state.operation.guard();
    if state.status.guard().chat_busy{return Err("Attendi la risposta prima di cambiare fornitore AI.".into());}
    let conn=state.db.guard();let mut settings=db::get_settings(&conn)?;settings.assistant_provider=provider;settings.validate()?;db::save_settings(&conn,&settings)?;state.catalog.guard().take();state.gemini_auth.discard_chat();Ok(settings)
}
#[tauri::command]
async fn claude_connectors(state: State<'_, Shared>) -> Result<Vec<connectors::Connector>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let settings = db::get_settings(&state.db.guard())?;
        if settings.assistant_provider=="custom" { return Ok(vec![]); }
        if settings.assistant_provider=="ollama" {return connectors::fetch(&settings, &state.data_dir.join("ollama-session"));}
        if settings.assistant_provider=="gemini" {return gemini::connectors(&state.data_dir.join("gemini-session"));}
        if settings.assistant_provider=="codex" {return codex::connectors(&settings,&state.data_dir.join("codex-session"));}
        connectors::fetch(&settings, &state.data_dir.join("claude-session"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn set_connector_enabled(state: State<Shared>, name: String, enabled: bool) -> Result<Settings> {
    let _guard = state.operation.guard();
    if state.status.guard().chat_busy { return Err("Attendi la fine della risposta prima di cambiare connettori.".into()); }
    if name.trim().is_empty() || name.len() > 512 || name.chars().any(char::is_control) { return Err("Nome connettore non valido.".into()); }
    let db = state.db.guard();
    let mut settings = db::get_settings(&db)?;
    settings.active_disabled_mut().retain(|s| s != &name);
    if !enabled { settings.active_disabled_mut().push(name); }
    db::save_settings(&db, &settings)?;
    Ok(settings)
}

#[tauri::command]
async fn test_notification(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<()> {
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        notifications::show(&app, &data_dir, "", tr("Notifica di prova · PARLO è pronto ad avvisarti.", "Test notification · PARLO is ready to notify you."))
    }).await.map_err(|error| error.to_string())?
}
#[tauri::command]
fn chat_permission(state: State<Shared>) -> Option<connectors::Permission> {
    state.chat_control.current()
}
#[tauri::command]
fn chat_permission_answer(state: State<Shared>, id: String, allow: bool, always: Option<bool>, answers: Option<serde_json::Value>) -> Result<Settings> {
    state.chat_control.decide_with_answers(&state.db.guard(), &id, allow, always.unwrap_or(false), answers)
}
#[tauri::command]
fn chat_progress(state: State<Shared>) -> Option<activity::ChatRun> { state.chat_control.progress() }
#[tauri::command]
fn revoke_tool_trust(state: State<Shared>, tool: String) -> Result<Settings> {
    let conn = state.db.guard();
    let mut settings = db::get_settings(&conn)?;
    settings.active_trusted_mut().retain(|s| s != &tool);
    db::save_settings(&conn, &settings)?;
    Ok(settings)
}
#[tauri::command]
fn chat_cancel(state: State<Shared>, id: String) -> Result<()> {
    if state.status.guard().chat_call.as_deref() != Some(&id) {
        return Err("Questa chat non è in esecuzione.".into());
    }
    state.chat_control.cancel();
    Ok(())
}
#[tauri::command]
fn reset_chat(app: tauri::AppHandle, state: State<Shared>, id: String) -> Result<model::Detail> {
    let _guard = state.operation.guard();
    let status = state.status.guard();
    if status.chat_busy && status.chat_call.as_deref() == Some(&id) { return Err("Interrompi la risposta prima di azzerare questa chat.".into()); }
    drop(status);
    let conn = state.db.guard();
    db::clear_chat(&conn, &id)?;
    let mut detail = db::detail(&conn, &id)?;
    let mut progress = state.chat_control.progress.guard();
    if progress.as_ref().is_some_and(|t| t.run.call_id == id) { progress.take(); }
    drop(progress); drop(conn);
    sharing::hydrate_shared_detail(state.inner(), &mut detail)?;
    state.gemini_auth.discard_call(&id);
    let _ = app.emit("chat-reset", serde_json::json!({"id":id}));
    detail_changed(&app, &id);
    Ok(detail)
}
#[tauri::command]
fn claude_connectors_browser() -> Result<()> {
    let mut command = std::process::Command::new(if cfg!(windows) {
        "explorer.exe"
    } else {
        "open"
    });
    command
        .arg("https://claude.ai/customize/connectors")
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
async fn audio_outputs() -> Result<Vec<audio::AudioOutput>> {
    tauri::async_runtime::spawn_blocking(audio::outputs).await.map_err(|e| e.to_string())?
}
#[tauri::command]
async fn start_recording(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_core(&app, &state))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn stop_recording(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || stop_core(&app, &state))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn retry_processing(app: tauri::AppHandle, state: State<Shared>, id: String) -> Result<()> {
    let _guard = state.operation.guard();
    updates::ensure_not_installing(&app)?;
    if state.provision.0.guard().running { return Err("Attendi il completamento dell’installazione del motore.".into()); }
    let detail = db::detail(&state.db.guard(), &id)?;
    if !["failed", "pending"].contains(&detail.call.status.as_str()) {
        return Err("Questa call non richiede una nuova elaborazione.".into());
    }
    processing::enqueue(&app, state.inner(), &id)?;
    changed(&app, &state);
    archive_changed(&app, Some(&id));
    Ok(())
}
#[tauri::command]
fn set_call_tags(app: tauri::AppHandle, state: State<Shared>, id: String, tags: Vec<String>) -> Result<()> {
    db::set_tags(&mut state.db.guard(), &id, tags)?;
    archive_changed(&app, Some(&id));
    let _ = app.emit("archive-tags-changed", ());
    Ok(())
}
#[tauri::command]
fn rename_call(
    app: tauri::AppHandle,
    state: State<Shared>,
    id: String,
    title: String,
) -> Result<()> {
    if title.trim().is_empty() || title.chars().count() > 200 {
        return Err("Titolo consentito: da 1 a 200 caratteri.".into());
    }
    db::rename(&mut state.db.guard(), &id, title.trim())?;
    archive_changed(&app, Some(&id));
    sharing::schedule_title_sync(&app, state.inner(), Some(id));
    Ok(())
}
#[tauri::command]
fn delete_call(app: tauri::AppHandle, state: State<Shared>, id: String) -> Result<()> {
    let _guard = state.operation.guard();
    let s = state.status.guard().clone();
    if s.capture.as_ref().map(|c| c.id.as_str()) == Some(&id)
        || s.processing.as_deref() == Some(&id)
        || s.queued.contains(&id)
        || s.chat_busy
    {
        return Err("Attendi la fine dell’operazione in corso.".into());
    }
    state.gemini_auth.discard_call(&id);
    remove_audio(&state, &id)?;
    screen::remove(&state,&id)?;
    db::delete(&mut state.db.guard(), &id)?;
    archive_changed(&app, Some(&id));
    let _ = app.emit("archive-tags-changed", ());
    Ok(())
}
#[tauri::command]
fn export_markdown(state: State<Shared>, id: String) -> Result<String> {
    let d = db::detail(&state.db.guard(), &id)?;
    Ok(model::markdown_detail(&d))
}
#[tauri::command]
async fn save_markdown(app: tauri::AppHandle, window: tauri::WebviewWindow, state: State<'_, Shared>, id: String, shared: Option<bool>) -> Result<bool> {
    let (name, text) = if shared.unwrap_or(false) {
        sharing::shared_markdown_export(state.inner().clone(), id).await?
    } else {
        let d = db::detail(&state.db.guard(), &id)?;
        (export::filename(&d.call.title), model::markdown_detail(&d))
    };
    export::save(app, window, name, text).await
}
#[tauri::command]
fn cleanup(app: tauri::AppHandle, state: State<Shared>) -> Result<usize> {
    cleanup_archive(&app, state.inner())
}
#[tauri::command]
async fn chat(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    id: String,
    prompt: String,
) -> Result<()> {
    let state = state.inner().clone();
    {
        let _guard = state.operation.guard();
        updates::ensure_not_installing(&app)?;
        let mut s = state.status.guard();
        if s.chat_busy {
            return Err("Una risposta dell’assistente è già in corso.".into());
        }
        s.chat_busy = true;
        s.chat_call = Some(id.clone());
        state
            .chat_control
            .cancelled
            .store(false, std::sync::atomic::Ordering::SeqCst);
        state.chat_control.progress.guard().take();
    }
    changed(&app, &state);
    let app_clone = app.clone();
    let state_clone = state.clone();
    let chat_id = id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        process::chat(&app_clone, &state_clone, &id, &prompt)
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    if let Some(run) = state.chat_control.finish_progress(&app, result.as_ref().err().map(String::as_str)) {
        if let Err(e) = db::save_activity(&state.db.guard(), &run) { let _ = app.emit("app-error", format!("Salvataggio attività chat: {e}")); }
    }
    let notify = result.is_ok() && !state.chat_control.cancelled.load(std::sync::atomic::Ordering::SeqCst);
    state.chat_control.finish(&app);
    {
        let mut s = state.status.guard();
        s.chat_busy = false;
        s.chat_call = None;
    }
    changed(&app, &state);
    detail_changed(&app, &chat_id);
    if notify {
        tauri::async_runtime::spawn_blocking(move || notifications::chat_completed(&app, &state, &chat_id));
    }
    result
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if let Some(result) = auth::browser_helper(&args) {
        // Do not send authentication URLs to another instance or to logs.
        if result.is_err() { std::process::exit(1); }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--tts-mcp") {
        if args.len() != 3 || tts::serve(&args[2]).is_err() { std::process::exit(1); }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--antigravity-login-url") {
        if args.len() != 3 || gemini::forward_login_url(&args[2]).is_err() {
            std::process::exit(1);
        }
        return;
    }
    crash::install();
    if std::env::args().any(|arg|arg=="--prepare-codex-runtime") {match codex_runtime::ensure(&engine_dir(),|m|println!("{m}")){Ok(path)=>println!("{}",path.display()),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}return;}
    if std::env::args().any(|arg|arg=="--check-codex") {let mut settings=Settings::default();settings.assistant_provider="codex".into();if let Ok(bin)=std::env::var("PARLO_CODEX_BINARY"){settings.codex_bin=bin;}let directory=engine_dir().join("qa-session");match codex::check(&settings,&directory).and_then(|auth|codex::models(&settings,&directory).map(|catalog|(auth,catalog))){Ok((auth,catalog))=>println!("{}",serde_json::json!({"auth":auth,"catalog":catalog})),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}return;}
    if std::env::args().any(|arg| arg == "--prepare-claude-runtime") {
        match claude_runtime::ensure(&engine_dir(), |message| println!("{message}")) {
            Ok(path) => println!("{}", path.display()),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--prepare-engine-runtime") {
        let root = engine_dir();
        let Some(bundle) = std::env::var_os("PARLO_ENGINE_BUNDLE").map(PathBuf::from) else { eprintln!("Specify PARLO_ENGINE_BUNDLE"); std::process::exit(1); };
        match python_runtime::ensure_prepared(&root, &bundle, |message| println!("{message}")) {
            Ok(python) => println!("{}", python.display()),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--prepare-python-runtime") {
        let root = engine_dir();
        let bundle = std::env::var_os("PARLO_PYTHON_BUNDLE").map(PathBuf::from);
        match python_runtime::ensure(&root, bundle.as_deref(), |message| println!("{message}")) {
            Ok(python) => println!("{}", python.display()),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--check-engine-setup") {
        let settings=Settings::default();
        println!("{}",serde_json::json!({"python_available":provision::python(&settings).is_some(),"whisper_ready":settings.whisper_ready(),"engine_root":engine_dir(),"default_model":settings.whisper_model}));
        return;
    }
    #[cfg(windows)]
    if std::env::args().any(|arg| arg == "--check-notifications") {
        let data = std::env::var_os("CALLNOTES_DATA_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
        match notifications::check(&data) { Ok(()) => println!("Notifica PARLO inviata e verificata nel centro notifiche Windows."), Err(e) => {eprintln!("{e}"); std::process::exit(1);} }
        return;
    }
    if std::env::args().any(|arg| arg == "--check-claude-connectors") {
        let settings = Settings::default();
        let dir = std::env::var_os("CALLNOTES_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("claude-session");
        match connectors::fetch(&settings, &dir) {
            Ok(servers) => println!("{}", serde_json::to_string(&servers).unwrap()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--check-claude-models") {
        let settings = Settings::default();
        let directory = std::env::var_os("CALLNOTES_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("claude-session");
        match catalog::fetch(&settings, &directory) {
            Ok(catalog) => println!("{}", serde_json::to_string(&catalog).unwrap()),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().any(|arg| arg == "--check-audio-formats") {
        match diagnostics::audio_formats(){Ok(())=>println!("WASAPI: microfono e loopback supportano il formato richiesto. Nessun audio registrato."),Err(e)=>{eprintln!("{e}");std::process::exit(1);}}
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app,_,_|{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}}))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app|{
            if updates::enabled(app.config()) {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            #[cfg(windows)]
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = app.default_window_icon() { window.set_icon(icon.clone())?; }
                window.set_decorations(false)?;
            }
            app.manage(updates::Updates::default());
            app.manage(notifications::Notifications::default());
            app.manage(telegram::Telegram::default());
            app.manage(screen_dock::RecorderDock::default());
            app.manage(call_notes::Notes::default());
            app.manage(drawing_interaction::DrawingInteraction::default());
            app.manage(screen::DrawingPresentation::default());
            let root = match std::env::var_os("CALLNOTES_DATA_DIR") {
                Some(dir) => PathBuf::from(dir),
                None => data_dir::resolve(app.path().app_local_data_dir()?, &app.config().identifier),
            };
            crash::set_folder(&root);
            std::fs::create_dir_all(root.join("temporary"))?;
            if std::env::var_os("CALLNOTES_ENGINE_DIR").is_none() { std::env::set_var("CALLNOTES_ENGINE_DIR",root.join("engine")); }
            let state=Arc::new(AppState{db:Mutex::new(db::open(&root.join("archive.sqlite")).map_err(std::io::Error::other)?),status:Mutex::new(Status::default()),recorder:Mutex::new(None),operation:Mutex::new(()),data_dir:root,processing: Default::default(),engine:Mutex::new(None),live_engine:Mutex::new(None),live_process:Mutex::new(None),auth:auth::AuthManager::default(),codex_auth:codex::AuthManager::default(),gemini_auth:Default::default(),catalog:Mutex::new(None),chat_control:connectors::ChatControl::default(),shortcut_capture:Default::default(),provision:Default::default()});
            std::fs::write(state.data_dir.join("worker.py"),include_str!("../../engine/worker.py"))?;
            std::fs::write(state.data_dir.join("import_media.py"),include_str!("../../engine/import_media.py"))?;
            std::fs::write(state.data_dir.join("video.py"),include_str!("../../engine/video.py"))?;
            std::fs::write(state.data_dir.join("acceleration.py"),include_str!("../../engine/acceleration.py"))?;
            std::fs::write(state.data_dir.join("cpp_live.py"),include_str!("../../engine/cpp_live.py"))?;
            std::fs::write(state.data_dir.join("transcript_quality.py"),include_str!("../../engine/transcript_quality.py"))?;
            app.manage(state.clone());
            db::upgrade_preview(&state.db.guard()).map_err(std::io::Error::other)?;
            // A crash between committing text and deleting audio must not lose a completed transcript.
            for id in recover_finalized_calls(&state).map_err(std::io::Error::other)? {
                sharing::auto_publish(app.handle(), &state, &id);
            }
            let settings=db::get_settings(&state.db.guard()).map_err(std::io::Error::other)?;
            if settings.live_transcription&&settings.whisper_ready()&&settings.preview_ready()&&python_runtime::prepared_installed(&engine_dir()).is_some(){
                let handle=app.handle().clone();let warm_state=state.clone();let warm_settings=settings.clone();
                std::thread::spawn(move||{if let Err(e)=process::inference(&handle,&warm_state,&warm_settings,&warm_state.data_dir,true,&serde_json::json!({"warmup":true})){eprintln!("Preparazione anteprima: {e}");}});
            }
            if let Err(e)=register_all_shortcuts(app.handle(),&settings){let _=app.emit("app-error",e);}
            let drawing = WebviewWindowBuilder::new(app,"drawing",WebviewUrl::App("overlay.html?drawing=1".into()))
                .title(tr("Disegna sullo schermo", "Draw on screen")).inner_size(800.,600.).resizable(false).decorations(false)
                .transparent(true).shadow(false).always_on_top(true).skip_taskbar(true).focused(false).visible(false);
            #[cfg(windows)]
            let drawing=drawing.data_directory(state.data_dir.join("overlay-webview"))
                .additional_browser_args(screen::OVERLAY_BROWSER_ARGS);
            let drawing=drawing.build()?;
            let _ = screen::prepare_overlay(&drawing);
            let recorder = WebviewWindowBuilder::new(app,"recorder",WebviewUrl::App("overlay.html?overlay=1".into()))
                .title(tr("PARLO · Registrazione", "PARLO · Recording"))
                .inner_size(screen_dock::EXPANDED_SIZE.width, screen_dock::EXPANDED_SIZE.height)
                .accept_first_mouse(true)
                .resizable(false).decorations(false).shadow(false).transparent(true).content_protected(true).always_on_top(true).skip_taskbar(true).visible(false).build()?;
            let _ = screen::prepare_overlay(&recorder);
            WebviewWindowBuilder::new(app,"note",WebviewUrl::App("index.html?note=1".into())).title(tr("PARLO · Nota manuale", "PARLO · Manual note")).inner_size(440.,270.).resizable(false).always_on_top(true).skip_taskbar(true).visible(false).build()?;
            apply_theme(app.handle(), &settings);
            drawing_interaction::start(app.handle().clone());
            let show=tauri::menu::MenuItem::with_id(app,"show",tr("Apri PARLO", "Open PARLO"),true,None::<&str>)?;
            let record=tauri::menu::MenuItem::with_id(app,"record",tr("Avvia registrazione / mostra indicatore", "Start recording / show indicator"),true,None::<&str>)?;
            let indicator=tauri::menu::MenuItem::with_id(app,"indicator",tr("Mostra indicatore registrazione", "Show recording indicator"),true,None::<&str>)?;
            let quit=tauri::menu::MenuItem::with_id(app,"quit",tr("Esci", "Quit"),true,None::<&str>)?;
            let menu=tauri::menu::Menu::with_items(app,&[&show,&record,&indicator,&quit])?;
            let rgba = include_bytes!("../icons/tray.rgba").to_vec();
            tauri::tray::TrayIconBuilder::with_id("callnotes").icon(tauri::image::Image::new_owned(rgba,32,32)).menu(&menu).tooltip("PARLO · pronto")
                .on_menu_event(|app,event|match event.id.as_ref(){
                    "show"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}},
                    "indicator"=>{if app.state::<Shared>().status.guard().capture.is_some(){let _=show_recorder(app,true);}},
                    "record"=>{let app=app.clone();let state=app.state::<Shared>().inner().clone();std::thread::spawn(move||{let active=state.status.guard().capture.is_some();let r=if active{show_recorder(&app,true)}else{start_core(&app,&state)};if let Err(e)=r{let _=app.emit("app-error",e);}});},
                    "quit" => {
                        let state = app.state::<Shared>();
                        let status = state.status.guard().clone();
                        if status.capture.is_some() || status.processing.is_some() || !status.queued.is_empty() {
                            let _ = app.emit("app-error", tr("Termina la registrazione e attendi l’elaborazione prima di uscire.", "Stop recording and wait for processing to finish before quitting."));
                            if let Some(window) = app.get_webview_window("main") { let _ = window.show(); }
                        } else {
                            if status.chat_busy { state.chat_control.cancel(); }
                            app.exit(0);
                        }
                    }, _=>{}
                }).build(app)?;
            if let Err(e)=cleanup_archive(app.handle(),&state){eprintln!("Pulizia archivio: {e}");}
            let handle=app.handle().clone();let periodic=state.clone();
            std::thread::spawn(move||loop{std::thread::sleep(Duration::from_secs(3600));match cleanup_archive(&handle,&periodic){Err(e)=>{let _=handle.emit("app-error",e);},_=>{}}});
            if let Some(window)=app.get_webview_window("main"){let _=window.show();}
            if let Err(error)=ensure_engine(app.handle(), &state){state.provision.0.guard().error=Some(error);}
            telegram::start(app.handle().clone(), state);
            Ok(())
        })
        .on_window_event(|window,event|{if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();if window.label()=="screenshot-area"{let app=window.app_handle().clone();let state=app.state::<Shared>().inner().clone();tauri::async_runtime::spawn_blocking(move||{if let Err(error)=screen::cancel_selection(&app,&state){let _=app.emit("app-error",error);}});}else if window.label()=="video-area"{let _=video::video_cancel_selection(window.app_handle().clone());}else if window.label()=="recorder"{let _=show_recorder(window.app_handle(),false);}else{let _=window.hide();}}})
        .invoke_handler(tauri::generate_handler![custom_endpoint::custom_endpoint_key,custom_endpoint::open_endpoint_guide,custom_endpoint::test_custom_endpoint,sharing::sharing_pick_folder,sharing::sharing_webhook,sharing::publish_transcript,sharing::publication_status,sharing::list_shared_transcripts,sharing::get_shared_transcript,sharing::open_shared_assistant,locale::ui_language,process::ollama_context_estimate,qwen_tts::qwen_tts_status,qwen_tts::qwen_tts_prepare,ollama::ollama_download,ollama::ollama_models,ollama::ollama_model_context,piper::piper_status,piper::piper_download,kokoro::kokoro_status,kokoro::kokoro_download,tts::tts_catalog,stt::stt_key_status,stt::stt_save_key,stt::stt_delete_key,tts::tts_key_status,tts::tts_save_key,tts::tts_delete_key,import_media::import_media,telegram::telegram_status,telegram::telegram_connect,telegram::telegram_disconnect,mac_permissions::reset_capture_permissions,audio_outputs,set_call_tags,video::prepare_call_video,video::video_drawing,video::video_monitors,video::video_select_monitor,video::video_cancel_selection,video::video_start_selection,video::video_hint_ready,video::open_call_video,chat_files::save_generated,chat_files::reveal_generated,sources::chat_sources,sources::add_chat_sources,sources::set_chat_source,sources::remove_chat_source,sources::reveal_chat_source,sources::reveal_chat_file,updates::update_status,updates::check_update,updates::install_update,call_notes::open_call_note,call_notes::get_call_note,call_notes::save_call_note,call_notes::cancel_call_note,drawing_interaction::set_drawing_interaction,screen_dock::dock_recorder,screen_dock::recorder_dock_status,screen_dock::recorder_frame_ready,screen::toggle_drawing,screen::drawing_frame,screen::capture_screenshot,screen::screenshot_monitors,screen::screenshot_select_monitor,screen::screenshot_cancel_selection,screen::screenshot_finish_selection,screen::screenshot_image,screen::set_brush,list_calls,list_tags,get_call,prepare_call_audio,open_call_audio,get_settings,get_status,get_health,save_settings,start_recording,stop_recording,retry_processing,processing::cancel_processing,rename_call,delete_call,export_markdown,save_markdown,cleanup,chat,recorder_size,claude_auth_status,claude_login,claude_login_code,claude_login_cancel,claude_login_browser,set_chat_model,set_assistant_provider,claude_models,claude_connectors,claude_connectors_browser,set_connector_enabled,test_notification,chat_permission,chat_permission_answer,chat_progress,revoke_tool_trust,chat_cancel,reset_chat,set_theme,shortcut_capture,engine_setup,engine_setup_status])
        .build(tauri::generate_context!()).expect("Impossibile avviare PARLO").run(|app,event|{if matches!(event,tauri::RunEvent::Exit){let state=app.state::<Shared>();state.auth.cancel();state.codex_auth.cancel();state.gemini_auth.cancel();state.engine.guard().take();state.live_engine.guard().take();}});
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn retained_audio_survives_restart_but_follows_call_expiry_and_deletion() {
        let root=std::env::temp_dir().join(format!("parlo-retained-audio-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("temporary")).unwrap();
        let state=AppState{db:Mutex::new(db::open(&root.join("archive.sqlite")).unwrap()),status:Mutex::new(Status::default()),recorder:Mutex::new(None),operation:Mutex::new(()),data_dir:root.clone(),processing: Default::default(),engine:Mutex::new(None),live_engine:Mutex::new(None),live_process:Mutex::new(None),chat_control:Default::default(),shortcut_capture:Default::default(),provision:Default::default(),auth:Default::default(),codex_auth:Default::default(),gemini_auth:Default::default(),catalog:Mutex::new(None)};
        let retained=uuid::Uuid::new_v4().to_string();let discarded=uuid::Uuid::new_v4().to_string();let expired=uuid::Uuid::new_v4().to_string();
        for (id,keep,date) in [(&retained,true,chrono::Utc::now().to_rfc3339()),(&discarded,false,chrono::Utc::now().to_rfc3339()),(&expired,true,"2020-01-01T00:00:00+00:00".into())] {
            db::create(&state.db.guard(),id,"Synthetic test",&date).unwrap();
            db::status(&state.db.guard(),id,"finalizing",None,None).unwrap();
            let dir = temp_dir(&state, id).unwrap();
            std::fs::create_dir_all(&dir).unwrap();
            let spec = hound::WavSpec {
                channels: 1, sample_rate: 16000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int,
            };
            let mut writer = hound::WavWriter::create(dir.join("system.wav"), spec).unwrap();
            writer.write_sample(1234i16).unwrap();
            writer.finalize().unwrap();
            if keep {std::fs::write(dir.join("keep-audio"),b"").unwrap();}
        }
        // Global default remains off: recovery must use each recording's marker.
        assert!(!db::get_settings(&state.db.guard()).unwrap().keep_audio);
        recover_finalized_calls(&state).unwrap();
        let retained_dir = retained_audio_directory(&state, &retained).unwrap().unwrap();
        let mut reader = hound::WavReader::open(retained_dir.join("registrazione.wav")).unwrap();
        assert_eq!(reader.samples::<i16>().next().unwrap().unwrap(), 1234);
        drop(reader);
        assert!(!retained_dir.join("system.wav").exists());
        assert_eq!(db::detail(&state.db.guard(),&retained).unwrap().call.status,"ready");
        assert!(!temp_dir(&state,&discarded).unwrap().exists());
        assert!(retained_audio_directory(&state,&discarded).unwrap().is_none());
        recover_finalized_calls(&state).unwrap();
        assert!(retained_dir.join("registrazione.wav").is_file());
        assert_eq!(cleanup_core(&state).unwrap(),1);
        assert!(!temp_dir(&state,&expired).unwrap().exists());
        assert!(temp_dir(&state,&retained).unwrap().exists());
        remove_audio(&state,&retained).unwrap();db::delete(&mut state.db.guard(),&retained).unwrap();
        assert!(!temp_dir(&state,&retained).unwrap().exists());
        drop(state);assert!(root.starts_with(std::env::temp_dir()));std::fs::remove_dir_all(root).unwrap();
    }
    const READY_FIXTURE: &str = "00000000-0000-4000-8000-000000000001";
    const FINAL_FIXTURE: &str = "00000000-0000-4000-8000-000000000002";
    const BUSY_FIXTURE: &str = "00000000-0000-4000-8000-000000000003";
    #[test]
    #[ignore = "Child process fixture; invoked only by the forced-crash test"]
    fn crash_writer_fixture() {
        let Some(root) = std::env::var_os("PARLO_CRASH_TEST_ROOT").map(PathBuf::from) else { return; };
        assert!(root.starts_with(std::env::temp_dir()));
        let mut conn = db::open(&root.join("archive.sqlite")).unwrap();
        let mut settings = Settings::default(); settings.theme="light".into(); settings.retention_months=12; settings.language="en".into();
        db::save_settings(&conn, &settings).unwrap();
        for id in [READY_FIXTURE, FINAL_FIXTURE, BUSY_FIXTURE] {
            db::create(&conn, id, "Crash fixture", &chrono::Utc::now().to_rfc3339()).unwrap();
            let audio = root.join("temporary").join(id);
            std::fs::create_dir_all(&audio).unwrap(); std::fs::write(audio.join("system.wav"), b"synthetic fixture").unwrap();
        }
        let segments = vec![Segment{start_ms:0,end_ms:1000,speaker:"Tu".into(),text:"Committed fixture".into(),source:"microphone".into(),quality:None,original_text:None}];
        db::store_segments(&mut conn, READY_FIXTURE, &segments).unwrap();
        db::status(&conn, READY_FIXTURE, "ready", Some(1000), None).unwrap();
        db::append_message(&conn, READY_FIXTURE, "user", "Committed chat fixture").unwrap();
        db::store_segments(&mut conn, FINAL_FIXTURE, &segments).unwrap();
        db::store_draft(&mut conn, BUSY_FIXTURE, &segments).unwrap();
        db::status(&conn, BUSY_FIXTURE, "processing", Some(1000), None).unwrap();
        // Keep a real SQLite write transaction open. The parent kills this
        // process; no graceful shutdown or Rust destructor can roll it back.
        conn.execute_batch("BEGIN IMMEDIATE; UPDATE segments SET text='UNCOMMITTED'; UPDATE settings SET json='{}';").unwrap();
        std::fs::write(root.join("writer-ready"), b"ready").unwrap();
        loop { std::thread::sleep(Duration::from_millis(100)); }
    }
    #[test]
    fn forced_crash_preserves_commits_rolls_back_partial_write_and_recovers_finalization() {
        use std::process::{Command, Stdio};
        let root = std::env::temp_dir().join(format!("parlo-crash-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "lifecycle_tests::crash_writer_fixture", "--ignored"])
            .env("PARLO_CRASH_TEST_ROOT", &root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
        let mut child = command.spawn().unwrap();
        let limit = std::time::Instant::now() + Duration::from_secs(20);
        while !root.join("writer-ready").is_file() && std::time::Instant::now() < limit {
            if child.try_wait().unwrap().is_some() { break; }
            std::thread::sleep(Duration::from_millis(20));
        }
        let ready = root.join("writer-ready").is_file();
        let _ = child.kill(); let _ = child.wait();
        assert!(ready, "Crash fixture never committed its baseline");
        let state = AppState {
            db:Mutex::new(db::open(&root.join("archive.sqlite")).unwrap()), status:Mutex::new(Status::default()),
            recorder:Mutex::new(None), operation:Mutex::new(()), data_dir:root.clone(), processing: Default::default(),
            engine:Mutex::new(None),live_engine:Mutex::new(None),live_process:Mutex::new(None),chat_control:connectors::ChatControl::default(),
            shortcut_capture:Default::default(),provision:Default::default(),auth:auth::AuthManager::default(),codex_auth:codex::AuthManager::default(),gemini_auth:Default::default(),catalog:Mutex::new(None),
        };
        {
            let conn = state.db.guard();
            let settings = db::get_settings(&conn).unwrap();
            assert_eq!((settings.theme.as_str(),settings.language.as_str(),settings.retention_months), ("light","en",12));
            let detail = db::detail(&conn, READY_FIXTURE).unwrap();
            assert_eq!(detail.call.status, "ready"); assert_eq!(detail.segments[0].text, "Committed fixture");
            assert_eq!(detail.messages[0].content, "Committed chat fixture");
            let interrupted = db::detail(&conn, BUSY_FIXTURE).unwrap();
            assert_eq!(interrupted.call.status, "failed"); assert_eq!(interrupted.segments[0].text, "Committed fixture");
            assert_eq!(db::detail(&conn, FINAL_FIXTURE).unwrap().call.status, "finalizing");
            let integrity:String=conn.query_row("PRAGMA integrity_check",[],|row|row.get(0)).unwrap(); assert_eq!(integrity,"ok");
        }
        recover_finalized_calls(&state).unwrap();
        assert_eq!(db::detail(&state.db.guard(),FINAL_FIXTURE).unwrap().call.status,"ready");
        assert!(!temp_dir(&state,FINAL_FIXTURE).unwrap().exists());
        assert!(temp_dir(&state,BUSY_FIXTURE).unwrap().join("system.wav").is_file());
        assert!(temp_dir(&state,READY_FIXTURE).unwrap().exists());
        drop(state);
        assert!(root.starts_with(std::env::temp_dir())); std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn removes_expired_audio_and_chat_without_touching_recent_calls() {
        let root = std::env::temp_dir().join(format!("callnotes-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("temporary")).unwrap();
        let state = AppState {
            db: Mutex::new(db::open(&root.join("test.sqlite")).unwrap()),
            status: Mutex::new(Status::default()),
            recorder: Mutex::new(None),
            operation: Mutex::new(()),
            data_dir: root.clone(),
            processing: Default::default(),
            engine: Mutex::new(None),
            live_engine: Mutex::new(None),
            live_process: Mutex::new(None),
            chat_control: connectors::ChatControl::default(),
            shortcut_capture: Default::default(),
            provision: Default::default(),
            auth: auth::AuthManager::default(),
            codex_auth: codex::AuthManager::default(),gemini_auth:Default::default(),
            catalog: Mutex::new(None),
        };
        let old = uuid::Uuid::new_v4().to_string();
        let recent = uuid::Uuid::new_v4().to_string();
        for (id, date) in [
            (&old, "2020-01-01T00:00:00+00:00".to_string()),
            (&recent, chrono::Utc::now().to_rfc3339()),
        ] {
            db::create(&state.db.guard(), id, "Test", &date).unwrap();
            db::status(&state.db.guard(), id, "ready", None, None).unwrap();
            db::append_message(&state.db.guard(), id, "user", "Test").unwrap();
            let audio = temp_dir(&state, id).unwrap();
            std::fs::create_dir_all(&audio).unwrap();
            std::fs::write(audio.join("system.wav"), b"test").unwrap();
        }
        assert_eq!(cleanup_core(&state).unwrap(), 1);
        assert!(!temp_dir(&state, &old).unwrap().exists());
        assert!(temp_dir(&state, &recent).unwrap().exists());
        assert_eq!(db::list(&state.db.guard(), "").unwrap().len(), 1);
        assert!(temp_dir(&state, "../elsewhere").is_err());
        drop(state);
        assert!(root.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(root).unwrap();
    }
}
