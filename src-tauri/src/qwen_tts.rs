//! Optional heavy voice engine: explicit private setup and pinned local weights.
use crate::{model::engine_dir, LockExt, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::Duration,
};

pub const MODEL: &str = "qwen3-tts-1.7b-customvoice";
pub const LANGUAGES: &[&str] = &[
    "Italian",
    "English",
    "French",
    "German",
    "Spanish",
    "Portuguese",
    "Chinese",
    "Japanese",
    "Korean",
    "Russian",
];
#[derive(Deserialize)]
struct Catalog {
    repo: String,
    revision: String,
    license: String,
    files: Vec<ModelFile>,
    voices: Vec<Voice>,
}
#[derive(Deserialize)]
struct ModelFile {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Voice {
    id: String,
    name: String,
    gender: String,
    native_language: String,
}
#[derive(Clone, Default, Serialize)]
struct Setup {
    running: bool,
    device: String,
    stage: String,
    downloaded: u64,
    total: u64,
    error: Option<String>,
}
static SETUP: OnceLock<Mutex<Setup>> = OnceLock::new();
fn setup() -> &'static Mutex<Setup> {
    SETUP.get_or_init(|| Mutex::new(Setup::default()))
}
fn catalog() -> Result<Catalog> {
    serde_json::from_str(include_str!("../../engine/qwen-tts-model.json"))
        .map_err(|e| e.to_string())
}
pub fn root() -> PathBuf {
    engine_dir().join("qwen-tts")
}
pub fn supported() -> bool {
    cfg!(all(windows, target_arch = "x86_64"))
}
pub fn known_voice(name: &str) -> bool {
    catalog().is_ok_and(|c| c.voices.iter().any(|v| v.id == name))
}
pub fn revision() -> Result<String> {
    Ok(catalog()?.revision)
}
fn requirements_hash() -> String {
    format!(
        "{:x}",
        Sha256::digest(include_bytes!("../../engine/qwen-tts-requirements.txt"))
    )
}
fn python_at(root: &Path, device: &str) -> Option<PathBuf> {
    if !["cpu", "cuda"].contains(&device) {
        return None;
    }
    let marker: Value =
        serde_json::from_slice(&fs::read(root.join(format!("runtime-{device}.json"))).ok()?)
            .ok()?;
    let python = root.join(format!("venv-{device}/Scripts/python.exe"));
    (marker["version"] == 1
        && marker["requirements"] == requirements_hash()
        && marker["device"] == device
        && python.is_file())
    .then_some(python)
}
pub fn python(device: &str) -> Option<PathBuf> {
    python_at(&root(), device)
}
fn model_ready_at(root: &Path, catalog: &Catalog) -> bool {
    fs::read_to_string(root.join("model.verified")).is_ok_and(|s| s == catalog.revision)
        && catalog.files.iter().all(|f| {
            fs::metadata(root.join("model").join(&f.path)).is_ok_and(|m| m.len() == f.size)
        })
}
pub fn model_path() -> Result<PathBuf> {
    if !model_ready_at(&root(), &catalog()?) {
        return Err("Prepara Qwen3-TTS da Impostazioni > Voce prima di generare audio. Il modello non viene scaricato dalla chat.".into());
    }
    Ok(root().join("model"))
}
pub fn voices() -> Result<Value> {
    Ok(json!(catalog()?.voices.iter().map(|voice|json!({"id":voice.id,"name":voice.name,
        "gender":voice.gender,"locale":"","labels":{"language":"Multilingua, italiano incluso","accent":voice.native_language}})).collect::<Vec<_>>()))
}
fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}
#[derive(Clone, Serialize)]
struct Hardware {
    ram_bytes: Option<u64>,
    gpu: Option<String>,
    vram_bytes: Option<u64>,
}
fn hardware() -> Hardware {
    static HARDWARE: OnceLock<Hardware> = OnceLock::new();
    HARDWARE
        .get_or_init(|| {
            let mut ram_bytes = None;
            #[cfg(windows)]
            unsafe {
                use windows::Win32::System::SystemInformation::{
                    GlobalMemoryStatusEx, MEMORYSTATUSEX,
                };
                let mut memory = MEMORYSTATUSEX {
                    dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                    ..Default::default()
                };
                if GlobalMemoryStatusEx(&mut memory).is_ok() {
                    ram_bytes = Some(memory.ullTotalPhys);
                }
            }
            let mut command = Command::new("nvidia-smi");
            hidden(&mut command);
            let gpu = command
                .args([
                    "--query-gpu=name,memory.total",
                    "--format=csv,noheader,nounits",
                ])
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.lines().next().map(str::to_owned))
                .and_then(|s| {
                    s.rsplit_once(',').map(|(name, size)| {
                        (name.trim().to_owned(), size.trim().parse::<u64>().ok())
                    })
                });
            Hardware {
                ram_bytes,
                gpu: gpu.as_ref().map(|g| g.0.clone()),
                vram_bytes: gpu.and_then(|g| g.1).map(|m| m * 1024 * 1024),
            }
        })
        .clone()
}
fn device_available(device: &str, hardware: &Hardware) -> bool {
    supported()
        && matches!(device, "cpu" | "cuda")
        && hardware
            .ram_bytes
            .is_none_or(|ram| ram >= 15 * 1024 * 1024 * 1024)
        && (device == "cpu"
            || hardware
                .vram_bytes
                .is_some_and(|vram| vram >= 7680 * 1024 * 1024))
}
pub fn sync_helpers() -> Result<()> {
    let root = root();
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    for (name, bytes) in [
        (
            "speech_tools.py",
            include_bytes!("../../engine/speech_tools.py").as_slice(),
        ),
        (
            "document_tools.py",
            include_bytes!("../../engine/document_tools.py").as_slice(),
        ),
        (
            "qwen_speech.py",
            include_bytes!("../../engine/qwen_speech.py").as_slice(),
        ),
        (
            "qwen_setup.py",
            include_bytes!("../../engine/qwen_setup.py").as_slice(),
        ),
        (
            "requirements.txt",
            include_bytes!("../../engine/qwen-tts-requirements.txt").as_slice(),
        ),
    ] {
        fs::write(root.join(name), bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn require_main(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("Gestisci Qwen3-TTS dalle impostazioni della finestra principale.".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn qwen_tts_status(window: tauri::WebviewWindow) -> Result<Value> {
    require_main(&window)?;
    tauri::async_runtime::spawn_blocking(||{
        let catalog=catalog()?; let hardware=hardware();
        Ok(json!({"supported":supported(),"model_ready":model_ready_at(&root(),&catalog),"model_bytes":catalog.files.iter().map(|f|f.size).sum::<u64>(),
            "cpu_ready":python("cpu").is_some(),"cuda_ready":python("cuda").is_some(),
            "cpu_available":device_available("cpu",&hardware),"cuda_available":device_available("cuda",&hardware),
            "hardware":hardware,"voices":voices()?,"setup":setup().guard().clone()}))
    }).await.map_err(|e|e.to_string())?
}
fn verify(path: &Path, file: &ModelFile) -> bool {
    if !fs::metadata(path).is_ok_and(|m| m.len() == file.size) {
        return false;
    }
    let Ok(mut input) = fs::File::open(path) else {
        return false;
    };
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        match input.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hash.update(&buffer[..n]),
            Err(_) => return false,
        }
    }
    format!("{:x}", hash.finalize()) == file.sha256
}
fn download_model(root: &Path, catalog: &Catalog) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(30))
        .timeout(Duration::from_secs(7200))
        .build()
        .map_err(|e| e.to_string())?;
    let mut offset = 0;
    for file in &catalog.files {
        let relative = Path::new(&file.path);
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err("Percorso del modello non valido.".into());
        }
        let path = root.join("model").join(relative);
        fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        if verify(&path, file) {
            offset += file.size;
            setup().guard().downloaded = offset;
            continue;
        }
        let partial = path.with_extension("part");
        let mut written = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
        if written > file.size {
            written = 0;
        }
        if written < file.size {
            let mut request = client.get(format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                catalog.repo, catalog.revision, file.path
            ));
            if written > 0 {
                request = request.header(reqwest::header::RANGE, format!("bytes={written}-"));
            }
            let mut response=request.send().and_then(|r|r.error_for_status()).map_err(|_|"Download Qwen3-TTS interrotto. Puoi riprovare dalle impostazioni; i dati parziali sono conservati.".to_string())?;
            if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                let expected = format!("bytes {written}-");
                if !response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|v| v.starts_with(&expected))
                {
                    return Err(
                        "Il server ha restituito un intervallo del modello non valido.".into(),
                    );
                }
            } else {
                written = 0;
            }
            let mut output = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(written == 0)
                .append(written > 0)
                .open(&partial)
                .map_err(|e| e.to_string())?;
            let mut buffer = [0u8; 65536];
            loop {
                let n = response.read(&mut buffer).map_err(|_| {
                    "Download Qwen3-TTS interrotto: riprova dalle impostazioni.".to_string()
                })?;
                if n == 0 {
                    break;
                }
                written += n as u64;
                if written > file.size {
                    return Err("Dimensione del modello Qwen3-TTS non valida.".into());
                }
                output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                setup().guard().downloaded = offset + written;
            }
            output.sync_all().map_err(|e| e.to_string())?;
        }
        if !verify(&partial, file) {
            if written == file.size {
                let _ = fs::remove_file(&partial);
            }
            return Err("Verifica del modello Qwen3-TTS non riuscita. Riprova il download.".into());
        }
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        fs::rename(&partial, &path).map_err(|e| e.to_string())?;
        offset += file.size;
    }
    fs::write(
        root.join("MODEL-LICENSE.txt"),
        format!(
            "Qwen3-TTS 1.7B CustomVoice\nLicense: {}\nhttps://huggingface.co/{}/tree/{}\n\n{}",
            catalog.license, catalog.repo, catalog.revision,
            include_str!("../../engine/QWEN-MODEL-LICENSE.txt")
        ),
    )
    .map_err(|e| e.to_string())?;
    fs::write(root.join("model.verified"), &catalog.revision).map_err(|e| e.to_string())
}
fn runtime_error(code: &str) -> String {
    match code {
        "disk"=>"Per preparare Qwen3-TTS servono almeno 15 GB liberi nella cartella dei componenti.",
        "vram"=>"Qwen3-TTS richiede una GPU NVIDIA con almeno 8 GB di VRAM per questa modalità.",
        "cuda_unavailable"=>"CUDA non disponibile: verifica il driver NVIDIA oppure scegli esplicitamente CPU nelle impostazioni.",
        "dependencies"=>"Installazione dei componenti Qwen3-TTS non riuscita. Verifica Internet e spazio libero, poi riprova.",
        _=>"Il motore Qwen3-TTS non si avvia su questo computer. Verifica i requisiti e riprova la preparazione.",
    }.into()
}
#[tauri::command]
pub fn qwen_tts_prepare(
    window: tauri::WebviewWindow,
    state: tauri::State<crate::Shared>,
    device: String,
    repair: Option<bool>,
) -> Result<()> {
    require_main(&window)?;
    if state.status.guard().chat_busy {
        return Err("Attendi il termine della chat prima di preparare Qwen3-TTS.".into());
    }
    if !device_available(&device, &hardware()) {
        return Err("Qwen3-TTS richiede Windows x64 e almeno 16 GB di RAM; per CUDA serve una GPU NVIDIA con almeno 8 GB di VRAM. La CPU è una scelta separata e molto lenta.".into());
    }
    let catalog = catalog()?;
    let mut status = setup().guard();
    if status.running {
        return Err("Preparazione Qwen3-TTS già in corso.".into());
    }
    *status = Setup {
        running: true,
        device: device.clone(),
        stage: "Preparazione di Qwen3-TTS…".into(),
        total: catalog.files.iter().map(|f| f.size).sum(),
        ..Default::default()
    };
    drop(status);
    std::thread::spawn(move || {
        let _power = crate::power::ProcessingPower::new();
        let result = (|| {
            sync_helpers()?;
            let root = root();
            if repair.unwrap_or(false) || python_at(&root, &device).is_none() {
                let marker = root.join(format!("runtime-{device}.json"));
                if marker.exists() {
                    fs::remove_file(marker).map_err(|e| e.to_string())?;
                }
                let base = crate::python_runtime::ensure(&root.join("runtime"), None, |s| {
                    setup().guard().stage = s.into()
                })?;
                let mut command = Command::new(base);
                hidden(&mut command);
                command
                    .args(["-I", "-u"])
                    .arg(root.join("qwen_setup.py"))
                    .arg("--root")
                    .arg(&root)
                    .arg("--device")
                    .arg(&device)
                    .env_remove("PYTHONHOME")
                    .env_remove("PYTHONPATH")
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());
                if repair.unwrap_or(false) {
                    command.arg("--repair");
                }
                let mut child = command.spawn().map_err(|e| e.to_string())?;
                let output = child.stdout.take().unwrap();
                let child = crate::engine_child::EngineChild::new(child)?;
                let mut error = None;
                for line in BufReader::new(output).lines() {
                    let line = line.map_err(|e| e.to_string())?;
                    if let Ok(value) = serde_json::from_str::<Value>(&line) {
                        if let Some(stage) = value["stage"].as_str() {
                            setup().guard().stage = stage.into();
                        }
                        if let Some(code) = value["error"].as_str() {
                            error = Some(runtime_error(code));
                        }
                    }
                }
                if !child.success() {
                    return Err(error.unwrap_or_else(|| runtime_error("setup")));
                }
                if python_at(&root, &device).is_none() {
                    return Err(runtime_error("probe"));
                }
            }
            setup().guard().stage = "Download del modello Qwen3-TTS (circa 4,5 GB)…".into();
            download_model(&root, &catalog)
        })();
        let mut status = setup().guard();
        status.running = false;
        status.error = result.err();
        status.stage = if status.error.is_none() {
            "Qwen3-TTS pronto".into()
        } else {
            "Preparazione Qwen3-TTS interrotta".into()
        };
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qwen_catalog_is_pinned_and_runtime_is_never_the_document_environment() {
        let catalog = catalog().unwrap();
        assert_eq!(catalog.revision.len(), 40);
        assert_eq!(catalog.repo, "Qwen/Qwen3-TTS-12Hz-1.7B-CustomVoice");
        assert!(catalog.files.iter().map(|f| f.size).sum::<u64>() > 4_000_000_000);
        assert!(known_voice("ryan"));
        assert!(!known_voice("../../voice"));
        for file in catalog.files {
            assert_eq!(file.sha256.len(), 64);
            assert!(Path::new(&file.path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))));
        }
        let temporary = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("qwen-runtime-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(temporary.join("venv-cpu/Scripts")).unwrap();
        fs::write(temporary.join("venv-cpu/Scripts/python.exe"), b"synthetic").unwrap();
        assert!(python_at(&temporary, "cpu").is_none());
        fs::write(
            temporary.join("runtime-cpu.json"),
            json!({"version":1,"requirements":requirements_hash(),"device":"cpu"}).to_string(),
        )
        .unwrap();
        assert!(python_at(&temporary, "cpu").is_some());
        assert!(python_at(&temporary, "cuda").is_none());
        fs::remove_dir_all(temporary).unwrap();
    }
}
