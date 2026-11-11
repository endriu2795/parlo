//! Native completion notifications, including the portable Windows build.
use crate::LockExt;
use tauri::{Emitter, Manager};

fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn toast_xml(heading: &str, title: &str) -> String {
    format!("<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>", escape(heading), escape(title))
}

#[cfg(windows)]
#[derive(Default)]
pub struct Notifications(std::sync::Mutex<Vec<windows::UI::Notifications::ToastNotification>>);
#[cfg(not(windows))]
#[derive(Default)]
pub struct Notifications;

#[cfg(windows)]
fn shortcut(data_dir: &std::path::Path, app_id: &str, app_name: &str) -> windows::core::Result<()> {
    use windows::{
        core::{Interface, HSTRING},
        Win32::{
            Foundation::PROPERTYKEY,
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
                    StructuredStorage::*, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
                },
                Variant::VT_LPWSTR,
            },
            UI::Shell::{
                FOLDERID_Programs, IShellLinkW, PropertiesSystem::IPropertyStore,
                SHGetKnownFolderPath, SHStrDupW, ShellLink, KF_FLAG_DEFAULT,
            },
        },
    };
    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if init.is_err() && init.0 != 0x80010106u32 as i32 {
            init.ok()?;
        }
        let _apartment = Apartment(init.is_ok());
        let programs = SHGetKnownFolderPath(&FOLDERID_Programs, KF_FLAG_DEFAULT, None)?;
        let folder = programs.to_string();
        CoTaskMemFree(Some(programs.0.cast()));
        let path = std::path::PathBuf::from(folder?).join(format!("{app_name}.lnk"));
        let exe = std::env::current_exe().map_err(|e| {
            windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), e.to_string())
        })?;
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        let project = data_dir.parent().unwrap_or(data_dir);
        let launcher = project.join("scripts/run.ps1");
        if launcher.is_file() {
            let powershell = std::path::PathBuf::from(
                std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
            )
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
            link.SetPath(&HSTRING::from(powershell.to_string_lossy().as_ref()))?;
            link.SetArguments(&HSTRING::from(format!(
                "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File \"{}\"",
                launcher.display()
            )))?;
        } else {
            link.SetPath(&HSTRING::from(exe.to_string_lossy().as_ref()))?;
        }
        link.SetDescription(&HSTRING::from(app_name))?;
        link.SetIconLocation(&HSTRING::from(exe.to_string_lossy().as_ref()), 0)?;
        let store: IPropertyStore = link.cast()?;
        let key = PROPERTYKEY {
            fmtid: windows::core::GUID::from_u128(0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3),
            pid: 5,
        };
        // PROPVARIANT's Drop calls PropVariantClear, so use a COM-owned string.
        let id = SHStrDupW(&HSTRING::from(app_id))?;
        let value = PROPVARIANT {
            Anonymous: PROPVARIANT_0 {
                Anonymous: std::mem::ManuallyDrop::new(PROPVARIANT_0_0 {
                    vt: VT_LPWSTR,
                    Anonymous: PROPVARIANT_0_0_0 { pwszVal: id },
                    ..Default::default()
                }),
            },
        };
        store.SetValue(&key, &value)?;
        store.Commit()?;
        let file: IPersistFile = link.cast()?;
        file.Save(&HSTRING::from(path.to_string_lossy().as_ref()), true)?;
    }
    Ok(())
}

#[cfg(windows)]
fn register(data_dir: &std::path::Path, app_id: &str, app_name: &str) -> windows::core::Result<()> {
    use windows::{
        core::{HSTRING, PCWSTR},
        Win32::System::Registry::*,
    };
    let icon = data_dir.join("notification-icon.png");
    std::fs::write(&icon, include_bytes!("../icons/64x64.png")).map_err(|e| {
        windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), e.to_string())
    })?;
    let mut key = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            &HSTRING::from(format!("Software\\Classes\\AppUserModelId\\{app_id}")),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
    }
    let result: windows::core::Result<()> = (|| {
        for (name, value) in [
            ("DisplayName", app_name.to_string()),
            ("IconUri", icon.to_string_lossy().into_owned()),
        ] {
            let bytes: Vec<u8> = value
                .encode_utf16()
                .chain(Some(0))
                .flat_map(u16::to_le_bytes)
                .collect();
            unsafe {
                RegSetValueExW(key, &HSTRING::from(name), None, REG_SZ, Some(&bytes)).ok()?;
            }
        }
        Ok(())
    })();
    unsafe {
        let _ = RegCloseKey(key);
    }
    result?;
    shortcut(data_dir, app_id, app_name)
}

