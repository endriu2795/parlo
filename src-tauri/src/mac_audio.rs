//! In-process Objective-C bridge: permissions belong to the app, not a helper.
use crate::LockExt;
use crate::{audio::Recording, model::{Capture, Settings}};
use std::{ffi::{c_char, c_void, CStr, CString}, path::PathBuf, sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc, Mutex}, time::{Duration, Instant}};
use tauri::{AppHandle, Emitter};

type Callback = extern "C" fn(*mut c_void, *const c_char);
extern "C" {
    fn parlo_capture_create(dir: *const c_char, mic: i32, system: i32, callback: Callback, context: *mut c_void) -> *mut c_void;
    fn parlo_capture_stop(handle: *mut c_void) -> *mut c_char;
    fn parlo_string_free(value: *mut c_char);
    fn parlo_notify(heading: *const c_char, title: *const c_char) -> *mut c_char;
}
struct Context { app: AppHandle, ready: mpsc::Sender<Result<(), String>>, stop: Arc<AtomicBool>, error: Mutex<Option<String>> }
extern "C" fn event(pointer: *mut c_void, json: *const c_char) {
    // The bridge drains its serial callback queue before releasing this context.
    let context = unsafe { &*(pointer as *const Context) };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(unsafe { CStr::from_ptr(json).to_bytes() }) else { return; };
    match value["type"].as_str() {
        Some("ready") => { let _ = context.ready.send(Ok(())); }
        Some("level") => { let _ = context.app.emit("audio-level", &value); }
        Some("route") => { let _ = context.app.emit("audio-route", &value); }
        Some("error") => {
            let error = value["message"].as_str().unwrap_or("Cattura audio Mac interrotta").to_string();
            *context.error.guard() = Some(error.clone());
            context.stop.store(true, Ordering::SeqCst);
            let _ = context.ready.send(Err(error.clone()));
            let _ = context.app.emit("app-error", error);
        }
        _ => {}
    }
}
pub fn start(app: AppHandle, settings: &Settings, info: Capture, dir: PathBuf) -> Result<Recording, String> {
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let context = Box::new(Context { app, ready: tx, stop: stop.clone(), error: Mutex::new(None) });
    let path = CString::new(dir.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let pointer = Box::into_raw(context);
    let native = unsafe { parlo_capture_create(path.as_ptr(), settings.microphone as i32, settings.system_audio as i32, event, pointer.cast()) };
    if native.is_null() { unsafe { drop(Box::from_raw(pointer)); } return Err("Impossibile preparare la cattura Mac.".into()); }
    let ready = rx.recv_timeout(Duration::from_secs(120)).map_err(|_| "macOS non ha completato l’avvio della cattura entro due minuti. Se il blocco è iniziato dopo un aggiornamento, usa Impostazioni → Registrazione → Ripristina permessi e riavvia PARLO.".to_string()).and_then(|r| r);
    if let Err(error) = ready {
        unsafe { let warning = parlo_capture_stop(native); parlo_string_free(warning); drop(Box::from_raw(pointer)); }
        return Err(error);
    }
    let start = Instant::now();
    let native = native as usize;
    let pointer = pointer as usize;
    let thread_stop = stop.clone();
    let handle = std::thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) { std::thread::sleep(Duration::from_millis(30)); }
        unsafe {
            let warning = parlo_capture_stop(native as *mut c_void);
            let text = if warning.is_null() { String::new() } else { CStr::from_ptr(warning).to_string_lossy().into_owned() };
            parlo_string_free(warning);
            let context = Box::from_raw(pointer as *mut Context);
            let error = context.error.guard().clone();
            if let Some(error) = error { Err(error) } else { Ok((!text.is_empty()).then_some(text)) }
        }
    });
    Ok(Recording { info, stop, start, handles: vec![handle], dir, video_input: None })
}
pub fn notify(heading: &str, title: &str) -> Result<(), String> {
    let heading = CString::new(heading).map_err(|e| e.to_string())?;
    let title = CString::new(title).map_err(|e| e.to_string())?;
    let result = unsafe { parlo_notify(heading.as_ptr(), title.as_ptr()) };
    if result.is_null() { return Ok(()); }
    let message = unsafe { CStr::from_ptr(result).to_string_lossy().into_owned() };
    unsafe { parlo_string_free(result); }
    Err(message)
}
