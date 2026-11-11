//! Optional Claude Code provisioning. Never installs globally or changes account configuration.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fs, io::{Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, time::Duration};

#[derive(Deserialize)]
struct Manifest { version: String, base_url: String, platforms: HashMap<String, Artifact> }
#[derive(Deserialize)]
struct Artifact { binary: String, size: u64, sha256: String }
fn manifest() -> Result<Manifest, String> {
    serde_json::from_str(include_str!("../../engine/claude-runtime.json")).map_err(|e| e.to_string())
}
fn platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("win32-x64"),
        ("macos", "aarch64") => Ok("darwin-arm64"),
        ("macos", "x86_64") => Ok("darwin-x64"),
        _ => Err("Installazione Claude supportata su Windows x64 e Mac Intel/Apple Silicon.".into()),
    }
}
fn target(root: &Path, manifest: &Manifest, artifact: &Artifact) -> PathBuf {
    root.join("claude-code").join(&manifest.version).join(&artifact.binary)
}
fn probe(path: &Path, version: &str) -> bool {
    let mut command = Command::new(path);
    command.arg("--version").env("DISABLE_AUTOUPDATER", "1").stdin(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    command.output().is_ok_and(|output| output.status.success() && String::from_utf8_lossy(&output.stdout).trim().split_whitespace().next() == Some(version))
}
pub fn installed(root: &Path) -> Option<PathBuf> {
    let m = manifest().ok()?;
    let a = m.platforms.get(platform().ok()?)?;
    let path = target(root, &m, a);
    // Marker is written only after SHA verification and successful execution.
    (path.is_file() && fs::read_to_string(path.with_extension("presentio-verified")).ok()?.trim() == a.sha256).then_some(path)
}
fn verify(path: &Path, a: &Artifact) -> Result<(), String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != a.size { return Err("Download Claude Code incompleto. Riprova.".into()); }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop { let n = file.read(&mut buffer).map_err(|e| e.to_string())?; if n == 0 { break; } hash.update(&buffer[..n]); }
    if format!("{:x}", hash.finalize()) != a.sha256 { return Err("Verifica di integrità di Claude Code non riuscita. Riprova il download.".into()); }
    Ok(())
}
pub fn ensure(root: &Path, progress: impl Fn(&str)) -> Result<PathBuf, String> {
    let m = manifest()?;
    let p = platform()?;
    let a = m.platforms.get(p).ok_or("Runtime Claude non disponibile")?;
    let path = target(root, &m, a);
    if let Some(existing) = installed(root) { if probe(&existing, &m.version) { return Ok(existing); } }
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let suffix = if cfg!(windows) { "part.exe" } else { "part" };
    let partial = path.parent().unwrap().join(format!(".claude-{}.{suffix}", uuid::Uuid::new_v4()));
    let result = (|| {
        progress(crate::locale::tr("Download di Claude Code…", "Downloading Claude Code…"));
        let client = reqwest::blocking::Client::builder().https_only(true).connect_timeout(Duration::from_secs(30)).timeout(Duration::from_secs(900))
            .user_agent("PARLO/0.1").build().map_err(|e| e.to_string())?;
        let url = format!("{}/{}/{}/{}", m.base_url, m.version, p, a.binary);
        let mut response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|e| format!("Impossibile scaricare Claude Code. Controlla la connessione e riprova. {e}"))?;
        let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut count = 0u64;
        let mut last = 101;
        let mut buffer = [0u8; 65536];
        loop {
            let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            count += n as u64;
            if count > a.size { return Err("Dimensione del download Claude Code inattesa.".into()); }
            file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            let percent = count * 100 / a.size;
            if percent != last { progress(&format!("{} {percent}%", crate::locale::tr("Download di Claude Code…", "Downloading Claude Code…"))); last = percent; }
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        progress(crate::locale::tr("Verifica di Claude Code…", "Verifying Claude Code…"));
        verify(&partial, a)?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(&partial, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?; }
        if !probe(&partial, &m.version) { return Err("Claude Code non si avvia su questo computer. Riprova o consulta il dettaglio dell’errore.".into()); }
        if path.exists() { fs::rename(&path, path.parent().unwrap().join(format!(".previous-{}", uuid::Uuid::new_v4()))).map_err(|e| e.to_string())?; }
        fs::rename(&partial, &path).map_err(|e| e.to_string())?;
        fs::write(path.with_extension("presentio-verified"), &a.sha256).map_err(|e| e.to_string())?;
        Ok(path)
    })();
    if result.is_err() { let _ = fs::remove_file(&partial); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_artifacts_are_pinned_for_windows_and_both_macs() {
        let m = manifest().unwrap();
        assert_eq!(m.platforms.len(), 3);
        assert_eq!(m.base_url, "https://downloads.claude.ai/claude-code-releases");
        for p in ["win32-x64", "darwin-x64", "darwin-arm64"] {
            let a = &m.platforms[p];
            assert_eq!(a.sha256.len(), 64);
            assert!(a.size > 200_000_000);
            assert_eq!(Path::new(&a.binary).components().count(), 1);
        }
    }
    #[test]
    fn unverified_binary_is_not_used_and_corruption_is_rejected() {
        let root = std::env::temp_dir().join(format!("parlo-claude-test-{}", uuid::Uuid::new_v4()));
        let m = manifest().unwrap();
        let a = &m.platforms[platform().unwrap()];
        let path = target(&root, &m, a);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"invalid").unwrap();
        assert!(installed(&root).is_none());
        assert!(verify(&path, a).is_err());
        let fake = Artifact {binary:a.binary.clone(), size:7, sha256:a.sha256.clone()};
        assert!(verify(&path, &fake).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
