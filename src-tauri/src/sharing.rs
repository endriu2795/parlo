use crate::{db, model, LockExt, Result, Shared};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::{HashMap, HashSet}, fs, io::{BufRead, BufReader, Read, Write}, path::{Path, PathBuf}, sync::Mutex, time::Duration};
use tauri::Emitter;
use uuid::Uuid;

const HEADER: &str = "<!-- PARLO-SHARE-V1 ";
const MAX_HEADER: u64 = 8192;
const MAX_TRANSCRIPT: u64 = 8 * 1024 * 1024;
const DESTINATIONS: [(&str, &str); 3] = [("folder", "Drive / OneDrive"), ("slack", "Slack"), ("teams", "Teams")];
static FOLDER_WRITE: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Sharing {
    pub name: String,
    pub folder: String,
    pub slack: bool,
    pub teams: bool,
    pub auto_folder: bool,
    pub auto_slack: bool,
    pub auto_teams: bool,
    pub read_shared: bool,
    pub expire_with_archive: bool,
}

impl Sharing {
    fn automatic(&self, destination: &str) -> bool {
        match destination {
            "folder" => self.auto_folder && !self.folder.is_empty(),
            "slack" => self.slack && self.auto_slack,
            "teams" => self.teams && self.auto_teams,
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct SharedTranscript {
    id: String,
    publisher_id: String,
    publisher: String,
    title: String,
    created_at: String,
    published_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duration_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct SharedDocument {
    transcript: SharedTranscript,
    markdown: String,
    segments: Vec<model::Segment>,
    duration_ms: i64,
}

fn filename_part(value: &str, max_bytes: usize, fallback: &str) -> String {
    let safe: String = value.chars().map(|ch| {
        if ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.') { ch } else { ' ' }
    }).collect();
    let mut safe = safe.split_whitespace().collect::<Vec<_>>().join(" ");
    safe.truncate(safe.floor_char_boundary(max_bytes));
    let safe = safe.trim_matches([' ', '.']);
    if safe.is_empty() { fallback.to_owned() } else { safe.to_owned() }
}

fn filename(info: &SharedTranscript, id: Uuid) -> Result<String> {
    let date = chrono::DateTime::parse_from_rfc3339(&info.created_at)
        .map_err(|_| "Data della trascrizione non valida.")?.with_timezone(&chrono::Local);
    let publisher = filename_part(&info.publisher, 48, "PARLO");
    let title = filename_part(&info.title, 72, "Trascrizione");
    Ok(format!("{} - {publisher} - {title} [parlo-{id}].md", date.format("%Y-%m-%d %H-%M")))
}

fn publication_id(name: &str) -> Option<Uuid> {
    let raw = if let Some((_, suffix)) = name.rsplit_once(" [parlo-") {
        suffix.strip_suffix("].md")?
    } else {
        name.strip_prefix("parlo-")?.strip_suffix(".md")?
    };
    let id = Uuid::parse_str(raw).ok()?;
    (id.to_string() == raw).then_some(id)
}

fn publications(folder: &Path) -> Result<impl Iterator<Item = (Uuid, PathBuf)>> {
    let entries = fs::read_dir(folder).map_err(|_| "Impossibile leggere la cartella condivisa.")?;
    Ok(entries.filter_map(|entry| {
        let entry = entry.ok()?;
        let id = publication_id(entry.file_name().to_str()?)?;
        Some((id, entry.path()))
    }))
}

fn webhook(destination: &str) -> Result<keyring::Entry> {
    if !matches!(destination, "slack" | "teams") {
        return Err("Canale non supportato.".into());
    }
    keyring::Entry::new("Parlo.Sharing", destination).map_err(|_| "Portachiavi non disponibile.".into())
}

fn main_window(window: &tauri::WebviewWindow) -> Result<()> {
    if window.label() != "main" { return Err("Apri la finestra principale di PARLO.".into()); }
    Ok(())
}

#[tauri::command]
pub fn sharing_webhook(window: tauri::WebviewWindow, destination: String, url: Option<String>) -> Result<bool> {
    main_window(&window)?;
    let entry = webhook(&destination)?;
    if let Some(url) = url {
        let url = reqwest::Url::parse(url.trim()).map_err(|_| "URL webhook non valido.")?;
        if url.scheme() != "https" || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
            return Err("Usa l’URL HTTPS del webhook.".into());
        }
        entry.set_password(url.as_str()).map_err(|_| "Impossibile salvare il webhook nel portachiavi.")?;
        return Ok(true);
    }
    match entry.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::Error::NoEntry) => Ok(false),
        Err(_) => Err("Impossibile leggere il portachiavi.".into()),
    }
}

#[tauri::command]
pub async fn sharing_pick_folder(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<Option<PathBuf>> {
    main_window(&window)?;
    Ok(crate::sources::pick(app, window, true, false).await?.into_iter().next())
}

fn save_text(folder: &Path, info: &SharedTranscript, text: &str) -> Result<()> {
    let id = Uuid::parse_str(&info.id).map_err(|_| "Identificatore call non valido.")?;
    if !folder.is_absolute() || !folder.is_dir() { return Err("La cartella di condivisione non è disponibile.".into()); }
    let folder = folder.canonicalize().map_err(|_| "La cartella di condivisione non è disponibile.")?;
    let existing = publications(&folder)?.find_map(|(candidate, path)| {
        if candidate != id { return None; }
        let (previous, _) = shared_header(&folder, &path, id)?;
        (previous.publisher_id == info.publisher_id).then_some((path, previous))
    });
    let target = match &existing {
        Some((path, previous)) if previous.title == info.title => path.clone(),
        _ => folder.join(filename(info, id)?),
    };
    write_publication(&target, existing.as_ref().map(|(path, _)| path.as_path()), info, text)
}

fn write_publication(target: &Path, previous: Option<&Path>, info: &SharedTranscript, text: &str) -> Result<()> {
    let header = STANDARD.encode(serde_json::to_vec(info).map_err(|e| e.to_string())?);
    if header.len() as u64 + HEADER.len() as u64 + 5 > MAX_HEADER || text.len() as u64 > MAX_TRANSCRIPT {
        return Err("La trascrizione è troppo grande per la cartella condivisa.".into());
    }
    if previous != Some(target) && target.symlink_metadata().is_ok() {
        let same_file = previous.and_then(|path| path.canonicalize().ok())
            .is_some_and(|path| target.canonicalize().is_ok_and(|resolved| resolved == path));
        if !same_file {
            return Err("Esiste già un file con questo nome nella cartella condivisa.".into());
        }
    }
    let partial = target.with_file_name(format!(".parlo-{}.part", Uuid::new_v4()));
    let mut pending = std::fs::File::create_new(&partial).map_err(|_| "Impossibile scrivere nella cartella di condivisione.")?;
    let written = writeln!(pending, "{HEADER}{header} -->\n").and_then(|_| pending.write_all(text.as_bytes()));
    drop(pending);
    let result = written.and_then(|_| {
        if let Some(previous) = previous.filter(|path| *path != target) {
            // Move the original first so a failed replacement leaves a single readable copy.
            fs::rename(previous, target)?;
        }
        fs::rename(&partial, target)
    });
    if result.is_err() { let _ = std::fs::remove_file(partial); }
    result.map_err(|_| "Impossibile aggiornare il file nella cartella condivisa.".into())
}

fn shared_header(folder: &Path, path: &Path, id: Uuid) -> Option<(SharedTranscript, BufReader<fs::File>)> {
    if !fs::symlink_metadata(path).ok()?.is_file() { return None; }
    let path = path.canonicalize().ok()?;
    if path.parent()? != folder { return None; }
    let mut reader = BufReader::new(fs::File::open(path).ok()?);
    let mut line = String::new();
    (&mut reader).take(MAX_HEADER).read_line(&mut line).ok()?;
    let encoded = line.trim_end().strip_prefix(HEADER)?.strip_suffix(" -->")?;
    let info: SharedTranscript = serde_json::from_slice(&STANDARD.decode(encoded).ok()?).ok()?;
    if info.id != id.to_string() || Uuid::parse_str(&info.publisher_id).is_err()
        || chrono::DateTime::parse_from_rfc3339(&info.published_at).is_err()
        || chrono::DateTime::parse_from_rfc3339(&info.created_at).is_err() {
        return None;
    }
    Some((info, reader))
}

fn peer_publications(folder: &Path, own: Uuid, only: Option<Uuid>) -> Result<impl Iterator<Item = (SharedTranscript, BufReader<fs::File>)> + '_> {
    let own = own.to_string();
    Ok(publications(folder)?.filter_map(move |(id, path)| {
        if only.is_some_and(|only| only != id) { return None; }
        let (info, reader) = shared_header(folder, &path, id)?;
        (info.publisher_id != own).then_some((info, reader))
    }))
}

fn sync_titles(folder: &Path, own: Uuid, titles: &HashMap<String, String>) -> Result<()> {
    if titles.is_empty() { return Ok(()); }
    let folder = match folder.canonicalize() {
        Ok(folder) => folder,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("La cartella di condivisione non è disponibile.".into()),
    };
    let own = own.to_string();
    let mut failed = false;
    for (id, path) in publications(&folder)? {
        let Some(title) = titles.get(&id.to_string()) else { continue; };
        let Some((mut info, reader)) = shared_header(&folder, &path, id) else { continue; };
        if info.publisher_id != own || info.title == *title { continue; }
        let result = (|| {
            let mut markdown = String::new();
            reader.take(MAX_TRANSCRIPT + 1).read_to_string(&mut markdown)
                .map_err(|_| "Trascrizione condivisa non leggibile.")?;
            if markdown.len() as u64 > MAX_TRANSCRIPT { return Err("La trascrizione condivisa è troppo grande.".into()); }
            let heading = format!("\n# {}", info.title);
            markdown = if markdown.contains(&format!("{heading}\n")) {
                markdown.replacen(&format!("{heading}\n"), &format!("\n# {title}\n"), 1)
            } else if markdown.contains(&format!("{heading}\r\n")) {
                markdown.replacen(&format!("{heading}\r\n"), &format!("\n# {title}\r\n"), 1)
            } else {
                format!("# {title}\n\n{}", markdown.trim_start_matches(['\r', '\n']))
            };
            info.title = title.clone();
            let target = folder.join(filename(&info, id)?);
            write_publication(&target, Some(&path), &info, markdown.trim_start_matches(['\r', '\n']))
        })();
        failed |= result.is_err();
    }
    if failed { return Err("Il titolo locale è stato salvato. Alcuni file condivisi verranno aggiornati al prossimo controllo.".into()); }
    Ok(())
}

pub fn schedule_title_sync(app: &tauri::AppHandle, state: &Shared, id: Option<String>) {
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let _write = FOLDER_WRITE.guard();
            let (folder, own, titles) = {
                let conn = state.db.guard();
                let folder = db::get_settings(&conn)?.sharing.folder;
                if folder.is_empty() { return Ok(()); }
                let path = Path::new(&folder);
                if !path.is_absolute() { return Err("La cartella di condivisione non è disponibile.".into()); }
                (folder, db::publisher_id(&conn)?, db::call_titles(&conn, id.as_deref())?)
            };
            sync_titles(Path::new(&folder), own, &titles)
        })();
        if let Err(error) = result { let _ = app.emit("app-error", error); }
    });
}

