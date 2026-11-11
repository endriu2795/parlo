//! Optional Telegram inbox. The desktop client polls Telegram directly.
use crate::{db, import_media, locale::tr, LockExt, Result, Shared};
use reqwest::blocking::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

const FILE_LIMIT: u64 = 20 * 1024 * 1024;
const INVALID_MEDIA: &str = "Invia un messaggio vocale, un file audio o un video.";
const LINK_CHANGED: &str = "Il collegamento Telegram è cambiato.";

#[derive(Clone)]
struct Bot {
    id: i64,
    username: String,
    token: String,
}

enum Destination {
    Pairing { code: String, expires: Instant },
    Linked { chat: i64 },
}

struct Session {
    bot: Bot,
    destination: Destination,
}

#[derive(Default)]
pub struct Telegram {
    session: Mutex<Option<Arc<Session>>>,
    wake: Condvar,
    error: Mutex<Option<String>>,
    // Serialize pairing, disconnect and explicit outbound sends.
    edit: Mutex<()>,
}

#[derive(Serialize)]
pub struct TelegramStatus {
    username: Option<String>,
    pairing_code: Option<String>,
    connected: bool,
    pending: i64,
    error: Option<String>,
}

fn client() -> Result<Client> {
    Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(35))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Impossibile inizializzare Telegram.".into())
}

fn keyring() -> Result<keyring::Entry> {
    keyring::Entry::new("PARLO.Telegram", "bot-token")
        .map_err(|_| "Portachiavi di sistema non disponibile.".into())
}

fn valid_token(token: &str) -> bool {
    token.len() <= 160
        && token.split_once(':').is_some_and(|(id, secret)| {
            !id.is_empty()
                && id.bytes().all(|c| c.is_ascii_digit())
                && !secret.is_empty()
                && secret
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        })
}

#[derive(Debug)]
enum ApiError {
    Unavailable,
    Rejected(u16),
}

impl From<ApiError> for String {
    fn from(error: ApiError) -> Self {
        // HTTP errors can contain the token in the request URL. Never expose them.
        match error {
            ApiError::Rejected(401 | 404) => tr(
                "Token del bot Telegram non valido o revocato.",
                "Invalid or revoked Telegram bot token.",
            ),
            ApiError::Rejected(409) => tr(
                "Il bot è già in uso. Usa un bot dedicato a PARLO.",
                "The bot is already in use. Use a dedicated bot for PARLO.",
            ),
            ApiError::Rejected(429) => tr(
                "Telegram ha limitato le richieste. Riprovo tra poco.",
                "Telegram rate-limited the requests. Retrying shortly.",
            ),
            ApiError::Rejected(_) => tr(
                "Telegram ha rifiutato la richiesta.",
                "Telegram rejected the request.",
            ),
            ApiError::Unavailable => {
                tr("Telegram non è raggiungibile.", "Telegram is unreachable.")
            }
        }
        .into()
    }
}

fn endpoint(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{token}/{method}")
}

fn api_reply(request: reqwest::blocking::RequestBuilder) -> std::result::Result<Value, ApiError> {
    let response = request.send().map_err(|_| ApiError::Unavailable)?;
    if !response.status().is_success() {
        return Err(ApiError::Rejected(response.status().as_u16()));
    }
    let data: Value = serde_json::from_reader(response.take(2 * 1024 * 1024))
        .map_err(|_| ApiError::Unavailable)?;
    if data["ok"].as_bool() != Some(true) {
        return Err(ApiError::Rejected(
            data["error_code"].as_u64().unwrap_or(400) as u16,
        ));
    }
    Ok(data["result"].clone())
}

fn request(
    client: &Client,
    token: &str,
    method: &str,
    data: Value,
) -> std::result::Result<Value, ApiError> {
    api_reply(
        client
            .post(endpoint(token, method))
            .header("Content-Type", "application/json")
            .body(data.to_string()),
    )
}

fn reply(client: &Client, session: &Session, chat: i64, text: &str) {
    let _ = api_reply(
        client
            .post(endpoint(&session.bot.token, "sendMessage"))
            .timeout(Duration::from_secs(6))
            .header("Content-Type", "application/json")
            .body(json!({"chat_id":chat,"text":text}).to_string()),
    );
}

fn main_window(window: &WebviewWindow) -> Result<()> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("Telegram è disponibile dalla finestra principale.".into())
    }
}

fn is_current(runtime: &Telegram, session: &Arc<Session>) -> bool {
    runtime
        .session
        .guard()
        .as_ref()
        .is_some_and(|current| Arc::ptr_eq(current, session))
}

