//! Private, relocatable CPython. No Python, pip, shell or admin rights needed to bootstrap.
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Component, Path, PathBuf}, process::{Command, Stdio}, time::Duration};

#[derive(Deserialize)]
struct Manifest { version: String, release: String, artifacts: Vec<Artifact> }
#[derive(Deserialize)]
struct Artifact { platform: String, file: String, url: String, size: u64, sha256: String, executable: String }
fn manifest() -> Result<Manifest, String> {
    serde_json::from_str(include_str!("../../engine/python-runtime.json")).map_err(|e| e.to_string())
}
fn platform() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("win-x64"),
        ("macos", "aarch64") => Ok("mac-arm64"),
        ("macos", "x86_64") => Ok("mac-x64"),
        _ => Err("Il runtime incluso supporta Windows x64 e Mac Intel/Apple Silicon.".into()),
    }
}
pub fn prepared_binary(root: &Path) -> PathBuf {
    root.join("prepared-python/python").join(if cfg!(windows) { "python.exe" } else { "bin/python3" })
}
#[derive(Clone, Copy)]
pub enum PreparedKind { Engine, Documents }
impl PreparedKind {
    fn requirements(self) -> &'static [u8] {
        match self {
            Self::Engine => include_bytes!("../../engine/requirements-lock.txt"),
            Self::Documents => include_bytes!("../../engine/documents-requirements-lock.txt"),
        }
    }
    fn archive(self) -> &'static str {
        match self { Self::Engine => "engine-runtime.tar.gz", Self::Documents => "documents-runtime.tar.gz" }
    }
    fn version(self) -> u32 { match self { Self::Engine => 1, Self::Documents => 2 } }
    fn probe(self) -> &'static str {
        match self {
            Self::Engine => "import numpy, av, psutil, faster_whisper, ctranslate2, onnxruntime",
            Self::Documents => include_str!("../../engine/document_runtime_probe.py"),
        }
    }
}
fn prepared_identity_for(kind: PreparedKind) -> Result<String, String> {
    let manifest = manifest()?;
    let artifact = manifest.artifacts.iter().find(|a| a.platform == platform().unwrap_or("")).ok_or("Runtime non disponibile")?;
    Ok(prepared_identity_parts(kind, &artifact.sha256, &format!("{:x}", Sha256::digest(kind.requirements()))))
}
fn prepared_identity_parts(kind: PreparedKind, python: &str, requirements: &str) -> String {
    let base = format!("{python}:{requirements}");
    match kind { PreparedKind::Engine => base, PreparedKind::Documents => format!("{base}:{:x}", Sha256::digest(kind.probe().as_bytes())) }
}
#[cfg(test)]
fn prepared_identity() -> Result<String, String> { prepared_identity_for(PreparedKind::Engine) }
pub fn prepared_installed(root: &Path) -> Option<PathBuf> {
    prepared_installed_for(root, PreparedKind::Engine)
}
pub fn prepared_installed_for(root: &Path, kind: PreparedKind) -> Option<PathBuf> {
    let binary = prepared_binary(root);
    (binary.is_file() && fs::read_to_string(root.join("prepared-python/.presentio-prepared")).ok()?.trim() == prepared_identity_for(kind).ok()?).then_some(binary)
}
#[derive(Deserialize)]
struct PreparedManifest { version: u32, platform: String, python_sha256: String, requirements_sha256: String, #[serde(default)] validation_sha256: String, file: String, size: u64, sha256: String }
fn prepared_lock(root: &Path) -> Result<fs::File, String> {
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(false).read(true).write(true);
    #[cfg(windows)] { use std::os::windows::fs::OpenOptionsExt; options.share_mode(0); }
    let file = options.open(root.join(".prepared-runtime.lock")).map_err(|error| {
        if cfg!(windows) && matches!(error.raw_os_error(), Some(32) | Some(33)) {
            "Il motore è già in preparazione in un altro processo. Attendi e riprova.".to_string()
        } else { format!("Impossibile accedere alla cartella del motore: {error}") }
    })?;
    #[cfg(unix)] {
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Il motore è già in preparazione in un altro processo. Attendi e riprova.".into());
        }
    }
    Ok(file)
}
fn clean_prepared_staging(root: &Path) -> Result<(), String> {
    let canonical = root.canonicalize().map_err(|e| e.to_string())?;
    for item in fs::read_dir(root).map_err(|e| e.to_string())? {
        let item = item.map_err(|e| e.to_string())?;
        let name = item.file_name().to_string_lossy().into_owned();
        let Some(id) = name.strip_prefix(".prepared-") else { continue; };
        if uuid::Uuid::parse_str(id).is_err() || !item.file_type().map_err(|e| e.to_string())?.is_dir() { continue; }
        let path = item.path().canonicalize().map_err(|e| e.to_string())?;
        if path.parent() != Some(canonical.as_path()) { return Err("Cartella temporanea del motore non sicura.".into()); }
        fs::remove_dir_all(path).map_err(|e| format!("Impossibile riprendere la preparazione del motore: {e}"))?;
    }
    Ok(())
}
pub fn ensure_prepared(root: &Path, bundle: &Path, progress: impl Fn(&str)) -> Result<PathBuf, String> {
    ensure_prepared_for(root, bundle, PreparedKind::Engine, progress)
}
pub fn ensure_prepared_for(root: &Path, bundle: &Path, kind: PreparedKind, progress: impl Fn(&str)) -> Result<PathBuf, String> {
    if let Some(binary) = prepared_installed_for(root, kind) { return Ok(binary); }
    let prepared: PreparedManifest = serde_json::from_slice(&fs::read(bundle.join("manifest.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if prepared.version != kind.version() || prepared.platform != platform()? || prepared.file != kind.archive()
        || prepared_identity_parts(kind, &prepared.python_sha256, &prepared.requirements_sha256) != prepared_identity_for(kind)?
        || matches!(kind, PreparedKind::Documents) && prepared.validation_sha256 != format!("{:x}", Sha256::digest(kind.probe().as_bytes())) {
        return Err("Il motore incluso non corrisponde a questa versione di PARLO. Installa il pacchetto aggiornato.".into());
    }
    progress(match kind { PreparedKind::Engine => crate::locale::tr("Preparazione della trascrizione locale…", "Preparing local transcription…"), PreparedKind::Documents => crate::locale::tr("Preparazione degli strumenti per documenti e slideshow…", "Preparing document and slideshow tools…") });
    let archive = bundle.join(&prepared.file);
    let artifact = Artifact { platform: prepared.platform, file: prepared.file, url: String::new(), size: prepared.size, sha256: prepared.sha256, executable: String::new() };
    verify(&archive, &artifact)?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let _lock = prepared_lock(root)?;
    if let Some(binary) = prepared_installed_for(root, kind) { return Ok(binary); }
    clean_prepared_staging(root)?;
    let staging = root.join(format!(".prepared-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        extract(&archive, &staging)?;
        let binary = staging.join("python").join(if cfg!(windows) { "python.exe" } else { "bin/python3" });
        let mut check = Command::new(&binary);
        check.args(["-I", "-c", kind.probe()])
            .env_remove("PYTHONHOME").env_remove("PYTHONPATH").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        #[cfg(windows)] { use std::os::windows::process::CommandExt; check.creation_flags(0x08000000); }
        progress(crate::locale::tr("Verifica del motore incluso…", "Verifying the bundled engine…"));
        let mut child = check.spawn().map_err(|e| e.to_string())?;
        let mut output = Vec::new();
        let stdout = child.stdout.take().unwrap();
        let child = crate::engine_child::EngineChild::new(child)?;
        stdout.take(8192).read_to_end(&mut output).map_err(|e| e.to_string())?;
        if !child.success() {
            let missing = serde_json::from_slice::<serde_json::Value>(&output).ok()
                .and_then(|value|value["module"].as_str().map(str::to_owned))
                .filter(|name|name.len() < 64 && name.chars().all(|c|c.is_ascii_alphanumeric() || c=='_'));
            return Err(match missing {
                Some(module) => format!("Il pacchetto degli strumenti è incompleto o incompatibile: il modulo {module} non si avvia. Serve un installer con il runtime corretto."),
                None => "Il motore incluso non si avvia su questo computer. Riprova con il pacchetto aggiornato.".into(),
            });
        }
        fs::write(staging.join(".presentio-prepared"), prepared_identity_for(kind)?).map_err(|e| e.to_string())?;
        let destination = root.join("prepared-python");
        let backup = root.join(format!(".prepared-previous-{}", uuid::Uuid::new_v4()));
        if destination.exists() { fs::rename(&destination, &backup).map_err(|e| e.to_string())?; }
        if let Err(error) = fs::rename(&staging, &destination) {
            if backup.exists() { let _ = fs::rename(&backup, &destination); }
            return Err(error.to_string());
        }
        Ok(prepared_binary(root))
    })();
    if staging.exists() { let _ = fs::remove_dir_all(&staging); }
    result
}
fn location(root: &Path, manifest: &Manifest, artifact: &Artifact) -> PathBuf {
    root.join("python").join(format!("cpython-{}-{}-{}", manifest.version, manifest.release, artifact.platform))
}
fn probe(binary: &Path, version: &str) -> bool {
    if !binary.is_file() { return false; }
    let mut command = Command::new(binary);
    command.args(["-I", "-c", &format!("import sys, ssl, venv, ensurepip; assert sys.version.split()[0] == '{version}'")])
        .env_remove("PYTHONHOME").env_remove("PYTHONPATH")
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    command.status().is_ok_and(|s| s.success())
}
pub fn installed(root: &Path) -> Option<PathBuf> {
    if let Some(binary) = prepared_installed(root) { return Some(binary); }
    let manifest = manifest().ok()?;
    let artifact = manifest.artifacts.iter().find(|a| a.platform == platform().ok().unwrap_or(""))?;
    let base = location(root, &manifest, artifact);
    let binary = base.join(&artifact.executable);
    (fs::read_to_string(base.join(".presentio-runtime")).ok()?.trim() == artifact.sha256 && probe(&binary, &manifest.version)).then_some(binary)
}
fn verify(archive: &Path, artifact: &Artifact) -> Result<(), String> {
    let mut file = fs::File::open(archive).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != artifact.size {
        return Err("Download Python incompleto. Riprova l’installazione.".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop { let count = file.read(&mut buffer).map_err(|e| e.to_string())?; if count == 0 { break; } hash.update(&buffer[..count]); }
    if format!("{:x}", hash.finalize()) != artifact.sha256 {
        return Err("Verifica del runtime Python non riuscita: il file non corrisponde alla versione prevista. Riprova.".into());
    }
    Ok(())
}
// Symlinks within the Python tree are needed by the macOS distribution.
fn within_python(path: &Path) -> bool {
    let mut components = Vec::new();
    for c in path.components() {
        match c { Component::Normal(p) => components.push(p), Component::CurDir => {}, Component::ParentDir => { if components.pop().is_none() { return false; } }, _ => return false }
    }
    components.first().is_some_and(|p| *p == "python")
}
fn extract(archive: &Path, staging: &Path) -> Result<(), String> {
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(GzDecoder::new(file));
    let mut total = 0u64;
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if !within_python(&path) { return Err("Percorso non valido nell’archivio Python.".into()); }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) { return Err("Tipo di file non valido nell’archivio Python.".into()); }
        if let Some(link) = entry.link_name().map_err(|e| e.to_string())? {
            let target = if kind.is_symlink() { path.parent().unwrap_or(Path::new("")).join(link) } else { link.into_owned() };
            if !within_python(&target) { return Err("Collegamento esterno nell’archivio Python.".into()); }
        }
        total = total.checked_add(entry.size()).ok_or("Archivio Python troppo grande")?;
        if total > 1024 * 1024 * 1024 { return Err("Archivio Python troppo grande".into()); }
        if !entry.unpack_in(staging).map_err(|e| e.to_string())? { return Err("Percorso non valido nell’archivio Python.".into()); }
    }
    Ok(())
}
pub fn ensure(root: &Path, bundle: Option<&Path>, progress: impl Fn(&str)) -> Result<PathBuf, String> {
    if let Some(binary) = installed(root) { progress("Python incluso pronto."); return Ok(binary); }
    let manifest = manifest()?;
    let target = platform()?;
    let artifact = manifest.artifacts.iter().find(|a| a.platform == target).ok_or("Runtime Python assente per questa architettura")?;
    let downloads = root.join("downloads");
    fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let cached = downloads.join(&artifact.file);
    let bundled = bundle.map(|p| p.join(&artifact.file)).filter(|p| p.is_file());
    let archive = if let Some(bundled) = bundled {
        progress("Preparazione del Python incluso nell’app…");
        verify(&bundled, artifact)?;
        bundled
    } else {
        if verify(&cached, artifact).is_err() {
            let partial = downloads.join(format!("{}.{}.part", artifact.file, uuid::Uuid::new_v4()));
            let download = (|| -> Result<(), String> {
                progress("Download del Python incluso…");
                let client = reqwest::blocking::Client::builder().https_only(true).connect_timeout(Duration::from_secs(30)).timeout(Duration::from_secs(600))
                    .user_agent("PARLO/0.1").build().map_err(|e| e.to_string())?;
                let mut response = client.get(&artifact.url).send().and_then(|r| r.error_for_status()).map_err(|e| format!("Impossibile scaricare Python. Controlla la connessione e riprova. {e}"))?;
                let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
                let mut buffer = [0u8; 65536];
                let mut count = 0u64;
                let mut last_percent = 101;
                loop {
                    let n = response.read(&mut buffer).map_err(|e| e.to_string())?;
                    if n == 0 { break; }
                    count += n as u64;
                    if count > artifact.size { return Err("Dimensione del runtime Python inattesa.".into()); }
                    file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                    let percent = count * 100 / artifact.size;
                    if percent != last_percent { progress(&format!("Download del Python incluso… {percent}%")); last_percent = percent; }
                }
                file.sync_all().map_err(|e| e.to_string())?;
                drop(file);
                verify(&partial, artifact)?;
                if cached.exists() { fs::remove_file(&cached).map_err(|e| e.to_string())?; }
                fs::rename(&partial, &cached).map_err(|e| e.to_string())?;
                Ok(())
            })();
            if let Err(error) = download { let _ = fs::remove_file(&partial); return Err(error); }
        }
        cached
    };
    progress("Installazione del Python privato di PARLO…");
    let destination = location(root, &manifest, artifact);
    let parent = destination.parent().unwrap();
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let staging = parent.join(format!(".staging-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let result = (|| {
        extract(&archive, &staging)?;
        if !probe(&staging.join(&artifact.executable), &manifest.version) { return Err("Il Python incluso non si avvia su questo computer. Controlla il dettaglio e riprova.".into()); }
        fs::write(staging.join(".presentio-runtime"), &artifact.sha256).map_err(|e| e.to_string())?;
        // Keep damaged/old versions recoverable; never remove an installed runtime in place.
        if destination.exists() { fs::rename(&destination, parent.join(format!(".previous-{}", uuid::Uuid::new_v4()))).map_err(|e| e.to_string())?; }
        fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
        let binary = destination.join(&artifact.executable);
        if !probe(&binary, &manifest.version) { return Err("Verifica finale del Python incluso non riuscita.".into()); }
        Ok(binary)
    })();
    if staging.exists() { let _ = fs::remove_dir_all(&staging); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_environment_never_reuses_the_speech_environment() {
        let root = std::env::temp_dir().join(format!("parlo-document-runtime-{}", uuid::Uuid::new_v4()));
        let python = prepared_binary(&root);
        fs::create_dir_all(python.parent().unwrap()).unwrap();
        fs::write(&python, b"synthetic runtime marker; never executed").unwrap();
        let marker = root.join("prepared-python/.presentio-prepared");
        fs::write(&marker, prepared_identity().unwrap()).unwrap();
        assert!(prepared_installed(&root).is_some());
        assert!(prepared_installed_for(&root, PreparedKind::Documents).is_none());
        fs::write(&marker, prepared_identity_for(PreparedKind::Documents).unwrap()).unwrap();
        assert!(prepared_installed(&root).is_none());
        assert!(prepared_installed_for(&root, PreparedKind::Documents).is_some());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn old_document_runtime_marker_is_invalidated_even_with_matching_requirements() {
        let root = std::env::temp_dir().join(format!("parlo-document-marker-{}", uuid::Uuid::new_v4()));
        let python = prepared_binary(&root);
        fs::create_dir_all(python.parent().unwrap()).unwrap();
        fs::write(&python, b"synthetic marker, never executed").unwrap();
        let identity = prepared_identity_for(PreparedKind::Documents).unwrap();
        let old = identity.rsplit_once(':').unwrap().0;
        fs::write(root.join("prepared-python/.presentio-prepared"), old).unwrap();
        assert!(prepared_installed_for(&root, PreparedKind::Documents).is_none());
        assert_eq!(PreparedKind::Documents.version(), 2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_staging_is_cleaned_under_exclusive_lock_and_backups_survive() {
        let root = std::env::temp_dir().join(format!("parlo-stage-test-{}", uuid::Uuid::new_v4()));
        let stage = root.join(format!(".prepared-{}", uuid::Uuid::new_v4()));
        let backup = root.join(format!(".prepared-previous-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&stage).unwrap(); fs::write(stage.join("incomplete"), b"unfinished").unwrap();
        fs::create_dir_all(&backup).unwrap(); fs::write(backup.join("previous"), b"working").unwrap();
        fs::write(root.join("archive.sqlite"), b"archive sentinel").unwrap();
        let lock = prepared_lock(&root).unwrap();
        assert!(prepared_lock(&root).is_err());
        clean_prepared_staging(&root).unwrap();
        assert!(!stage.exists()); assert!(backup.join("previous").exists());
        assert_eq!(fs::read(root.join("archive.sqlite")).unwrap(), b"archive sentinel");
        drop(lock);
        assert!(prepared_lock(&root).is_ok());
        assert!(root.starts_with(std::env::temp_dir())); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn prepared_bundle_mismatch_never_changes_an_existing_engine() {
        let folder = std::env::temp_dir().join(format!("parlo-prepared-test-{}", uuid::Uuid::new_v4()));
        let bundle = folder.join("bundle");
        let root = folder.join("engine");
        fs::create_dir_all(&bundle).unwrap();
        fs::create_dir_all(root.join("prepared-python")).unwrap();
        fs::write(root.join("prepared-python/previous"), b"working runtime").unwrap();
        fs::write(bundle.join("manifest.json"), serde_json::json!({"version":1,"platform":platform().unwrap(),"python_sha256":"wrong","requirements_sha256":"wrong","file":"engine-runtime.tar.gz","size":1,"sha256":"wrong"}).to_string()).unwrap();
        assert!(ensure_prepared(&root, &bundle, |_| {}).is_err());
        assert_eq!(fs::read(root.join("prepared-python/previous")).unwrap(), b"working runtime");
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn archive_paths_and_links_stay_inside_python() {
        for valid in ["python/bin/python3", "python/bin/../lib/libpython.dylib", "python"] { assert!(within_python(Path::new(valid)), "{valid}"); }
        for invalid in ["/python/bin", "../python", "python/../../escape", "other/bin", "python/bin/../../escape"] { assert!(!within_python(Path::new(invalid)), "{invalid}"); }
    }
    #[test]
    fn runtime_manifest_is_pinned_for_all_supported_platforms() {
        let manifest = manifest().unwrap();
        assert_eq!(manifest.artifacts.len(), 3);
        for platform in ["win-x64", "mac-x64", "mac-arm64"] {
            let a = manifest.artifacts.iter().find(|a| a.platform == platform).unwrap();
            assert!(a.url.starts_with("https://github.com/astral-sh/python-build-standalone/releases/download/"));
            assert_eq!(a.sha256.len(), 64);
            assert!(a.size > 20_000_000 && a.size < 30_000_000);
            assert!(within_python(Path::new(&a.executable)));
        }
    }
    #[test]
    fn corrupted_download_is_rejected_before_execution() {
        let folder = std::env::temp_dir().join(format!("parlo-runtime-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&folder).unwrap();
        let path = folder.join("corrupt.gz");
        fs::write(&path, b"wrong payload").unwrap();
        let mut a = manifest().unwrap().artifacts.remove(0);
        assert!(verify(&path, &a).is_err());
        a.size = 13;
        assert!(verify(&path, &a).is_err());
        fs::remove_dir_all(folder).unwrap();
    }
}
