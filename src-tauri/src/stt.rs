//! Optional cloud transcription. Audio stays on disk until the complete
//! transcript is committed. Credentials never enter settings, scripts or logs.
use crate::{
    model::{Segment, Settings},
    AppState, LockExt, Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::Emitter;

const MODEL: &str = "whisper-large-v3-turbo";
const RATE: usize = 16000;
const CHUNK: usize = 300 * RATE;
const OVERLAP: usize = RATE;
const RESPONSE_LIMIT: u64 = 8 * 1024 * 1024;

fn credential() -> Result<keyring::Entry> {
    // Preserve credentials already saved in the system vault.
    keyring::Entry::new("Presentio.Transcript.STT", "groq-api-key")
        .map_err(|_| "Portachiavi di sistema non disponibile.".into())
}
fn api_key() -> Result<String> {
    credential()?
        .get_password()
        .ok()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| "Inserisci la chiave Groq in Impostazioni > Registrazione.".into())
}
pub fn ready(settings: &Settings) -> bool {
    ensure_ready(settings).is_ok()
}
pub fn ensure_ready(settings: &Settings) -> Result<()> {
    if settings.stt_provider == "custom" {
        settings.custom_stt.validate("stt")?;
        crate::custom_endpoint::key("stt", &settings.custom_stt).map(|_| ())
    } else if settings.stt_provider == "groq" {
        api_key().map(|_| ())
    } else if settings.whisper_ready() {
        Ok(())
    } else {
        Err("Il motore locale non è pronto. Attendi la preparazione nelle impostazioni.".into())
    }
}
fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" {
        return Err("Gestisci la chiave dalla finestra principale.".into());
    }
    Ok(())
}
#[tauri::command]
pub fn stt_key_status(window: tauri::WebviewWindow) -> Result<bool> {
    main_window(&window)?;
    match credential()?.get_password() {
        Ok(key) => Ok(!key.is_empty()),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err("Impossibile accedere al portachiavi di sistema.".into()),
    }
}
#[tauri::command]
pub fn stt_save_key(
    window: tauri::WebviewWindow,
    state: tauri::State<crate::Shared>,
    key: String,
) -> Result<()> {
    main_window(&window)?;
    let _guard = state.operation.guard();
    ensure_key_editable(&state)?;
    let key = key.trim();
    if key.is_empty()
        || key.len() > 4096
        || key.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return Err("Chiave API non valida.".into());
    }
    credential()?
        .set_password(key)
        .map_err(|_| "Impossibile salvare la chiave nel portachiavi di sistema.".into())
}
#[tauri::command]
pub fn stt_delete_key(
    window: tauri::WebviewWindow,
    state: tauri::State<crate::Shared>,
) -> Result<()> {
    main_window(&window)?;
    let _guard = state.operation.guard();
    ensure_key_editable(&state)?;
    match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("Impossibile rimuovere la chiave dal portachiavi di sistema.".into()),
    }
}
fn ensure_key_editable(state: &AppState) -> Result<()> {
    let status = state.status.guard();
    if status.capture.is_some() || status.processing.is_some() || !status.queued.is_empty() {
        return Err("Attendi il termine della registrazione e dell’elaborazione prima di cambiare la chiave Groq.".into());
    }
    Ok(())
}

type Wav = hound::WavReader<std::io::BufReader<fs::File>>;
fn open_track(path: &Path) -> Result<Wav> {
    let reader =
        hound::WavReader::open(path).map_err(|_| "Audio temporaneo non leggibile.".to_string())?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != RATE as u32
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err("Formato audio non supportato per Groq.".into());
    }
    Ok(reader)
}
fn read_track(reader: &mut Wav, start: usize, count: usize) -> Result<Vec<f32>> {
    let available = reader.duration() as usize;
    let mut samples = vec![0.; count];
    if start < available {
        reader
            .seek(start as u32)
            .map_err(|_| "Lettura audio interrotta.".to_string())?;
        for (index, sample) in reader
            .samples::<i16>()
            .take(count.min(available - start))
            .enumerate()
        {
            samples[index] =
                sample.map_err(|_| "Audio temporaneo danneggiato.".to_string())? as f32 / 32768.;
        }
    }
    Ok(samples)
}
fn trim_trailing_silence(samples: &mut Vec<f32>) -> usize {
    let Some(last_signal)=samples.iter().rposition(|sample|sample.abs()>=0.0001) else {return 0;};
    let keep=(last_signal+1).saturating_add(RATE/2).min(samples.len());
    let removed=samples.len()-keep;
    samples.truncate(keep);
    removed
}
fn wav_bytes(samples: &[f32]) -> Result<Vec<u8>> {
    let mut cursor = Cursor::new(Vec::with_capacity(samples.len() * 2 + 44));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec).map_err(|e| e.to_string())?;
        for sample in samples {
            writer
                .write_sample((sample.clamp(-1., 1.) * 32767.).round() as i16)
                .map_err(|e| e.to_string())?;
        }
        writer.finalize().map_err(|e| e.to_string())?;
    }
    Ok(cursor.into_inner())
}