fn status(runtime: &Telegram, db: &Connection) -> Result<TelegramStatus> {
    let session = runtime.session.guard().clone();
    let mut output = TelegramStatus {
        username: session.as_ref().map(|s| s.bot.username.clone()),
        pairing_code: None,
        connected: false,
        pending: 0,
        error: runtime.error.guard().clone(),
    };
    if let Some(session) = session {
        match &session.destination {
            Destination::Pairing { code, .. } => output.pairing_code = Some(code.clone()),
            Destination::Linked { chat } => {
                output.connected = true;
                output.pending = db.query_row(
                    "SELECT count(*) FROM telegram_inbox WHERE bot_id=?1 AND chat_id=?2 AND file_id IS NOT NULL",
                    params![session.bot.id, chat], |row| row.get(0),
                ).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(output)
}

#[tauri::command]
pub fn telegram_status(window: WebviewWindow, state: State<'_, Shared>) -> Result<TelegramStatus> {
    main_window(&window)?;
    status(&window.app_handle().state::<Telegram>(), &state.db.guard())
}

#[tauri::command]
pub async fn telegram_connect(window: WebviewWindow, token: String) -> Result<()> {
    main_window(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let runtime = window.app_handle().state::<Telegram>();
        let _edit = runtime.edit.guard();
        if runtime.session.guard().is_some() {
            return Err("Scollega il bot corrente prima di collegarne un altro.".into());
        }
        let token = token.trim();
        if !valid_token(token) {
            return Err(tr(
                "Inserisci il token ricevuto da BotFather.",
                "Enter the token you received from BotFather.",
            )
            .into());
        }
        let client = client()?;
        let me = request(&client, token, "getMe", json!({}))?;
        let id = me["id"].as_i64().ok_or("Identificatore bot non valido.")?;
        let username = me["username"]
            .as_str()
            .ok_or("Nome del bot non disponibile.")?;
        let hook = request(&client, token, "getWebhookInfo", json!({}))?;
        if hook["url"].as_str().is_some_and(|url| !url.is_empty()) {
            return Err(tr(
                "Questo bot usa già un webhook. Crea un bot dedicato a PARLO.",
                "This bot already uses a webhook. Create a dedicated bot for PARLO.",
            )
            .into());
        }
        let code = uuid::Uuid::new_v4().simple().to_string()[..12].to_uppercase();
        *runtime.session.guard() = Some(Arc::new(Session {
            bot: Bot {
                id,
                username: username.into(),
                token: token.into(),
            },
            destination: Destination::Pairing {
                code,
                expires: Instant::now() + Duration::from_secs(600),
            },
        }));
        *runtime.error.guard() = None;
        runtime.wake.notify_one();
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn telegram_disconnect(window: WebviewWindow, state: State<'_, Shared>) -> Result<()> {
    main_window(&window)?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let runtime = window.app_handle().state::<Telegram>();
        let _edit = runtime.edit.guard();
        let mut db = state.db.guard();
        let tx = db.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM telegram_link", [])
            .map_err(|e| e.to_string())?;
        match keyring()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err("Impossibile rimuovere il token dal portachiavi.".into()),
        }
        tx.commit().map_err(|e| e.to_string())?;
        *runtime.session.guard() = None;
        *runtime.error.guard() = None;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

fn restore(runtime: &Telegram, db: &Connection) -> Result<()> {
    let row: Option<(i64, String, i64)> = db
        .query_row(
            "SELECT bot_id,username,chat_id FROM telegram_link WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some((id, username, chat)) = row {
        let token = keyring()?.get_password().map_err(|_| {
            tr(
                "Token Telegram non disponibile. Collega di nuovo il bot.",
                "Telegram token unavailable. Connect the bot again.",
            )
        })?;
        if !valid_token(&token) {
            return Err("Token del bot Telegram non valido.".into());
        }
        *runtime.session.guard() = Some(Arc::new(Session {
            bot: Bot {
                id,
                username,
                token,
            },
            destination: Destination::Linked { chat },
        }));
    }
    Ok(())
}

fn link(runtime: &Telegram, state: &Shared, session: &Arc<Session>, chat: i64) -> Result<()> {
    let _edit = runtime.edit.guard();
    if !is_current(runtime, session) {
        return Err(LINK_CHANGED.into());
    }
    let bot = &session.bot;
    let entry = keyring()?;
    entry
        .set_password(&bot.token)
        .map_err(|_| "Impossibile salvare il token nel portachiavi.")?;
    let saved = state.db.guard().execute(
        "INSERT OR REPLACE INTO telegram_link(id,bot_id,username,chat_id) VALUES(1,?1,?2,?3)",
        params![bot.id, bot.username, chat],
    );
    if let Err(error) = saved {
        let _ = entry.delete_credential();
        return Err(error.to_string());
    }
    *runtime.session.guard() = Some(Arc::new(Session {
        bot: bot.clone(),
        destination: Destination::Linked { chat },
    }));
    Ok(())
}

struct Media<'a> {
    file_id: &'a str,
    extension: String,
    title: String,
    size: Option<u64>,
}

fn media(message: &Value) -> Option<Media<'_>> {
    // Telegram can expose a silent animation as both animation and document.
    if message.get("animation").is_some() {
        return None;
    }
    let (file, fallback) = [
        ("voice", "ogg"),
        ("audio", "mp3"),
        ("video", "mp4"),
        ("video_note", "mp4"),
        ("document", ""),
    ]
    .into_iter()
    .find_map(|(kind, ext)| {
        message
            .get(kind)
            .filter(|v| v.is_object())
            .map(|v| (v, ext))
    })?;
    let name = file["file_name"].as_str().map(Path::new);
    let extension = name
        .and_then(Path::extension)
        .and_then(|s| s.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|ext| import_media::supported_extension(ext));
    if fallback.is_empty()
        && !file["mime_type"].as_str().is_none_or(|mime| {
            mime.starts_with("audio/")
                || mime.starts_with("video/")
                || mime == "application/octet-stream"
        })
    {
        return None;
    }
    let extension = extension.or_else(|| (!fallback.is_empty()).then(|| fallback.to_owned()))?;
    let title = name
        .and_then(Path::file_stem)
        .and_then(|s| s.to_str())
        .map(|s| {
            s.chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect::<String>()
        })
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let date = message["date"]
                .as_i64()
                .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
                .map(|date| date.with_timezone(&chrono::Local))
                .unwrap_or_else(chrono::Local::now);
            format!(
                "{} {}",
                tr("Trascrizione Telegram del", "Telegram transcript from"),
                date.format("%d/%m/%Y, %H:%M")
            )
        });
    Some(Media {
        file_id: file["file_id"].as_str().filter(|id| !id.is_empty())?,
        extension,
        title,
        size: file["file_size"].as_u64(),
    })
}

fn receive(
    client: &Client,
    runtime: &Telegram,
    state: &Shared,
    session: &Arc<Session>,
    update: &Value,
) -> Result<()> {
    let message = &update["message"];
    let Some(chat) = message["chat"]["id"].as_i64() else {
        return Ok(());
    };
    if message["chat"]["type"].as_str() != Some("private")
        || message["from"]["id"].as_i64() != Some(chat)
    {
        return Ok(());
    }
    match &session.destination {
        Destination::Pairing { code, expires } => {
            if Instant::now() < *expires
                && message["text"]
                    .as_str()
                    .is_some_and(|text| text.trim() == code)
            {
                link(runtime, state, session, chat)?;
                reply(
                    client,
                    session,
                    chat,
                    tr(
                        "PARLO collegato. Invia un vocale, un audio o un video.",
                        "PARLO connected. Send a voice message, audio file, or video.",
                    ),
                );
            }
        }
        Destination::Linked { chat: allowed } if *allowed == chat => {
            let _edit = runtime.edit.guard();
            if !is_current(runtime, session) {
                return Ok(());
            }
            let Some(media) = media(message) else {
                reply(
                    client,
                    session,
                    chat,
                    tr(INVALID_MEDIA, "Send a voice message, audio file, or video."),
                );
                return Ok(());
            };
            if media.size.is_some_and(|size| size > FILE_LIMIT) {
                reply(
                    client,
                    session,
                    chat,
                    tr(
                        "Il file supera 20 MB. Importalo direttamente in PARLO.",
                        "The file exceeds 20 MB. Import it directly in PARLO.",
                    ),
                );
                return Ok(());
            }
            let Some(update_id) = update["update_id"].as_i64() else {
                return Ok(());
            };
            let inserted = {
                let db = state.db.guard();
                db.execute("DELETE FROM telegram_inbox WHERE file_id IS NULL AND received_at < unixepoch()-172800", [])
                    .map_err(|e| e.to_string())?;
                db.execute(
                    "INSERT OR IGNORE INTO telegram_inbox(bot_id,chat_id,update_id,call_id,file_id,extension,title) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![session.bot.id, chat, update_id, uuid::Uuid::new_v4().to_string(), media.file_id, media.extension, media.title],
                ).map_err(|e| e.to_string())?
            };
            if inserted == 1 {
                reply(
                    client,
                    session,
                    chat,
                    tr(
                        "Ricevuto. Il file è in coda su PARLO.",
                        "Received. The file is queued in PARLO.",
                    ),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

struct Job {
    id: String,
    file: String,
    extension: String,
    title: String,
}

fn download(client: &Client, token: &str, job: &Job, dir: &Path) -> Result<Option<PathBuf>> {
    if uuid::Uuid::parse_str(&job.id).is_err() || !import_media::supported_extension(&job.extension)
    {
        return Ok(None);
    }
    let path = dir.join(format!("{}.{}", job.id, job.extension));
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
        return Ok(Some(path));
    }
    let metadata = match request(client, token, "getFile", json!({"file_id":job.file})) {
        Ok(metadata) => metadata,
        Err(ApiError::Rejected(400 | 413)) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let Some(remote) = metadata["file_path"].as_str() else {
        return Ok(None);
    };
    if metadata["file_size"]
        .as_u64()
        .is_some_and(|size| size > FILE_LIMIT)
    {
        return Ok(None);
    }
    let mut url =
        reqwest::Url::parse("https://api.telegram.org/file/").expect("fixed Telegram origin");
    {
        let mut segments = url.path_segments_mut().expect("hierarchical Telegram URL");
        segments.pop_if_empty().push(&format!("bot{token}"));
        for segment in remote.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Ok(None);
            }
            segments.push(segment);
        }
    }
    let response = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|_| String::from(ApiError::Unavailable))?;
    if response
        .content_length()
        .is_some_and(|size| size > FILE_LIMIT)
    {
        return Ok(None);
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let partial = path.with_extension("part");
    let _ = fs::remove_file(&partial);
    let mut output = fs::File::create_new(&partial).map_err(|e| e.to_string())?;
    let copied = io::copy(&mut response.take(FILE_LIMIT + 1), &mut output);
    drop(output);
    if matches!(copied, Ok(size) if size > 0 && size <= FILE_LIMIT) {
        fs::rename(&partial, &path).map_err(|e| e.to_string())?;
        return Ok(Some(path));
    }
    let _ = fs::remove_file(&partial);
    copied.map_err(|e| e.to_string()).map(|_| None)
}

fn complete(db: &Connection, id: &str) -> Result<()> {
    db.execute(
        "UPDATE telegram_inbox SET file_id=NULL,title='',extension='' WHERE call_id=?1",
        [id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn process(
    app: &AppHandle,
    state: &Shared,
    runtime: &Telegram,
    client: &Client,
    session: &Arc<Session>,
) -> Result<()> {
    let Destination::Linked { chat } = session.destination else {
        return Ok(());
    };
    if state.status.guard().capture.is_some()
        || state.provision.0.guard().running
        || crate::updates::ensure_not_installing(app).is_err()
    {
        return Ok(());
    }
    let job = {
        let db = state.db.guard();
        db.query_row(
            "SELECT call_id,file_id,extension,title FROM telegram_inbox WHERE bot_id=?1 AND chat_id=?2 AND file_id IS NOT NULL ORDER BY received_at, update_id LIMIT 1",
            params![session.bot.id, chat], |row| Ok(Job { id: row.get(0)?, file: row.get(1)?, extension: row.get(2)?, title: row.get(3)? }),
        ).optional().map_err(|e| e.to_string())?
    };
    let Some(job) = job else {
        return Ok(());
    };
    let settings = db::get_settings(&state.db.guard())?;
    if crate::stt::ensure_ready(&settings).is_err() {
        return Ok(());
    }
    let dir = state.data_dir.join("telegram-incoming");
    let Some(path) = download(client, &session.bot.token, &job, &dir)? else {
        complete(&state.db.guard(), &job.id)?;
        if is_current(runtime, session) {
            reply(client, session, chat, tr("File Telegram non disponibile o troppo grande. Importalo direttamente in PARLO.", "The Telegram file is unavailable or too large. Import it directly in PARLO."));
        }
        return Ok(());
    };
    if !is_current(runtime, session) {
        return Ok(());
    }
    // A crash before the SQLite commit can leave a partially converted WAV.
    crate::remove_audio(state, &job.id)?;
    let result = import_media::import_file(app, state, &path, &job.id, &job.title, "telegram", |db| {
        if !is_current(runtime, session) {
            return Err(LINK_CHANGED.into());
        }
        complete(db, &job.id)
    });
    if let Err(error) = result {
        if error == import_media::RECORDING_BUSY
            || error == LINK_CHANGED
            || crate::updates::ensure_not_installing(app).is_err()
        {
            return Ok(());
        }
        complete(&state.db.guard(), &job.id)?;
        crate::changed(app, state);
        let _ = app.emit("app-error", format!("Telegram: {error}"));
        if is_current(runtime, session) {
            reply(
                client,
                session,
                chat,
                tr(
                    "Impossibile elaborare il file. Controlla PARLO sul computer.",
                    "Could not process the file. Check PARLO on your computer.",
                ),
            );
        }
    }
    let _ = fs::remove_file(path);
    Ok(())
}

pub fn start(app: AppHandle, state: Shared) {
    thread::spawn(move || {
        let runtime = app.state::<Telegram>();
        {
            let _edit = runtime.edit.guard();
            if runtime.session.guard().is_none() {
                if let Err(error) = restore(&runtime, &state.db.guard()) {
                    *runtime.error.guard() = Some(error);
                }
            }
        }
        let client = match client() {
            Ok(client) => client,
            Err(error) => {
                *runtime.error.guard() = Some(error);
                return;
            }
        };
        loop {
            let session = runtime
                .wake
                .wait_while(runtime.session.guard(), |session| session.is_none())
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .expect("Telegram worker waits for a session")
                .clone();
            if matches!(session.destination, Destination::Pairing { expires, .. } if Instant::now() >= expires)
            {
                let _edit = runtime.edit.guard();
                if is_current(&runtime, &session) {
                    *runtime.session.guard() = None;
                    *runtime.error.guard() = Some(
                        tr(
                            "Codice scaduto. Ricollega il bot.",
                            "Code expired. Reconnect the bot.",
                        )
                        .into(),
                    );
                }
                continue;
            }
            let cycle = || -> Result<()> {
                let updates = request(
                    &client,
                    &session.bot.token,
                    "getUpdates",
                    json!({"limit":25,"timeout":8,"allowed_updates":["message"]}),
                )?;
                let mut offset = None;
                for update in updates.as_array().into_iter().flatten() {
                    if !is_current(&runtime, &session) {
                        break;
                    }
                    receive(&client, &runtime, &state, &session, update)?;
                    offset = update["update_id"]
                        .as_i64()
                        .and_then(|id| id.checked_add(1));
                }
                if let Some(offset) = offset {
                    request(
                        &client,
                        &session.bot.token,
                        "getUpdates",
                        json!({"offset":offset,"limit":1,"timeout":0,"allowed_updates":["message"]}),
                    )?;
                }
                if is_current(&runtime, &session) {
                    process(&app, &state, &runtime, &client, &session)?;
                }
                Ok(())
            };
            let outcome = cycle();
            if is_current(&runtime, &session) {
                *runtime.error.guard() = outcome.as_ref().err().cloned();
            }
            if outcome.is_err() {
                thread::sleep(Duration::from_secs(5));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_audio_and_video_attachments() {
        for message in [
            json!({"text":"https://example.test/video"}),
            json!({"photo":[{"file_id":"photo"}]}),
            json!({"sticker":{"file_id":"sticker"}}),
            json!({"animation":{"file_id":"gif"},"document":{"file_id":"gif","file_name":"loop.mp4","mime_type":"video/mp4"}}),
            json!({"document":{"file_id":"pdf","file_name":"letter.pdf"}}),
        ] {
            assert!(media(&message).is_none());
        }
        for (message, extension) in [
            (json!({"voice":{"file_id":"voice"}}), "ogg"),
            (json!({"video_note":{"file_id":"video-note"}}), "mp4"),
            (
                json!({"video":{"file_id":"video","file_name":"Call.MOV"}}),
                "mov",
            ),
            (
                json!({"document":{"file_id":"audio","file_name":"Call.flac","mime_type":"audio/flac"}}),
                "flac",
            ),
        ] {
            assert_eq!(media(&message).unwrap().extension, extension);
        }
    }

    #[test]
    fn bot_tokens_cannot_change_the_request_url() {
        assert!(valid_token("123456:abc_-DEF"));
        for token in [
            "token",
            "1:/secret",
            "1:secret?method",
            "1:secret#token",
            "1:secret\n",
        ] {
            assert!(!valid_token(token));
        }
    }
}