fn expire_publications(folder: &Path, own: Uuid, cutoff: chrono::DateTime<chrono::Utc>, active: &HashSet<String>) -> Result<()> {
    if !folder.is_absolute() { return Err("La cartella di condivisione non è disponibile.".into()); }
    let folder = match folder.canonicalize() {
        Ok(folder) => folder,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("La cartella di condivisione non è disponibile.".into()),
    };
    let mut failed = false;
    let own = own.to_string();
    for (id, path) in publications(&folder)? {
        let Some((info, reader)) = shared_header(&folder, &path, id) else { continue; };
        if info.publisher_id != own || active.contains(&info.id) { continue; }
        let Ok(created) = chrono::DateTime::parse_from_rfc3339(&info.created_at) else { continue; };
        if created >= cutoff { continue; }
        drop(reader);
        if let Err(error) = fs::remove_file(&path) {
            if error.kind() != std::io::ErrorKind::NotFound { failed = true; }
        }
    }
    if failed { return Err("Alcune pubblicazioni scadute non sono state eliminate. Verifica i permessi della cartella condivisa.".into()); }
    Ok(())
}

pub fn schedule_expiration(app: &tauri::AppHandle, state: &Shared) {
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let _write = FOLDER_WRITE.guard();
            if state.status.guard().chat_busy { return Ok(()); }
            let (folder, own, cutoff, active) = {
                let conn = state.db.guard();
                let config = db::get_settings(&conn)?;
                if !config.sharing.expire_with_archive || config.sharing.folder.is_empty() { return Ok(()); }
                let Some(cutoff) = db::retention_cutoff(config.retention_months)? else { return Ok(()); };
                (config.sharing.folder, db::publisher_id(&conn)?, cutoff, db::active_call_ids(&conn)?)
            };
            expire_publications(Path::new(&folder), own, cutoff, &active)
        })();
        if let Err(error) = result {
            let _ = app.emit("app-error", format!("Scadenza delle pubblicazioni: {error}"));
        }
    });
}

