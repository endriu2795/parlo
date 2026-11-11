use crate::{LockExt, Shared};
use serde::{Deserialize, Serialize};
use std::{
    sync::{atomic::{AtomicBool, Ordering}, Mutex},
    thread,
    time::Duration,
};
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize};
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}
struct Placement {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
}
#[derive(Default)]
pub struct RecorderDock {
    restore_position: Mutex<Option<PhysicalPosition<i32>>>,
    compact: AtomicBool,
    pending_frame: Mutex<Option<String>>,
}

pub const EXPANDED_SIZE: LogicalSize<f64> = LogicalSize::new(440., 188.);
const COMPACT_SIZE: LogicalSize<f64> = LogicalSize::new(238., 52.);

pub fn resize(window: &tauri::WebviewWindow, expanded: bool) -> Result<()> {
    window.set_size(if expanded { EXPANDED_SIZE } else { COMPACT_SIZE })
        .map_err(|e| e.to_string())?;
    window.app_handle().state::<RecorderDock>().compact.store(!expanded, Ordering::Relaxed);
    let _ = window.emit("recorder-expanded", expanded);
    Ok(())
}

fn restore_controls(window: &tauri::WebviewWindow, position: PhysicalPosition<i32>, compact: bool) -> Result<()> {
    // Move to the destination display before applying the logical size.
    window.set_position(position).map_err(|e| e.to_string())?;
    resize(window, !compact)
}

fn tab_placement(area: &tauri::PhysicalRect<i32, u32>, scale: f64) -> Placement {
    let size = PhysicalSize::new(
        (30. * scale).round() as u32,
        (64. * scale).round() as u32,
    );

    // Show only a small restore tab, flush with the right work-area edge.
    let position = PhysicalPosition::new(
        area.position.x + area.size.width.saturating_sub(size.width) as i32,
        area.position.y + (area.size.height.saturating_sub(size.height) as f64 * 0.75).round() as i32,
    );
    Placement { position, size }
}
fn apply(window: &tauri::WebviewWindow, placement: &Placement) -> Result<()> {
    window.set_size(placement.size).map_err(|e| e.to_string())?;
    window
        .set_position(placement.position)
        .map_err(|e| e.to_string())
}
pub fn restore(app: &tauri::AppHandle) -> Result<()> {
    let Some(state) = app.try_state::<RecorderDock>() else {
        return Ok(());
    };
    // Never hold this mutex while querying native windows: synchronous UI
    // commands may also restore the controls while those queries are dispatched.
    let saved = *state.restore_position.guard();
    if let Some(mut position) = saved {
        let window = app
            .get_webview_window("recorder")
            .ok_or("Registratore non disponibile")?;
        // If a display was unplugged, keep the restored controls on a visible screen.
        let monitors = app.available_monitors().map_err(|e| e.to_string())?;
        let visible = monitors.iter().any(|m| {
            let a = m.work_area();
            position.x >= a.position.x
                && position.y >= a.position.y
                && position.x < a.position.x + a.size.width as i32
                && position.y < a.position.y + a.size.height as i32
        });
        if !visible {
            if let Some(m) = monitors.first() {
                position = m.work_area().position;
            }
        }
        restore_controls(&window, position, state.compact.load(Ordering::Relaxed))?;
        *state.restore_position.guard() = None;
        let pending = state.pending_frame.guard().take().is_some();
        if pending {
            let _ = crate::screen::cloak(&window, false);
        }
        let _ = window.emit("recorder-docked", Option::<Edge>::None);
    }
    Ok(())
}
pub fn reset(app: &tauri::AppHandle) {
    let _ = restore(app);
}
pub fn raise_recorder(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("recorder") {
        if window.is_visible().unwrap_or(false) {
            // Reinsert above the full-screen editor without taking keyboard focus.
            let _ = window.set_always_on_top(false);
            let _ = window.set_always_on_top(true);
        }
    }
}

