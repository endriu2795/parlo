//! Official Antigravity ACP runtime. `gemini_bin` is retained for settings compatibility.
use crate::{model::Settings, LockExt};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::Duration,
};
static INSTALL: Mutex<()> = Mutex::new(());
pub const SUPPORTED: bool = false;
pub fn unavailable() -> String {
    "Il supporto Antigravity è stato rimosso. Scegli Claude Code o Codex nelle impostazioni.".into()
}
#[derive(Deserialize)]
struct Manifest {
    version: String,
    platforms: HashMap<String, Artifact>,
}
#[derive(Deserialize)]
struct Artifact {
    url: String,
    size: u64,
    sha256: String,
    binary: String,
    helper: String,
}
fn manifest() -> Result<Manifest, String> {
    serde_json::from_str(include_str!("../../engine/antigravity-runtime.json"))
        .map_err(|e| e.to_string())
}
fn platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("win-x64"),
        ("macos", "aarch64") => Ok("mac-arm64"),
        ("macos", "x86_64") => Ok("mac-x64"),
        _ => Err("Antigravity disponibile su Windows x64 e Mac Intel/Apple Silicon.".into()),
    }
}
pub fn installed(root: &Path) -> Option<PathBuf> {
    if !SUPPORTED { return None; }
    let m = manifest().ok()?;
    let a = m.platforms.get(platform().ok()?)?;
    let dir = root.join("antigravity-acp").join(&m.version);
    (dir.join(&a.binary).is_file()
        && dir.join(&a.helper).is_file()
        && fs::read_to_string(dir.join("presentio-verified"))
            .ok()?
            .trim()
            == a.sha256)
        .then(|| dir.join(&a.binary))
}
pub fn legacy(bin: &str) -> bool {
    let name = Path::new(bin)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    bin.trim().is_empty()
        || matches!(
            name.as_str(),
            "gemini" | "gemini.js" | "gemini.exe" | "gemini.cmd" | "gemini.bat"
        )
}
pub fn command(bin: &str) -> Result<Command, String> {
    if !SUPPORTED { return Err(unavailable()); }
    let path = if legacy(bin) {
        installed(&crate::model::engine_dir()).ok_or("Collega Antigravity da Impostazioni → Assistente. Il precedente accesso Gemini CLI non è più utilizzabile con account personali.")?
    } else {
        let path = Path::new(bin);
        if path.is_file() {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|e| e.to_string())?
                    .join(path)
            }
        } else {
            which::which(bin).map_err(|_| {
                "Antigravity ACP non trovato. Usa Installa e accedi dalle impostazioni.".to_string()
            })?
        }
    };
    if matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("cmd" | "bat" | "js" | "mjs")
    ) || matches!(
        path.file_stem().and_then(|s| s.to_str()),
        Some("agy" | "antigravity")
    ) {
        return Err("Serve il componente Antigravity ACP (agy_acp_server), selezionabile dalle impostazioni; il comando agy e Gemini CLI usano un protocollo diverso.".into());
    }
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    Ok(cmd)
}
pub fn ensure(
    root: &Path,
    settings: &Settings,
    progress: impl Fn(&str),
) -> Result<PathBuf, String> {
    if !SUPPORTED { return Err(unavailable()); }
    let _guard = INSTALL.guard();
    if let Some(path) = installed(root) {
        return Ok(path);
    }
    let m = manifest()?;
    let a = &m.platforms[platform()?];
    let python = match crate::provision::python(settings) {
        Some(p) => p,
        None => crate::python_runtime::ensure(root, None, &progress)?,
    };
    let parent = root.join("antigravity-acp");
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let staging = parent.join(format!(".install-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        progress("Download di Antigravity ACP…");
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(900))
            .build()
            .map_err(|e| e.to_string())?;
        let mut response = client
            .get(&a.url)
            .send()
            .and_then(|r| r.error_for_status())
            .map_err(|e| format!("Download Antigravity non riuscito: {e}"))?;
        let archive = staging.join("runtime.zip");
        let mut file = fs::File::create(&archive).map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        let mut count = 0u64;
        let mut last = 101;
        let mut buffer = [0u8; 65536];
        loop {
            let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > a.size {
                return Err("Dimensione download Antigravity inattesa.".into());
            }
            digest.update(&buffer[..n]);
            file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            let percent = count * 100 / a.size;
            if percent != last {
                progress(&format!("Download di Antigravity ACP… {percent}%"));
                last = percent;
            }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        if count != a.size || format!("{:x}", digest.finalize()) != a.sha256 {
            return Err("Verifica di integrità Antigravity non riuscita. Riprova.".into());
        }
        progress("Preparazione di Antigravity ACP…");
        let mut extractor = Command::new(python);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            extractor.creation_flags(0x08000000);
        }
        let output = extractor
            .args([
                "-I",
                "-X",
                "utf8",
                "-c",
                include_str!("../../scripts/extract-antigravity.py"),
            ])
            .arg(&staging)
            .arg(&a.binary)
            .arg(&a.helper)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success()
            || !staging.join(&a.binary).is_file()
            || !staging.join(&a.helper).is_file()
        {
            return Err("Estrazione Antigravity non riuscita. Riprova.".into());
        }
        fs::remove_file(archive).map_err(|e| e.to_string())?;
        fs::write(staging.join("presentio-verified"), &a.sha256).map_err(|e| e.to_string())?;
        let target = parent.join(&m.version);
        if target.exists() {
            fs::rename(
                &target,
                parent.join(format!(".old-{}", uuid::Uuid::new_v4())),
            )
            .map_err(|e| e.to_string())?;
        }
        fs::rename(&staging, &target).map_err(|e| e.to_string())?;
        Ok(target.join(&a.binary))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}
