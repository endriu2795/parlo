//! Recover grants associated with the old identity of an ad-hoc signed release.
#[tauri::command]
pub async fn reset_capture_permissions(app: tauri::AppHandle, state: tauri::State<'_, crate::Shared>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use crate::LockExt;
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            {
                let _operation = state.operation.guard();
                crate::updates::ensure_not_installing(&app)?;
                crate::updates::ensure_idle(&state).map_err(|_| "Termina la registrazione e attendi elaborazione, chat e preparazione del motore prima di ripristinare i permessi.".to_string())?;
                // Never reset other apps or unrelated privacy categories.
                for service in ["Microphone", "ScreenCapture"] {
                    let output = std::process::Command::new("/usr/bin/tccutil")
                        .args(["reset", service, app.config().identifier.as_str()])
                        .output().map_err(|error| format!("Ripristino permesso {service} non riuscito: {error}"))?;
                    if !output.status.success() {
                        return Err(format!("macOS non ha ripristinato il permesso {service}. Rimuovi PARLO dalla relativa lista Privacy e sicurezza e aggiungi nuovamente l’app."));
                    }
                }
            }
            // Closing the window only hides PARLO. TCC needs a new process.
            app.restart();
        }).await.map_err(|error| error.to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, state);
        Err("Ripristino dei permessi disponibile solo su macOS.".into())
    }
}