fn shared_folder(state: &Shared) -> Result<Option<(PathBuf, Uuid)>> {
    let (folder, own) = {
        let conn = state.db.guard();
        let settings = db::get_settings(&conn)?.sharing;
        if !settings.read_shared || settings.folder.is_empty() { return Ok(None); }
        (settings.folder, db::publisher_id(&conn)?)
    };
    let path = Path::new(&folder);
    if !path.is_absolute() { return Err("La cartella di condivisione non è disponibile.".into()); }
    Ok(path.canonicalize().ok().filter(|path| path.is_dir()).map(|path| (path, own)))
}

#[tauri::command]
pub async fn list_shared_transcripts(window: tauri::WebviewWindow, state: tauri::State<'_, Shared>) -> Result<Vec<SharedTranscript>> {
    main_window(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some((folder, own)) = shared_folder(&state)? else { return Ok(vec![]); };
        let mut results = peer_publications(&folder, own, None)?.map(|(info, _)| info).collect::<Vec<_>>();
        results.sort_by(|a, b| b.published_at.cmp(&a.published_at).then_with(|| a.id.cmp(&b.id)));
        let mut seen = HashSet::new();
        results.retain(|info| seen.insert(info.id.clone()));
        Ok(results)
    }).await.map_err(|_| "Lettura cartella interrotta.".to_string())?
}

#[tauri::command]
pub async fn get_shared_transcript(window: tauri::WebviewWindow, state: tauri::State<'_, Shared>, id: String) -> Result<SharedDocument> {
    main_window(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        read_shared_document(&state, &id)
    }).await.map_err(|_| "Lettura trascrizione interrotta.".to_string())?
}

