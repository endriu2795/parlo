//! Public, pinned voice catalog and explicitly requested, verified downloads.
use crate::{model::engine_dir, LockExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};
type Result<T> = std::result::Result<T, String>;

#[derive(Deserialize)]
struct Catalog {
    revision: String,
    voices: Vec<Voice>,
}
#[derive(Deserialize)]
struct Voice {
    id: String,
    name: String,
    locale: String,
    gender: String,
    quality: String,
    license: String,
    path: String,
    size: u64,
    sha256: String,
    config_size: u64,
    config_sha256: String,
    source: String,
}
#[derive(Default, Clone, Serialize)]
struct Download {
    running: bool,
    voice: Option<String>,
    downloaded: u64,
    total: u64,
    error: Option<String>,
}
static DOWNLOAD: OnceLock<Mutex<Download>> = OnceLock::new();
fn download() -> &'static Mutex<Download> {
    DOWNLOAD.get_or_init(|| Mutex::new(Download::default()))
}
fn catalog() -> Result<Catalog> {
    serde_json::from_str(include_str!("../../engine/piper-voices.json")).map_err(|e| e.to_string())
}
pub fn known_voice(id: &str) -> bool {
    catalog().is_ok_and(|catalog| catalog.voices.iter().any(|voice| voice.id == id))
}
fn root() -> PathBuf {
    engine_dir().join("piper-voices")
}
fn ready_at(root: &Path, voice: &Voice) -> bool {
    let marker = fs::read(root.join(format!("{}.verified.json", voice.id)))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    marker.is_some_and(|marker| {
        marker["model"] == voice.sha256 && marker["config"] == voice.config_sha256
    }) && fs::metadata(root.join(format!("{}.onnx", voice.id))).is_ok_and(|m| m.len() == voice.size)
        && fs::metadata(root.join(format!("{}.onnx.json", voice.id)))
            .is_ok_and(|m| m.len() == voice.config_size)
}
pub fn voices() -> Result<Value> {
    Ok(json!(catalog()?.voices.iter().map(|voice|json!({"id":voice.id,"name":voice.name,"locale":voice.locale,
        "gender":voice.gender,"quality":voice.quality,"license":voice.license,"download_bytes":voice.size+voice.config_size,
        "ready":ready_at(&root(),voice)})).collect::<Vec<_>>()))
}
pub fn paths(id: &str) -> Result<(PathBuf, PathBuf)> {
    let catalog = catalog()?;
    let voice = catalog
        .voices
        .iter()
        .find(|voice| voice.id == id)
        .ok_or("Voce Piper non disponibile nel catalogo.")?;
    let root = root();
    if !ready_at(&root, voice) {
        return Err(
            "Scarica la voce Piper da Impostazioni > Voce prima di generare l’audio.".into(),
        );
    }
    Ok((
        root.join(format!("{id}.onnx")),
        root.join(format!("{id}.onnx.json")),
    ))
}
fn window_main(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("Gestisci le voci dalle impostazioni della finestra principale.".into());
    }
    Ok(())
}
#[tauri::command]
pub fn piper_status(window: tauri::WebviewWindow) -> Result<Value> {
    window_main(&window)?;
    Ok(json!({"voices":voices()?,"download":download().guard().clone()}))
}
fn verify(path: &Path, size: u64, expected: &str) -> Result<()> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() != size {
        return Err("Download Piper incompleto: dimensione non valida.".into());
    }
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut bytes = [0u8; 65536];
    loop {
        let count = file.read(&mut bytes).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&bytes[..count]);
    }
    if format!("{:x}", hash.finalize()) != expected {
        return Err("Integrità della voce Piper non valida. Riprova il download.".into());
    }
    Ok(())
}
fn fetch(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    size: u64,
    hash: &str,
    offset: u64,
) -> Result<()> {
    if verify(path, size, hash).is_ok() {
        download().guard().downloaded = offset + size;
        return Ok(());
    }
    let temporary = path.with_extension("part");
    let result = (|| {
        let mut response = client
            .get(url)
            .send()
            .map_err(|_| "Download Piper non raggiungibile. Verifica Internet e riprova.")?
            .error_for_status()
            .map_err(|_| "La voce Piper non è disponibile sul server. Riprova più tardi.")?;
        let mut file = fs::File::create(&temporary).map_err(|e| e.to_string())?;
        let mut written = 0u64;
        let mut buffer = [0u8; 65536];
        loop {
            let count = response
                .read(&mut buffer)
                .map_err(|_| "Download Piper interrotto. Puoi riprovare dalle impostazioni.")?;
            if count == 0 {
                break;
            }
            written += count as u64;
            if written > size {
                return Err("Dimensione della voce Piper non valida.".into());
            }
            file.write_all(&buffer[..count])
                .map_err(|e| e.to_string())?;
            download().guard().downloaded = offset + written;
        }
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        verify(&temporary, size, hash)?;
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[tauri::command]
pub fn piper_download(window: tauri::WebviewWindow, voice: String) -> Result<()> {
    window_main(&window)?;
    let catalog = catalog()?;
    let voice = catalog
        .voices
        .into_iter()
        .find(|item| item.id == voice)
        .ok_or("Voce Piper non disponibile.")?;
    let mut status = download().guard();
    if status.running {
        return Err("Download di una voce già in corso.".into());
    }
    *status = Download {
        running: true,
        voice: Some(voice.id.clone()),
        total: voice.size + voice.config_size,
        ..Default::default()
    };
    drop(status);
    std::thread::spawn(move || {
        let result = (|| {
            let root = root();
            fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            let client = reqwest::blocking::Client::builder()
                .https_only(true)
                .connect_timeout(Duration::from_secs(30))
                .timeout(Duration::from_secs(600))
                .build()
                .map_err(|e| e.to_string())?;
            let url = format!(
                "https://huggingface.co/rhasspy/piper-voices/resolve/{}/{}",
                catalog.revision, voice.path
            );
            fetch(
                &client,
                &url,
                &root.join(format!("{}.onnx", voice.id)),
                voice.size,
                &voice.sha256,
                0,
            )?;
            fetch(
                &client,
                &format!("{url}.json"),
                &root.join(format!("{}.onnx.json", voice.id)),
                voice.config_size,
                &voice.config_sha256,
                voice.size,
            )?;
            fs::write(root.join(format!("{}.LICENSE.txt",voice.id)),format!("{}\nLicense: {}\nSource: {}\nDataset: https://huggingface.co/datasets/committa/serena-synthetic-it-27h\n",voice.name,voice.license,voice.source)).map_err(|e|e.to_string())?;
            fs::write(
                root.join(format!("{}.verified.json", voice.id)),
                json!({"model":voice.sha256,"config":voice.config_sha256}).to_string(),
            )
            .map_err(|e| e.to_string())
        })();
        let mut status = download().guard();
        status.running = false;
        status.error = result.err();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_catalog_has_complete_hashes_and_rejects_external_paths() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.revision.len(), 40);
        assert!(known_voice("it_IT-serena-medium"));
        assert!(!known_voice("../../outside"));
        for voice in catalog.voices {
            assert_eq!(voice.sha256.len(), 64);
            assert_eq!(voice.config_sha256.len(), 64);
            assert!(voice.size > 0);
        }
    }
    #[test]
    fn incomplete_or_modified_voice_is_not_ready() {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        let root = parent.join(format!("piper-fixture-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let mut voice = catalog().unwrap().voices.remove(0);
        voice.size = 4;
        voice.config_size = 2;
        assert!(!ready_at(&root, &voice));
        fs::write(root.join(format!("{}.onnx", voice.id)), b"test").unwrap();
        fs::write(root.join(format!("{}.onnx.json", voice.id)), b"{}").unwrap();
        assert!(!ready_at(&root, &voice));
        fs::write(
            root.join(format!("{}.verified.json", voice.id)),
            json!({"model":voice.sha256,"config":voice.config_sha256}).to_string(),
        )
        .unwrap();
        assert!(ready_at(&root, &voice));
        let hash = format!("{:x}", Sha256::digest(b"test"));
        assert!(verify(&root.join(format!("{}.onnx", voice.id)), 4, &hash).is_ok());
        assert!(verify(&root.join(format!("{}.onnx", voice.id)), 4, &voice.sha256).is_err());
        assert_eq!(
            root.canonicalize().unwrap().parent(),
            Some(parent.canonicalize().unwrap().as_path())
        );
        fs::remove_dir_all(root).unwrap();
    }
}
