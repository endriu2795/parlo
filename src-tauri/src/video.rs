use crate::{db, engine_child::EngineChild, model::Settings, AppState, LockExt, Shared};
use serde::{Deserialize, Serialize};
use std::{io::{BufRead, BufReader, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc, Mutex}, thread, time::{Duration, Instant}};
use tauri::{Emitter, Manager};

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Serialize)]
pub struct Region { x: i32, y: i32, width: u32, height: u32 }
#[derive(Clone, Serialize)]
pub struct Monitor { name: String, region: Region }
static SELECTION: Mutex<Option<Vec<Monitor>>> = Mutex::new(None);
static HINT_GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static WARMUP: Mutex<Option<Warmup>> = Mutex::new(None);

pub fn select(app: &tauri::AppHandle, state: &Shared, settings: &Settings) -> Result<()> {
    if !cfg!(windows) { return Err("La registrazione video è disponibile su Windows.".into()); }
    let cursor = app.cursor_position().map_err(|e| e.to_string())?;
    let mut monitors: Vec<_> = app.available_monitors().map_err(|e| e.to_string())?.into_iter()
        .enumerate().map(|(i, m)| Monitor {
            name: format!("{} {} · {} × {}", crate::locale::tr("Schermo", "Screen"), i + 1, m.size().width, m.size().height),
            region: Region { x: m.position().x, y: m.position().y, width: m.size().width, height: m.size().height },
        }).collect();
    monitors.sort_by_key(|m| {
        let r = &m.region;
        !(cursor.x >= r.x as f64 && cursor.y >= r.y as f64 && cursor.x < r.x as f64 + r.width as f64 && cursor.y < r.y as f64 + r.height as f64)
    });
    let first = monitors.first().ok_or("Nessuno schermo disponibile")?.region.clone();
    *SELECTION.guard() = Some(monitors);
    // Publish monitors before loading the selector, including its first mount.
    // Keep the default audio-only startup free of an extra WebView.
    if app.get_webview_window("video-area").is_none() {
        let window = tauri::WebviewWindowBuilder::new(app, "video-area", tauri::WebviewUrl::App("overlay.html?video-area=1".into()))
            .title(crate::locale::tr("Seleziona area video", "Select video area")).inner_size(800., 600.).resizable(false).decorations(false)
            .transparent(true).shadow(false).always_on_top(true).skip_taskbar(true).visible(false)
            .build().map_err(|e| e.to_string())?;
        let _ = crate::screen::prepare_overlay(&window);
    }
    place(app, &first)?;
    let window = app.get_webview_window("video-area").ok_or("Selezione video non disponibile")?;
    HINT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    window.set_ignore_cursor_events(false).map_err(|e|e.to_string())?;
    window.set_content_protected(false).map_err(|e|e.to_string())?;
    window.emit("video-selection-reset", ()).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    warm_up(state.clone(), settings.clone());
    Ok(())
}
fn place(app: &tauri::AppHandle, region: &Region) -> Result<()> {
    let window = app.get_webview_window("video-area").ok_or("Selezione video non disponibile")?;
    window.set_position(tauri::PhysicalPosition::new(region.x, region.y)).map_err(|e| e.to_string())?;
    window.set_size(tauri::PhysicalSize::new(region.width, region.height)).map_err(|e| e.to_string())
}
#[tauri::command]
pub fn video_monitors() -> Result<Vec<Monitor>> {
    SELECTION.guard().clone().ok_or("Avvia prima la selezione dell’area video.".into())
}
#[tauri::command]
pub fn video_select_monitor(app: tauri::AppHandle, index: usize) -> Result<()> {
    let monitors = video_monitors()?;
    place(&app, &monitors.get(index).ok_or("Schermo non disponibile")?.region)
}
#[tauri::command]
pub fn video_cancel_selection(app: tauri::AppHandle) -> Result<()> {
    SELECTION.guard().take();
    hide_selection(&app)
}
pub fn hide_selection(app: &tauri::AppHandle) -> Result<()> {
    let pending = WARMUP.guard().take();
    drop(pending);
    HINT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("video-area") {
        window.hide().map_err(|e|e.to_string())?;
        let _ = crate::screen::cloak(&window, false);
    }
    Ok(())
}
fn show_recording_hint(app: &tauri::AppHandle, region: &Region) -> Result<()> {
    let window = app.get_webview_window("video-area").ok_or("Indicatore area non disponibile")?;
    window.set_position(tauri::PhysicalPosition::new(region.x, region.y)).map_err(|e|e.to_string())?;
    window.set_size(tauri::PhysicalSize::new(region.width, region.height)).map_err(|e|e.to_string())?;
    window.set_content_protected(true).map_err(|e|e.to_string())?;
    window.set_ignore_cursor_events(true).map_err(|e|e.to_string())?;
    let generation = HINT_GENERATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    // Show cloaked until the WebView has painted the recording indicator;
    // the selection toolbar must never flash over the captured area.
    crate::screen::cloak(&window, true)?;
    if let Err(error) = window.emit("video-recording-started", serde_json::json!({"width":region.width,"height":region.height,"generation":generation})).and_then(|_| window.show()) {
        let _ = window.hide();
        let _ = crate::screen::cloak(&window, false);
        return Err(error.to_string());
    }
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(4));
        if HINT_GENERATION.compare_exchange(generation, generation + 1, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() {
            let _ = window.hide();
            let _ = crate::screen::cloak(&window, false);
            let _ = window.app_handle().emit("app-error", "Il bordo dell’area video non è disponibile. La registrazione continua.");
        }
    });
    Ok(())
}
#[tauri::command]
pub fn video_hint_ready(window: tauri::WebviewWindow, generation: u64) -> Result<()> {
    if window.label() != "video-area" { return Err("Indicatore video non disponibile".into()); }
    // Claim this generation so the readiness watchdog cannot hide the border.
    if HINT_GENERATION.compare_exchange(generation, generation + 1, std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst).is_ok() && window.is_visible().unwrap_or(false) {
        crate::screen::cloak(&window, false)?;
    }
    Ok(())
}
#[derive(Deserialize)]
pub struct Rectangle { x: f64, y: f64, width: f64, height: f64 }
fn region(monitor: &Region, rectangle: Rectangle) -> Result<Region> {
    let r = rectangle;
    if [r.x, r.y, r.width, r.height].iter().any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        || r.x + r.width > 1.000001 || r.y + r.height > 1.000001 {
        return Err("Area video non valida".into());
    }
    let width = (r.width * monitor.width as f64).floor() as u32;
    let height = (r.height * monitor.height as f64).floor() as u32;
    if width < 32 || height < 32 || width as u64 * height as u64 > 64000000 { return Err("Seleziona un’area di almeno 32 × 32 pixel.".into()); }
    Ok(Region { x: monitor.x + (r.x * monitor.width as f64).floor() as i32, y: monitor.y + (r.y * monitor.height as f64).floor() as i32, width, height })
}
#[tauri::command]
pub async fn video_start_selection(app: tauri::AppHandle, state: tauri::State<'_, Shared>, index: usize, rectangle: Rectangle) -> Result<()> {
    let selected = {
        let monitors = video_monitors()?;
        region(&monitors.get(index).ok_or("Schermo non disponibile")?.region, rectangle)?
    };
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Keep the busy selector visible while preparing Python and the codec.
        // start_with_region hides it immediately before actual audio/video capture.
        match crate::start_with_region(&app, &state, Some(selected.clone())) {
            Ok(()) => {
                // Do not reopen a stale border after a concurrent stop.
                let _operation = state.operation.guard();
                if state.status.guard().capture.is_none() {
                    return Err("La registrazione è già terminata.".into());
                }
                SELECTION.guard().take();
                if let Err(error) = show_recording_hint(&app, &selected) { let _ = app.emit("app-error", error); }
                Ok(())
            }
            Err(error) => {
                if let Some(window) = app.get_webview_window("video-area") { let _ = window.show(); let _ = window.set_focus(); }
                Err(error)
            }
        }
    }).await.map_err(|e| e.to_string())?
}