fn read_shared_document(state: &Shared, id: &str) -> Result<SharedDocument> {
    let id = Uuid::parse_str(id).map_err(|_| "Identificativo non valido.")?;
    let (folder, own) = shared_folder(state)?.ok_or("La cartella condivisa non è disponibile.")?;
    let (transcript, reader) = peer_publications(&folder, own, Some(id))?
        .max_by(|(a, _), (b, _)| a.published_at.cmp(&b.published_at))
        .ok_or("La trascrizione condivisa non è più disponibile.")?;
    let mut markdown = String::new();
    reader.take(MAX_TRANSCRIPT + 1).read_to_string(&mut markdown).map_err(|_| "Trascrizione condivisa non leggibile.")?;
    if markdown.len() as u64 > MAX_TRANSCRIPT { return Err("La trascrizione condivisa è troppo grande.".into()); }
    let segments = shared_segments(&markdown);
    let duration_ms = transcript.duration_ms.filter(|duration| *duration >= 0).unwrap_or_else(|| {
        let minutes = markdown.lines().take_while(|line| !line.starts_with("**["))
            .find_map(|line| line.strip_prefix("Durata: ")?.strip_suffix(" minuti")?.parse::<i64>().ok())
            .and_then(|minutes| minutes.checked_mul(60_000)).unwrap_or(0);
        minutes.max(segments.iter().map(|segment| segment.end_ms).max().unwrap_or(0))
    });
    Ok(SharedDocument { transcript, markdown, segments, duration_ms })
}

pub(crate) async fn shared_markdown_export(state: Shared, id: String) -> Result<(String, String)> {
    tauri::async_runtime::spawn_blocking(move || {
        let document = read_shared_document(&state, &id)?;
        Ok((crate::export::filename(&document.transcript.title), document.markdown))
    }).await.map_err(|_| "Lettura trascrizione interrotta.".to_string())?
}

fn shared_segments(markdown: &str) -> Vec<model::Segment> {
    let mut segments: Vec<model::Segment> = Vec::new();
    for line in markdown.lines() {
        let header = line.strip_prefix("**[").and_then(|line| line.strip_suffix("**"));
        let parsed = header.and_then(|line| {
            let (time, speaker) = line.split_once("] ")?;
            let (minutes, seconds) = time.split_once(':')?;
            let minutes = minutes.parse::<i64>().ok()?;
            let seconds = seconds.parse::<i64>().ok()?;
            if minutes < 0 || !(0..60).contains(&seconds) { return None; }
            Some((minutes.checked_mul(60)?.checked_add(seconds)?.checked_mul(1000)?, speaker))
        });
        if let Some((start_ms, speaker)) = parsed {
            segments.push(model::Segment { start_ms, end_ms: start_ms.saturating_add(1000),
                speaker: speaker.to_owned(), text: String::new(), source: "shared".into(), quality: None, original_text: None });
        } else if let Some(segment) = segments.last_mut() {
            if !line.trim().is_empty() {
                if !segment.text.is_empty() { segment.text.push('\n'); }
                segment.text.push_str(line);
            }
        }
    }
    segments.retain(|segment| !segment.text.trim().is_empty());
    if segments.is_empty() && !markdown.trim().is_empty() {
        segments.push(model::Segment { start_ms: 0, end_ms: 1000, speaker: "Speaker".into(),
            text: markdown.trim().into(), source: "shared".into(), quality: None, original_text: None });
    }
    segments
}

#[tauri::command]
pub async fn open_shared_assistant(app: tauri::AppHandle, window: tauri::WebviewWindow, state: tauri::State<'_, Shared>, id: String) -> Result<String> {
    main_window(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        use rusqlite::OptionalExtension;
        let document = read_shared_document(&state, &id)?;
        let _guard = state.operation.guard();
        let mut conn = state.db.guard();
        let existing: Option<String> = conn.query_row(
            "SELECT call_id FROM shared_chat_links WHERE source_id=?1 AND publisher_id=?2",
            [&document.transcript.id, &document.transcript.publisher_id], |row| row.get(0),
        ).optional().map_err(|e| e.to_string())?;
        if let Some(id) = existing { return Ok(id); }
        let title = format!("{} · {}", document.transcript.title, document.transcript.publisher);
        let local_id = db::create_shared_chat(&mut conn, &document.transcript.id, &document.transcript.publisher_id, &title)?;
        drop(conn);
        crate::changed(&app, &state);
        Ok(local_id)
    }).await.map_err(|_| "Apertura della trascrizione condivisa interrotta.".to_string())?
}

pub fn hydrate_shared_detail(state: &Shared, detail: &mut model::Detail) -> Result<()> {
    if detail.call.origin != "shared_chat" { return Ok(()); }
    let source: String = state.db.guard().query_row(
        "SELECT source_id FROM shared_chat_links WHERE call_id=?1", [&detail.call.id], |row| row.get(0),
    ).map_err(|_| "Chat della trascrizione condivisa non disponibile.")?;
    let document = read_shared_document(state, &source)?;
    detail.call.title = format!("{} · {}", document.transcript.title, document.transcript.publisher);
    detail.call.created_at = document.transcript.created_at;
    detail.segments = document.segments;
    detail.call.duration_ms = document.duration_ms;
    Ok(())
}

