//! Private stdio tool host. Credentials stay in the OS vault and native HTTP client.
use crate::{model::Settings, python_runtime::{self, PreparedKind}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use crate::LockExt;
use std::sync::{Mutex, OnceLock};
use std::{fs, io::{BufRead, BufReader, Read, Write}, path::{Component, Path, PathBuf}, process::{Command, Stdio}, time::Duration};

pub const SERVER: &str = "parlo_tts";
type Result<T> = std::result::Result<T, String>;
pub fn is_tool(name: &str) -> bool {
    matches!(name, "mcp__parlo_tts__generate_speech" | "mcp__parlo_tts__generate_speech_from_file" | "mcp__parlo_tts__list_voices" | "mcp__parlo_tts__list_models")
}

fn credential() -> Result<keyring::Entry> {
    if !cfg!(any(windows, target_os = "macos")) { return Err("Portachiavi di sistema non supportato.".into()); }
    // Preserve credentials already saved in the system vault.
    keyring::Entry::new("Presentio.Transcript.TTS", "elevenlabs-api-key")
        .map_err(|_| "Portachiavi di sistema non disponibile.".into())
}
fn api_key() -> Result<String> {
    credential()?.get_password().map_err(|_| "Inserisci la chiave ElevenLabs nelle impostazioni Voce.".into())
}
fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" { return Err("Gestisci la chiave dalla finestra principale.".into()); }
    Ok(())
}
#[tauri::command]
pub fn tts_key_status(window: tauri::WebviewWindow) -> Result<bool> {
    main_window(&window)?;
    match credential()?.get_password() {
        Ok(value) => Ok(!value.is_empty()),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err("Impossibile accedere al portachiavi di sistema.".into()),
    }
}
#[tauri::command]
pub fn tts_save_key(window: tauri::WebviewWindow, key: String) -> Result<()> {
    main_window(&window)?;
    let key = key.trim();
    if key.is_empty() || key.len() > 4096 || key.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("Chiave API non valida.".into());
    }
    credential()?.set_password(key).map_err(|_| "Impossibile salvare la chiave nel portachiavi di sistema.".into())
}
#[tauri::command]
pub fn tts_delete_key(window: tauri::WebviewWindow) -> Result<()> {
    main_window(&window)?;
    match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Impossibile rimuovere la chiave dal portachiavi di sistema.".into()),
    }
}

#[derive(Serialize, Deserialize)]
struct Config {
    directory: PathBuf,
    python: Option<PathBuf>,
    helper: PathBuf,
    provider: String,
    voice: String,
    model: String,
    #[serde(default, skip_serializing_if="String::is_empty")]
    qwen_device: String,
    #[serde(default, skip_serializing_if="String::is_empty")]
    qwen_language: String,
    #[serde(default, skip_serializing_if="Option::is_none")]
    custom: Option<crate::custom_endpoint::Endpoint>,
}
pub fn server_config(settings: &Settings, directory: &Path) -> Result<Option<Value>> {
    if !settings.tts_enabled { return Ok(None); }
    let root = crate::model::engine_dir().join("documents");
    if settings.tts_provider=="qwen" {crate::qwen_tts::sync_helpers()?;}
    let config = Config {
        directory: directory.to_path_buf(),
        python: if settings.tts_provider=="qwen" {crate::qwen_tts::python(&settings.tts_qwen_device)} else {python_runtime::prepared_installed_for(&root, PreparedKind::Documents)},
        helper: if settings.tts_provider=="qwen" {crate::qwen_tts::root().join("speech_tools.py")}else{root.join("speech_tools.py")},
        provider: settings.tts_provider.clone(),
        voice: match settings.tts_provider.as_str() { "custom" => settings.custom_tts.voice.clone(), "edge" => settings.tts_edge_voice.clone(), "piper" => settings.tts_piper_voice.clone(), "kokoro" => settings.tts_kokoro_voice.clone(), "qwen"=>settings.tts_qwen_voice.clone(), _ => settings.tts_eleven_voice.clone() },
        model: if settings.tts_provider == "custom" { settings.custom_tts.model.clone() } else if settings.tts_provider == "elevenlabs" { settings.tts_eleven_model.clone() } else if settings.tts_provider=="qwen" {crate::qwen_tts::MODEL.into()} else { settings.tts_provider.clone() },
        custom: (settings.tts_provider == "custom").then(|| settings.custom_tts.clone()),
        qwen_device:if settings.tts_provider=="qwen" {settings.tts_qwen_device.clone()} else {String::new()},
        qwen_language:if settings.tts_provider=="qwen" {settings.tts_qwen_language.clone()} else {String::new()},
    };
    Ok(Some(json!({"command":std::env::current_exe().map_err(|e|e.to_string())?,
        "args":["--tts-mcp",serde_json::to_string(&config).map_err(|e|e.to_string())?],
        "env":{"CALLNOTES_ENGINE_DIR":crate::model::engine_dir()}})))
}
fn provider_name(provider: &str) -> &str {
    match provider { "piper"=>"Piper", "kokoro"=>"Kokoro", "qwen"=>"Qwen3-TTS", "edge"=>"Edge", "elevenlabs"=>"ElevenLabs", "custom"=>"Endpoint personalizzato", _=>"servizio voce non disponibile" }
}
pub fn context(settings: &Settings) -> String {
    if !settings.tts_enabled { return "\n<voce>Il TTS integrato è disattivato. Per generare voce invita ad abilitarlo in Impostazioni > Voce. Non installare altri servizi o cercare credenziali.</voce>\n".into(); }
    let (voice, details) = match settings.tts_provider.as_str() {
        "piper" => (&settings.tts_piper_voice, "Piper è locale e funziona offline. Usa la voce preparata nelle impostazioni; se manca invita a scaricarla lì. Il modello resta caricato fra le scene. Restituisce MP3 e durata reale, senza tempi delle parole o SRT: non inventarli."),
        "kokoro" => (&settings.tts_kokoro_voice, "Kokoro è locale e funziona offline. Usa la voce italiana configurata nelle impostazioni e prepara il modello dalle impostazioni se manca. Restituisce MP3 e durata reale, senza tempi delle parole o SRT."),
        "qwen" => (&settings.tts_qwen_voice, "Qwen3-TTS 1.7B è locale, separato dal modello chat. Si prepara solo da Impostazioni > Voce. instruct specifica tono ed espressività; language sceglie la lingua. Usa il dispositivo configurato, senza ripiegare su altri dispositivi o servizi. CPU molto lenta. Restituisce MP3 e durata reale, senza tempi delle parole o SRT."),
        "custom" => (&settings.custom_tts.voice, "Invia il testo all’endpoint configurato. Restituisce MP3 con durata, senza allineamento."),
        "edge" => (&settings.tts_edge_voice, "Edge è online e gratuito, senza disponibilità o quote illimitate garantite. rate e pitch regolano velocità e tono. Restituisce MP3 e durata reale; tempi delle parole e SRT solo quando disponibili."),
        "elevenlabs" => (&settings.tts_eleven_voice, "ElevenLabs è online e consuma i crediti dell’account configurato. Il modello scelto nelle impostazioni è già applicato; voice_settings regola l’interpretazione. Le credenziali restano nel portachiavi: non chiederle o cercarle. Restituisce MP3 e durata reale; tempi delle parole e SRT solo quando disponibili."),
        _ => return "\n<voce>Servizio voce non disponibile: verifica Impostazioni > Voce.</voce>\n".into(),
    };
    let details = if settings.tts_provider=="elevenlabs" {format!("{} Modello attivo: {}.",details,json!(settings.tts_eleven_model))} else {details.to_owned()};
    format!("\n<voce>Configurazione ATTUALE: servizio {}, voce {}. {} Per la SOLA sintesi vocale usa i comandi MCP parlo_tts e i parametri esposti nel loro schema attuale. Per le altre richieste usa liberamente gli strumenti disponibili per file, shell e web, rispettando i consensi. Le indicazioni di servizi nella cronologia possono essere superate: questa configurazione è quella attiva. list_voices e list_models elencano solo le scelte del servizio attivo. generate_speech richiede text e output .mp3. Per un copione UTF-8 già salvato nella cartella della call usa generate_speech_from_file con text_file e output. Voce e motore configurati sono già applicati dal servizio. Riusa l’audio invariato. Non installare componenti né cambiare servizio. Non ripetere una chiamata fallita identica: correggi i parametri una volta oppure segnala l’errore; se il motore non è pronto chiedi di prepararlo nelle impostazioni. Non dichiarare un file creato finché lo strumento non conferma il risultato.</voce>\n", provider_name(&settings.tts_provider), json!(voice), details)
}