fn engine_command(settings: &Settings, state: &AppState) -> Result<Command> {
    let python = crate::provision::python(settings).ok_or("Motore Python non disponibile per il video")?;
    let mut command = Command::new(python);
    command.args(["-I", "-X", "utf8", "-u"]).arg(state.data_dir.join("video.py"))
        .env_remove("PYTHONHOME").env_remove("PYTHONPATH");
    #[cfg(windows)] {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00004000); // Hidden, below normal priority: protect audio capture.
    }
    #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
    Ok(command)
}
fn command(settings: &Settings, state: &AppState, directory: &Path) -> Result<Command> {
    let mut command = engine_command(settings, state)?;
    command.arg("--directory").arg(directory);
    Ok(command)
}

type RuntimeKey = (PathBuf, String, String);
fn runtime_key(settings: &Settings, state: &AppState) -> RuntimeKey {
    (state.data_dir.clone(), settings.python_bin.clone(), settings.stt_provider.clone())
}
struct WarmWorker {
    key: RuntimeKey,
    child: Arc<EngineChild>,
    input: std::process::ChildStdin,
    output: std::process::ChildStdout,
    errors: std::process::ChildStderr,
}
impl WarmWorker {
    fn spawn(settings: &Settings, state: &AppState) -> Result<Self> {
        // Import libraries only. The worker receives no region or recording
        // directory until the user explicitly confirms the selection.
        let mut child = engine_command(settings, state)?.arg("--warmup")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("Controllo video non disponibile")?;
        let output = child.stdout.take().ok_or("Stato video non disponibile")?;
        let errors = child.stderr.take().ok_or("Diagnostica video non disponibile")?;
        Ok(Self { key: runtime_key(settings, state), child: Arc::new(EngineChild::new(child)?), input, output, errors })
    }
}
struct Warmup {
    result: std::sync::mpsc::Receiver<Result<WarmWorker>>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
}
impl Warmup {
    fn take(self, key: &RuntimeKey) -> Result<Option<WarmWorker>> {
        match self.result.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(worker)) if &worker.key == key => Ok(Some(worker)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err("Preparazione video troppo lenta. Riprova l’avvio.".into()),
            _ => Ok(None),
        }
    }
}
impl Drop for Warmup {
    fn drop(&mut self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        // Dropping the receiver also drops a queued worker. EngineChild then
        // terminates its whole job; a later send after cancellation does too.
    }
}
fn warm_up(state: Shared, mut settings: Settings) {
    let (sender, result) = std::sync::mpsc::channel();
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let previous = WARMUP.guard().replace(Warmup { result, cancelled: cancelled.clone() });
    drop(previous);
    thread::spawn(move || {
        let worker = (|| {
            if cancelled.load(std::sync::atomic::Ordering::SeqCst) { return Err("Preparazione video annullata".into()); }
            if settings.remote_stt() {
                // Do not install a runtime speculatively or compete with the
                // actual start operation for its provisioning lock.
                let python = crate::python_runtime::prepared_installed(&crate::model::engine_dir())
                    .ok_or("Il motore video verrà preparato all’avvio")?;
                settings.python_bin = python.to_string_lossy().into_owned();
            }
            if cancelled.load(std::sync::atomic::Ordering::SeqCst) { return Err("Preparazione video annullata".into()); }
            WarmWorker::spawn(&settings, &state)
        })();
        let _ = sender.send(worker);
    });
}
pub struct Prepared { child: Arc<EngineChild>, input: std::process::ChildStdin, error: PathBuf }
fn error(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    if !text.trim().is_empty() {
        return format!("Registrazione video: {}", text.chars().rev().take(2000).collect::<String>().chars().rev().collect::<String>().trim());
    }
    let diagnostics = path.parent().and_then(|parent| std::fs::read(parent.join("video-diagnostics.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let detail = diagnostics.as_ref().and_then(|value| value["failure"]["message"].as_str()).map(str::to_owned)
        .unwrap_or_else(|| match diagnostics.as_ref().and_then(|value| value["stop_reason"].as_str()) {
            Some("control_eof") => "Canale di controllo del video chiuso inaspettatamente.".into(),
            Some("control_read_error") => "Errore nella lettura del controllo video.".into(),
            _ => "Il processo video è terminato senza fornire un errore dettagliato.".into(),
        });
    format!("Registrazione video: {detail}")
}
pub fn prepare(state: &AppState, settings: &Settings, directory: &Path, region: &Region) -> Result<Prepared> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let error_path = directory.join("video-error.log");
    let pending = WARMUP.guard().take();
    let warmed = match pending {
        Some(pending) => pending.take(&runtime_key(settings, state))?,
        None => None,
    };
    let worker = match warmed {
        Some(worker) => worker,
        None => WarmWorker::spawn(settings, state)?,
    };
    let WarmWorker { child, mut input, output, mut errors, .. } = worker;
    let mut log = std::fs::File::create(&error_path).map_err(|e| e.to_string())?;
    let error_reader = thread::spawn(move || { let _ = std::io::copy(&mut errors, &mut log); });
    if let Err(error) = writeln!(input, "{}", serde_json::json!({"directory":directory,"region":region})) {
        child.cancel();
        let _ = error_reader.join();
        return Err(format!("{} {error}", self::error(&error_path)));
    }
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut output = BufReader::new(output);
        let mut line = String::new();
        while output.read_line(&mut line).unwrap_or(0) > 0 && line.trim() == "warmed" { line.clear(); }
        let _ = tx.send(line);
    });
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(line) if line.trim() == "ready" => Ok(Prepared { child, input, error: error_path }),
        _ => {
            child.cancel();
            let _ = error_reader.join();
            Err(format!("{} Avvio non riuscito.", error(&error_path)))
        }
    }
}
impl Prepared {
    pub fn attach(mut self, app: tauri::AppHandle, recording: &mut crate::audio::Recording) -> Result<()> {
        writeln!(self.input, "{}", serde_json::json!({"offset": recording.start.elapsed().as_secs_f64()})).map_err(|e| e.to_string())?;
        // The editor is protected when actually opened; the clean annotation
        // canvas is composited independently from the toolbar.
        let input = Arc::new(Mutex::new(self.input));
        recording.video_input = Some(input.clone());
        let stop = recording.stop.clone();
        let call_id = recording.info.id.clone();
        recording.handles.push(thread::spawn(move || {
            let warning = self.error.parent().unwrap().join("video-recovery-warning.txt");
            let mut warned = false;
            while !stop.load(std::sync::atomic::Ordering::SeqCst) && !self.child.exited() {
                if !warned && warning.is_file() {
                    warned = true;
                    let message = "Il video è ripartito dopo un’interruzione dello schermo. Per alcuni secondi rimane l’ultimo fotogramma; l’audio è continuato.";
                    let _ = app.emit("app-error", message);
                    let state = app.state::<Shared>();
                    crate::notifications::recording_issue(&app, &state.data_dir, &call_id, "Registrazione video ripresa", message);
                }
                thread::sleep(Duration::from_millis(50));
            }
            let expected = stop.load(std::sync::atomic::Ordering::SeqCst);
            let _ = writeln!(input.guard(), "stop");
            let deadline = Instant::now() + Duration::from_secs(15);
            while !self.child.exited() && Instant::now() < deadline { thread::sleep(Duration::from_millis(50)); }
            if !self.child.exited() { self.child.cancel(); }
            if !expected || !self.child.success() {
                stop.store(true, std::sync::atomic::Ordering::SeqCst);
                let message = format!("{} Codice uscita: {}.", error(&self.error), self.child.exit_code().map(|code| code.to_string()).unwrap_or_else(||"non disponibile".into()));
                let _ = app.emit("app-error", &message);
                return Err(message);
            }
            let warning_text = std::fs::read_to_string(warning).unwrap_or_default();
            Ok((!warning_text.trim().is_empty()).then(||format!("Video: {}", warning_text.trim())))
        }));
        Ok(())
    }
}
fn path(state: &AppState, id: &str) -> Result<PathBuf> {
    let id = uuid::Uuid::parse_str(id).map_err(|_| "Identificatore call non valido")?;
    Ok(state.data_dir.join("attachments").join(id.to_string()).join("registrazione.mp4"))
}
pub fn retained(state: &AppState, id: &str) -> Result<Option<PathBuf>> {
    let path = path(state, id)?;
    if !path.is_file() { return Ok(None); }
    let parent = state.data_dir.join("attachments").canonicalize().map_err(|e| e.to_string())?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if canonical.parent().and_then(|p| p.parent()) != Some(parent.as_path()) { return Err("Percorso video non sicuro".into()); }
    Ok(Some(canonical))
}
pub fn finalize(state: &AppState, id: &str, settings: &Settings) -> Result<()> {
    let directory = crate::temp_dir(state, id)?;
    if !directory.join("keep-video").is_file() || retained(state, id)?.is_some() { return Ok(()); }
    let destination = path(state, id)?;
    std::fs::create_dir_all(destination.parent().ok_or("Percorso video non valido")?).map_err(|e| e.to_string())?;
    let result = command(settings, state, &directory)?.arg("--output").arg(&destination).output().map_err(|e| e.to_string())?;
    if !result.status.success() { return Err(format!("Salvataggio video: {}", String::from_utf8_lossy(&result.stderr).chars().take(2000).collect::<String>())); }
    Ok(())
}
#[tauri::command]
pub fn open_call_video(state: tauri::State<Shared>, id: String) -> Result<()> {
    db::detail(&state.db.guard(), &id)?;
    let path = retained(&state, &id)?.ok_or("Video locale non disponibile")?;
    let program = if cfg!(windows) { "explorer.exe" } else { "open" };
    Command::new(program).arg(path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn prepare_call_video(app: tauri::AppHandle, state: tauri::State<Shared>, id: String) -> Result<String> {
    db::detail(&state.db.guard(), &id)?;
    let path = retained(&state, &id)?.ok_or("Video locale non disponibile")?;
    app.asset_protocol_scope().allow_file(&path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn video_drawing(window: tauri::WebviewWindow, state: tauri::State<Shared>, overlay_png: Option<String>) -> Result<()> {
    use base64::Engine;
    if window.label() != "drawing" { return Err("Comando riservato all’editor annotazioni".into()); }
    let guard = state.recorder.guard();
    let Some(recording) = guard.as_ref() else { return Ok(()); };
    let Some(input) = recording.video_input.as_ref() else { return Ok(()); };
    let at = recording.start.elapsed().as_secs_f64();
    if overlay_png.is_none() || !window.is_visible().map_err(|e|e.to_string())? {
        return writeln!(input.guard(), "{}", serde_json::json!({"at":at,"png":null})).map_err(|e|e.to_string());
    }
    let overlay_png = overlay_png.unwrap();
    let encoded = overlay_png.strip_prefix("data:image/png;base64,").ok_or("Livello video non valido")?;
    if encoded.len() > 32_000_000 { return Err("Livello video troppo grande".into()); }
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_limits(png::Limits { bytes: 256_000_000 });
    let reader = decoder.read_info().map_err(|e| e.to_string())?;
    let info = reader.info();
    if info.width == 0 || info.height == 0 || info.width as u64 * info.height as u64 > 64_000_000 { return Err("Dimensione annotazioni non valida".into()); }
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as usize;
    #[cfg(not(windows))]
    let hwnd = 0usize;
    let data = serde_json::json!({"at":at,"x":origin.x,"y":origin.y,"width":size.width,"height":size.height,"window":hwnd,"png":encoded});
    // Feed the running capture, never a reusable final image on disk.
    let result = writeln!(input.guard(), "{data}").map_err(|e|e.to_string());
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    fn warmup_reuses_matching_worker_and_closes_cancelled_or_stale_workers() {
        use std::os::windows::process::CommandExt;
        use windows::Win32::{Foundation::{CloseHandle, WAIT_OBJECT_0, WAIT_TIMEOUT}, System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE}};
        let key = (PathBuf::from("synthetic"), "python".into(), "local".into());
        for action in ["reuse", "cancel", "stale", "cancel_before_spawn"] {
            let (sender, result) = std::sync::mpsc::channel();
            let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let mut pending = Some(Warmup { result, cancelled: cancelled.clone() });
            if action == "cancel_before_spawn" { drop(pending.take()); }
            let mut process = Command::new("powershell.exe")
                .args(["-NoProfile", "-Command", "[Console]::WriteLine('warmed'); Start-Sleep -Seconds 30"])
                .creation_flags(0x08000000).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
                .spawn().unwrap();
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, process.id()).unwrap() };
            let input = process.stdin.take().unwrap();
            let output = process.stdout.take().unwrap();
            let errors = process.stderr.take().unwrap();
            let child = Arc::new(EngineChild::new(process).unwrap());
            let identity = Arc::as_ptr(&child);
            let worker = WarmWorker { key: key.clone(), child, input, output, errors };
            if action == "cancel_before_spawn" {
                assert!(sender.send(Ok(worker)).is_err());
            } else {
                sender.send(Ok(worker)).unwrap_or_else(|_| panic!("Warmup receiver closed"));
                let pending = pending.take().unwrap();
                match action {
                    "reuse" => {
                        let worker = pending.take(&key).unwrap().unwrap();
                        assert_eq!(Arc::as_ptr(&worker.child), identity);
                        assert_eq!(unsafe { WaitForSingleObject(handle, 0) }, WAIT_TIMEOUT);
                        drop(worker);
                    }
                    "stale" => {
                        let mut changed = key.clone(); changed.1 = "different-python".into();
                        assert!(pending.take(&changed).unwrap().is_none());
                    }
                    _ => drop(pending),
                }
            }
            assert!(cancelled.load(std::sync::atomic::Ordering::SeqCst));
            assert_eq!(unsafe { WaitForSingleObject(handle, 5000) }, WAIT_OBJECT_0, "{action} left a worker running");
            unsafe { let _ = CloseHandle(handle); }
        }
    }
    #[test]
    fn region_uses_physical_pixels_and_rejects_outside_or_tiny_rectangles() {
        let monitor = Region { x: -2560, y: 0, width: 2560, height: 1440 };
        let selected = region(&monitor, Rectangle { x: 0.25, y: 0.25, width: 0.5, height: 0.5 }).unwrap();
        assert_eq!((selected.x, selected.y, selected.width, selected.height), (-1920, 360, 1280, 720));
        assert!(region(&monitor, Rectangle { x: 0.8, y: 0., width: 0.5, height: 1. }).is_err());
        assert!(region(&monitor, Rectangle { x: 0., y: 0., width: 0.001, height: 1. }).is_err());
    }
}