#[derive(Clone, Serialize, Deserialize)]
struct Word {
    #[serde(alias = "text")]
    word: String,
    start: f64,
    end: f64,
}
#[derive(Serialize, Deserialize)]
struct Transcript {
    text: String,
    #[serde(default)]
    words: Vec<Word>,
    #[serde(default)]
    segments: Vec<Word>,
}
fn multipart(settings: &Settings, audio: &[u8], boundary: &str, segments_only: bool) -> Vec<u8> {
    let mut body = Vec::with_capacity(audio.len() + 2048);
    let mut field = |name: &str, value: &str| {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n").as_bytes());
    };
    field("model", if settings.stt_provider == "custom" { &settings.custom_stt.model } else { MODEL });
    field("response_format", "verbose_json");
    if !segments_only { field("timestamp_granularities[]", "word"); }
    field("timestamp_granularities[]", "segment");
    field("temperature", "0");
    if settings.language != "auto" {
        field("language", &settings.language);
    }
    let prompt = settings
        .dictionary
        .iter()
        .map(|entry| entry.term.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if !prompt.is_empty() {
        field("prompt", &prompt.chars().take(600).collect::<String>());
    }
    body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"audio.wav\"\r\nContent-Type: audio/wav\r\n\r\n").as_bytes());
    body.extend_from_slice(audio);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}
struct RequestError {
    message: String,
    transient: bool,
}
impl RequestError {
    fn permanent(message: impl Into<String>) -> Self { Self { message: message.into(), transient: false } }
    fn transient(message: impl Into<String>) -> Self { Self { message: message.into(), transient: true } }
}
fn retry_transient<T>(mut send: impl FnMut() -> std::result::Result<T, RequestError>, mut before_retry: impl FnMut(usize) -> Result<()>) -> Result<T> {
    for attempt in 0..3 {
        match send() {
            Ok(value) => return Ok(value),
            Err(error) if error.transient && attempt < 2 => before_retry(attempt + 1)?,
            Err(error) if error.transient => return Err(format!("{} Tentativi automatici esauriti; le richieste interrotte potrebbero essere conteggiate.", error.message)),
            Err(error) => return Err(error.message),
        }
    }
    unreachable!()
}
fn request(
    client: &reqwest::blocking::Client,
    key: &str,
    settings: &Settings,
    audio: &[u8],
    segments_only: bool,
) -> std::result::Result<Transcript, RequestError> {
    let custom = settings.stt_provider == "custom";
    let url = if custom { settings.custom_stt.url("audio/transcriptions").map_err(RequestError::permanent)? }
        else { "https://api.groq.com/openai/v1/audio/transcriptions".into() };
    let brand = if custom { "L’endpoint STT" } else { "Groq" };
    let boundary = format!("parlo-{}", uuid::Uuid::new_v4());
    let body = multipart(settings, audio, &boundary, segments_only);
    // Retries are explicitly limited by the caller; keep raw errors and keys private.
    let mut request = client.post(url).header(reqwest::header::CONTENT_TYPE, format!("multipart/form-data; boundary={boundary}")).body(body);
    if !key.is_empty() { request = request.bearer_auth(key); }
    let response = request.send()
        .map_err(|error| RequestError::transient(if custom {
            "Connessione all’endpoint STT interrotta. Audio conservato."
        } else if error.is_timeout() {
            "Groq non ha risposto entro il tempo limite. Audio conservato."
        } else if error.is_connect() {
            "Impossibile connettersi a Groq. Verifica la connessione Internet. Audio conservato."
        } else {
            "Connessione a Groq interrotta durante la richiesta. Audio conservato."
        }))?;
    if !response.status().is_success() {
        let status = response.status().as_u16();
        if custom {
            let message = format!("L’endpoint STT ha risposto HTTP {status}. Audio conservato.");
            return Err(if matches!(status, 500 | 502 | 503 | 504) { RequestError::transient(message) } else { RequestError::permanent(message) });
        }
        return Err(match status {
            401 | 403 => RequestError::permanent("Groq: verifica chiave API e permessi dell’account. Audio conservato."),
            429 => RequestError::permanent("Groq: quota gratuita o limite di richieste raggiunto. Audio e parti completate conservati: riprova quando la quota è disponibile."),
            400 | 413 | 422 => RequestError::permanent("Groq non ha accettato l’audio o i parametri. Audio conservato; nessun tentativo automatico."),
            500 | 502 | 503 | 504 | 520 => RequestError::transient(format!("Groq temporaneamente non disponibile (HTTP {status}). Audio conservato.")),
            _ => RequestError::permanent(format!("Groq ha risposto HTTP {status}. Audio conservato: puoi riprovare.")),
        });
    }
    let mut bytes = vec![];
    response.take(RESPONSE_LIMIT + 1).read_to_end(&mut bytes)
        .map_err(|_| RequestError::transient(format!("{brand}: risposta interrotta. Audio conservato.")))?;
    if bytes.len() as u64 > RESPONSE_LIMIT {
        return Err(RequestError::permanent(format!("{brand}: risposta troppo grande. Audio conservato.")));
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| RequestError::permanent(format!("{brand}: risposta non valida. Audio conservato.")))
}

pub fn test_endpoint(endpoint: crate::custom_endpoint::Endpoint, key: &str) -> Result<()> {
    let settings = Settings { stt_provider: "custom".into(), custom_stt: endpoint, ..Settings::default() };
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(30))
        .build().map_err(|_| "Client HTTP non disponibile.")?;
    let audio = wav_bytes(&vec![0.0; RATE])?;
    let result = request(&client, key, &settings, &audio, true).map_err(|error| error.message)?;
    validate(&result, 1.0)
}

