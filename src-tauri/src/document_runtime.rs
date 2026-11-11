//! Private document/media tools, installed independently of speech inference.
use crate::python_runtime::{self, PreparedKind};
use serde_json::{json, Value};
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};
use tauri::Manager;

fn root() -> PathBuf { crate::model::engine_dir().join("documents") }
fn platform() -> &'static str {
    if cfg!(windows) { "win-x64" } else if cfg!(target_arch = "aarch64") { "mac-arm64" } else { "mac-x64" }
}
fn helper(root: &Path) -> PathBuf { root.join("document_tools.py") }

pub(crate) fn prepare(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let root = root();
    let python = if let Some(python) = python_runtime::prepared_installed_for(&root, PreparedKind::Documents) {
        python
    } else {
        let relative = PathBuf::from("engine/runtime/documents").join(platform());
        let bundled = app.path().resource_dir().ok().map(|p| p.join(&relative))
            .filter(|p| p.join("manifest.json").is_file());
        let developer = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(&relative);
        let bundle = bundled.or_else(|| developer.join("manifest.json").is_file().then_some(developer))
            .ok_or("Gli strumenti per documenti e slideshow non sono inclusi in questa build. Serve il pacchetto aggiornato.")?;
        python_runtime::ensure_prepared_for(&root, &bundle, PreparedKind::Documents, |_| {})?
    };
    fs::write(helper(&root), include_bytes!("../../engine/document_tools.py")).map_err(|e| e.to_string())?;
    fs::write(root.join("speech_tools.py"), include_bytes!("../../engine/speech_tools.py")).map_err(|e| e.to_string())?;
    fs::write(root.join("PIPER-LICENSES.md"), include_bytes!("../../engine/PIPER-LICENSES.md")).map_err(|e| e.to_string())?;
    Ok(python)
}

fn probe(python: &Path, script: &Path) -> Result<Value, String> {
    let mut command = Command::new(python);
    command.args(["-I", "-X", "utf8"]).arg(script).arg("info")
        .env_remove("PYTHONHOME").env_remove("PYTHONPATH")
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let output = command.output().map_err(|e| e.to_string())?;
    if !output.status.success() { return Err("Gli strumenti per documenti non si avviano. Ripristina il pacchetto dell’app.".into()); }
    serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())
}

fn instructions(tools: &Value) -> String {
    format!("\n<strumenti_documenti>\n{}\n{}\n</strumenti_documenti>\n",
        json!({"tools": tools}), include_str!("../../engine/DOCUMENT_TOOLS.md"))
}

// Preview only: no runtime installation, Python process or file writes.
pub fn preview_context() -> String {
    instructions(&json!({"python":root().join("prepared-python"),"helper":helper(&root()),"ffmpeg":"ffmpeg"}))
}
pub fn context(app: &tauri::AppHandle) -> String {
    let result = prepare(app).and_then(|python| probe(&python, &helper(&root())));
    match result {
        Ok(tools) => instructions(&tools),
        Err(error) => format!("\n<strumenti_documenti>Gli strumenti privati di PARLO per documenti e media non sono disponibili: {}. Resta libera la scelta di altri strumenti disponibili nel contesto per realizzare il risultato richiesto.</strumenti_documenti>\n", json!(error)),
    }
}

/// Help subprocesses choose the same private interpreter; leave the system untouched.
pub fn configure(command: &mut Command) {
    let root = root();
    if let Some(python) = python_runtime::prepared_installed_for(&root, PreparedKind::Documents) {
        let mut paths = vec![python.parent().unwrap().to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") { paths.extend(std::env::split_paths(&existing)); }
        if let Ok(path) = std::env::join_paths(paths) { command.env("PATH", path); }
        command.env("PARLO_DOCUMENT_PYTHON", &python).env("PARLO_DOCUMENT_TOOLS", helper(&root));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tools_context_preserves_paths_with_spaces_and_explains_real_capabilities() {
        let tools = json!({"python": "C:\\test tools\\python.exe", "ffmpeg": "C:\\test tools\\ffmpeg.exe"});
        let context = instructions(&tools);
        assert!(context.contains(&serde_json::to_string(&tools["python"]).unwrap()));
        assert!(context.contains("MoviePy 2.x"));
        assert!(context.contains("python-pptx"));
        assert!(context.contains("Su PowerShell avvia"));
        assert!(context.contains("non richiede Office"));
        assert!(!context.contains("GUIDA.md"));
        assert!(!context.contains("Leggi la guida"));
    }
}
