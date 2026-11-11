//! Panic report for testers: message, location and backtrace only, never call content.
use std::{io::Write, path::PathBuf, sync::OnceLock};

static LOG: OnceLock<PathBuf> = OnceLock::new();
const MAX_SIZE: u64 = 1024 * 1024;

pub fn install() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write(info);
        default(info);
    }));
}

/// Until the data folder is known, reports go to the system temporary folder.
pub fn set_folder(dir: &std::path::Path) {
    let _ = LOG.set(dir.join("crash.log"));
}

fn write(info: &std::panic::PanicHookInfo) {
    let path = LOG.get().cloned().unwrap_or_else(|| std::env::temp_dir().join("parlo-crash.log"));
    // Keep only the latest reports: a loop of panics must not fill the disk.
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_SIZE) {
        let _ = std::fs::remove_file(&path);
    }
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) else { return };
    let thread = std::thread::current();
    let _ = writeln!(
        file,
        "=== {} · versione {} · thread {}\n{info}\n{}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        env!("CARGO_PKG_VERSION"),
        thread.name().unwrap_or("senza nome"),
        std::backtrace::Backtrace::force_capture()
    );
}

pub fn message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "errore sconosciuto".into())
}