fn send_channel(destination: &str, text: &str) -> Result<String> {
    let url = webhook(destination)?.get_password().map_err(|_| "Salva prima il webhook nelle impostazioni.")?;
    let fingerprint = webhook_fingerprint(&url);
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(25))
        .build().map_err(|_| "Client HTTP non disponibile.")?;
    let payload = if destination == "slack" {
        if text.chars().count() > 39_000 { return Err("Trascrizione troppo lunga per un messaggio Slack. Puoi pubblicarla nella cartella.".into()); }
        json!({"text": text, "mrkdwn": false, "unfurl_links": false, "unfurl_media": false})
    } else {
        json!({"type":"message","attachments":[{"contentType":"application/vnd.microsoft.card.adaptive","contentUrl":null,
            "content":{"type":"AdaptiveCard","version":"1.2","body":[{"type":"TextBlock","text":text,"wrap":true}]}}]})
    };
    let bytes = serde_json::to_vec(&payload).map_err(|_| "Messaggio non valido.")?;
    if destination == "teams" && bytes.len() > 27_000 { return Err("Trascrizione troppo lunga per un messaggio Teams. Puoi pubblicarla nella cartella.".into()); }
    let response = client.post(url).header("Content-Type", "application/json").body(bytes)
        .send().map_err(|_| "Il webhook non ha risposto. Verifica il canale prima di riprovare.")?;
    if !response.status().is_success() { return Err(format!("Il webhook ha risposto HTTP {}.", response.status().as_u16())); }
    Ok(fingerprint)
}

fn webhook_fingerprint(url: &str) -> String {
    format!("{:x}", Sha256::digest(url.as_bytes()))
}

fn record_channel(conn: &rusqlite::Connection, id: &str, destination: &str, fingerprint: &str) -> Result<()> {
    conn.execute("INSERT INTO channel_publications(call_id,destination,webhook_hash,published_at) VALUES(?1,?2,?3,?4) ON CONFLICT(call_id,destination,webhook_hash) DO UPDATE SET published_at=excluded.published_at",
        rusqlite::params![id, destination, fingerprint, chrono::Utc::now().to_rfc3339()])
        .map_err(|_| "Trascrizione inviata, ma stato di condivisione non salvato.".to_string())?;
    Ok(())
}

#[derive(Serialize)]
pub struct PublicationStatus {
    destinations: Vec<String>,
    folder_unavailable: bool,
}

fn own_folder_publication(path: &Path, call_id: Uuid, own: &str) -> Result<bool> {
    if !path.is_absolute() { return Err("Cartella non valida.".into()); }
    let folder = path.canonicalize().map_err(|_| "Cartella non disponibile.")?;
    Ok(publications(&folder)?.any(|(candidate, path)| candidate == call_id
        && shared_header(&folder, &path, candidate).is_some_and(|(info, _)| info.publisher_id == own)))
}

#[tauri::command]
pub async fn publication_status(window: tauri::WebviewWindow, state: tauri::State<'_, Shared>, id: String) -> Result<PublicationStatus> {
    main_window(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let call_id = Uuid::parse_str(&id).map_err(|_| "Identificativo non valido.")?;
        let (sharing, own) = {
            let conn = state.db.guard();
            let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM calls WHERE id=?1 AND origin != 'shared_chat')", [&id], |row| row.get(0)).map_err(|e| e.to_string())?;
            if !exists { return Err("Registrazione non trovata.".into()); }
            let sharing = db::get_settings(&conn)?.sharing;
            let own = if sharing.folder.is_empty() { String::new() } else { db::publisher_id(&conn)?.to_string() };
            (sharing, own)
        };
        let mut status = PublicationStatus { destinations: Vec::new(), folder_unavailable: false };
        if !sharing.folder.is_empty() {
            match own_folder_publication(Path::new(&sharing.folder), call_id, &own) {
                Ok(true) => status.destinations.push("folder".into()),
                Ok(false) => {},
                Err(_) => status.folder_unavailable = true,
            }
        }
        for destination in ["slack", "teams"] {
            let recorded: bool = state.db.guard().query_row("SELECT EXISTS(SELECT 1 FROM channel_publications WHERE call_id=?1 AND destination=?2)",
                rusqlite::params![id, destination], |row| row.get(0)).map_err(|e| e.to_string())?;
            if !recorded { continue; }
            let url = match webhook(destination)?.get_password() {
                Ok(url) => url,
                Err(keyring::Error::NoEntry) => continue,
                Err(_) => return Err("Impossibile verificare lo stato del canale nel portachiavi.".into()),
            };
            let exists: bool = state.db.guard().query_row("SELECT EXISTS(SELECT 1 FROM channel_publications WHERE call_id=?1 AND destination=?2 AND webhook_hash=?3)",
                rusqlite::params![id, destination, webhook_fingerprint(&url)], |row| row.get(0)).map_err(|e| e.to_string())?;
            if exists { status.destinations.push(destination.into()); }
        }
        Ok(status)
    }).await.map_err(|_| "Verifica condivisione interrotta.".to_string())?
}

