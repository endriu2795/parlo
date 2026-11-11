use crate::LockExt;
use serde::Serialize;
use std::{io::{BufRead, BufReader}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::Mutex};
use tauri::Manager;

#[derive(Clone, Default, Serialize)]
pub struct Status { pub running: bool, pub message: String, pub error: Option<String>, pub log: Vec<String>, pub model: Option<String>, pub preview: bool }
#[derive(Default)]
pub struct Provision(pub Mutex<Status>);

fn quiet(command: &mut Command) {
    #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let _ = command;
}
fn isolated_python(binary: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(binary);
    // -I ignores PYTHONIOENCODING. Explicit UTF-8 is required for the pipes
    // read by Rust, including on Windows accounts with a legacy code page.
    command.args(["-I", "-X", "utf8", "-u"])
        .env_remove("PYTHONHOME").env_remove("PYTHONPATH");
    quiet(&mut command);
    command
}
fn read_log(reader: impl std::io::Read, provision: &Provision) {
    let mut reader = BufReader::new(reader);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                // Third-party native tools may still emit a local encoding.
                // Keep draining the pipe instead of closing it on those bytes.
                let line = String::from_utf8_lossy(&bytes);
                let line = line.trim_end_matches(['\r', '\n']);
                let mut status = provision.0.guard();
                if let Some(stage) = line.strip_prefix("PARLO_STAGE:") { status.message=stage.chars().take(300).collect(); }
                status.log.push(line.trim_start_matches("PARLO_STAGE:").chars().take(1000).collect());
                if status.log.len() > 40 { status.log.remove(0); }
            }
        }
    }
}
fn run_command(command: &mut Command, provision: &Provision) -> Result<(), String> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let supervisor = crate::engine_child::EngineChild::new(child)?;
    std::thread::scope(|scope| {
        scope.spawn(|| read_log(stdout, provision));
        scope.spawn(|| read_log(stderr, provision));
    });
    if supervisor.success() { return Ok(()); }
    let status = provision.0.guard();
    let detail = status.log.iter().rev().find(|line| line.contains("Error:") || line.contains("Exception:") || line.contains("Errore:"))
        .or_else(|| status.log.iter().rev().find(|line| !line.trim().is_empty()))
        .map(|line| line.chars().take(500).collect::<String>());
    Err(match detail {
        Some(detail) => format!("Preparazione del modello non riuscita: {detail}. I file già scaricati vengono conservati; puoi riprovare."),
        None => "Preparazione del modello non riuscita. I file già scaricati vengono conservati; puoi riprovare.".into(),
    })
}
pub fn python(settings: &crate::model::Settings) -> Option<PathBuf> {
    if let Some(binary) = crate::python_runtime::installed(&crate::model::engine_dir()) { return Some(binary); }
    // Health remains compatible with a working legacy engine. Setup always uses
    // the included runtime and never invokes PATH or the Windows Store aliases.
    for candidate in [&settings.python_bin] {
        let Ok(bin) = crate::process::executable(&candidate) else { continue; };
        let mut command = isolated_python(&bin);
        command.args(["-c", "import sys; assert (3,10) <= sys.version_info[:2] < (3,14); print(sys.executable)"]).stdin(Stdio::null());
        if let Ok(output) = command.output() { if output.status.success() {
            return Some(PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()));
        }}
    }
    None
}
/// Media decoding/capture needs the bundled Python tools, but no Whisper model.
pub fn media_python(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = crate::model::engine_dir();
    if let Some(python) = crate::python_runtime::prepared_installed(&root) { return Ok(python); }
    let platform = if cfg!(windows) { "win-x64" } else if cfg!(target_arch="aarch64") { "mac-arm64" } else { "mac-x64" };
    let relative = PathBuf::from("engine/runtime/prepared").join(platform);
    let developer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(&relative);
    let bundle = app.path().resource_dir().ok().map(|path| path.join(&relative))
        .filter(|path| path.join("manifest.json").is_file())
        .or_else(|| developer.join("manifest.json").is_file().then_some(developer))
        .ok_or("Gli strumenti per importazione e video non sono inclusi in questa build.")?;
    crate::python_runtime::ensure_prepared(&root, &bundle, |_| {})
}
fn sources(folder: &Path) -> Result<(), String> {
    for directory in ["scripts", "engine"] { std::fs::create_dir_all(folder.join(directory)).map_err(|e| e.to_string())?; }
    for (name, content) in [
        ("scripts/setup-engine.py", include_str!("../../scripts/setup-engine.py")),
        ("scripts/setup-accelerator.py", include_str!("../../scripts/setup-accelerator.py")),
        ("engine/requirements-lock.txt", include_str!("../../engine/requirements-lock.txt")),
        ("engine/worker.py", include_str!("../../engine/worker.py")),
        ("engine/acceleration.py", include_str!("../../engine/acceleration.py")),
        ("engine/cpp_live.py", include_str!("../../engine/cpp_live.py")),
        ("engine/transcript_quality.py", include_str!("../../engine/transcript_quality.py")),
        ("engine/calibrate.py", include_str!("../../engine/calibrate.py")),
        ("engine/model_download.py", include_str!("../../engine/model_download.py")),
    ] { std::fs::write(folder.join(name), content).map_err(|e| e.to_string())?; }
    Ok(())
}
pub fn run(app: &tauri::AppHandle, state: &crate::AppState, model: &str, preview: bool, settings: &crate::model::Settings) -> Result<(), String> {
    let folder = state.data_dir.join("provisioning");
    sources(&folder)?;
    let root = crate::model::engine_dir();
    let platform = if cfg!(windows) { "win-x64" } else if cfg!(target_arch="aarch64") { "mac-arm64" } else { "mac-x64" };
    let relative = PathBuf::from("engine/runtime/prepared").join(platform);
    let developer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(&relative);
    let bundle = app.path().resource_dir().ok().map(|p| p.join(&relative)).filter(|p| p.join("manifest.json").is_file())
        .or_else(|| developer.join("manifest.json").is_file().then_some(developer));
    let python = if let Some(bundle) = bundle {
        crate::python_runtime::ensure_prepared(&root, &bundle, |message| {
            state.provision.0.guard().message = message.into();
        })?
    } else if let Some(binary) = crate::python_runtime::prepared_installed(&root) { binary }
    else { return Err("Il pacchetto non contiene il motore pronto. Installa la versione aggiornata di PARLO.".into()); };
    // Commit only the runtime path; concurrent changes to other settings survive.
    {
        let _guard = state.operation.guard();
        let conn = state.db.guard();
        let mut latest = crate::db::get_settings(&conn)?;
        latest.python_bin = python.to_string_lossy().into_owned();
        crate::db::save_settings(&conn, &latest)?;
    }
    let mut command = isolated_python(python);
    command.arg(folder.join("scripts/setup-engine.py")).arg("--engine").arg(&root).args(["--model", model, "--compute-mode", if preview { &settings.live_compute_mode } else { &settings.compute_mode }, "--live-compute-mode", &settings.live_compute_mode])
        .env("CALLNOTES_ENGINE_DIR", &root).env("PYTHONIOENCODING", "utf-8").env("PARLO_UI_LANGUAGE", crate::locale::ui_language())
        .env("HF_HOME", root.join("downloads/huggingface")).env("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
        .env_remove("PYTHONHOME").env_remove("PYTHONPATH")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if preview { command.arg("--preview-only"); }
    else if settings.live_transcription {
        if let Some(id) = crate::model::bundled_model_id(&settings.live_whisper_model) { command.args(["--live-model", id]); }
    }
    let bundle_name = if cfg!(windows) { "win-x64-vulkan" } else if cfg!(target_arch="aarch64") { "mac-arm64-metal" } else { "mac-x64-metal" };
    // Development checkout, or the resources directory in a packaged app.
    let developer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../engine/runtime").join(bundle_name);
    let bundle = app.path().resource_dir().ok().map(|p| p.join("engine/runtime").join(bundle_name)).filter(|p| p.is_dir()).or_else(|| developer.is_dir().then_some(developer));
    if let Some(bundle) = bundle { command.env("PARLO_RUNTIME_BUNDLE", bundle); }
    run_command(&mut command, &state.provision)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_log_encoding_does_not_close_the_pipe_before_following_progress() {
        let provision = Provision::default();
        read_log(std::io::Cursor::new(b"Native: \x85\nPARLO_STAGE:Pronto\n"), &provision);
        let status = provision.0.guard();
        assert_eq!(status.message, "Pronto");
        assert_eq!(status.log.len(), 2);
    }
    #[test]
    #[ignore = "Requires the prepared private Python and installed public model"]
    fn private_python_unicode_progress_and_real_setup_complete_through_native_pipes() {
        let python = std::env::var_os("PARLO_TEST_PYTHON").expect("Set PARLO_TEST_PYTHON");
        let engine = PathBuf::from(std::env::var_os("PARLO_TEST_ENGINE").expect("Set PARLO_TEST_ENGINE"));
        let provision = Provision::default();
        let mut probe = isolated_python(&python);
        probe.env("PYTHONIOENCODING", "cp1252").args(["-c", "print('PARLO_STAGE:Préparation… ☃', flush=True); print('Hardware: è pronto', flush=True)"]);
        run_command(&mut probe, &provision).unwrap();
        assert_eq!(provision.0.guard().message, "Préparation… ☃");
        assert!(provision.0.guard().log.iter().all(|line| !line.contains('\u{fffd}')));
        let mut failing = isolated_python(&python);
        failing.args(["-c", "raise RuntimeError('modello di prova non disponibile')"]);
        assert!(run_command(&mut failing, &provision).unwrap_err().contains("RuntimeError: modello di prova non disponibile"));
        let folder = std::env::temp_dir().join(format!("parlo-setup-test-{}", uuid::Uuid::new_v4()));
        sources(&folder).unwrap();
        let mut setup = isolated_python(python);
        setup.arg(folder.join("scripts/setup-engine.py")).arg("--engine").arg(&engine).args(["--model", "turbo"])
            .env("CALLNOTES_ENGINE_DIR", &engine).env("HF_HUB_OFFLINE", "1").env("PYTHONIOENCODING", "utf-8");
        run_command(&mut setup, &provision).unwrap();
        assert_eq!(provision.0.guard().message, "Trascrizione locale pronta.");
        println!("Private Python Unicode progress and real model setup completed through native UTF-8 pipes.");
        assert!(folder.starts_with(std::env::temp_dir())); std::fs::remove_dir_all(folder).unwrap();
    }
}