fn cache_id(audio: &[u8], settings: &Settings) -> String {
    let mut hash = Sha256::new();
    hash.update(b"presentio-groq-v1");
    hash.update(serde_json::to_vec(&(MODEL, &settings.language, &settings.dictionary)).unwrap());
    if settings.stt_provider == "custom" {
        hash.update(serde_json::to_vec(&settings.custom_stt).unwrap());
    }
    hash.update(audio);
    format!("{:x}", hash.finalize())
}
fn save_checkpoint(path: &Path, response: &Transcript) -> Result<()> {
    use std::io::Write;
    let partial = path.with_extension("part");
    let bytes = serde_json::to_vec(response).map_err(|e| e.to_string())?;
    let mut file = fs::File::create(&partial).map_err(|e| e.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    drop(file);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    fs::rename(partial, path).map_err(|e| e.to_string())
}
// Keep malformed alignment separate from successful checkpoints. The next
// explicit retry requests only segment times for this exact audio chunk;
// it never repeats a billable request automatically or fabricates timestamps.
fn timestamp_failure_path(path: &Path) -> PathBuf { path.with_extension("unaligned.json") }
fn chunk_error(error: &str, index: usize, first: usize, last: usize, total: usize) -> String {
    let clock=|sample:usize| {let seconds=sample/RATE;format!("{:02}:{:02}:{:02}",seconds/3600,seconds/60%60,seconds%60)};
    format!("Trascrizione Groq incompleta: parte {}/{} ({}–{}). {} Le parti precedenti sono conservate; Riprova riusa quelle già completate.",index+1,total,clock(first),clock(last),error)
}
fn checkpoint_response(path: &Path, response: &Transcript, seconds: f64) -> Result<bool> {
    let unaligned=timestamp_failure_path(path);
    if validate(response,seconds).is_err() {
        save_checkpoint(&unaligned,response)?;
        return Ok(false);
    }
    save_checkpoint(path,response)?;
    if unaligned.is_file() {let _=fs::remove_file(unaligned);}
    Ok(true)
}
fn cached_response(path: &Path, seconds: f64) -> Option<Transcript> {
    [path.to_path_buf(),timestamp_failure_path(path)].iter().find_map(|path|
        fs::read(path).ok().and_then(|bytes|serde_json::from_slice::<Transcript>(&bytes).ok())
            .filter(|response|validate(response,seconds).is_ok()))
}
fn ensure_all_parts_completed(failures: &[String]) -> Result<()> {
    if failures.is_empty() {return Ok(());}
    Err(format!("{} parti Groq da recuperare. Le altre parti sono state elaborate e l’audio completo è conservato.\n{}",failures.len(),failures.join("\n")))
}
fn valid_timestamps(items: &[Word], seconds: f64) -> bool {
    !items.is_empty()
        && !items.windows(2).any(|pair| pair[1].start < pair[0].start)
        && !items.iter().any(|word| {
            !word.start.is_finite()
                || !word.end.is_finite()
                || word.start < 0.
                || word.end < word.start
                || word.end > seconds + 1.
        })
        && items.iter().any(|word| !word.word.trim().is_empty())
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum TimingQuality { EndClipped, UnalignedText }
struct TimedUnits<'a> { items: Cow<'a,[Word]>, quality: Option<TimingQuality> }
impl std::ops::Deref for TimedUnits<'_> {
    type Target=[Word];
    fn deref(&self) -> &Self::Target { &self.items }
}
fn timed_units(response: &Transcript, seconds: f64) -> Result<TimedUnits<'_>> {
    if valid_timestamps(&response.words, seconds) {
        return Ok(TimedUnits{items:Cow::Borrowed(&response.words),quality:None});
    }
    // Groq may omit word alignment or return inconsistent word times while
    // retaining valid segment timestamps. Use those actual ranges unchanged.
    if valid_timestamps(&response.segments, seconds) {
        return Ok(TimedUnits{items:Cow::Borrowed(&response.segments),quality:None});
    }
    // Whisper can align the last segment into its trailing 30-second window,
    // beyond the actual file. Only clip that final end to the known audio
    // boundary; never sort words, invent starts or accept arbitrary overruns.
    if seconds.is_finite() && seconds > 0. {
        if response.segments.last().is_some_and(|last|
            last.start.is_finite() && last.end.is_finite()
            && last.start >= 0. && last.start < seconds
            && last.end > seconds && last.end <= seconds+30.) {
            let mut segments=response.segments.clone();
            segments.last_mut().unwrap().end=seconds;
            if valid_timestamps(&segments,seconds) {return Ok(TimedUnits{items:Cow::Owned(segments),quality:Some(TimingQuality::EndClipped)});}
        }
    }
    // Text is useful even when the provider cannot align it. Keep the whole
    // returned text against the known upload interval, explicitly unaligned.
    // This is not an inferred word/phrase timestamp.
    let text=if !response.text.trim().is_empty() {response.text.trim().to_owned()}
        else {
            let units=if response.segments.iter().any(|unit|!unit.word.trim().is_empty()) {&response.segments} else {&response.words};
            units.iter().map(|unit|unit.word.trim()).filter(|text|!text.is_empty()).collect::<Vec<_>>().join(" ")
        };
    if !text.is_empty() && seconds.is_finite() && seconds>0. {
        return Ok(TimedUnits {items:Cow::Owned(vec![Word{word:text,start:0.,end:seconds}]),quality:Some(TimingQuality::UnalignedText)});
    }
    if response.text.trim().is_empty() && response.words.is_empty() && response.segments.is_empty()
    {
        return Ok(TimedUnits{items:Cow::Borrowed(&[]),quality:None});
    }
    Err("Groq non ha restituito testo recuperabile o tempi utilizzabili per questa parte. Audio conservato: puoi riprovare.".into())
}
fn validate(response: &Transcript, seconds: f64) -> Result<()> {
    timed_units(response, seconds).map(|_| ())
}