fn safe_path(root: &Path, name: &str) -> Result<PathBuf> {
    let path = Path::new(name);
    let canonical = root.canonicalize().map_err(|_| "Cartella della call non disponibile.")?;
    let regular = crate::sources::canonical_path(root)?;
    let relative = if path.is_absolute() { path.strip_prefix(root).or_else(|_|path.strip_prefix(&canonical)).or_else(|_|path.strip_prefix(&regular)).map_err(|_| "Usa file nella cartella della call.")? } else { path };
    if relative.as_os_str().is_empty() || !relative.components().all(|c| matches!(c, Component::Normal(_)))
        || name.contains(':') && !path.is_absolute() {
        return Err("Percorso non valido nella cartella della call.".into());
    }
    let mut full = canonical.clone();
    for part in relative.components() {
        // Windows streams, device names and directory links cannot escape this root.
        let value = part.as_os_str().to_string_lossy();
        if value.contains(':') || value.ends_with(['.', ' ']) { return Err("Nome di file non valido.".into()); }
        full.push(part);
        if let Ok(meta) = fs::symlink_metadata(&full) {
            #[cfg(windows)] {
                use std::os::windows::fs::MetadataExt;
                if meta.file_attributes() & 0x400 != 0 { return Err("I collegamenti non sono consentiti nei percorsi TTS.".into()); }
            }
            if meta.file_type().is_symlink() || !full.canonicalize().map_err(|_| "Percorso TTS non valido.")?.starts_with(&canonical) {
                return Err("I collegamenti non sono consentiti nei percorsi TTS.".into());
            }
        }
    }
    Ok(regular.join(relative))
}

fn python(config: &Config, request: &Value) -> Result<Value> {
    if ["piper","kokoro","qwen"].contains(&config.provider.as_str()) { return local_python(config,request); }
    let python = config.python.as_ref().filter(|p|p.is_file()).ok_or("TTS non incluso in questa build. Serve il pacchetto aggiornato con gli strumenti documenti e voce.")?;
    let mut command = Command::new(python);
    command.args(["-I", "-X", "utf8"]).arg(&config.helper)
        .env_remove("PYTHONHOME").env_remove("PYTHONPATH")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child = command.spawn().map_err(|_| "Impossibile avviare gli strumenti voce.")?;
    let input = serde_json::to_vec(request).map_err(|e|e.to_string())?;
    let write = child.stdin.take().unwrap().write_all(&input);
    if write.is_err() { let _ = child.kill(); let _ = child.wait(); return Err("Invio al motore vocale non riuscito.".into()); }
    let output = child.wait_with_output().map_err(|_| "Motore vocale interrotto.")?;
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|_| "Risposta del motore vocale non valida.")?;
    if !output.status.success() {
        return Err(format!("Sintesi vocale non riuscita ({}). Verifica Internet, voce e parametri; nessun tentativo automatico.", value["type"].as_str().unwrap_or("errore audio")));
    }
    Ok(value)
}