fn prepare(conn: &rusqlite::Connection, sharing: &Sharing, id: &str) -> Result<(SharedTranscript, String)> {
    let detail = db::detail(conn, id)?;
    if detail.call.status != "ready" || detail.segments.is_empty() {
        return Err("La trascrizione non è ancora pronta.".into());
    }
    let publisher_id = db::publisher_id(conn)?.to_string();
    let name = sharing.name.trim();
    let publisher = if name.is_empty() {
        format!("PARLO {}", &publisher_id[..8])
    } else {
        name.to_owned()
    };
    let text = format!("Pubblicato da {publisher}\n\n{}", model::markdown(&detail.call, &detail.segments));
    let info = SharedTranscript {
        id: detail.call.id, publisher_id, publisher, title: detail.call.title,
        created_at: detail.call.created_at, published_at: chrono::Utc::now().to_rfc3339(),
        duration_ms: Some(detail.call.duration_ms),
    };
    Ok((info, text))
}

fn publish_folder(state: &Shared, id: &str, automatic_folder: Option<&str>) -> Result<()> {
    let _write = FOLDER_WRITE.guard();
    let (config, info, text) = {
        let conn = state.db.guard();
        let config = db::get_settings(&conn)?;
        if let Some(folder) = automatic_folder {
            if !config.sharing.automatic("folder") || folder != config.sharing.folder { return Ok(()); }
        }
        if config.sharing.folder.is_empty() { return Err("Destinazione disattivata.".into()); }
        let (info, text) = prepare(&conn, &config.sharing, id)?;
        (config, info, text)
    };
    if config.sharing.expire_with_archive {
        let created = chrono::DateTime::parse_from_rfc3339(&info.created_at).map_err(|_| "Data della trascrizione non valida.")?;
        if db::retention_cutoff(config.retention_months)?.is_some_and(|cutoff| created < cutoff) {
            return Err("La trascrizione ha superato la scadenza dell’archivio.".into());
        }
    }
    save_text(Path::new(&config.sharing.folder), &info, &text)
}

fn publish_channel(sharing: &Sharing, text: &str, destination: &str) -> Result<String> {
    match destination {
        "slack" if sharing.slack => send_channel(destination, text),
        "teams" if sharing.teams => send_channel(destination, text),
        _ => Err("Destinazione disattivata.".into()),
    }
}

pub fn auto_publish(app: &tauri::AppHandle, state: &Shared, id: &str) {
    let job = (|| {
        let conn = state.db.guard();
        let sharing = db::get_settings(&conn)?.sharing;
        if !DESTINATIONS.iter().any(|(destination, _)| sharing.automatic(destination)) { return Ok(None); }
        let (info, text) = prepare(&conn, &sharing, id)?;
        Ok::<_, String>(Some((sharing, info, text)))
    })();
    let (sharing, info, text) = match job {
        Ok(Some(job)) => job,
        Ok(None) => return,
        Err(error) => { let _ = app.emit("app-error", format!("Pubblicazione automatica: {error}")); return; }
    };
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut errors = vec![];
        for (destination, label) in DESTINATIONS {
            if !sharing.automatic(destination) { continue; }
            if destination == "folder" {
                if let Err(error) = publish_folder(&state, &info.id, Some(&sharing.folder)) {
                    errors.push(format!("{label}: {error}"));
                }
                continue;
            }
            let current = db::get_settings(&state.db.guard());
            match current {
                Ok(current) if current.sharing.automatic(destination) => {
                    match publish_channel(&current.sharing, &text, destination) {
                        Ok(fingerprint) => if let Err(error) = record_channel(&state.db.guard(), &info.id, destination, &fingerprint) { errors.push(format!("{label}: {error}")); },
                        Err(error) => errors.push(format!("{label}: {error}")),
                    }
                }
                Err(error) => errors.push(format!("{label}: {error}")),
                _ => {}
            }
        }
        let _ = app.emit("publication-changed", &info.id);
        if !errors.is_empty() {
            let _ = app.emit("app-error", format!("Pubblicazione automatica di «{}». {}", info.title, errors.join(" ")));
        }
    });
}

