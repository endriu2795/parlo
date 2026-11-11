//! Private, verified official Codex package with native helper processes.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};
#[derive(Deserialize)]
struct Manifest {
    version: String,
    base_url: String,
    platforms: HashMap<String, Artifact>,
}
#[derive(Deserialize)]
struct Artifact {
    archive: String,
    size: u64,
    sha256: String,
    binary: String,
}
fn manifest() -> Result<Manifest, String> {
    serde_json::from_str(include_str!("../../engine/codex-runtime.json")).map_err(|e| e.to_string())
}
fn platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("win-x64"),
        ("macos", "aarch64") => Ok("mac-arm64"),
        ("macos", "x86_64") => Ok("mac-x64"),
        _ => {
            Err("Installazione Codex disponibile su Windows x64 e Mac Intel/Apple Silicon.".into())
        }
    }
}
fn verified(path: &Path, a: &Artifact) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    if file.metadata().map(|m| m.len()).ok() != Some(a.size) {
        return false;
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hash.update(&buffer[..n]),
            Err(_) => return false,
        }
    }
    format!("{:x}", hash.finalize()) == a.sha256
}
pub fn installed(root: &Path) -> Option<PathBuf> {
    let m = manifest().ok()?;
    let a = m.platforms.get(platform().ok()?)?;
    let dir = root.join("codex-code").join(&m.version);
    let binary = dir.join(&a.binary);
    (binary.is_file()
        && fs::read_to_string(dir.join("presentio-verified"))
            .ok()?
            .trim()
            == a.sha256)
        .then_some(binary)
}
pub fn ensure(root: &Path, progress: impl Fn(&str)) -> Result<PathBuf, String> {
    if let Some(binary) = installed(root) {
        return Ok(binary);
    }
    let m = manifest()?;
    let p = platform()?;
    let a = &m.platforms[p];
    let parent = root.join("codex-code");
    fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let archive = parent.join(format!("{}-{p}.tar.gz", m.version));
    if !verified(&archive, a) {
        let partial = archive.with_extension("part");
        let result = (|| {
            progress(crate::locale::tr("Download di Codex…", "Downloading Codex…"));
            let client = reqwest::blocking::Client::builder()
                .https_only(true)
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(900))
                .user_agent("PARLO/0.1")
                .build()
                .map_err(|e| e.to_string())?;
            let mut response = client
                .get(format!("{}/{}", m.base_url, a.archive))
                .send()
                .and_then(|r| r.error_for_status())
                .map_err(|e| {
                    format!("Download Codex non riuscito. Controlla Internet e riprova. {e}")
                })?;
            let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
            let mut buffer = [0u8; 65536];
            let (mut count, mut last) = (0u64, 101u64);
            loop {
                let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                if count > a.size {
                    return Err("Dimensione download Codex inattesa.".into());
                }
                file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                let percent = count * 100 / a.size;
                if percent != last {
                    progress(&format!("{} {percent}%", crate::locale::tr("Download di Codex…", "Downloading Codex…")));
                    last = percent;
                }
            }
            file.sync_all().map_err(|e| e.to_string())?;
            drop(file);
            progress(crate::locale::tr("Verifica di Codex…", "Verifying Codex…"));
            if !verified(&partial, a) {
                return Err("Verifica di integrità di Codex non riuscita. Riprova.".into());
            }
            fs::rename(&partial, &archive).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&partial);
        }
        result?;
    }
    progress(crate::locale::tr("Preparazione di Codex…", "Preparing Codex…"));
    let staging = parent.join(format!(".install-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(
            fs::File::open(&archive).map_err(|e| e.to_string())?,
        ));
        let mut size = 0u64;
        for entry in tar.entries().map_err(|e| e.to_string())? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?;
            if path
                .components()
                .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
                || !(entry.header().entry_type().is_file() || entry.header().entry_type().is_dir())
            {
                return Err("Archivio Codex non sicuro.".into());
            }
            size += entry.size();
            if size > 2_000_000_000 {
                return Err("Archivio Codex troppo grande.".into());
            }
            if !entry.unpack_in(&staging).map_err(|e| e.to_string())? {
                return Err("Percorso archivio Codex non valido.".into());
            }
        }
        if !staging.join(&a.binary).is_file() {
            return Err("Binario Codex mancante nell’archivio.".into());
        }
        fs::write(staging.join("presentio-verified"), &a.sha256).map_err(|e| e.to_string())?;
        let dir = parent.join(&m.version);
        if dir.exists() {
            fs::rename(
                &dir,
                parent.join(format!(".previous-{}", uuid::Uuid::new_v4())),
            )
            .map_err(|e| e.to_string())?;
        }
        fs::rename(&staging, &dir).map_err(|e| e.to_string())?;
        Ok(dir.join(&a.binary))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platforms_are_pinned_and_unverified_install_is_not_reused() {
        let m = manifest().unwrap();
        assert_eq!(m.platforms.len(), 3);
        for a in m.platforms.values() {
            assert_eq!(a.sha256.len(), 64);
            assert!(a.size > 100_000_000);
            assert!(a.binary.starts_with("bin/"));
        }
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        assert!(installed(&root).is_none());
    }
}