struct PiperWorker {
    child: crate::engine_child::EngineChild,
    input: std::process::ChildStdin,
    output: BufReader<std::process::ChildStdout>,
}
static PIPER_WORKER: OnceLock<Mutex<Option<PiperWorker>>> = OnceLock::new();
static QWEN_WORKER: OnceLock<Mutex<Option<PiperWorker>>> = OnceLock::new();
static KOKORO_WORKER: OnceLock<Mutex<Option<PiperWorker>>> = OnceLock::new();
fn local_python(config: &Config, request: &Value) -> Result<Value> {
    let _power = crate::power::ProcessingPower::new();
    let qwen=config.provider=="qwen";
    let python = config.python.as_ref().filter(|path|path.is_file()).ok_or(if qwen {"Prepara Qwen3-TTS per il dispositivo scelto in Impostazioni > Voce. La chat non installa componenti automaticamente."}else{"Piper non incluso in questa build. Serve il pacchetto aggiornato con la voce locale."})?;
    let mut worker = (if qwen {&QWEN_WORKER} else if config.provider=="kokoro" {&KOKORO_WORKER} else {&PIPER_WORKER}).get_or_init(||Mutex::new(None)).guard();
    if worker.as_ref().is_some_and(|worker|worker.child.exited()) { worker.take(); }
    if worker.is_none() {
        let mut command = Command::new(python);
        command.args(["-I","-X","utf8","-u"]).arg(&config.helper).arg("--server")
            .env_remove("PYTHONHOME").env_remove("PYTHONPATH")
            .env("OMP_NUM_THREADS","2").env("OPENBLAS_NUM_THREADS","2")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        if qwen {command.env("HF_HUB_OFFLINE","1").env("TRANSFORMERS_OFFLINE","1").env("HF_HUB_DISABLE_TELEMETRY","1").env("DO_NOT_TRACK","1");}
        #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
        #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
        let mut child = command.spawn().map_err(|_|"Impossibile avviare il motore voce locale.")?;
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        *worker = Some(PiperWorker { child:crate::engine_child::EngineChild::new(child)?,input,output });
    }
    let result = (|| {
        let worker = worker.as_mut().unwrap();
        serde_json::to_writer(&mut worker.input,request).map_err(|_|"Invio al motore voce locale non riuscito.")?;
        worker.input.write_all(b"\n").and_then(|_|worker.input.flush()).map_err(|_|"Motore voce locale interrotto.")?;
        let mut line=String::new();
        worker.output.by_ref().take(1024*1024).read_line(&mut line).map_err(|_|"Risposta del motore voce locale interrotta.")?;
        if !line.ends_with('\n') { return Err("Motore voce locale interrotto. Puoi riprovare; i file precedenti sono conservati.".into()); }
        let value:Value=serde_json::from_str(&line).map_err(|_|"Risposta del motore voce locale non valida.")?;
        if value.get("error").is_some() {
            if config.provider=="kokoro" { return Err(if value["type"]=="ModuleNotFoundError" || value["type"]=="ImportError" {"Kokoro non incluso nel runtime di questa build: serve il pacchetto aggiornato."} else {"Generazione Kokoro non riuscita. Verifica il modello scaricato; i file precedenti sono conservati."}.into()); }
            if qwen {
                return Err(if value["type"]=="OutOfMemoryError" {"Memoria insufficiente per Qwen3-TTS. Chiudi altri modelli o scegli un dispositivo con più memoria; non viene cambiato motore automaticamente."}
                    else if value["error"]=="cuda_unavailable" || value["error"]=="vram" {"GPU CUDA non disponibile o memoria insufficiente. Verifica il driver NVIDIA e il dispositivo scelto in Impostazioni > Voce."}
                    else if value["type"]=="ModuleNotFoundError" || value["type"]=="ImportError" {"Componenti Qwen3-TTS mancanti. Ripeti la preparazione da Impostazioni > Voce."}
                    else {"Sintesi Qwen3-TTS non riuscita. Verifica i requisiti e la preparazione del modello; i file precedenti sono conservati."}.into());
            }
            return Err(if value["type"]=="ModuleNotFoundError" || value["type"]=="ImportError" {
                "Piper non incluso nel runtime di questa build: serve il pacchetto aggiornato.".into()
            } else { "Generazione Piper non riuscita. Verifica la voce scaricata; i file precedenti sono conservati.".into() });
        }
        Ok(value)
    })();
    if result.is_err() { worker.take(); }
    result
}

#[tauri::command]
pub async fn tts_catalog(app: tauri::AppHandle, window: tauri::WebviewWindow, provider: String) -> Result<Value> {
    main_window(&window)?;
    tauri::async_runtime::spawn_blocking(move|| {
        if provider == "piper" { return Ok(json!({"voices":crate::piper::voices()?,"models":[]})); }
        if !["edge","elevenlabs"].contains(&provider.as_str()) { return Err("Servizio voce non disponibile.".into()); }
        let root=crate::model::engine_dir().join("documents");
        let python=if provider=="edge" {Some(crate::document_runtime::prepare(&app)?)}else{None};
        let config=Config{directory:root.clone(),python,helper:root.join("speech_tools.py"),provider,custom:None,
            voice:String::new(),model:String::new(),qwen_device:String::new(),qwen_language:String::new()};
        let voices=call(&config,"list_voices",json!({"language":""}))?;
        let models=call(&config,"list_models",json!({}))?;
        Ok(json!({"voices":if voices.is_array(){voices}else{voices["voices"].clone()},"models":models}))
    }).await.map_err(|_|"Lettura del catalogo voce interrotta.".to_string())?
}

fn eleven(path: &str, body: Option<Value>) -> Result<Value> {
    // Fixed host, no redirects, no credentials in URLs, subprocess args, logs or error bodies.
    let client = reqwest::blocking::Client::builder().redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(30)).timeout(Duration::from_secs(if body.is_some(){600}else{30}))
        .build().map_err(|_| "Client ElevenLabs non disponibile.")?;
    let mut key = reqwest::header::HeaderValue::from_str(&api_key()?).map_err(|_| "Chiave ElevenLabs non valida.")?;
    key.set_sensitive(true);
    let url = format!("https://api.elevenlabs.io{path}");
    let request = match body { Some(body) => client.post(url).header("Content-Type", "application/json").body(body.to_string()), None => client.get(url) };
    let response = request.header("xi-api-key", key).send().map_err(|_| "ElevenLabs non raggiungibile. Verifica Internet; la richiesta non viene ripetuta automaticamente.")?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => "ElevenLabs: chiave o permessi insufficienti. Verifica l’account e l’accesso a voce e modello.".into(),
            429 => "ElevenLabs: quota o concorrenza del piano raggiunta. Riprova quando disponibile.".into(),
            400 | 422 => "ElevenLabs: testo o parametri non accettati. Controlla voce e modello; per testi lunghi suddividi in scene senza troncare.".into(),
            status => format!("ElevenLabs ha risposto HTTP {status}. Nessun tentativo automatico."),
        });
    }
    let bytes = response.bytes().map_err(|_| "Risposta ElevenLabs interrotta. La richiesta potrebbe essere già conteggiata; nessun tentativo automatico.")?;
    serde_json::from_slice(&bytes).map_err(|_| "Risposta ElevenLabs non valida; nessun tentativo automatico.".into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Generate {
    text: Option<String>, text_file: Option<String>, output: String,
    voice: Option<String>, model: Option<String>, rate: Option<String>, pitch: Option<String>,
    voice_settings: Option<Value>, previous_text: Option<String>, next_text: Option<String>,
    instruct:Option<String>,language:Option<String>,
    #[serde(default)] force_regenerate: bool,
}
fn identifier(value: &str) -> bool { !value.is_empty() && value.len() <= 200 && value.chars().all(|c|c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.')) }

fn output_lock(root: &Path, output: &Path) -> Result<fs::File> {
    let identity = output.to_string_lossy().to_lowercase();
    let name = format!(".work/tts-locks/{:x}.lock", Sha256::digest(identity.as_bytes()));
    let path = safe_path(root, &name)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|_| "Cartella temporanea voce non disponibile.")?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(windows)] { use std::os::windows::fs::OpenOptionsExt; options.share_mode(0); }
    let file = options.open(path).map_err(|_| "Questo audio è già in generazione. Attendi il risultato prima di rigenerarlo.".to_string())?;
    #[cfg(unix)] {
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Questo audio è già in generazione. Attendi il risultato prima di rigenerarlo.".into());
        }
    }
    Ok(file)
}