#[cfg(windows)]
fn step<T>(result: windows::core::Result<T>, context: &str) -> windows::core::Result<T> {
    result.map_err(|e| windows::core::Error::new(e.code(), format!("{context}: {e}")))
}

#[cfg(windows)]
fn create(
    data_dir: &std::path::Path,
    app_id: &str,
    app_name: &str,
    heading: &str,
    title: &str,
) -> windows::core::Result<(
    windows::UI::Notifications::ToastNotifier,
    windows::UI::Notifications::ToastNotification,
)> {
    use windows::{
        core::HSTRING,
        Data::Xml::Dom::XmlDocument,
        UI::Notifications::{NotificationSetting, ToastNotification, ToastNotificationManager},
    };
    step(register(data_dir, app_id, app_name), "Registrazione notifiche")?;
    unsafe {
        step(
            windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(app_id)),
            "Identità del processo",
        )?;
    }
    let notifier = step(
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id)),
        "Creazione notifier",
    )?;
    match notifier.Setting() {
        Ok(NotificationSetting::Enabled) => {}
        Ok(_) => return Err(windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            crate::locale::tr("Le notifiche di PARLO sono disattivate nelle impostazioni di Windows.", "PARLO notifications are disabled in Windows settings."),
        )),
        // An unregistered notification settings entry must not block the first Show.
        Err(e) if e.code().0 == 0x80070490u32 as i32 => {}
        Err(e) => return step(Err(e), "Impostazioni notifiche Windows"),
    }
    let xml = step(XmlDocument::new(), "Documento notifica")?;
    step(
        xml.LoadXml(&HSTRING::from(toast_xml(heading, title))),
        "Contenuto notifica",
    )?;
    Ok((
        notifier,
        step(
            ToastNotification::CreateToastNotification(&xml),
            "Creazione toast",
        )?,
    ))
}

#[cfg(windows)]
pub fn check(data_dir: &std::path::Path) -> Result<(), String> {
    use windows::{core::HSTRING, UI::Notifications::ToastNotificationManager};
    let result = (|| -> windows::core::Result<()> {
        let (notifier, toast) = create(
            data_dir,
            crate::APP_ID,
            "Parlo",
            crate::locale::tr("Trascrizione completata", "Transcript ready"),
            crate::locale::tr("Notifica di prova · PARLO è pronto ad avvisarti.", "Test notification · PARLO is ready to notify you."),
        )?;
        toast.SetTag(&HSTRING::from("desktop-check"))?;
        step(notifier.Show(&toast), "Invio notifica")?;
        std::thread::sleep(std::time::Duration::from_millis(800));
        let manager = step(ToastNotificationManager::History(), "Centro notifiche")?;
        let history = step(
            manager.GetHistoryWithId(&HSTRING::from(crate::APP_ID)),
            "Archivio notifiche PARLO",
        )?;
        let mut found = false;
        for notification in history {
            if notification.Tag()? == HSTRING::from("desktop-check") {
                found = true;
            }
        }
        if !found {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                "Windows non ha registrato la notifica nel centro notifiche.",
            ));
        }
        Ok(())
    })();
    result.map_err(|e| e.to_string())
}

#[cfg(windows)]
fn show_message(
    app: &tauri::AppHandle,
    data_dir: &std::path::Path,
    call_id: &str,
    heading: &str,
    title: &str,
) -> Result<(), String> {
    use windows::{
        core::IInspectable, Foundation::TypedEventHandler, UI::Notifications::ToastNotification,
    };
    let result = (|| -> windows::core::Result<()> {
        let config = app.config();
        let app_name = config.product_name.as_deref().unwrap_or(&app.package_info().name);
        let shortcut_name = if config.identifier == crate::APP_ID && app_name == "PARLO" { "Parlo" } else { app_name };
        let (notifier, toast) = create(data_dir, &config.identifier, shortcut_name, heading, title)?;
        let handle = app.clone();
        let id = call_id.to_string();
        toast.Activated(&TypedEventHandler::<ToastNotification, IInspectable>::new(
            move |_, _| {
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    if !id.is_empty() {
                        let _ = window.emit("open-call", &id);
                    }
                }
                Ok(())
            },
        ))?;
        notifier.Show(&toast)?;
        let state = app.state::<Notifications>();
        let mut shown = state.0.guard();
        shown.push(toast);
        if shown.len() > 100 {
            shown.remove(0);
        }
        Ok(())
    })();
    result.map_err(|e| e.to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn show_message(_: &tauri::AppHandle, _: &std::path::Path, _: &str, _: &str, _: &str) -> Result<(), String> {
    Err("Le notifiche desktop macOS non sono ancora implementate.".into())
}
#[cfg(target_os = "macos")]
fn show_message(_: &tauri::AppHandle, _: &std::path::Path, _: &str, heading: &str, title: &str) -> Result<(), String> {
    crate::mac_audio::notify(heading, title)
}

pub fn show(app: &tauri::AppHandle, data_dir: &std::path::Path, call_id: &str, title: &str) -> Result<(), String> {
    show_message(app, data_dir, call_id, crate::locale::tr("Trascrizione completata", "Transcript ready"), title)
}

pub fn recording_issue(app: &tauri::AppHandle, data_dir: &std::path::Path, call_id: &str, heading: &str, message: &str) {
    if show_message(app, data_dir, call_id, heading, message).is_err() {
        // Critical capture problems must not remain hidden in a minimized app.
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
        }
    }
}