// The caller holds the operation lock when a capture is starting or changing.
pub fn dock(app: &tauri::AppHandle) -> Result<()> {
    let window = app.get_webview_window("recorder").ok_or("Registratore non disponibile")?;
    let monitor = match window.current_monitor().map_err(|e| e.to_string())? {
        Some(monitor) => monitor,
        None => app.primary_monitor().map_err(|e| e.to_string())?.ok_or("Schermo non disponibile")?,
    };
    let managed = app.state::<RecorderDock>();
    let previous = *managed.restore_position.guard();
    let saved = match previous {
        Some(saved) => saved,
        None => window.outer_position().map_err(|e| e.to_string())?,
    };
    let tab = tab_placement(monitor.work_area(), monitor.scale_factor());
    if let Err(error) = apply(&window, &tab) {
        if previous.is_none() {
            let _ = restore_controls(&window, saved, managed.compact.load(Ordering::Relaxed));
        }
        return Err(error);
    }
    *managed.restore_position.guard() = Some(saved);
    window.emit("recorder-docked", Edge::Right).map_err(|e| e.to_string())
}

pub fn show_capture(app: &tauri::AppHandle, state: &Shared, capture_id: &str) -> Result<()> {
    let window = app.get_webview_window("recorder").ok_or("Registratore non disponibile")?;
    let dock = app.state::<RecorderDock>();
    // A hidden WebView cannot paint. On Windows show it under the compositor
    // cloak until React has painted the tab at its final size.
    let docked = dock.restore_position.guard().is_some();
    let cloaked = cfg!(windows) && docked && crate::screen::cloak(&window, true).is_ok();
    if cloaked {
        *dock.pending_frame.guard() = Some(capture_id.to_owned());
    }
    if let Err(error) = window.show() {
        if cloaked {
            dock.pending_frame.guard().take();
            let _ = crate::screen::cloak(&window, false);
        }
        return Err(error.to_string());
    }
    if cloaked {
        let state = state.clone();
        let capture_id = capture_id.to_owned();
        thread::spawn(move || {
            // Keep recording controls available if the WebView cannot signal.
            thread::sleep(Duration::from_secs(3));
            let _operation = state.operation.guard();
            let _ = reveal_frame(&window, &capture_id);
        });
    }
    Ok(())
}

fn reveal_frame(window: &tauri::WebviewWindow, capture_id: &str) -> Result<()> {
    let dock = window.app_handle().state::<RecorderDock>();
    {
        let mut pending = dock.pending_frame.guard();
        if pending.as_deref() != Some(capture_id) {
            return Ok(());
        }
        pending.take();
    }
    crate::screen::cloak(window, false)
}

#[tauri::command]
pub async fn recorder_frame_ready(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Shared>,
    capture_id: String,
) -> Result<()> {
    if window.label() != "recorder" {
        return Err("Comando riservato all’indicatore di registrazione".into());
    }
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = state.operation.guard();
        reveal_frame(&window, &capture_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn recorder_dock_status(app: tauri::AppHandle) -> Option<Edge> {
    app.try_state::<RecorderDock>().and_then(|state| state.restore_position.guard().as_ref().map(|_| Edge::Right))
}

#[tauri::command]
pub async fn dock_recorder(
    app: tauri::AppHandle,
    state: tauri::State<'_, Shared>,
    edge: Option<Edge>,
) -> Result<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = state.operation.guard();
        if state.status.guard().capture.is_none() {
            return Err("Nessuna registrazione in corso".into());
        }
        let Some(_) = edge else {
            return restore(&app);
        };
        dock(&app)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorder_tab_stays_above_taskbar_on_scaled_secondary_display() {
        let area = tauri::PhysicalRect {
            position: PhysicalPosition::new(-1920, -100),
            size: PhysicalSize::new(1920, 1040),
        };
        let tab = tab_placement(&area, 1.5);
        assert_eq!(tab.position.x, -45);
        assert_eq!(tab.position.y, 608);
        assert_eq!(tab.size, PhysicalSize::new(45, 96));
    }
}