fn audio_output(root: &Path, name: &str) -> Result<PathBuf> {
    let output=safe_path(root,name)?;
    if name.ends_with(['/', '\\']) {return Err("Specifica un nome di file audio, ad esempio prova.mp3.".into());}
    match output.extension().and_then(|value|value.to_str()) {
        None => safe_path(root,output.with_extension("mp3").to_str().ok_or("Percorso audio non valido.")?),
        Some(extension) if extension.eq_ignore_ascii_case("mp3") => Ok(output),
        _=>Err("Usa un file MP3, ad esempio prova.mp3.".into()),
    }
}
fn generate(config: &Config, args: Value) -> Result<Value> {
    let args: Generate = serde_json::from_value(args).map_err(|_| "Parametri voce non validi.")?;
    let text = match (args.text, args.text_file) {
        (Some(text), None) => text,
        (None, Some(path)) => fs::read_to_string(safe_path(&config.directory, &path)?).map_err(|_| "Copione non leggibile come UTF-8.")?,
        _ => return Err("Specifica text oppure text_file, uno solo.".into()),
    };
    if text.trim().is_empty() { return Err("Il testo della voce è vuoto.".into()); }
    let output = audio_output(&config.directory, &args.output)?;
    let _lock = output_lock(&config.directory, &output)?;
    let timings = output.with_extension("speech.json");
    let subtitles = output.with_extension("srt");
    // Validate sidecars too: an existing link must not redirect a write outside the call.
    for path in [&timings, &subtitles] { safe_path(&config.directory, path.to_str().ok_or("Percorso non UTF-8.")?)?; }
    let voice = args.voice.unwrap_or_else(||config.voice.clone());
    let model = if config.provider == "custom" { config.model.clone() } else if config.provider == "elevenlabs" { args.model.clone().unwrap_or_else(||config.model.clone()) } else if config.provider=="qwen" {crate::qwen_tts::MODEL.into()} else { config.provider.clone() };
    if config.provider != "custom" && !identifier(&voice) { return Err("Scegli una voce nelle impostazioni oppure usa list_voices.".into()); }
    if config.provider != "custom" && !identifier(&model) { return Err("Identificativo modello non valido.".into()); }
    if config.provider != "elevenlabs" && args.model.as_ref().is_some_and(|requested| requested != &model) {
        return Err(format!("Modello non disponibile per {}. Ometti model per usare il motore attivo.",provider_name(&config.provider)));
    }
    let mut request = json!({"provider":config.provider,"voice":voice,"model":model,"text":text});
    if config.provider!="qwen" && (args.instruct.is_some() || args.language.is_some()) {return Err(format!("Questi parametri non sono disponibili per {}. Usa solo lo schema del servizio attivo.",provider_name(&config.provider)));}
    if config.provider == "edge" {
        if args.voice_settings.is_some() || args.previous_text.is_some() || args.next_text.is_some() {
            return Err("Questi parametri non sono disponibili per Edge. Usa solo quelli esposti nello schema attivo.".into());
        }
        request["rate"] = json!(args.rate.unwrap_or_else(||"+0%".into()));
        request["pitch"] = json!(args.pitch.unwrap_or_else(||"+0Hz".into()));
    } else if config.provider == "elevenlabs" {
        if args.rate.is_some() || args.pitch.is_some() { return Err("Per regolare la velocità usa voice_settings.speed.".into()); }
        if let Some(value) = args.voice_settings { request["voice_settings"] = value; }
        if let Some(value) = args.previous_text { request["previous_text"] = json!(value); }
        if let Some(value) = args.next_text { request["next_text"] = json!(value); }
    } else if config.provider == "piper" {
        if !crate::piper::known_voice(&voice) { return Err("Scegli una voce Piper dal catalogo delle impostazioni.".into()); }
        if voice != config.voice {return Err("La voce Piper è già configurata nelle impostazioni. Ometti voice per usare quella attiva.".into());}
        if args.rate.is_some() || args.pitch.is_some() || args.voice_settings.is_some() || args.previous_text.is_some() || args.next_text.is_some() {
            return Err("Questi parametri non si applicano a Piper locale. Usa solo quelli esposti nello schema attivo.".into());
        }
    } else if config.provider=="kokoro" {
        if !crate::kokoro::known_voice(&voice) || voice != config.voice {return Err("Scegli una voce Kokoro nelle impostazioni e ometti voice.".into());}
        if args.rate.is_some() || args.pitch.is_some() || args.voice_settings.is_some() || args.previous_text.is_some() || args.next_text.is_some() {return Err("Questi parametri non sono disponibili per Kokoro.".into());}
    } else if config.provider=="qwen" {
        if !crate::qwen_tts::known_voice(&voice) {return Err("Scegli una voce Qwen3-TTS dalle impostazioni.".into());}
        if args.model.as_ref().is_some_and(|m|m!=crate::qwen_tts::MODEL) || args.rate.is_some() || args.pitch.is_some() || args.voice_settings.is_some() || args.previous_text.is_some() || args.next_text.is_some() {
            return Err("Questi parametri non sono applicabili al motore Qwen3-TTS attivo.".into());
        }
        let language=args.language.unwrap_or_else(||config.qwen_language.clone());
        if !crate::qwen_tts::LANGUAGES.contains(&language.as_str()) {return Err("Lingua Qwen3-TTS non supportata.".into());}
        let instruct=args.instruct.unwrap_or_default();
        if instruct.chars().count()>1000 {return Err("Indicazioni vocali troppo lunghe: massimo 1000 caratteri.".into());}
        request["qwen_language"]=json!(language);request["instruct"]=json!(instruct);
        request["qwen_revision"]=json!(crate::qwen_tts::revision()?);
    } else if let Some(endpoint) = &config.custom {
        if voice != config.voice || args.rate.is_some() || args.pitch.is_some() || args.voice_settings.is_some() || args.previous_text.is_some() || args.next_text.is_some() {
            return Err("Usa la voce configurata per l’endpoint.".into());
        }
        request["endpoint"] = json!(endpoint.url("audio/speech")?);
    } else { return Err("Fornitore voce non supportato.".into()); }
    let hash = format!("{:x}", Sha256::digest(serde_json::to_vec(&request).map_err(|e|e.to_string())?));
    if !args.force_regenerate {
        if let Ok(bytes) = fs::read(&timings) {
            if let Ok(saved) = serde_json::from_slice::<Value>(&bytes) {
                let has_words = saved["words"].as_array().is_some_and(|w|!w.is_empty());
                if saved["request_hash"] == hash && (!has_words || subtitles.is_file()) {
                    if let Ok(audio) = fs::read(&output) {
                        if saved["audio_sha256"] == format!("{:x}", Sha256::digest(&audio)) && saved["duration_seconds"].as_f64().is_some_and(|d|d.is_finite() && d>0.0) {
                            return Ok(json!({"audio":output,"timings":timings,"subtitles":if has_words {json!(subtitles)} else {Value::Null},
                                "duration_seconds":saved["duration_seconds"],"word_timings_available":has_words,
                                "warnings":saved.get("warnings").cloned().unwrap_or(json!([])),"cached":true}));
                        }
                    }
                }
            }
        }
    }
    let owned = fs::read(&timings).ok().and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|m|m["request_hash"].as_str().is_some_and(|s|s.len()==64)
            && m["audio_sha256"].as_str().is_some_and(|s|s.len()==64) && ["edge","piper","kokoro","qwen","elevenlabs","custom"].iter().any(|p|m["provider"]==*p));
    if (output.exists() || timings.exists() || subtitles.exists()) && !owned {
        return Err("Esistono già file con questo nome non riconosciuti come output TTS: scegli un altro nome.".into());
    }
    // Fail before a paid request when this package cannot finalize the audio.
    if config.python.as_ref().is_none_or(|p|!p.is_file()) || !config.helper.is_file() {
        if config.provider=="qwen" {return Err("Prepara Qwen3-TTS da Impostazioni > Voce per il dispositivo scelto. Nessun download viene avviato dalla chat.".into());}
        return Err("Strumenti voce mancanti: serve il pacchetto aggiornato.".into());
    }
    fs::create_dir_all(output.parent().unwrap()).map_err(|_| "Impossibile creare la cartella audio.")?;
    if config.provider == "elevenlabs" {
        let mut body = json!({"text":text,"model_id":model});
        for key in ["voice_settings", "previous_text", "next_text"] { if let Some(value) = request.get(key) { body[key] = value.clone(); } }
        request["response"] = eleven(&format!("/v1/text-to-speech/{voice}/with-timestamps?output_format=mp3_44100_128"), Some(body))?;
    }
    if let Some(endpoint) = &config.custom {
        use base64::Engine;
        let audio = crate::custom_endpoint::speech(endpoint, &crate::custom_endpoint::key("tts", endpoint)?, &text, &voice)?;
        request["response"] = json!({"audio_base64":base64::engine::general_purpose::STANDARD.encode(audio)});
    }
    request["output"] = json!(output);
    request["request_hash"] = json!(hash);
    if config.provider == "piper" {
        let (model,voice_config)=crate::piper::paths(&voice)?;
        request["piper_model"]=json!(model);
        request["piper_config"]=json!(voice_config);
    }
    if config.provider == "kokoro" {let (model, voices)=crate::kokoro::paths()?; request["kokoro_model"]=json!(model); request["kokoro_voices"]=json!(voices);}
    if config.provider=="qwen" {request["qwen_model"]=json!(crate::qwen_tts::model_path()?);request["qwen_device"]=json!(config.qwen_device);}
    python(config, &request)
}