fn foreground(app: &tauri::AppHandle) -> bool {
    app.get_webview_window("main").is_some_and(|w| {
        w.is_visible().unwrap_or(false)
            && w.is_focused().unwrap_or(false)
            && !w.is_minimized().unwrap_or(false)
    })
}

pub fn completed(app: &tauri::AppHandle, state: &crate::AppState, call_id: &str) {
    if foreground(app) {
        return;
    }
    let title = {
        let db = state.db.guard();
        let Ok(settings) = crate::db::get_settings(&db) else {
            return;
        };
        if !settings.completion_notifications {
            return;
        }
        let Ok(detail) = crate::db::detail(&db, call_id) else {
            return;
        };
        if detail.call.status != "ready" {
            return;
        }
        detail.call.title
    };
    if let Err(e) = show(app, &state.data_dir, call_id, &title) {
        let _ = app.emit(
            "app-error",
            format!("Trascrizione salvata. Notifica desktop non disponibile: {e}"),
        );
    }
}

fn chat_title(conn: &rusqlite::Connection, call_id: &str, in_foreground: bool, cancelled: bool) -> Option<String> {
    if in_foreground || cancelled || !crate::db::get_settings(conn).ok()?.completion_notifications {
        return None;
    }
    conn.query_row("SELECT title FROM calls WHERE id=?1", [call_id], |row| row.get(0)).ok()
}

pub fn chat_completed(app: &tauri::AppHandle, state: &crate::AppState, call_id: &str) {
    let in_foreground = foreground(app);
    let cancelled = state.chat_control.cancelled.load(std::sync::atomic::Ordering::SeqCst);
    let title = chat_title(&state.db.guard(), call_id, in_foreground, cancelled);
    let Some(title) = title else { return; };
    if let Err(e) = show_message(app, &state.data_dir, call_id, crate::locale::tr("Risposta dell’assistente pronta", "Assistant response ready"), &title) {
        let _ = app.emit("app-error", format!("Risposta salvata. Notifica desktop non disponibile: {e}"));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn user_title_cannot_inject_xml_and_no_transcript_is_included() {
        let xml = super::toast_xml("Trascrizione completata", "Call <test> & \"nome\"\0");
        assert!(xml.contains("Call &lt;test&gt; &amp; &quot;nome&quot;"));
        assert!(!xml.contains('\0'));
        assert_eq!(xml.matches("<text>").count(), 2);
    }
    #[test]
    fn chat_notifications_respect_focus_cancellation_and_settings() {
        let conn = crate::db::open(std::path::Path::new(":memory:")).unwrap();
        crate::db::create(&conn, "chat-test", "Call di prova", "2026-10-24T12:00:00Z").unwrap();
        crate::db::append_response(&conn, "chat-test", "Testo riservato della risposta", "test").unwrap();
        assert_eq!(super::chat_title(&conn, "chat-test", false, false).as_deref(), Some("Call di prova"));
        assert!(super::chat_title(&conn, "chat-test", true, false).is_none());
        assert!(super::chat_title(&conn, "chat-test", false, true).is_none());
        assert!(super::chat_title(&conn, "missing", false, false).is_none());
        let xml = super::toast_xml("Risposta dell’assistente pronta", "Call di prova");
        assert!(xml.contains("Risposta dell’assistente pronta"));
        assert!(!xml.contains("Testo riservato"));
        let mut settings = crate::db::get_settings(&conn).unwrap();
        settings.completion_notifications = false;
        crate::db::save_settings(&conn, &settings).unwrap();
        assert!(super::chat_title(&conn, "chat-test", false, false).is_none());
    }
}
