use std::sync::OnceLock;

#[cfg(windows)]
fn uses_italian() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // LANGIDs carry the primary language in their low ten bits.
    unsafe { GetUserDefaultUILanguage() & 0x03ff == 0x10 }
}

#[cfg(target_os = "macos")]
fn uses_italian() -> bool {
    extern "C" {
        fn parlo_uses_italian() -> bool;
    }
    unsafe { parlo_uses_italian() }
}

#[cfg(not(any(windows, target_os = "macos")))]
fn uses_italian() -> bool {
    false
}

#[tauri::command]
pub fn ui_language() -> &'static str {
    static ITALIAN: OnceLock<bool> = OnceLock::new();
    if *ITALIAN.get_or_init(uses_italian) {
        "it"
    } else {
        "en"
    }
}

pub fn tr<'a>(italian: &'a str, english: &'a str) -> &'a str {
    if ui_language() == "it" {
        italian
    } else {
        english
    }
}