// Source labels follow input energy, as opposed to identifying individual
// participants. Prefix sums bound both memory and the cost of word lookups.
fn energy(samples: &[f32]) -> Vec<f64> {
    let mut sums = Vec::with_capacity(samples.len() / 320 + 2);
    sums.push(0.);
    for frame in samples.chunks(320) {
        sums.push(
            sums.last().unwrap()
                + frame
                    .iter()
                    .map(|value| (*value as f64).powi(2))
                    .sum::<f64>()
                    / frame.len() as f64,
        );
    }
    sums
}
fn boost_microphone(samples: &mut [f32]) {
    let peak = samples
        .iter()
        .fold(0f32, |peak, value| peak.max(value.abs()));
    if peak < 0.0001 {
        return;
    }
    let mut levels: Vec<f32> = samples
        .chunks(1600)
        .map(|frame| {
            (frame.iter().map(|value| value * value).sum::<f32>() / frame.len() as f32).sqrt()
        })
        .collect();
    levels.sort_by(f32::total_cmp);
    let voice = levels[((levels.len() - 1) as f64 * 0.9).round() as usize];
    let gain = 12f32
        .min(0.06 / voice.max(0.000001))
        .min(0.95 / peak)
        .max(1.);
    for sample in samples {
        *sample *= gain;
    }
}
fn level(sums: &[f64], start: f64, end: f64) -> f64 {
    let first = ((start / 0.02).floor() as usize).min(sums.len() - 2);
    let last = ((end / 0.02).ceil() as usize)
        .max(first + 1)
        .min(sums.len() - 1);
    ((sums[last] - sums[first]).max(0.) / (last - first) as f64).sqrt()
}
fn append_words(
    result: &mut Vec<Segment>,
    words: &[Word],
    offset: usize,
    first: usize,
    last: usize,
    mic: Option<&[f64]>,
    pc: Option<&[f64]>,
) {
    for word in words {
        let text = word.word.trim();
        if text.is_empty() {
            continue;
        }
        let midpoint = offset as f64 + (word.start + word.end) * 0.5 * RATE as f64;
        // Each overlap word belongs to exactly one chunk, by its midpoint.
        if midpoint < first as f64 || midpoint >= last as f64 {
            continue;
        }
        let speaker = match (mic, pc) {
            (Some(m), Some(p)) => {
                if level(m, word.start, word.end) > 0.006
                    && level(m, word.start, word.end) >= level(p, word.start, word.end) * 0.65
                {
                    "Tu"
                } else {
                    "Speaker"
                }
            }
            (Some(_), None) => "Tu",
            _ => "Speaker",
        };
        let start_ms = ((offset as f64 / RATE as f64 + word.start) * 1000.).round() as i64;
        let end_ms = ((offset as f64 / RATE as f64 + word.end) * 1000.).round() as i64;
        if let Some(previous) = result.last_mut().filter(|previous| {
            previous.speaker == speaker
                && start_ms - previous.end_ms < 2000
                && end_ms - previous.start_ms < 20000
        }) {
            previous.text.push(' ');
            previous.text.push_str(text);
            previous.end_ms = previous.end_ms.max(end_ms);
        } else {
            result.push(Segment {
                start_ms,
                end_ms,
                text: text.into(),
                speaker: speaker.into(),
                source: "mixed".into(),
                quality: None,
                original_text: None,
            });
        }
    }
}

