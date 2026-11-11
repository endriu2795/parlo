use crate::LockExt;
use serde::Deserialize;
use std::{sync::Mutex, time::Duration};
use tauri::{Emitter, Manager};

#[derive(Clone, Deserialize)]
pub struct Region { x: f64, y: f64, width: f64, height: f64 }
#[derive(Default)]
struct Mode { enabled: bool, regions: Vec<Region> }
#[derive(Default)]
pub struct DrawingInteraction(Mutex<Mode>);

pub fn enabled(app: &tauri::AppHandle) -> bool {
    app.state::<DrawingInteraction>().0.guard().enabled
}

#[tauri::command]
pub fn set_drawing_interaction(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, DrawingInteraction>,
    passthrough: bool,
    regions: Vec<Region>,
) -> Result<(), String> {
    if window.label() != "drawing" { return Err("Comando riservato all’editor".into()); }
    if regions.len() > 20 || regions.iter().any(|r| [r.x, r.y, r.width, r.height].iter().any(|v| !v.is_finite()) || r.width < 0. || r.height < 0.) {
        return Err("Area editor non valida".into());
    }
    if !passthrough { window.set_ignore_cursor_events(false).map_err(|e| e.to_string())?; }
    *state.0.guard() = Mode { enabled: passthrough, regions };
    Ok(())
}

// Keep the toolbar reachable even when the transparent canvas passes clicks
// through to other windows. Poll at full rate only during interaction mode.
pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut ignored = false;
        #[cfg(windows)]
        let mut escape_down = false;
        while let Some(window) = app.get_webview_window("drawing") {
            let (active, regions) = {
                let state = app.state::<DrawingInteraction>();
                let mode = state.0.guard();
                (mode.enabled, mode.regions.clone())
            };
            #[cfg(windows)] {
                // In desktop interaction mode the underlying app owns focus.
                // Still let Esc close the annotation editor.
                let down = active && window.is_visible().unwrap_or(false)
                    && unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(0x1B) } < 0;
                if down && !escape_down { let _ = window.emit("annotation-exit", ()); }
                escape_down = down;
            }
            let next = if active && window.is_visible().unwrap_or(false) {
                match (app.cursor_position(), window.outer_position(), window.scale_factor()) {
                    (Ok(cursor), Ok(origin), Ok(scale)) => {
                        let x = (cursor.x - origin.x as f64) / scale;
                        let y = (cursor.y - origin.y as f64) / scale;
                        !regions.iter().any(|r| x >= r.x && x <= r.x + r.width && y >= r.y && y <= r.y + r.height)
                    }
                    _ => false,
                }
            } else { false };
            if next != ignored && window.set_ignore_cursor_events(next).is_ok() { ignored = next; }
            std::thread::sleep(Duration::from_millis(if active { 32 } else { 200 }));
        }
    });
}