#[tauri::command]
pub async fn publish_transcript(window: tauri::WebviewWindow, state: tauri::State<'_, Shared>, id: String, destination: String) -> Result<()> {
    main_window(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if destination == "folder" { return publish_folder(&state, &id, None); }
        let (sharing, text) = {
            let conn = state.db.guard();
            let sharing = db::get_settings(&conn)?.sharing;
            let (_, text) = prepare(&conn, &sharing, &id)?;
            (sharing, text)
        };
        let fingerprint = publish_channel(&sharing, &text, &destination)?;
        record_channel(&state.db.guard(), &id, &destination, &fingerprint)
    }).await.map_err(|_| "Pubblicazione interrotta.".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publication_status_tracks_successful_channel_and_existing_owned_file() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("sharing-status-{}", Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let id = Uuid::new_v4();
        let own = Uuid::new_v4();
        let other = Uuid::new_v4();
        let info = SharedTranscript { id: id.to_string(), publisher_id: own.to_string(), publisher: "Test".into(), title: "Prova".into(),
            created_at: "2026-01-20T10:00:00Z".into(), published_at: "2026-01-20T10:05:00Z".into(), duration_ms: None };
        assert!(!own_folder_publication(&folder, id, &own.to_string()).unwrap());
        save_text(&folder, &info, "Trascrizione fittizia").unwrap();
        assert!(own_folder_publication(&folder, id, &own.to_string()).unwrap());
        assert!(!own_folder_publication(&folder, id, &other.to_string()).unwrap());
        let mut conn = db::open(Path::new(":memory:")).unwrap();
        db::create(&conn, &id.to_string(), "Prova", &info.created_at).unwrap();
        let fingerprint = webhook_fingerprint("https://example.com/synthetic");
        record_channel(&conn, &id.to_string(), "slack", &fingerprint).unwrap();
        record_channel(&conn, &id.to_string(), "slack", &fingerprint).unwrap();
        let count: usize = conn.query_row("SELECT count(*) FROM channel_publications WHERE call_id=?1 AND destination='slack' AND webhook_hash=?2", rusqlite::params![id.to_string(), fingerprint], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
        db::delete(&mut conn, &id.to_string()).unwrap();
        let count: usize = conn.query_row("SELECT count(*) FROM channel_publications", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
        fs::remove_file(folder.join(filename(&info, id).unwrap())).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn shared_chat_stores_no_transcript_and_stays_out_of_archive() {
        let text = "Pubblicato da Collega\n\n# Call\n\n**[00:00] Tu**\n\nPrimo passaggio.\n\n**[00:19] Speaker**\n\nSecondo passaggio.\n";
        let segments = shared_segments(text);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].start_ms, 19_000);
        assert_eq!(segments[1].text, "Secondo passaggio.");
        let mut conn = db::open(Path::new(":memory:")).unwrap();
        let source = Uuid::new_v4().to_string();
        let publisher = Uuid::new_v4().to_string();
        let id = db::create_shared_chat(&mut conn, &source, &publisher, "Call · Collega").unwrap();
        let detail = db::detail(&conn, &id).unwrap();
        assert_eq!(detail.call.origin, "shared_chat");
        assert!(detail.segments.is_empty());
        assert_eq!(db::page_filtered(&conn, "", 0, 10, None, None, &[]).unwrap().items.len(), 0);
        db::delete(&mut conn, &id).unwrap();
        let count: i64 = conn.query_row("SELECT count(*) FROM shared_chat_links", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn shared_chat_reads_current_file_without_storing_its_transcript() {
        use std::sync::{Arc, Mutex};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("parlo-shared-chat-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let state = Arc::new(crate::AppState {
            db: Mutex::new(db::open(Path::new(":memory:")).unwrap()),
            status: Default::default(), recorder: Mutex::new(None), operation: Mutex::new(()),
            data_dir: root.clone(), queue: Default::default(), engine: Mutex::new(None),
            live_engine: Mutex::new(None), live_process: Mutex::new(None), final_process: Mutex::new(None),
            processing_cancel: Default::default(), chat_control: Default::default(),
            shortcut_capture: Default::default(), provision: Default::default(),
            auth: Default::default(), codex_auth: Default::default(), gemini_auth: Default::default(),
            catalog: Mutex::new(None),
        });
        let id = Uuid::new_v4();
        let peer = Uuid::new_v4();
        let info = SharedTranscript { id: id.to_string(), publisher_id: peer.to_string(),
            publisher: "Collega".into(), title: "Call sintetica".into(),
            created_at: "2026-01-20T10:00:00Z".into(), published_at: "2026-01-20T10:05:00Z".into(), duration_ms: Some(12_345) };
        let chat_id = {
            let mut conn = state.db.guard();
            let mut settings = db::get_settings(&conn).unwrap();
            settings.sharing.folder = root.to_string_lossy().into();
            settings.sharing.read_shared = true;
            db::save_settings(&conn, &settings).unwrap();
            db::create_shared_chat(&mut conn, &info.id, &info.publisher_id, "Call sintetica · Collega").unwrap()
        };
        save_text(&root, &info, "# Call sintetica\n\n**[00:00] Speaker**\n\nPrimo testo.").unwrap();
        let mut detail = db::detail(&state.db.guard(), &chat_id).unwrap();
        hydrate_shared_detail(&state, &mut detail).unwrap();
        assert_eq!(detail.segments[0].text, "Primo testo.");
        assert_eq!(detail.call.created_at, info.created_at);
        assert_eq!(detail.call.duration_ms, 12_345);
        assert!(crate::sources::context(&state, &detail).unwrap().contains("Primo testo."));
        save_text(&root, &info, "# Call sintetica\n\n**[00:00] Speaker**\n\nTesto aggiornato.").unwrap();
        hydrate_shared_detail(&state, &mut detail).unwrap();
        assert_eq!(detail.segments[0].text, "Testo aggiornato.");
        assert!(db::detail(&state.db.guard(), &chat_id).unwrap().segments.is_empty());
        fs::remove_file(root.join(filename(&info, id).unwrap())).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn archive_expiration_removes_only_own_expired_publications() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("sharing-expiration-{}", Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let own = Uuid::new_v4();
        let peer = Uuid::new_v4();
        let cutoff = "2026-04-01T00:00:00Z".parse().unwrap();
        let mut active = HashSet::new();
        let mut files = vec![];
        for (publisher, created_at, is_active, deleted) in [
            (own, "2026-03-01T00:00:00Z", false, true),
            (own, "2026-03-01T00:00:00Z", true, false),
            (peer, "2026-03-01T00:00:00Z", false, false),
            (own, "2026-04-01T00:00:00Z", false, false),
        ] {
            let id = Uuid::new_v4();
            let info = SharedTranscript {
                id: id.to_string(), publisher_id: publisher.to_string(), publisher: "Test".into(),
                title: "Trascrizione sintetica".into(), created_at: created_at.into(),
                published_at: "2026-04-20T00:00:00Z".into(),
                duration_ms: None,
            };
            if is_active { active.insert(info.id.clone()); }
            save_text(&folder, &info, "Contenuto sintetico").unwrap();
            files.push((folder.join(filename(&info, id).unwrap()), deleted));
        }
        let unrelated = folder.join(format!("parlo-{}.md", Uuid::new_v4()));
        fs::write(&unrelated, "Documento senza intestazione PARLO").unwrap();
        for _ in 0..2 {
            expire_publications(&folder, own, cutoff, &active).unwrap();
        }
        for (file, deleted) in files {
            assert_eq!(!file.exists(), deleted);
            if !deleted { fs::remove_file(file).unwrap(); }
        }
        fs::remove_file(unrelated).unwrap();
        fs::remove_dir(folder).unwrap();
    }

    #[test]
    fn sharing_folder_round_trip_excludes_own_and_unrelated_files() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join(format!("sharing-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let folder = folder.canonicalize().unwrap();
        let owner = Uuid::new_v4();
        let colleague = Uuid::new_v4();
        let id = Uuid::new_v4();
        let info = SharedTranscript {
            id: id.to_string(), publisher_id: owner.to_string(), publisher: "Giulia · PC".into(),
            title: "Riunione sintetica".into(), created_at: "2026-01-20T10:00:00Z".into(),
            published_at: "2026-01-20T10:05:00Z".into(),
            duration_ms: None,
        };
        save_text(&folder, &info, "Pubblicato da Giulia · PC\n\n# Riunione sintetica\n\nTest.").unwrap();
        let name = filename(&info, id).unwrap();
        assert!(name.contains(" - Giulia PC - Riunione sintetica [parlo-"));
        assert!(folder.join(&name).is_file());
        let titles = HashMap::from([(id.to_string(), "Nuovo titolo".into())]);
        sync_titles(&folder, colleague, &titles).unwrap();
        assert!(folder.join(&name).is_file());
        for _ in 0..2 {
            sync_titles(&folder, owner, &titles).unwrap();
        }
        assert!(!folder.join(&name).exists());
        assert_eq!(publications(&folder).unwrap().count(), 1);
        let (renamed, mut reader) = peer_publications(&folder, colleague, Some(id)).unwrap().next().unwrap();
        assert_eq!(renamed.title, "Nuovo titolo");
        assert_eq!(renamed.published_at, info.published_at);
        let mut content = String::new();
        reader.read_to_string(&mut content).unwrap();
        assert_eq!(content.trim(), "Pubblicato da Giulia · PC\n\n# Nuovo titolo\n\nTest.");
        drop(reader);
        let renamed_path = folder.join(filename(&renamed, id).unwrap());
        assert!(renamed_path.is_file());
        let updated = SharedTranscript { title: "Nuovo titolo".into(), ..info };
        save_text(&folder, &updated, "# Trascrizione\n\nAggiornata.").unwrap();
        assert_eq!(publications(&folder).unwrap().count(), 1);
        assert!(peer_publications(&folder, owner, None).unwrap().next().is_none());
        let (received, mut reader) = peer_publications(&folder, colleague, Some(id)).unwrap().next().unwrap();
        assert_eq!(received.publisher, "Giulia · PC");
        assert_eq!(received.title, "Nuovo titolo");
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text.trim(), "# Trascrizione\n\nAggiornata.");
        drop(reader);
        let legacy = folder.join(format!("parlo-{id}.md"));
        fs::rename(renamed_path, &legacy).unwrap();
        assert!(peer_publications(&folder, colleague, Some(id)).unwrap().next().is_some());
        let unrelated = Uuid::new_v4();
        let unrelated = folder.join(format!("parlo-{unrelated}.md"));
        fs::write(&unrelated, "Documento senza intestazione PARLO").unwrap();
        assert_eq!(peer_publications(&folder, colleague, None).unwrap().count(), 1);
        fs::remove_file(legacy).unwrap();
        fs::remove_file(unrelated).unwrap();
        fs::remove_dir(folder).unwrap();
    }
}