fn append_response(result: &mut Vec<Segment>, response: &Transcript, seconds: f64, offset: usize, first: usize, last: usize, mic: Option<&[f64]>, pc: Option<&[f64]>) -> Result<bool> {
    let units=timed_units(response,seconds)?;
    if units.quality==Some(TimingQuality::UnalignedText) {
        // Without alignment the midpoint ownership rule could discard a short
        // final chunk. Keep every word and show the complete upload interval.
        result.push(Segment {
            start_ms:(offset as f64/RATE as f64*1000.).round() as i64,
            end_ms:((offset as f64/RATE as f64+seconds)*1000.).round() as i64,
            text:units[0].word.clone(),speaker:"Speaker".into(),source:"mixed".into(),
            quality:Some("Tempi puntuali non disponibili: testo dell’intero intervallo audio, con possibili ripetizioni ai confini. Audio conservato per la verifica.".into()),original_text:None,
        });
        return Ok(true);
    }
    let adjusted_start=if units.quality==Some(TimingQuality::EndClipped) {
        units.last().map(|unit|((offset as f64/RATE as f64+unit.start)*1000.).round() as i64)
    } else {None};
    append_words(result,&units,offset,first,last,mic,pc);
    if let Some(adjusted_start)=adjusted_start {
        for segment in result.iter_mut().rev().take_while(|segment|segment.end_ms>=adjusted_start) {
            segment.quality=Some("Groq: il tempo finale superava la durata dell’audio ed è stato limitato alla fine del file. Verifica questo passaggio.".into());
        }
    }
    Ok(false)
}
pub fn transcribe(
    app: &tauri::AppHandle,
    state: &AppState,
    settings: &Settings,
    directory: &Path,
) -> Result<Vec<Segment>> {
    let custom = settings.stt_provider == "custom";
    let key = if custom { crate::custom_endpoint::key("stt", &settings.custom_stt)? } else { api_key()? };
    let mut tracks: Vec<(String, Wav)> = ["microphone.wav", "system.wav"]
        .iter()
        .map(|name| (name.to_string(), directory.join(name)))
        .filter(|(_, path)| path.is_file())
        .map(|(name, path)| open_track(&path).map(|reader| (name, reader)))
        .collect::<Result<_>>()?;
    if tracks.is_empty() {
        tracks.push((
            "system.wav".into(),
            open_track(&directory.join("registrazione.wav"))?,
        ));
    }
    let length = tracks
        .iter()
        .map(|(_, reader)| reader.duration() as usize)
        .max()
        .unwrap_or(0);
    if length == 0 {
        return Ok(vec![]);
    }
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| "Client Groq non disponibile.".to_string())?;
    let cache = directory.join("groq-checkpoints");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let mut result = vec![];
    let mut alignment_failures=Vec::new();
    for (index, first) in (0..length).step_by(CHUNK).enumerate() {
        crate::ensure_processing_active(state)?;
        let last = (first + CHUNK).min(length);
        let start = first.saturating_sub(OVERLAP);
        let end = (last + OVERLAP).min(length);
        state.status.guard().stage = format!(
            "{} {}/{}",
            if custom { crate::locale::tr("Trascrizione tramite endpoint · parte", "Endpoint transcription · part") } else { crate::locale::tr("Trascrizione Groq · parte", "Groq transcription · part") },
            index + 1,
            length.div_ceil(CHUNK)
        );
        crate::changed(app, state);
        let mut mixed = vec![0f32; end - start];
        let (mut mic, mut pc) = (None, None);
        for (name, reader) in &mut tracks {
            let mut samples = read_track(reader, start, end - start)?;
            if name == "microphone.wav" && settings.microphone_voice_boost {
                boost_microphone(&mut samples);
            }
            if name == "microphone.wav" {
                mic = Some(energy(&samples));
            } else {
                pc = Some(energy(&samples));
            }
            for (out, value) in mixed.iter_mut().zip(samples) {
                *out += value;
            }
        }
        let peak = mixed.iter().fold(0f32, |peak, value| peak.max(value.abs()));
        if peak < 0.0001 {
            continue;
        }
        if peak > 0.95 {
            for sample in &mut mixed {
                *sample *= 0.95 / peak;
            }
        }
        let original_seconds=mixed.len() as f64/RATE as f64;
        let original_audio=wav_bytes(&mixed)?;
        let original_path:PathBuf=cache.join(format!("{}.json",cache_id(&original_audio,settings)));
        let old_cached=cached_response(&original_path,original_seconds);
        let (audio,path,seconds,cached)=if old_cached.is_some() {
            (original_audio,original_path,original_seconds,old_cached)
        } else {
            let removed=trim_trailing_silence(&mut mixed);
            let seconds=mixed.len() as f64/RATE as f64;
            let audio=if removed>0 {wav_bytes(&mixed)?} else {original_audio};
            let path=cache.join(format!("{}.json",cache_id(&audio,settings)));
            let cached=cached_response(&path,seconds);
            (audio,path,seconds,cached)
        };
        drop(mixed);
        let response = match cached {
            Some(response) => {
                if timestamp_failure_path(&path).is_file() {checkpoint_response(&path,&response,seconds)?;}
                response
            },
            None => {
                crate::ensure_processing_active(state)?;
                let unaligned=timestamp_failure_path(&path);
                let response = retry_transient(
                    || request(&client, &key, settings, &audio, unaligned.is_file()),
                    |retry| {
                        state.status.guard().stage = format!("{} {}/3 · {} {}/{}", crate::locale::tr("Connessione interrotta · nuovo tentativo", "Connection interrupted · retry"), retry + 1, crate::locale::tr("parte", "part"), index + 1, length.div_ceil(CHUNK));
                        crate::changed(app, state);
                        // Wait briefly without preventing the user from cancelling.
                        for _ in 0..if retry == 1 { 12 } else { 40 } {
                            crate::ensure_processing_active(state)?;
                            std::thread::sleep(Duration::from_millis(250));
                        }
                        crate::ensure_processing_active(state)
                    },
                )
                    .map_err(|error|chunk_error(&error,index,first,last,length.div_ceil(CHUNK)))?;
                // Save even a malformed alignment, but never accept it as a
                // complete chunk. Continue with later chunks; keep gaps explicit.
                let completed=checkpoint_response(&path,&response,seconds)
                    .map_err(|error|chunk_error(&error,index,first,last,length.div_ceil(CHUNK)))?;
                if !completed {
                    let error=validate(&response,seconds).unwrap_err();
                    alignment_failures.push(chunk_error(&error,index,first,last,length.div_ceil(CHUNK)));
                    continue;
                }
                response
            }
        };
        crate::ensure_processing_active(state)?;
        if append_response(&mut result,&response,seconds,start,first,last,mic.as_deref(),pc.as_deref())? {
            // Imprecise alignment must not trigger deletion of the source audio.
            fs::write(directory.join("keep-audio"),b"groq-unaligned-text").map_err(|error|format!("Impossibile conservare l’audio da verificare: {error}"))?;
        }
        let id = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let _guard = state.operation.guard();
        crate::ensure_processing_active(state)?;
        crate::db::store_draft(&mut state.db.guard(), id, &result)?;
        let _ = app.emit("calls-changed", ());
    }
    ensure_all_parts_completed(&alignment_failures)?;
    Ok(crate::transcript_quality::review(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn temporary_failures_retry_but_quota_errors_and_cancellation_do_not() {
        let mut calls=0;
        let mut retries=Vec::new();
        let result=retry_transient(|| { calls+=1; if calls<3 {Err(RequestError::transient("temporaneo"))} else {Ok(42)} }, |attempt| {retries.push(attempt); Ok(())});
        assert_eq!(result.unwrap(),42);
        assert_eq!(calls,3);
        assert_eq!(retries,vec![1,2]);
        let mut calls=0;
        let result: Result<()> = retry_transient(|| {calls+=1; Err(RequestError::permanent("quota"))}, |_| panic!("quota must not retry"));
        assert_eq!(result.unwrap_err(),"quota");
        assert_eq!(calls,1);
        let mut calls=0;
        let result: Result<()> = retry_transient(|| {calls+=1; Err(RequestError::transient("rete"))}, |_| Err("annullato".into()));
        assert_eq!(result.unwrap_err(),"annullato");
        assert_eq!(calls,1);
        let mut calls=0;
        let result: Result<()> = retry_transient(|| {calls+=1; Err(RequestError::transient("rete"))}, |_| Ok(()));
        assert_eq!(calls,3);
        assert!(result.unwrap_err().contains("potrebbero essere conteggiate"));
    }
    #[test]
    fn overlap_words_keep_global_times_and_are_not_duplicated() {
        let response = Transcript {
            text: "prima dopo".into(),
            segments: vec![],
            words: vec![
                Word {
                    word: "prima".into(),
                    start: 0.2,
                    end: 0.8,
                },
                Word {
                    word: "dopo".into(),
                    start: 1.2,
                    end: 1.8,
                },
            ],
        };
        let mut result = vec![];
        append_words(
            &mut result,
            &response.words,
            299 * RATE,
            300 * RATE,
            600 * RATE,
            None,
            None,
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "dopo");
        assert_eq!(result[0].start_ms, 300200);
        assert_eq!(result[0].end_ms, 300800);
    }
    #[test]
    fn checkpoints_are_bound_to_audio_language_and_dictionary() {
        let mut settings = Settings::default();
        let original = cache_id(b"synthetic", &settings);
        assert_ne!(original, cache_id(b"changed", &settings));
        settings.language = "en".into();
        assert_ne!(original, cache_id(b"synthetic", &settings));
        settings.language = Settings::default().language;
        settings.dictionary.push(crate::model::DictionaryEntry {
            term: "PARLO".into(),
        });
        assert_ne!(original, cache_id(b"synthetic", &settings));
    }
    #[test]
    fn text_without_alignment_is_preserved_and_empty_bad_responses_are_rejected() {
        let response = Transcript {
            text: "test".into(),
            words: vec![],
            segments: vec![],
        };
        let recovered=timed_units(&response,3.).unwrap();
        assert_eq!(recovered.quality,Some(TimingQuality::UnalignedText));
        assert_eq!(recovered[0].word,"test");
        let response = Transcript {
            text: "".into(),
            segments: vec![],
            words: vec![Word {
                word: " ".into(),
                start: 2.,
                end: 1.,
            }],
        };
        assert!(validate(&response, 3.).is_err());
    }
    #[test]
    fn segment_only_response_keeps_service_times_and_imported_source() {
        let response: Transcript = serde_json::from_str(
            r#"{
            "text": "Una frase completa.",
            "segments": [{"id": 0, "text": " Una frase completa.", "start": 1.25, "end": 4.5}]
        }"#,
        )
        .unwrap();
        let units = timed_units(&response, 5.).unwrap();
        let mut result = vec![];
        let system = energy(&vec![0.1; 5 * RATE]);
        append_words(
            &mut result,
            &units,
            299 * RATE,
            300 * RATE,
            600 * RATE,
            None,
            Some(&system),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "Una frase completa.");
        assert_eq!(result[0].start_ms, 300250);
        assert_eq!(result[0].end_ms, 303500);
        assert_eq!(result[0].speaker, "Speaker");
        // Checkpoints retain both levels and can reuse segment-only results.
        let cached: Transcript =
            serde_json::from_slice(&serde_json::to_vec(&response).unwrap()).unwrap();
        assert_eq!(timed_units(&cached, 5.).unwrap()[0].end, 4.5);
    }
    #[test]
    fn invalid_word_alignment_uses_valid_segments_but_never_fabricates_times() {
        for words in [
            r#"[{"word":"test","start":2,"end":1}]"#,
            r#"[{"word":"test","start":0,"end":50}]"#,
            r#"[{"word":"a","start":2,"end":3},{"word":"b","start":1,"end":2}]"#,
        ] {
            let mut response: Transcript = serde_json::from_str(&format!(
                r#"{{
                "text":"test",
                "words":{words},
                "segments":[{{"text":"test","start":0.5,"end":3.5}}]
            }}"#
            ))
            .unwrap();
            assert_eq!(timed_units(&response, 4.).unwrap()[0].start, 0.5);
            response.segments[0].end = 50.;
            assert_eq!(timed_units(&response,4.).unwrap().quality,Some(TimingQuality::UnalignedText));
        }
    }
    #[test]
    fn valid_word_alignment_is_preferred_and_old_checkpoints_remain_readable() {
        let mut response: Transcript = serde_json::from_str(
            r#"{
            "text":"test",
            "words":[{"word":"test","start":0.25,"end":1}]
        }"#,
        )
        .unwrap();
        response.segments.push(Word {
            word: "test".into(),
            start: 0.,
            end: 2.,
        });
        assert_eq!(timed_units(&response, 3.).unwrap()[0].start, 0.25);
        let silent: Transcript = serde_json::from_str(r#"{"text":""}"#).unwrap();
        assert!(timed_units(&silent, 3.).unwrap().is_empty());
    }
    #[test]
    fn final_segment_padding_recovers_text_when_word_times_go_backwards() {
        let seconds=173.5094375;
        let response=Transcript {
            text:"Prima frase. Finale sintetico.".into(),
            words:vec![Word{word:"Prima".into(),start:98.78,end:119.98},Word{word:"finale".into(),start:81.754715,end:81.97472}],
            segments:vec![Word{word:"Prima frase.".into(),start:0.,end:6.82},Word{word:"Finale sintetico.".into(),start:171.75471,end:201.73471}],
        };
        assert!(!valid_timestamps(&response.words,seconds));
        assert!(!valid_timestamps(&response.segments,seconds));
        let units=timed_units(&response,seconds).unwrap();
        assert_eq!(units.quality,Some(TimingQuality::EndClipped));
        assert_eq!(units.last().unwrap().end,seconds);
        assert_eq!(response.segments.last().unwrap().end,201.73471);
        let mut result=vec![];
        append_words(&mut result,&units,4499*RATE,4500*RATE,(4672.5094375*RATE as f64) as usize,None,None);
        assert!(result.iter().any(|segment|segment.text.contains("Finale sintetico.")));
        assert!(result.iter().all(|segment|segment.end_ms<=4_672_510));
    }
    #[test]
    fn tail_repair_does_not_accept_bad_prefixes_or_large_timestamp_errors() {
        let mut response=Transcript{text:"Testo sintetico".into(),words:vec![],segments:vec![Word{word:"Testo sintetico".into(),start:10.,end:204.}]};
        assert_eq!(timed_units(&response,173.).unwrap().quality,Some(TimingQuality::UnalignedText));
        response.segments[0].end=201.;response.segments[0].start=174.;
        assert_eq!(timed_units(&response,173.).unwrap().quality,Some(TimingQuality::UnalignedText));
        response.segments[0].start=-1.;
        assert_eq!(timed_units(&response,173.).unwrap().quality,Some(TimingQuality::UnalignedText));
        response.segments=vec![Word{word:"Prima".into(),start:20.,end:21.},Word{word:"Seconda".into(),start:10.,end:201.}];
        assert_eq!(timed_units(&response,173.).unwrap().quality,Some(TimingQuality::UnalignedText));
    }
    #[test]
    fn formerly_unaligned_checkpoint_can_be_recovered_without_a_new_request() {
        let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("groq-tail-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let complete=root.join("chunk.json");
        let failed=timestamp_failure_path(&complete);
        let response=Transcript{text:"Finale sintetico".into(),words:vec![],segments:vec![Word{word:"Finale sintetico".into(),start:171.75,end:201.73}]};
        save_checkpoint(&failed,&response).unwrap();
        let saved:Transcript=serde_json::from_slice(&fs::read(&failed).unwrap()).unwrap();
        assert!(checkpoint_response(&complete,&saved,173.51).unwrap());
        assert!(complete.is_file());assert!(!failed.exists());
        assert_eq!(timed_units(&saved,173.51).unwrap()[0].word,response.text);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn upload_trims_only_trailing_silence_and_keeps_quiet_speech_and_inner_pauses() {
        let mut samples=vec![0.;10*RATE];
        samples[RATE/2]=0.001;
        samples[3*RATE-1]=0.00011;
        let original=samples.clone();
        assert_eq!(trim_trailing_silence(&mut samples),10*RATE-(3*RATE+RATE/2));
        assert_eq!(samples,&original[..3*RATE+RATE/2]);
        assert_eq!(original.len(),10*RATE);
        let mut ongoing=vec![0.;RATE];ongoing[RATE-1]=0.00011;
        assert_eq!(trim_trailing_silence(&mut ongoing),0);
    }
    #[test]
    fn unaligned_text_is_never_dropped_by_short_chunk_overlap_and_requires_audio_retention() {
        let response=Transcript{text:"Tutto il finale resta disponibile.".into(),words:vec![],segments:vec![]};
        let mut result=vec![];
        assert!(append_response(&mut result,&response,1.1,299*RATE,300*RATE,300*RATE+RATE/10,None,None).unwrap());
        assert_eq!(result.len(),1);assert_eq!(result[0].text,response.text);
        assert_eq!(result[0].start_ms,299000);assert_eq!(result[0].end_ms,300100);
        assert!(result[0].quality.as_deref().unwrap().contains("Tempi puntuali non disponibili"));
    }
    #[test]
    fn text_can_be_recovered_from_units_even_when_top_level_text_is_missing() {
        let response=Transcript{text:"".into(),words:vec![Word{word:"Prima".into(),start:3.,end:2.},Word{word:"dopo".into(),start:1.,end:1.5}],segments:vec![]};
        let units=timed_units(&response,4.).unwrap();
        assert_eq!(units.quality,Some(TimingQuality::UnalignedText));
        assert_eq!(units[0].word,"Prima dopo");
    }
    #[test]
    fn multipart_preserves_binary_audio_and_omits_auto_language() {
        let settings = Settings::default();
        let body = multipart(&settings, &[0, 255, 1], "synthetic-boundary", false);
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"timestamp_granularities[]\"\r\n\r\nword\r\n"));
        assert!(text.contains("name=\"timestamp_granularities[]\"\r\n\r\nsegment\r\n"));
        assert!(!text.contains("name=\"language\""));
        assert!(body.windows(3).any(|bytes| bytes == [0, 255, 1]));
        assert!(body.ends_with(b"\r\n--synthetic-boundary--\r\n"));
    }
    #[test]
    fn explicit_alignment_retry_requests_only_segment_timestamps() {
        let body=multipart(&Settings::default(),b"audio","retry-boundary",true);
        let text=String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"timestamp_granularities[]\"\r\n\r\nsegment\r\n"));
        assert!(!text.contains("name=\"timestamp_granularities[]\"\r\n\r\nword\r\n"));
        assert!(text.contains(MODEL));
    }
    #[test]
    fn interrupted_long_call_error_names_the_missing_interval_without_transcript_text() {
        let error=chunk_error("Timestamp non disponibili.",15,75*60*RATE,80*60*RATE,20);
        assert!(error.contains("parte 16/20"));
        assert!(error.contains("01:15:00–01:20:00"));
        assert!(error.contains("incompleta"));
        assert!(error.contains("parti precedenti sono conservate"));
    }
    #[test]
    fn unaligned_checkpoint_preserves_text_without_becoming_a_completed_chunk() {
        let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("groq-unaligned-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let completed=root.join("chunk.json");
        let failed=timestamp_failure_path(&completed);
        let response=Transcript{text:"".into(),words:vec![],segments:vec![Word{word:" ".into(),start:2.,end:1.}]};
        assert!(!checkpoint_response(&completed,&response,300.).unwrap());
        assert!(!completed.exists());
        assert!(failed.is_file());
        let preserved:Transcript=serde_json::from_slice(&fs::read(failed).unwrap()).unwrap();
        assert_eq!(preserved.text,response.text);
        assert!(validate(&preserved,300.).is_err());
        // A later successful chunk remains usable even with this earlier gap.
        let good=Transcript{text:"Finale sintetico".into(),words:vec![Word{word:"Finale sintetico".into(),start:0.,end:2.}],segments:vec![]};
        let tail=root.join("tail.json");
        assert!(checkpoint_response(&tail,&good,300.).unwrap());
        assert!(tail.is_file());
        assert!(ensure_all_parts_completed(&["parte 16/20".into()]).is_err());
        // Explicit retry fills the gap and removes only its failed alignment.
        assert!(checkpoint_response(&completed,&good,300.).unwrap());
        assert!(!timestamp_failure_path(&completed).exists());
        assert!(ensure_all_parts_completed(&[]).is_ok());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn checkpoint_is_durable_and_partial_write_does_not_replace_it() {
        let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        fs::create_dir_all(&parent).unwrap();
        let parent = parent.canonicalize().unwrap();
        let root = parent.join(format!("parlo-groq-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("checkpoint.json");
        let response = Transcript {
            text: "sintetico".into(),
            segments: vec![],
            words: vec![Word {
                word: "sintetico".into(),
                start: 0.,
                end: 1.,
            }],
        };
        save_checkpoint(&path, &response).unwrap();
        // Simulate interruption while writing a later response: the completed
        // response must still be readable after reopening its file.
        fs::write(path.with_extension("part"), b"{interrupted").unwrap();
        let recovered: Transcript = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        validate(&recovered, 1.).unwrap();
        assert_eq!(recovered.words[0].word, "sintetico");
        assert_eq!(
            root.canonicalize().unwrap().parent(),
            Some(parent.as_path())
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn source_labels_follow_each_input_channel() {
        let response = Transcript {
            text: "test".into(),
            segments: vec![],
            words: vec![Word {
                word: "test".into(),
                start: 0.,
                end: 0.5,
            }],
        };
        let mic = energy(&vec![0.1; RATE]);
        let silence = energy(&vec![0.; RATE]);
        let mut result = vec![];
        append_words(
            &mut result,
            &response.words,
            0,
            0,
            RATE,
            Some(&mic),
            Some(&silence),
        );
        assert_eq!(result[0].speaker, "Tu");
        result.clear();
        append_words(
            &mut result,
            &response.words,
            0,
            0,
            RATE,
            Some(&silence),
            Some(&mic),
        );
        assert_eq!(result[0].speaker, "Speaker");
    }
    #[test]
    fn generated_wav_fits_free_upload_limit_and_preserves_format() {
        let bytes = wav_bytes(&vec![0.1; CHUNK + 2 * OVERLAP]).unwrap();
        assert!(bytes.len() < 25_000_000);
        let reader = hound::WavReader::new(Cursor::new(bytes)).unwrap();
        assert_eq!(reader.spec().sample_rate, 16000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.duration() as usize, CHUNK + 2 * OVERLAP);
    }
}