fn call(config: &Config, name: &str, args: Value) -> Result<Value> {
    match name {
        "generate_speech" => generate(config, args),
        "generate_speech_from_file" if args["text_file"].is_string() && args.get("text").is_none() => generate(config,args),
        "generate_speech_from_file" => Err("Specifica text_file e output per leggere un copione da file.".into()),
        "list_voices" if config.provider == "edge" => python(config, &json!({"operation":"voices","language":args["language"].as_str().unwrap_or("it")})),
        "list_voices" if config.provider == "piper" => ready_piper_voices(),
        "list_voices" if config.provider == "kokoro" => Ok(crate::kokoro::voices()),
        "list_voices" if config.provider == "qwen" => crate::qwen_tts::voices(),
        "list_voices" if config.provider == "custom" => Ok(json!({"voices":[{"id":config.voice,"name":config.voice}]})),
        "list_voices" if config.provider == "elevenlabs" => {
            let value = eleven("/v1/voices", None)?;
            Ok(json!({"voices": value["voices"].as_array().map(|voices| voices.iter().map(|v| json!({"id":v["voice_id"],"name":v["name"],"labels":v["labels"]})).collect::<Vec<_>>())}))
        }
        "list_models" if config.provider == "elevenlabs" => {
            let value = eleven("/v1/models", None)?;
            Ok(json!(value.as_array().map(|models| models.iter().filter(|m|m["can_do_text_to_speech"]==true)
                .map(|m|json!({"id":m["model_id"],"name":m["name"],"max_characters":m["max_characters_request_subscribed_user"]})).collect::<Vec<_>>())))
        }
        "list_models" if config.provider == "piper" => Ok(json!([{"id":"piper","name":"Piper locale"}])),
        "list_models" if config.provider == "kokoro" => Ok(json!([{"id":"kokoro","name":"Kokoro locale"}])),
        "list_models" if config.provider == "custom" => Ok(json!([{"id":config.model,"name":config.model}])),
        "list_models" if config.provider == "qwen" => Ok(json!([{"id":crate::qwen_tts::MODEL,"name":"Qwen3-TTS 1.7B CustomVoice"}])),
        "list_models" if config.provider == "edge" => Ok(json!([{"id":"edge","name":"Microsoft Edge online"}])),
        _ => Err("Strumento voce sconosciuto.".into()),
    }
}
fn ready_piper_voices() -> Result<Value> {
    Ok(json!(crate::piper::voices()?.as_array().into_iter().flatten().filter(|voice|voice["ready"]==true).cloned().collect::<Vec<_>>()))
}
fn tools(config: &Config) -> Value {
    let local = matches!(config.provider.as_str(), "piper"|"kokoro"|"qwen");
    let details = match config.provider.as_str() {
        "piper"=>"Piper locale: genera MP3 con durata reale. Non fornisce tempi delle parole o SRT. Voce e motore sono già applicati dalle impostazioni.",
        "kokoro"=>"Kokoro locale: genera MP3 con durata reale. Non fornisce tempi delle parole o SRT. Voce e motore sono già applicati dalle impostazioni.",
        "qwen"=>"Qwen3-TTS locale: genera MP3 con durata reale. Non fornisce tempi delle parole o SRT. instruct e language sono facoltativi.",
        "custom"=>"Endpoint personalizzato: genera MP3 con la voce impostata, senza tempi delle parole.",
        "edge"=>"Edge online: genera MP3 con durata reale, tempi delle parole e SRT quando disponibili.",
        "elevenlabs"=>"ElevenLabs online: genera MP3 con durata reale e allineamento quando disponibile. Consuma i crediti dell’account configurato.",
        _=>return json!({"tools":[]}),
    };
    let details = if config.provider=="elevenlabs" {format!("{} Modello attivo: {}.",details,json!(config.model))} else {details.to_owned()};
    let mut properties = json!({
        "text":{"type":"string","description":"Testo completo da leggere ad alta voce."},
        "text_file":{"type":"string","description":"Percorso del copione UTF-8 nella cartella della call."},
        "output":{"type":"string","description":"Nome del file audio nella cartella della call, ad esempio prova.mp3. Se manca l’estensione viene aggiunta .mp3."},
        "voice":{"type":"string","description":"ID voce del servizio attivo. Ometti per usare la voce configurata.","default":config.voice},
        "force_regenerate":{"type":"boolean","default":false}
    });
    match config.provider.as_str() {
        "edge"=>{
            properties["rate"]=json!({"type":"string","description":"Velocità, ad esempio +5%."});
            properties["pitch"]=json!({"type":"string","description":"Tono, ad esempio -2Hz."});
        },
        "elevenlabs"=>{
            properties["voice_settings"]=json!({"type":"object","description":"stability, similarity_boost, style, use_speaker_boost, speed."});
            properties["previous_text"]=json!({"type":"string"});
            properties["next_text"]=json!({"type":"string"});
        },
        "qwen"=>{
            properties["instruct"]=json!({"type":"string","maxLength":1000,"description":"Indicazioni su tono ed espressività."});
            properties["language"]=json!({"type":"string","enum":crate::qwen_tts::LANGUAGES,"default":config.qwen_language});
        },
        _=>{}
    }
    if config.provider == "piper" || config.provider == "kokoro" {
        properties.as_object_mut().unwrap().remove("voice");
        properties.as_object_mut().unwrap().remove("force_regenerate");
    }
    let mut file_properties=properties.clone();
    file_properties.as_object_mut().unwrap().remove("text");
    properties.as_object_mut().unwrap().remove("text_file");
    let voices_properties = if config.provider=="edge" {json!({"language":{"type":"string","default":"it"}})} else {json!({})};
    json!({"tools":[
        {"name":"generate_speech","description":format!("{} Riusa lo stesso output se testo e impostazioni coincidono. Se fallisce non ripetere la stessa chiamata invariata; correggi il problema o segnala l’errore.",details),
        "inputSchema":{"type":"object","properties":properties,"required":["text","output"],"additionalProperties":false},
        "annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":!local}},
        {"name":"generate_speech_from_file","description":format!("{} Legge un copione UTF-8 già salvato nella cartella della call e genera il file audio richiesto.",details),
        "inputSchema":{"type":"object","properties":file_properties,"required":["text_file","output"],"additionalProperties":false},
        "annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false,"openWorldHint":!local}},
        {"name":"list_voices","description":format!("Elenca esclusivamente le voci {} disponibili. Non genera audio.",provider_name(&config.provider)),"inputSchema":{"type":"object","properties":voices_properties,"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":!local}},
        {"name":"list_models","description":format!("Elenca esclusivamente i modelli {}. Non genera audio.",provider_name(&config.provider)),"inputSchema":{"type":"object","properties":{},"additionalProperties":false},"annotations":{"readOnlyHint":true,"openWorldHint":!local}}
    ]})
}

