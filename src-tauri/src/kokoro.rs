//! Explicit, verified download of the shared Kokoro model and voices.
use crate::{model::engine_dir, LockExt};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::{Read, Write}, path::{Path, PathBuf}, sync::{Mutex, OnceLock}, time::Duration};
type Result<T> = std::result::Result<T, String>;
const FILES: [(&str, u64, &str); 2] = [
    ("kokoro-v1.0.onnx", 325505369, "beb0d1848dee9a49da392cc3df26958d46cfa35d321edf434f52949153f0df3a"),
    ("voices-v1.0.bin", 28214398, "bca610b8308e8d99f32e6fe4197e7ec01679264efed0cac9140fe9c29f1fbf7d"),
];
#[derive(Default, Clone, Serialize)]
struct Download { running: bool, downloaded: u64, total: u64, error: Option<String> }
static DOWNLOAD: OnceLock<Mutex<Download>> = OnceLock::new();
fn status() -> &'static Mutex<Download> { DOWNLOAD.get_or_init(|| Mutex::new(Download::default())) }
fn root() -> PathBuf { engine_dir().join("kokoro") }
pub fn known_voice(id: &str) -> bool { ["if_sara", "im_nicola"].contains(&id) }
fn ready() -> bool {
    let root = root();
    fs::read_to_string(root.join("verified.txt")).is_ok_and(|s| s == FILES.iter().map(|f| f.2).collect::<String>())
        && FILES.iter().all(|(name,size,_)| fs::metadata(root.join(name)).is_ok_and(|m| m.len()==*size))
}
pub fn paths() -> Result<(PathBuf, PathBuf)> {
    if !ready() { return Err("Scarica Kokoro da Impostazioni > Voce prima di generare l’audio.".into()); }
    Ok((root().join(FILES[0].0), root().join(FILES[1].0)))
}
pub fn voices() -> Value { json!([
    {"id":"if_sara","name":"Sara","locale":"it-IT","gender":"Female","ready":ready()},
    {"id":"im_nicola","name":"Nicola","locale":"it-IT","gender":"Male","ready":ready()}
]) }
fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" { return Err("Gestisci le voci dalla finestra principale.".into()); }
    Ok(())
}
#[tauri::command]
pub fn kokoro_status(window: tauri::WebviewWindow) -> Result<Value> {
    main_window(&window)?;
    Ok(json!({"voices":voices(),"ready":ready(),"download":status().guard().clone()}))
}
fn fetch(client: &reqwest::blocking::Client, root: &Path, name: &str, size: u64, expected: &str, offset: u64) -> Result<()> {
    let path = root.join(name); let partial = root.join(format!("{name}.part"));
    let result = (|| {
        let url = format!("https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.1/{name}");
        let mut response = client.get(url).send().and_then(|r| r.error_for_status()).map_err(|_| "Download Kokoro non raggiungibile. Verifica Internet e riprova.")?;
        let mut file=fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut hash=Sha256::new(); let mut buffer=[0u8;65536]; let mut written=0u64;
        loop { let n=response.read(&mut buffer).map_err(|_| "Download Kokoro interrotto. Riprova.")?; if n==0 {break;}
            written += n as u64; if written>size {return Err("Dimensione Kokoro non valida.".into());}
            hash.update(&buffer[..n]); file.write_all(&buffer[..n]).map_err(|e|e.to_string())?; status().guard().downloaded=offset+written;
        }
        file.sync_all().map_err(|e|e.to_string())?; drop(file);
        if written!=size || format!("{:x}",hash.finalize())!=expected {return Err("Integrità del modello Kokoro non valida. Riprova.".into());}
        if path.exists() {fs::remove_file(&path).map_err(|e|e.to_string())?;}
        fs::rename(&partial,&path).map_err(|e|e.to_string())
    })();
    if result.is_err() {let _=fs::remove_file(partial);} result
}
#[tauri::command]
pub fn kokoro_download(window: tauri::WebviewWindow) -> Result<()> {
    main_window(&window)?;
    let mut progress=status().guard(); if progress.running {return Err("Download Kokoro già in corso.".into());}
    *progress=Download {running:true,total:FILES.iter().map(|f|f.1).sum(),..Default::default()}; drop(progress);
    std::thread::spawn(|| {
        let result=(|| { let root=root(); fs::create_dir_all(&root).map_err(|e|e.to_string())?;
            let _=fs::remove_file(root.join("verified.txt"));
            let client=reqwest::blocking::Client::builder().https_only(true).connect_timeout(Duration::from_secs(30)).timeout(Duration::from_secs(900)).build().map_err(|e|e.to_string())?;
            let mut offset=0; for (name,size,hash) in FILES { fetch(&client,&root,name,size,hash,offset)?; offset+=size; }
            fs::write(root.join("verified.txt"),FILES.iter().map(|f|f.2).collect::<String>()).map_err(|e|e.to_string())
        })(); let mut progress=status().guard(); progress.running=false; progress.error=result.err();
    }); Ok(())
}
