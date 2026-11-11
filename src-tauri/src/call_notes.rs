use crate::{db, model::CallNote, LockExt, Shared};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Serialize)]
pub struct Draft { call_id: String, pub timestamp_ms: i64 }
#[derive(Default)]
pub struct Notes(Mutex<Option<Draft>>);

pub fn open(app: &tauri::AppHandle, state: &Shared) -> Result<()> {
    let _guard = state.operation.guard();
    let window = app.get_webview_window("note").ok_or("Editor note non disponibile")?;
    if window.is_visible().unwrap_or(false) { return window.set_focus().map_err(|e|e.to_string()); }
    let call_id = state.status.guard().capture.as_ref().map(|c|c.id.clone()).ok_or("Avvia una registrazione per aggiungere una nota")?;
    let timestamp_ms = state.recorder.guard().as_ref().ok_or("Registrazione non disponibile")?.start.elapsed().as_millis() as i64;
    let draft = Draft { call_id, timestamp_ms };
    *app.state::<Notes>().0.guard() = Some(draft.clone());
    let _ = window.center();
    window.emit("note-opened", &draft).map_err(|e|e.to_string())?;
    window.show().map_err(|e|e.to_string())?;
    window.set_focus().map_err(|e|e.to_string())
}
#[tauri::command]
pub async fn open_call_note(app: tauri::AppHandle, state: tauri::State<'_, Shared>) -> Result<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || open(&app, &state)).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub fn get_call_note(app: tauri::AppHandle) -> Option<Draft> { app.state::<Notes>().0.guard().clone() }
#[tauri::command]
pub async fn save_call_note(app: tauri::AppHandle, state: tauri::State<'_, Shared>, text: String) -> Result<()> {
    if text.trim().is_empty() || text.chars().count() > 4000 { return Err("Inserisci una nota di massimo 4000 caratteri".into()); }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = state.operation.guard();
        let draft = app.state::<Notes>().0.guard().clone().ok_or("Nessuna nota da salvare")?;
        let note = CallNote { id:uuid::Uuid::new_v4().to_string(), timestamp_ms:draft.timestamp_ms, text:text.trim().into() };
        db::add_note(&mut state.db.guard(), &draft.call_id, &note)?;
        *app.state::<Notes>().0.guard() = None;
        if let Some(window) = app.get_webview_window("note") { let _ = window.hide(); }
        let _ = app.emit("calls-changed", ());
        Ok(())
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub fn cancel_call_note(app: tauri::AppHandle) -> Result<()> {
    *app.state::<Notes>().0.guard() = None;
    if let Some(window) = app.get_webview_window("note") { window.hide().map_err(|e|e.to_string())?; }
    Ok(())
}