fn response(config: &Config, request: Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let result = match request["method"].as_str().unwrap_or_default() {
        "initialize" => json!({"protocolVersion":request["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05"),"capabilities":{"tools":{}},"serverInfo":{"name":SERVER,"version":env!("CARGO_PKG_VERSION")}}),
        "ping" => json!({}),
        "tools/list" => tools(config),
        "tools/call" => match call(config, request["params"]["name"].as_str().unwrap_or_default(), request["params"].get("arguments").cloned().unwrap_or(json!({}))) {
            Ok(value) => json!({"content":[{"type":"text","text":value.to_string()}],"isError":false}),
            Err(error) => json!({"content":[{"type":"text","text":format!("{}: {}",provider_name(&config.provider),error)}],"isError":true}),
        },
        _ => return Some(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}

// Only our native, provider-prefixed diagnostics are public activity. Do not
// retain arbitrary tool output, file contents or third-party response bodies.
pub fn public_error(content: &Value) -> Option<String> {
    public_error_inner(content,0)
}
fn public_error_inner(content: &Value, depth: usize) -> Option<String> {
    if depth > 3 {return None;}
    let extract = |text: &str| {
        let text=text.trim().strip_prefix("Error: ").unwrap_or(text.trim());
        if ["Piper: ","Kokoro: ","Qwen3-TTS: ","Edge: ","ElevenLabs: ","WebSearch: "].iter().any(|prefix|text.starts_with(prefix)) {
            return Some(text.chars().take(1500).collect());
        }
        serde_json::from_str::<Value>(text).ok().and_then(|value| {
            public_error_inner(value.get("content").unwrap_or(&value),depth+1)
        })
    };
    if let Some(text)=content.as_str() {return extract(text);}
    content.as_array()?.iter().filter_map(|block|block["text"].as_str()).find_map(extract)
}
pub fn serve(config: &str) -> Result<()> {
    let config: Config = serde_json::from_str(config).map_err(|_| "Configurazione voce non valida.")?;
    let input = std::io::stdin();
    let mut input = BufReader::new(input.lock());
    let output = std::sync::Mutex::new(std::io::stdout());
    std::thread::scope(|scope| -> Result<()> { loop {
        // Large copioni can be passed by file; keep the protocol bounded without truncating text.
        let mut line = String::new();
        let count = input.by_ref().take(8 * 1024 * 1024).read_line(&mut line).map_err(|e|e.to_string())?;
        if count == 0 { break; }
        if !line.ends_with('\n') { return Err("Richiesta MCP troppo grande: usa text_file.".into()); }
        let request = serde_json::from_str(&line).map_err(|_| "JSON MCP non valido.")?;
        // Different scenes can run concurrently. Only the same output is locked.
        let config = &config;
        let output = &output;
        scope.spawn(move || {
            if let Some(value) = response(config, request) {
                if let Ok(mut output) = output.lock() {
                    let _ = writeln!(output, "{value}").and_then(|_|output.flush());
                }
            }
        });
    } Ok(()) })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (PathBuf, Config) {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("parlo-tts-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&directory).unwrap();
        let config = Config { directory:directory.clone(),python:None,helper:directory.join("missing.py"),provider:"edge".into(),voice:"it-IT-IsabellaNeural".into(),model:"eleven_multilingual_v2".into(),qwen_device:"cpu".into(),qwen_language:"Italian".into(),custom:None };
        (directory,config)
    }
    #[test]
    fn audio_output_normalizes_missing_extension_and_preserves_path_safety() {
        let (root,_)=fixture();
        let canonical=crate::sources::canonical_path(&root).unwrap();
        assert_eq!(audio_output(&root,"prova").unwrap(),canonical.join("prova.mp3"));
        assert_eq!(audio_output(&root,"prova.MP3").unwrap(),canonical.join("prova.MP3"));
        for invalid in ["prova.wav","../prova","sottocartella/", ""] {
            assert!(audio_output(&root,invalid).is_err(),"{invalid}");
        }
        fs::create_dir(root.join("scena")).unwrap();
        assert_eq!(audio_output(&root,"scena/voce").unwrap(),canonical.join("scena/voce.mp3"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn active_tts_capabilities_expose_only_selected_service() {
        let (root,mut config)=fixture();
        let providers=["piper","kokoro","qwen","edge","elevenlabs"];
        for provider in providers {
            let mut settings=Settings {tts_enabled:true,tts_provider:provider.into(),..Default::default()};
            settings.tts_eleven_voice="voice-id".into();
            config.provider=provider.into();
            config.voice=match provider {"piper"=>"it_IT-serena-high","kokoro"=>"if_sara","qwen"=>"ryan","edge"=>"it-IT-IsabellaNeural",_=>"voice-id"}.into();
            config.model=provider.into();
            let exposed=tools(&config);
            let serialized=format!("{}{}",exposed,context(&settings)).to_lowercase();
            for inactive in providers.into_iter().filter(|name|*name!=provider) {
                assert!(!serialized.contains(inactive),"{provider} exposes {inactive}");
            }
            let properties=&exposed["tools"][0]["inputSchema"]["properties"];
            assert!(properties.get("model").is_none());
            if provider=="piper" || provider=="kokoro" {
                assert_eq!(properties.as_object().unwrap().keys().map(String::as_str).collect::<Vec<_>>(),vec!["output","text"]);
                assert_eq!(exposed["tools"][0]["inputSchema"]["required"],json!(["text","output"]));
                assert_eq!(exposed["tools"][1]["name"],"generate_speech_from_file");
                assert!(exposed["tools"][1]["inputSchema"]["properties"].get("text").is_none());
            }
            assert_eq!(properties.get("voice_settings").is_some(),provider=="elevenlabs");
            assert_eq!(properties.get("rate").is_some(),provider=="edge");
            assert_eq!(properties.get("instruct").is_some(),provider=="qwen");
            let reply=response(&config,json!({"id":1,"method":"tools/list"})).unwrap();
            assert_eq!(reply["result"],exposed);
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_tts_errors_identify_the_active_engine_without_exposing_other_services() {
        let (root,mut config)=fixture(); config.provider="piper".into();config.voice="it_IT-serena-high".into();
        let value=response(&config,json!({"id":2,"method":"tools/call","params":{"name":"generate_speech","arguments":{"text":"Prova.","output":"voice.mp3"}}})).unwrap();
        assert_eq!(value["result"]["isError"],true);
        let error=public_error(&value["result"]["content"]).unwrap();
        assert!(error.starts_with("Piper: ")); assert!(error.contains("Strumenti voce mancanti"));
        assert!(!error.contains("ElevenLabs"));
        assert!(public_error(&json!("PRIVATE TOOL RESULT")).is_none());
        let wrapped=json!({"content":[{"type":"text","text":"Piper: Parametri voce non validi."}],"isError":true}).to_string();
        assert_eq!(public_error(&json!(format!("Error: {wrapped}"))).unwrap(),"Piper: Parametri voce non validi.");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn qwen_validates_options_and_requires_explicit_setup_without_network() {
        let (root,mut config)=fixture();
        config.provider="qwen".into();config.voice="ryan".into();
        for (extra, message) in [
            (json!({"voice":"outside"}), "voce Qwen3-TTS"),
            (json!({"model":"cloud"}), "Modello non disponibile"),
            (json!({"rate":"+5%"}), "non sono applicabili"),
            (json!({"language":"invalid"}), "Lingua"),
        ] {
            let mut args=json!({"text":"Sintetico.","output":"voice.mp3"});
            for (key,value) in extra.as_object().unwrap() {args[key]=value.clone();}
            assert!(generate(&config,args).unwrap_err().contains(message));
        }
        assert!(generate(&config,json!({"text":"Sintetico.","output":"voice.mp3"})).unwrap_err().contains("Nessun download"));
        assert!(!root.join("voice.mp3").exists());
        assert!(call(&config,"list_voices",json!({})).unwrap().as_array().unwrap().iter().any(|v|v["id"]=="ryan"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn piper_rejects_cloud_parameters_and_unknown_voice_without_network() {
        let (root,mut config)=fixture();
        config.provider="piper".into();config.voice="it_IT-serena-medium".into();
        for extra in [json!({"rate":"+5%"}),json!({"voice_settings":{}}),json!({"pitch":"+2Hz"})] {
            let mut args=json!({"text":"Sintetico.","output":"voice.mp3"});
            for (key,value) in extra.as_object().unwrap() {args[key]=value.clone();}
            assert!(generate(&config,args).unwrap_err().contains("non si applicano"));
        }
        assert!(generate(&config,json!({"text":"Sintetico.","output":"voice.mp3","voice":"outside"})).unwrap_err().contains("catalogo"));
        assert!(generate(&config,json!({"text":"Sintetico.","output":"voice.mp3","model":"eleven_multilingual_v2"})).unwrap_err().contains("Modello non disponibile per Piper"));
        let voices=call(&config,"list_voices",json!({})).unwrap();
        assert!(voices.as_array().unwrap().iter().all(|voice|voice["ready"]==true && crate::piper::known_voice(voice["id"].as_str().unwrap())));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn piper_verified_audio_cache_does_not_require_runtime_or_redownload() {
        let (root,mut config)=fixture();
        config.provider="piper".into();config.voice="it_IT-serena-medium".into();config.model="piper".into();
        let request=json!({"provider":"piper","voice":config.voice,"model":"piper","text":"Sintetico."});
        let hash=format!("{:x}",Sha256::digest(serde_json::to_vec(&request).unwrap()));
        fs::write(root.join("scene.mp3"),b"synthetic local audio").unwrap();
        fs::write(root.join("scene.speech.json"),json!({"provider":"piper","request_hash":hash,
            "audio_sha256":format!("{:x}",Sha256::digest(b"synthetic local audio")),"duration_seconds":2.,"words":[],"warnings":[]}).to_string()).unwrap();
        let result=generate(&config,json!({"text":"Sintetico.","output":"scene.mp3"})).unwrap();
        assert_eq!(result["cached"],true);assert_eq!(result["word_timings_available"],false);
        assert!(result["subtitles"].is_null());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn tool_protocol_and_missing_runtime_do_not_need_credentials() {
        let (root,config) = fixture();
        assert!(response(&config,json!({"method":"notifications/initialized"})).is_none());
        let result=response(&config,json!({"id":1,"method":"tools/list"})).unwrap();
        assert_eq!(result["result"]["tools"][0]["name"],"generate_speech");
        assert!(generate(&config,json!({"text":"Prova sintetica.","output":"voice.mp3"})).unwrap_err().contains("pacchetto aggiornato"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn edge_uses_its_model_even_when_eleven_model_is_empty() {
        let (root,mut config)=fixture();
        config.model.clear();
        for model in [Value::Null,json!("edge")] {
            let error=generate(&config,json!({"text":"Prova.","output":"test.mp3","model":model})).unwrap_err();
            assert!(error.contains("pacchetto aggiornato"),"{error}");
        }
        assert!(generate(&config,json!({"text":"Prova.","output":"test.mp3","model":"eleven_multilingual_v2"})).unwrap_err().contains("Modello non disponibile per Edge"));
        config.provider="elevenlabs".into();
        assert!(generate(&config,json!({"text":"Prova.","output":"test.mp3","model":""})).unwrap_err().contains("modello non valido"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn destinations_and_sources_cannot_escape_call() {
        let (root,config) = fixture();
        for path in ["../outside.mp3", "sub/../../outside.mp3", "bad.mp3:secret"] { assert!(safe_path(&root,path).is_err()); }
        assert!(generate(&config,json!({"text_file":"../outside.txt","output":"a.mp3"})).is_err());
        assert!(generate(&config,json!({"text":"a","text_file":"b","output":"a.mp3"})).is_err());
        assert!(generate(&config,json!({"text":"a","output":"a.wav"})).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn verified_cache_reuses_audio_without_a_network_request() {
        let (root,config) = fixture();
        let request=json!({"provider":"edge","voice":config.voice,"model":"edge","text":"Prova.","rate":"+0%","pitch":"+0Hz"});
        let hash=format!("{:x}",Sha256::digest(serde_json::to_vec(&request).unwrap()));
        fs::write(root.join("voice.mp3"),b"synthetic cached bytes").unwrap();
        fs::write(root.join("voice.srt"),"").unwrap();
        let mut metadata=json!({"provider":"edge","request_hash":hash,"audio_sha256":format!("{:x}",Sha256::digest(b"synthetic cached bytes")),"duration_seconds":1.2,
            "words":[{"text":"Prova.","start":0.0,"end":1.0}],"warnings":[]});
        fs::write(root.join("voice.speech.json"),metadata.to_string()).unwrap();
        let result=generate(&config,json!({"text":"Prova.","output":"voice.mp3"})).unwrap();
        assert_eq!(result["cached"],true);
        assert_eq!(result["word_timings_available"],true);
        assert!(result["subtitles"].is_string());
        assert!(!result["audio"].as_str().unwrap().starts_with(r"\\?\"));
        metadata["words"]=json!([]);
        metadata["warnings"]=json!(["Tempi delle parole non disponibili."]);
        fs::write(root.join("voice.speech.json"),metadata.to_string()).unwrap();
        fs::remove_file(root.join("voice.srt")).unwrap();
        let result=generate(&config,json!({"text":"Prova.","output":"voice.mp3"})).unwrap();
        assert_eq!(result["cached"],true);
        assert_eq!(result["word_timings_available"],false);
        assert!(result["subtitles"].is_null());
        assert_eq!(result["warnings"].as_array().unwrap().len(),1);
        assert!(generate(&config,json!({"text":"Prova.","output":"voice.mp3","force_regenerate":true})).is_err());
        fs::write(root.join("voice.mp3"),b"modified").unwrap();
        assert!(generate(&config,json!({"text":"Prova.","output":"voice.mp3"})).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn same_output_is_locked_but_different_scenes_can_run_together() {
        let (root,_) = fixture();
        let first=output_lock(&root,&root.join("scene-01.mp3")).unwrap();
        assert!(output_lock(&root,&root.join("scene-01.mp3")).is_err());
        let second=output_lock(&root,&root.join("scene-02.mp3")).unwrap();
        drop(first);
        assert!(output_lock(&root,&root.join("scene-01.mp3")).is_ok());
        drop(second);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn os_vault_roundtrip_uses_an_isolated_test_entry() {
        if !cfg!(any(windows,target_os="macos")) { return; }
        let entry=keyring::Entry::new("PARLO.TTS.Test",&uuid::Uuid::new_v4().to_string()).unwrap();
        entry.set_password("synthetic-test-value").unwrap();
        let read=entry.get_password();
        let deleted=entry.delete_credential();
        assert_eq!(read.unwrap(),"synthetic-test-value");
        deleted.unwrap();
        assert!(matches!(entry.get_password(),Err(keyring::Error::NoEntry)));
    }
}
