use crate::model::*;
use chrono::{Months, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
type Result<T> = std::result::Result<T, String>;
fn err(e: rusqlite::Error) -> String {
    e.to_string()
}

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).map_err(err)?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL; PRAGMA secure_delete=ON;
        CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), json TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS sharing_identity (id INTEGER PRIMARY KEY CHECK(id=1), publisher TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS calls (
          id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL,
          duration_ms INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL, error TEXT);
        CREATE TABLE IF NOT EXISTS channel_publications (
          call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
          destination TEXT NOT NULL, webhook_hash TEXT NOT NULL,
          published_at TEXT NOT NULL, PRIMARY KEY(call_id,destination,webhook_hash));
        CREATE TABLE IF NOT EXISTS shared_chat_links (
          source_id TEXT NOT NULL, publisher_id TEXT NOT NULL,
          call_id TEXT NOT NULL UNIQUE REFERENCES calls(id) ON DELETE CASCADE,
          PRIMARY KEY(source_id, publisher_id));
        CREATE TABLE IF NOT EXISTS segments (
          id INTEGER PRIMARY KEY, call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
          start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, speaker TEXT NOT NULL,
          text TEXT NOT NULL, source TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS segments_call ON segments(call_id, start_ms);
        CREATE TABLE IF NOT EXISTS screenshots (
          id TEXT PRIMARY KEY, call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
          timestamp_ms INTEGER NOT NULL, created_at TEXT NOT NULL, width INTEGER NOT NULL, height INTEGER NOT NULL);
        CREATE INDEX IF NOT EXISTS screenshots_call ON screenshots(call_id, timestamp_ms);
        CREATE TABLE IF NOT EXISTS call_notes (
          id TEXT PRIMARY KEY, call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
          timestamp_ms INTEGER NOT NULL, text TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS call_notes_call ON call_notes(call_id, timestamp_ms);
        CREATE TABLE IF NOT EXISTS chat_sources (
          call_id TEXT PRIMARY KEY REFERENCES calls(id) ON DELETE CASCADE, json TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS shared_source (
          id INTEGER PRIMARY KEY CHECK(id=1), path TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS telegram_link (
          id INTEGER PRIMARY KEY CHECK(id=1), bot_id INTEGER NOT NULL,
          username TEXT NOT NULL, chat_id INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS telegram_inbox (
          bot_id INTEGER NOT NULL, chat_id INTEGER NOT NULL, update_id INTEGER NOT NULL,
          call_id TEXT NOT NULL UNIQUE, file_id TEXT, extension TEXT NOT NULL, title TEXT NOT NULL,
          received_at INTEGER NOT NULL DEFAULT (unixepoch()),
          PRIMARY KEY(bot_id, chat_id, update_id));
        CREATE TABLE IF NOT EXISTS generated_files (
          id TEXT PRIMARY KEY, call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
          kind TEXT NOT NULL, name TEXT NOT NULL, created_at TEXT NOT NULL,
          extension TEXT, artifact_url TEXT);
        CREATE TABLE IF NOT EXISTS messages (
          id INTEGER PRIMARY KEY, call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
          role TEXT NOT NULL, content TEXT NOT NULL, created_at TEXT);
        CREATE VIRTUAL TABLE IF NOT EXISTS call_search USING fts5(call_id UNINDEXED, title, body, tokenize='unicode61');").map_err(err)?;
    let has_origin: bool = conn.query_row(
        "SELECT count(*) > 0 FROM pragma_table_info('calls') WHERE name='origin'", [], |row| row.get(0),
    ).map_err(err)?;
    if !has_origin {
        conn.execute_batch("BEGIN;
            ALTER TABLE calls ADD COLUMN origin TEXT NOT NULL DEFAULT '';
            UPDATE calls SET origin='telegram' WHERE id IN (SELECT call_id FROM telegram_inbox)
              OR title='Registrazione Telegram' OR title LIKE 'Trascrizione Telegram del %';
            COMMIT;").map_err(err)?;
    }
    migrate_shared_sources(&conn)?;
    let has_model = {
        let mut stmt = conn.prepare("PRAGMA table_info(messages)").map_err(err)?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        columns.iter().any(|name| name == "model")
    };
    if !has_model {
        conn.execute("ALTER TABLE messages ADD COLUMN model TEXT", [])
            .map_err(err)?;
    }
    let has_activity: bool = conn
        .query_row(
            "SELECT count(*) > 0 FROM pragma_table_info('messages') WHERE name='activity_json'",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    if !has_activity {
        conn.execute(
            "ALTER TABLE messages ADD COLUMN activity_json TEXT NOT NULL DEFAULT '[]'",
            [],
        )
        .map_err(err)?;
    }
    let has_created_at: bool = conn.query_row(
        "SELECT count(*) > 0 FROM pragma_table_info('messages') WHERE name='created_at'",
        [], |r| r.get(0),
    ).map_err(err)?;
    if !has_created_at {
        // Older messages have no recoverable send time; keep them NULL.
        conn.execute("ALTER TABLE messages ADD COLUMN created_at TEXT", []).map_err(err)?;
    }
    for column in ["quality", "original_text"] {
        let present: bool = conn.query_row("SELECT count(*) > 0 FROM pragma_table_info('segments') WHERE name=?1", [column], |r|r.get(0)).map_err(err)?;
        if !present {conn.execute(&format!("ALTER TABLE segments ADD COLUMN {column} TEXT"), []).map_err(err)?;}
    }
    conn.execute_batch("CREATE TABLE IF NOT EXISTS capture_diagnostics (
        call_id TEXT PRIMARY KEY REFERENCES calls(id) ON DELETE CASCADE,
        json TEXT NOT NULL);").map_err(err)?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS call_tags (
        call_id TEXT NOT NULL REFERENCES calls(id) ON DELETE CASCADE,
        tag TEXT NOT NULL COLLATE NOCASE, PRIMARY KEY(call_id,tag));").map_err(err)?;
    setup_passage_search(&conn)?;
    // Interrupted operations stay recoverable, never pretend to be complete.
    conn.execute("UPDATE calls SET status='failed', error='Elaborazione interrotta. Puoi riprovare o eliminare la call.' WHERE status IN ('recording','pending','processing')", []).map_err(err)?;
    Ok(conn)
}
fn migrate_shared_sources(conn: &Connection) -> Result<()> {
    // Keep the legacy ID so per-chat exclusions of the folder and its children survive.
    conn.execute_batch("BEGIN;
        CREATE TABLE IF NOT EXISTS shared_sources (id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE);
        INSERT OR IGNORE INTO shared_sources(id,path) SELECT 'shared',path FROM shared_source WHERE id=1;
        DELETE FROM shared_source;
        COMMIT;").map_err(err)
}
fn setup_passage_search(conn: &Connection) -> Result<()> {
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='passage_search')", [], |row| row.get(0)).map_err(err)?;
    if exists { return Ok(()); }
    conn.execute_batch("BEGIN;
      CREATE VIRTUAL TABLE passage_search USING fts5(call_id UNINDEXED,source UNINDEXED,source_id UNINDEXED,timestamp_ms UNINDEXED,text,tokenize='unicode61');
      INSERT INTO passage_search SELECT call_id,'text',id,start_ms,text FROM segments;
      INSERT INTO passage_search SELECT call_id,'note',id,timestamp_ms,text FROM call_notes;
      CREATE TRIGGER passage_segment_insert AFTER INSERT ON segments BEGIN
        INSERT INTO passage_search(call_id,source,source_id,timestamp_ms,text) VALUES(new.call_id,'text',new.id,new.start_ms,new.text); END;
      CREATE TRIGGER passage_segment_delete AFTER DELETE ON segments BEGIN
        DELETE FROM passage_search WHERE source='text' AND source_id=old.id; END;
      CREATE TRIGGER passage_segment_update AFTER UPDATE ON segments BEGIN
        DELETE FROM passage_search WHERE source='text' AND source_id=old.id;
        INSERT INTO passage_search(call_id,source,source_id,timestamp_ms,text) VALUES(new.call_id,'text',new.id,new.start_ms,new.text); END;
      CREATE TRIGGER passage_note_insert AFTER INSERT ON call_notes BEGIN
        INSERT INTO passage_search(call_id,source,source_id,timestamp_ms,text) VALUES(new.call_id,'note',new.id,new.timestamp_ms,new.text); END;
      CREATE TRIGGER passage_note_delete AFTER DELETE ON call_notes BEGIN
        DELETE FROM passage_search WHERE source='note' AND source_id=old.id; END;
      CREATE TRIGGER passage_note_update AFTER UPDATE ON call_notes BEGIN
        DELETE FROM passage_search WHERE source='note' AND source_id=old.id;
        INSERT INTO passage_search(call_id,source,source_id,timestamp_ms,text) VALUES(new.call_id,'note',new.id,new.timestamp_ms,new.text); END;
      COMMIT;").map_err(err)
}
pub fn get_settings(conn: &Connection) -> Result<Settings> {
    let result = conn.query_row("SELECT json FROM settings WHERE id=1", [], |r| {
        r.get::<_, String>(0)
    });
    match result {
        Ok(s) => {
            let mut settings: Settings = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            settings.upgrade_ollama_context(&serde_json::from_str(&s).map_err(|e|e.to_string())?);
            settings.upgrade_ollama_web_policy(&serde_json::from_str(&s).map_err(|e|e.to_string())?);
            settings.upgrade_screen_shortcuts(&serde_json::from_str(&s).map_err(|e|e.to_string())?);
            settings.upgrade_platform_shortcuts();
            settings.resolve_at(&engine_dir())?;
            settings.diarization = false; // Legacy option: labels now follow input channels.
            // Keep legacy account settings, but never start the retired runtime.
            if settings.assistant_provider == "gemini" { settings.assistant_provider = "claude".into(); }
            Ok(settings)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Settings::default()),
        Err(e) => Err(err(e)),
    }
}
pub fn save_settings(conn: &Connection, s: &Settings) -> Result<()> {
    let mut portable = s.portable_at(&engine_dir());
    portable.diarization = false;
    conn.execute("INSERT INTO settings(id,json) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET json=excluded.json", [serde_json::to_string(&portable).map_err(|e| e.to_string())?]).map_err(err)?;
    Ok(())
}
pub fn upgrade_preview(conn: &Connection) -> Result<()> {
    let json = conn.query_row("SELECT json FROM settings WHERE id=1", [], |r| {
        r.get::<_, String>(0)
    });
    let Ok(json) = json else {
        return Ok(());
    };
    let value: serde_json::Value = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if value["live_pipeline_version"].as_u64().unwrap_or(0) >= 2 {
        return Ok(());
    }
    let mut settings = get_settings(conn)?;
    let defaults = Settings::default();
    let tiny = Path::new(&defaults.live_whisper_model);
    let old = tiny
        .parent()
        .unwrap_or(Path::new("."))
        .join("whisper-small");
    if Path::new(&settings.live_whisper_model) == old && tiny.join("model.bin").is_file() {
        settings.live_whisper_model = defaults.live_whisper_model;
    }
    settings.resolve_at(&engine_dir())?;
    settings.live_transcription = false;
    settings.live_pipeline_version = 2;
    save_settings(conn, &settings)
}
fn row_call(row: &rusqlite::Row<'_>) -> rusqlite::Result<Call> {
    Ok(Call {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        duration_ms: row.get(3)?,
        status: row.get(4)?,
        error: row.get(5)?,
        preview: row.get(6)?,
        origin: row.get(8)?,
        tags: serde_json::from_str(&row.get::<_,String>(7)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?,
    })
}
#[cfg(test)]
pub(crate) fn list(conn: &Connection, query: &str) -> Result<Vec<Call>> {
    Ok(page(conn, query, 0, 10000)?.items)
}
#[cfg(test)]
pub fn page(conn: &Connection, query: &str, offset: usize, limit: usize) -> Result<CallPage> {
    page_filtered(conn,query,offset,limit,None,None,&[])
}
/// Every tag in the archive with the number of calls using it, most used first.
pub fn tags(conn: &Connection) -> Result<Vec<TagCount>> {
    let mut stmt = conn.prepare("SELECT min(tag),count(*) FROM call_tags GROUP BY tag COLLATE NOCASE ORDER BY count(*) DESC,min(tag) COLLATE NOCASE").map_err(err)?;
    let rows = stmt.query_map([], |r| Ok(TagCount { tag: r.get(0)?, count: r.get(1)? })).map_err(err)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)
}
/// `tags` keeps calls that carry at least one of the given tags (case-insensitive).
pub fn page_filtered(conn: &Connection, query: &str, offset: usize, limit: usize, date_from: Option<&str>, date_to: Option<&str>, tags: &[String]) -> Result<CallPage> {
    let parse=|date:Option<&str>| date.map(chrono::DateTime::parse_from_rfc3339).transpose().map_err(|_|"Intervallo di date non valido".to_string());
    let from=parse(date_from)?;
    let to=parse(date_to)?;
    if from.zip(to).is_some_and(|(from,to)|from>=to) {return Err("La data iniziale deve precedere quella finale".into());}
    let q = query.trim();
    let select = "SELECT c.id,c.title,c.created_at,c.duration_ms,c.status,c.error,COALESCE((SELECT substr(text,1,120) FROM segments WHERE call_id=c.id ORDER BY start_ms LIMIT 1),''),COALESCE((SELECT json_group_array(tag) FROM call_tags WHERE call_id=c.id),'[]'),c.origin FROM calls c";
    let filter: String = if q.is_empty() {
        " WHERE 1=1".into()
    } else {
        " WHERE (c.id IN (SELECT call_id FROM passage_search WHERE passage_search MATCH ?1) OR c.id IN (SELECT call_id FROM call_search WHERE call_search MATCH ?3) OR instr(lower(c.title),lower(?2))>0 OR c.id IN (SELECT call_id FROM call_tags WHERE instr(lower(tag),lower(?2))>0))".into()
    };
    let filter=format!("{filter} AND c.origin != 'shared_chat' AND (?4 IS NULL OR julianday(c.created_at)>=julianday(?4)) AND (?5 IS NULL OR julianday(c.created_at)<julianday(?5)) AND (?6 IS NULL OR c.id IN (SELECT call_id FROM call_tags WHERE tag IN (SELECT value FROM json_each(?6))))");
    let tag_filter = (!tags.is_empty()).then(|| serde_json::to_string(tags)).transpose().map_err(|e| e.to_string())?;
    let limit = limit.clamp(1, 10000);
    let sql = format!(
        "{select}{filter} ORDER BY c.created_at DESC,c.id DESC LIMIT {limit} OFFSET {offset}"
    );
    // Keep the input literal and let the final word match a prefix (ricor → ricordati).
    let search = format!("\"{}\"*", q.replace('"', "\"\""));
    let title_search = format!("title : {search}");
    let count_sql = format!("SELECT count(*) FROM calls c{filter}");
    let total: usize = conn.query_row(&count_sql, params![search,q,title_search,date_from,date_to,tag_filter], |r|r.get(0)).map_err(err)?;
    let mut stmt = conn.prepare(&sql).map_err(err)?;
    let rows = stmt.query_map(params![search,q,title_search,date_from,date_to,tag_filter],row_call).map_err(err)?;
    let items: Vec<Call> = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    let mut search_results = std::collections::BTreeMap::new();
    if !q.is_empty() {
        for call in &items {
            let count = conn.query_row("SELECT count(*) FROM passage_search WHERE passage_search MATCH ?1 AND call_id=?2", params![search, call.id], |r| r.get(0)).map_err(err)?;
            let mut statement = conn.prepare("SELECT source,CAST(timestamp_ms AS INTEGER),snippet(passage_search,4,?3,?4,' … ',28) FROM passage_search WHERE passage_search MATCH ?1 AND call_id=?2 ORDER BY CAST(timestamp_ms AS INTEGER),rowid LIMIT 3").map_err(err)?;
            let hits = statement.query_map(params![search, call.id, "\u{e000}", "\u{e001}"], |r| Ok(SearchHit { source:r.get(0)?, timestamp_ms:r.get(1)?, excerpt:r.get(2)? })).map_err(err)?.collect::<std::result::Result<Vec<_>,_>>().map_err(err)?;
            search_results.insert(call.id.clone(), CallSearchResult { count, hits });
        }
    }
    Ok(CallPage {
        has_more: offset + items.len() < total,
        items,
        total,
        search_results,
    })
}
pub fn detail(conn: &Connection, id: &str) -> Result<Detail> {
    let call = conn
        .query_row(
            "SELECT c.id,c.title,c.created_at,c.duration_ms,c.status,c.error,'',COALESCE((SELECT json_group_array(tag) FROM call_tags WHERE call_id=c.id),'[]'),c.origin FROM calls c WHERE c.id=?1",
            [id],
            row_call,
        )
        .map_err(err)?;
    let mut stmt = conn.prepare("SELECT start_ms,end_ms,speaker,text,source,quality,original_text FROM segments WHERE call_id=?1 ORDER BY start_ms,id").map_err(err)?;
    let segments: Vec<Segment> = stmt
        .query_map([id], |r| {
            Ok(Segment {
                start_ms: r.get(0)?,
                end_ms: r.get(1)?,
                speaker: r.get(2)?,
                text: r.get(3)?,
                source: r.get(4)?,
                quality: r.get(5)?,
                original_text: r.get(6)?,
            })
        })
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    let segments = simple_speakers(crate::transcript_quality::review(segments));
    let mut stmt = conn
        .prepare(
            "SELECT role,content,model,activity_json,created_at FROM messages WHERE call_id=?1 ORDER BY id",
        )
        .map_err(err)?;
    let messages = stmt
        .query_map([id], |r| {
            Ok(Message {
                role: r.get(0)?,
                content: r.get(1)?,
                model: r.get(2)?,
                activity: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                created_at: r.get(4)?,
            })
        })
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(Detail {
        call,
        segments,
        messages,
        audio_directory: None,
        video_path: None,
        screenshots: screenshots(conn, id)?,
        notes: notes(conn, id)?,
    })
}
fn simple_speakers(segments: Vec<Segment>) -> Vec<Segment> {
    let mut result: Vec<Segment> = Vec::new();
    for mut segment in segments {
        segment.speaker = if segment.speaker == "Tu" {"Tu"} else {"Speaker"}.into();
        if let Some(previous) = result.last_mut() {
            if previous.speaker == segment.speaker && previous.source == segment.source
                && previous.quality.is_none() && segment.quality.is_none()
                && segment.start_ms >= previous.end_ms && segment.start_ms - previous.end_ms < 2000
                && segment.end_ms - previous.start_ms < 20000 {
                previous.text.push(' ');
                previous.text.push_str(&segment.text);
                previous.end_ms = segment.end_ms;
                continue;
            }
        }
        result.push(segment);
    }
    result
}
pub fn notes(conn: &Connection, id: &str) -> Result<Vec<CallNote>> {
    let mut stmt = conn.prepare("SELECT id,timestamp_ms,text FROM call_notes WHERE call_id=?1 ORDER BY timestamp_ms,id").map_err(err)?;
    let rows = stmt.query_map([id], |r| Ok(CallNote { id:r.get(0)?, timestamp_ms:r.get(1)?, text:r.get(2)? })).map_err(err)?;
    rows.collect::<std::result::Result<Vec<_>,_>>().map_err(err)
}
pub fn add_note(conn: &mut Connection, call_id: &str, note: &CallNote) -> Result<()> {
    let tx = conn.transaction().map_err(err)?;
    tx.execute("INSERT INTO call_notes(id,call_id,timestamp_ms,text) VALUES(?1,?2,?3,?4)", params![note.id,call_id,note.timestamp_ms,note.text]).map_err(err)?;
    tx.execute("UPDATE call_search SET body=COALESCE((SELECT group_concat(text,' ') FROM (SELECT text FROM segments WHERE call_id=?1 UNION ALL SELECT text FROM call_notes WHERE call_id=?1)),'') WHERE call_id=?1", [call_id]).map_err(err)?;
    tx.commit().map_err(err)
}
pub fn screenshots(conn: &Connection, id: &str) -> Result<Vec<Screenshot>> {
    let mut statement = conn.prepare("SELECT id,timestamp_ms,created_at,width,height FROM screenshots WHERE call_id=?1 ORDER BY timestamp_ms,id").map_err(err)?;
    let rows = statement.query_map([id], |r| Ok(Screenshot { id:r.get(0)?,timestamp_ms:r.get(1)?,created_at:r.get(2)?,width:r.get(3)?,height:r.get(4)? })).map_err(err)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)
}
pub fn add_screenshot(conn: &Connection, call_id: &str, image: &Screenshot) -> Result<()> {
    conn.execute("INSERT INTO screenshots(id,call_id,timestamp_ms,created_at,width,height) VALUES(?1,?2,?3,?4,?5,?6)",
        params![image.id,call_id,image.timestamp_ms,image.created_at,image.width,image.height]).map_err(err)?;
    Ok(())
}
pub fn publisher_id(conn: &Connection) -> Result<uuid::Uuid> {
    let publisher: Option<String> = conn.query_row("SELECT publisher FROM sharing_identity WHERE id=1", [], |row| row.get(0)).optional().map_err(err)?;
    if let Some(publisher) = publisher {
        return uuid::Uuid::parse_str(&publisher).map_err(|_| "Identificativo di pubblicazione non valido.".into());
    }
    let publisher = uuid::Uuid::new_v4();
    conn.execute("INSERT INTO sharing_identity(id,publisher) VALUES(1,?1)", [publisher.to_string()]).map_err(err)?;
    Ok(publisher)
}

pub fn create(conn: &Connection, id: &str, title: &str, date: &str) -> Result<()> {
    create_with_origin(conn, id, title, date, "recording")
}

pub fn create_with_origin(conn: &Connection, id: &str, title: &str, date: &str, origin: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO calls(id,title,created_at,status,origin) VALUES(?1,?2,?3,'recording',?4)",
        params![id, title, date, origin],
    )
    .map_err(err)?;
    conn.execute(
        "INSERT INTO call_search(call_id,title,body) VALUES(?1,?2,'')",
        params![id, title],
    )
    .map_err(err)?;
    Ok(())
}

pub fn create_shared_chat(conn: &mut Connection, source: &str, publisher: &str, title: &str) -> Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let tx = conn.transaction().map_err(err)?;
    create_with_origin(&tx, &id, title, &Utc::now().to_rfc3339(), "shared_chat")?;
    status(&tx, &id, "ready", None, None)?;
    tx.execute("INSERT INTO shared_chat_links(source_id,publisher_id,call_id) VALUES(?1,?2,?3)", params![source,publisher,id]).map_err(err)?;
    tx.commit().map_err(err)?;
    Ok(id)
}
pub fn status(
    conn: &Connection,
    id: &str,
    status: &str,
    duration: Option<i64>,
    error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE calls SET status=?2,duration_ms=COALESCE(?3,duration_ms),error=?4 WHERE id=?1",
        params![id, status, duration, error],
    )
    .map_err(err)?;
    Ok(())
}
pub fn save_capture_diagnostics(conn: &Connection, id: &str, dir: &Path) -> Result<()> {
    let mut sources = serde_json::Map::new();
    for source in ["microphone", "system", "video"] {
        let file = dir.join(if source == "video" { "video-diagnostics.json".into() } else { format!("{source}.diagnostics.json") });
        if !file.is_file() { continue; }
        let bytes = std::fs::read(file).map_err(|e|e.to_string())?;
        let value = serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;
        sources.insert(source.into(), value);
    }
    if !sources.is_empty() {
        conn.execute("INSERT INTO capture_diagnostics(call_id,json) VALUES(?1,?2)
            ON CONFLICT(call_id) DO UPDATE SET json=excluded.json",
            params![id, serde_json::Value::Object(sources).to_string()]).map_err(err)?;
    }
    Ok(())
}
pub fn store_segments(conn: &mut Connection, id: &str, segments: &[Segment]) -> Result<()> {
    save_segments(conn, id, segments, true)
}
pub fn store_draft(conn: &mut Connection, id: &str, segments: &[Segment]) -> Result<()> {
    save_segments(conn, id, segments, false)
}
fn save_segments(
    conn: &mut Connection,
    id: &str,
    segments: &[Segment],
    finalizing: bool,
) -> Result<()> {
    let tx = conn.transaction().map_err(err)?;
    tx.execute("DELETE FROM segments WHERE call_id=?1", [id])
        .map_err(err)?;
    for s in segments {
        tx.execute("INSERT INTO segments(call_id,start_ms,end_ms,speaker,text,source,quality,original_text) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",params![id,s.start_ms,s.end_ms,s.speaker,s.text,s.source,s.quality,s.original_text]).map_err(err)?;
    }
    let mut body = segments
        .iter()
        .map(|s| s.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    for note in notes(&tx, id)? { body.push(' '); body.push_str(&note.text); }
    tx.execute(
        "UPDATE call_search SET body=?2 WHERE call_id=?1",
        params![id, body],
    )
    .map_err(err)?;
    if finalizing {
        tx.execute("UPDATE calls SET status='finalizing' WHERE id=?1", [id])
            .map_err(err)?;
    }
    tx.commit().map_err(err)
}
pub fn rename(conn: &mut Connection, id: &str, title: &str) -> Result<()> {
    let tx = conn.transaction().map_err(err)?;
    tx.execute("UPDATE calls SET title=?2 WHERE id=?1", params![id, title])
        .map_err(err)?;
    tx.execute(
        "UPDATE call_search SET title=?2 WHERE call_id=?1",
        params![id, title],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}
pub fn call_titles(conn: &Connection, id: Option<&str>) -> Result<std::collections::HashMap<String, String>> {
    let mut statement = conn.prepare("SELECT id,title FROM calls WHERE ?1 IS NULL OR id=?1").map_err(err)?;
    let rows = statement.query_map([id], |row| Ok((row.get(0)?, row.get(1)?))).map_err(err)?;
    rows.collect::<std::result::Result<_, _>>().map_err(err)
}
pub fn delete(conn: &mut Connection, id: &str) -> Result<()> {
    let tx = conn.transaction().map_err(err)?;
    tx.execute("DELETE FROM call_search WHERE call_id=?1", [id])
        .map_err(err)?;
    tx.execute("DELETE FROM calls WHERE id=?1", [id])
        .map_err(err)?;
    tx.commit().map_err(err)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .map_err(err)?;
    Ok(())
}
pub fn retention_cutoff(months: u32) -> Result<Option<chrono::DateTime<Utc>>> {
    if months == 0 { return Ok(None); }
    Utc::now()
        .checked_sub_months(Months::new(months))
        .map(Some)
        .ok_or_else(|| "Scadenza non valida".into())
}
pub fn expired(conn: &Connection, months: u32) -> Result<Vec<String>> {
    let Some(cutoff) = retention_cutoff(months)? else { return Ok(Vec::new()); };
    let cutoff = cutoff.to_rfc3339();
    let mut stmt = conn.prepare("SELECT id FROM calls WHERE created_at < ?1 AND status NOT IN ('recording','pending','processing','finalizing')").map_err(err)?;
    let result = stmt
        .query_map([cutoff], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(result)
}

pub fn active_call_ids(conn: &Connection) -> Result<std::collections::HashSet<String>> {
    let mut statement = conn.prepare("SELECT id FROM calls WHERE status IN ('recording','pending','processing','finalizing')").map_err(err)?;
    let ids = statement.query_map([], |row| row.get(0)).map_err(err)?;
    ids.collect::<std::result::Result<_, _>>().map_err(err)
}
pub fn append_message(conn: &Connection, id: &str, role: &str, content: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO messages(call_id,role,content,created_at) VALUES(?1,?2,?3,?4)",
        params![id, role, content, Utc::now().to_rfc3339()],
    )
    .map_err(err)?;
    Ok(())
}
pub fn finalizing(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT id FROM calls WHERE status='finalizing'")
        .map_err(err)?;
    let ids = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(ids)
}

pub fn append_response(conn: &Connection, id: &str, content: &str, model: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO messages(call_id,role,content,model,created_at) VALUES(?1,'assistant',?2,?3,?4)",
        params![
            id,
            content,
            if model.is_empty() { None } else { Some(model) },
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(err)?;
    Ok(())
}

pub fn save_activity(conn: &Connection, run: &crate::activity::ChatRun) -> Result<()> {
    conn.execute("UPDATE messages SET activity_json=?3 WHERE id=(SELECT id FROM messages WHERE call_id=?1 ORDER BY id LIMIT 1 OFFSET ?2) AND role='user'", params![run.call_id, run.user_index, serde_json::to_string(&run.entries).map_err(|e| e.to_string())?]).map_err(err)?;
    Ok(())
}
pub fn clear_chat(conn: &Connection, id: &str) -> Result<()> {
    // Validate the call before deleting, and retain its transcript/search/settings.
    conn.query_row("SELECT id FROM calls WHERE id=?1", [id], |r| {
        r.get::<_, String>(0)
    })
    .map_err(err)?;
    conn.execute("DELETE FROM messages WHERE call_id=?1", [id])
        .map_err(err)?;
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .map_err(err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn ollama_web_requires_opt_in_after_the_policy_upgrade() {
        let conn=super::open(std::path::Path::new(":memory:")).unwrap();
        for (saved,expected) in [
            (serde_json::json!({}),false),
            (serde_json::json!({"ollama_web_search":true}),false),
            (serde_json::json!({"ollama_web_search":false}),false),
            (serde_json::json!({"ollama_web_search":true,"ollama_web_policy_version":1}),true),
        ] {
            conn.execute("INSERT OR REPLACE INTO settings(id,json) VALUES(1,?1)",[saved.to_string()]).unwrap();
            let settings=super::get_settings(&conn).unwrap();
            assert_eq!(settings.ollama_web_search,expected);
            assert_eq!(settings.ollama_web_policy_version,1);
            super::save_settings(&conn,&settings).unwrap();
            assert_eq!(super::get_settings(&conn).unwrap().ollama_web_search,expected);
        }
    }
    #[test]
    fn ollama_context_default_upgrade_preserves_custom_values_and_new_explicit_16k() {
        let conn=super::open(std::path::Path::new(":memory:")).unwrap();
        for (saved, expected) in [
            (serde_json::json!({"ollama_context_length":16384}),32768),
            (serde_json::json!({"ollama_context_length":65536}),65536),
            (serde_json::json!({"ollama_context_length":16384,"ollama_context_default_version":1}),16384),
            (serde_json::json!({}),32768),
        ] {
            conn.execute("INSERT OR REPLACE INTO settings(id,json) VALUES(1,?1)",[saved.to_string()]).unwrap();
            let settings=super::get_settings(&conn).unwrap();
            assert_eq!(settings.ollama_context_length,expected);
            assert_eq!(settings.ollama_context_default_version,1);
            super::save_settings(&conn,&settings).unwrap();
            assert_eq!(super::get_settings(&conn).unwrap().ollama_context_length,expected);
        }
    }
    #[test]
    fn shared_sources_migration_preserves_legacy_ids_and_does_not_restore_removed_links() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE shared_source(id INTEGER PRIMARY KEY, path TEXT); INSERT INTO shared_source VALUES(1,'C:/reference');
            CREATE TABLE chat_sources(call_id TEXT, json TEXT); INSERT INTO chat_sources VALUES('call','{\"excluded\":[\"shared\",\"shared/style.json\"]}');").unwrap();
        super::migrate_shared_sources(&conn).unwrap();
        super::migrate_shared_sources(&conn).unwrap();
        let result: (String,String)=conn.query_row("SELECT id,path FROM shared_sources",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!(result,("shared".into(),"C:/reference".into()));
        let preferences:String=conn.query_row("SELECT json FROM chat_sources",[],|r|r.get(0)).unwrap();
        assert!(preferences.contains("shared/style.json"));
        conn.execute("DELETE FROM shared_sources",[]).unwrap();
        super::migrate_shared_sources(&conn).unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM shared_sources",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    }
    #[test]
    fn screenshots_survive_transcript_updates_and_delete_with_the_call() {
        let mut conn=super::open(std::path::Path::new(":memory:")).unwrap();
        super::create(&conn,"call","Synthetic call","2026-10-16T10:00:00Z").unwrap();
        let image=crate::model::Screenshot{id:"image".into(),timestamp_ms:12500,created_at:"2026-10-16T10:00:12Z".into(),width:1920,height:1080};
        super::add_screenshot(&conn,"call",&image).unwrap();
        super::store_segments(&mut conn,"call",&[]).unwrap();
        let detail=super::detail(&conn,"call").unwrap();
        assert_eq!(detail.screenshots.len(),1);assert_eq!(detail.screenshots[0].timestamp_ms,12500);
        assert!(super::add_screenshot(&conn,"missing",&crate::model::Screenshot{id:"other".into(),..image}).is_err());
        super::delete(&mut conn,"call").unwrap();
        assert!(super::screenshots(&conn,"call").unwrap().is_empty());
    }
    use super::*;
    #[test]
    fn old_archive_schema_upgrades_without_losing_preferences_transcript_or_chat() {
        let path=std::env::temp_dir().join(format!("parlo-old-schema-{}.sqlite",uuid::Uuid::new_v4()));
        {
            let conn=Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE settings(id INTEGER PRIMARY KEY,json TEXT NOT NULL);
                INSERT INTO settings VALUES(1,'{\"theme\":\"light\",\"retention_months\":12,\"language\":\"en\",\"live_pipeline_version\":2}');
                CREATE TABLE calls(id TEXT PRIMARY KEY,title TEXT NOT NULL,created_at TEXT NOT NULL,duration_ms INTEGER NOT NULL DEFAULT 0,status TEXT NOT NULL,error TEXT);
                INSERT INTO calls VALUES('legacy','Legacy fixture','2026-10-14T00:00:00Z',5000,'ready',NULL);
                CREATE TABLE segments(id INTEGER PRIMARY KEY,call_id TEXT NOT NULL,start_ms INTEGER NOT NULL,end_ms INTEGER NOT NULL,speaker TEXT NOT NULL,text TEXT NOT NULL,source TEXT NOT NULL);
                INSERT INTO segments VALUES(1,'legacy',0,1000,'Tu','Legacy transcript','microphone');
                CREATE TABLE messages(id INTEGER PRIMARY KEY,call_id TEXT NOT NULL,role TEXT NOT NULL,content TEXT NOT NULL);
                INSERT INTO messages VALUES(1,'legacy','user','Legacy chat');").unwrap();
        }
        {
            let conn=open(&path).unwrap(); upgrade_preview(&conn).unwrap();
            let detail=detail(&conn,"legacy").unwrap();
            assert_eq!(detail.call.status,"ready"); assert_eq!(detail.segments[0].text,"Legacy transcript");
            assert_eq!(detail.messages[0].content,"Legacy chat"); assert!(detail.messages[0].activity.is_empty());
            let settings=get_settings(&conn).unwrap(); assert_eq!(settings.theme,"light"); assert_eq!(settings.retention_months,12); assert_eq!(settings.language,"en");
        }
        assert!(path.starts_with(std::env::temp_dir())); std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn reviewing_old_loops_does_not_rewrite_raw_archive_and_new_quality_survives_reopen() {
        let mut conn=open(Path::new(":memory:")).unwrap();create(&conn,"loop","Test","2026-10-13T00:00:00Z").unwrap();
        let raw=format!("{}poi decidiamo.","dei ".repeat(328));
        store_segments(&mut conn,"loop",&[Segment{start_ms:10000,end_ms:10520,speaker:"Speaker".into(),text:raw.clone(),source:"mixed".into(),quality:None,original_text:None}]).unwrap();
        let reviewed=detail(&conn,"loop").unwrap();assert_eq!(reviewed.segments[0].original_text.as_deref(),Some(raw.as_str()));assert_eq!(reviewed.segments[0].text,"dei dei dei poi decidiamo.");
        let stored:String=conn.query_row("SELECT text FROM segments WHERE call_id='loop'",[],|r|r.get(0)).unwrap();assert_eq!(stored,raw);
        assert!(crate::model::markdown(&reviewed.call,&reviewed.segments).contains("Da verificare"));
        store_segments(&mut conn,"loop",&reviewed.segments).unwrap();let saved=detail(&conn,"loop").unwrap();assert_eq!(saved.segments[0].original_text,reviewed.segments[0].original_text);assert_eq!(saved.segments[0].quality,reviewed.segments[0].quality);
    }
    #[test]
    fn historical_speaker_labels_are_simplified_without_merging_local_voice() {
        let segment = |speaker: &str, start: i64, text: &str| Segment {
            start_ms: start, end_ms: start + 1000, speaker: speaker.into(), text: text.into(),
            source: if speaker == "Tu" {"microphone"} else {"system"}.into(),
              quality: None, original_text: None,
        };
        let result = simple_speakers(vec![segment("Speaker H", 0, "Prima"),
            segment("Speaker C", 1000, "risposta"), segment("Tu", 2000, "Domanda"),
            segment("Voci sovrapposte", 3000, "Altro")]);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].speaker, "Speaker");
        assert_eq!(result[0].text, "Prima risposta");
        assert_eq!(result[1].speaker, "Tu");
        assert_eq!(result[2].speaker, "Speaker");
    }
    #[test]
    fn existing_preview_is_disabled_once_and_later_user_choices_are_kept() {
        let conn = memory();
        conn.execute(
            "INSERT INTO settings(id,json) VALUES(1,?1)",
            [r#"{"live_pipeline_version":1,"live_transcription":true}"#],
        )
        .unwrap();
        upgrade_preview(&conn).unwrap();
        let mut settings = get_settings(&conn).unwrap();
        assert!(!settings.live_transcription);
        assert_eq!(settings.live_pipeline_version, 2);
        settings.live_transcription = true;
        save_settings(&conn, &settings).unwrap();
        upgrade_preview(&conn).unwrap();
        assert!(get_settings(&conn).unwrap().live_transcription);
        let stored: String = conn
            .query_row("SELECT json FROM settings", [], |row| row.get(0))
            .unwrap();
        assert!(stored.contains("@engine/models/whisper-turbo"));
    }
    fn memory() -> Connection {
        open(Path::new(":memory:")).unwrap()
    }
    #[test]
    fn call_tags_round_trip_search_validation_and_cascade() {
        let mut conn = memory();
        create(&conn, "tags", "Synthetic call", "2026-10-18T10:00:00Z").unwrap();
        set_tags(&mut conn, "tags", vec!["  Cliente   Rossi  ".into(), "cliente rossi".into(), "Da rivedere".into()]).unwrap();
        assert_eq!(detail(&conn, "tags").unwrap().call.tags, vec!["Cliente Rossi", "Da rivedere"]);
        assert_eq!(page(&conn, "rossi", 0, 10).unwrap().total, 1);
        assert_eq!(page(&conn, "rossi", 0, 10).unwrap().items[0].tags.len(), 2);
        create(&conn, "untagged", "Other call", "2026-10-19T10:00:00Z").unwrap();
        assert_eq!(page_filtered(&conn, "", 0, 10, None, None, &["CLIENTE rossi".into()]).unwrap().total, 1);
        assert_eq!(page_filtered(&conn, "", 0, 10, None, None, &["Altro".into()]).unwrap().total, 0);
        assert_eq!(tags(&conn).unwrap().iter().map(|t| (t.tag.as_str(), t.count)).collect::<Vec<_>>(), vec![("Cliente Rossi", 1), ("Da rivedere", 1)]);
        assert!(set_tags(&mut conn, "tags", vec!["".into()]).is_err());
        assert!(set_tags(&mut conn, "missing", vec!["Tag".into()]).is_err());
        assert_eq!(detail(&conn, "tags").unwrap().call.tags.len(), 2);
        delete(&mut conn, "tags").unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM call_tags", [], |r|r.get::<_,usize>(0)).unwrap(), 0);
    }
    #[test]
    fn capture_diagnostics_survive_audio_cleanup_and_cascade_with_call() {
        let mut conn = memory();
        create(&conn, "call", "Call", "2026-10-13T10:00:00Z").unwrap();
        let dir = std::env::temp_dir().join(format!("parlo-diagnostics-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("microphone.diagnostics.json");
        std::fs::write(&file, r#"{"version":1,"packets":{"device_flags":44,"missing_sample_ms":490}}"#).unwrap();
        let video_file = dir.join("video-diagnostics.json");
        std::fs::write(&video_file, r#"{"stage":"completed","frames":2,"stop_reason":"control_eof"}"#).unwrap();
        save_capture_diagnostics(&conn, "call", &dir).unwrap();
        std::fs::remove_file(file).unwrap();
        std::fs::remove_file(video_file).unwrap();
        std::fs::remove_dir(dir).unwrap();
        let raw: String = conn.query_row("SELECT json FROM capture_diagnostics WHERE call_id='call'", [], |r|r.get(0)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(json["microphone"]["packets"]["device_flags"], 44);
        assert_eq!(json["microphone"]["packets"]["missing_sample_ms"], 490);
        assert!(json["system"].is_null());
        assert_eq!(json["video"]["frames"], 2);
        assert_eq!(json["video"]["stop_reason"], "control_eof");
        delete(&mut conn, "call").unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM capture_diagnostics", [], |r|r.get::<_,u32>(0)).unwrap(), 0);
    }
    #[test]
    fn clearing_one_chat_preserves_transcripts_other_chats_and_global_preferences() {
        let mut conn = memory();
        for id in ["a", "b"] {
            create(&conn, id, &format!("Call {id}"), "2026-10-12T10:00:00Z").unwrap();
            store_segments(
                &mut conn,
                id,
                &[Segment {
                    start_ms: 0,
                    end_ms: 1000,
                    speaker: "Tu".into(),
                    text: "Trascrizione preservata".into(),
                    source: "microphone".into(),
                      quality: None, original_text: None,
                }],
            )
            .unwrap();
            status(&conn, id, "ready", Some(1000), None).unwrap();
            append_message(&conn, id, "user", "Vecchia domanda").unwrap();
            append_response(&conn, id, "Vecchia risposta", "sonnet").unwrap();
        }
        let settings = Settings {
            trusted_tools: vec!["mcp__drive__read".into()],
            project_instructions: "Progetto Demo".into(),
            ..Settings::default()
        };
        save_settings(&conn, &settings).unwrap();
        let mut t = crate::activity::Tracker::new("a", 0);
        t.reconcile_answer("Vecchia risposta");
        t.finish(None, false);
        save_activity(&conn, &t.run).unwrap();
        clear_chat(&conn, "a").unwrap();
        let a = detail(&conn, "a").unwrap();
        assert!(a.messages.is_empty());
        assert_eq!(a.segments[0].text, "Trascrizione preservata");
        assert_eq!(a.call.status, "ready");
        assert_eq!(a.call.title, "Call a");
        assert_eq!(detail(&conn, "b").unwrap().messages.len(), 2);
        assert_eq!(list(&conn, "preservata").unwrap().len(), 2);
        assert_eq!(
            get_settings(&conn).unwrap().project_instructions,
            "Progetto Demo"
        );
        assert_eq!(
            get_settings(&conn).unwrap().trusted_tools,
            settings.trusted_tools
        );
        append_message(&conn, "a", "user", "Nuova domanda").unwrap();
        assert_eq!(
            detail(&conn, "a").unwrap().messages[0].content,
            "Nuova domanda"
        );
        assert!(clear_chat(&conn, "missing").is_err());
        assert_eq!(detail(&conn, "b").unwrap().messages.len(), 2);
    }
    #[test]
    fn activity_survives_reopening_and_cascades_with_call() {
        let path = std::env::temp_dir().join(format!(
            "parlo-activity-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        {
            let conn = open(&path).unwrap();
            create(&conn, "call", "Call", "2026-10-12T10:00:00Z").unwrap();
            append_message(&conn, "call", "user", "Riassumi").unwrap();
            let mut t = crate::activity::Tracker::new("call", 0);
            t.observe(&serde_json::json!({"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"Riepilogo pubblico."}]}}));
            t.finish(None, false);
            save_activity(&conn, &t.run).unwrap();
        }
        {
            let mut conn = open(&path).unwrap();
            let messages = detail(&conn, "call").unwrap().messages;
            assert_eq!(messages[0].activity[1].text, "Riepilogo pubblico.");
            assert_eq!(messages[0].activity.last().unwrap().status, "complete");
            delete(&mut conn, "call").unwrap();
            assert_eq!(
                conn.query_row("SELECT count(*) FROM messages", [], |r| r.get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn response_model_preserves_old_messages() {
        let c = memory();
        create(&c, "model-test", "Call", "2026-10-12T10:00:00Z").unwrap();
        append_message(&c, "model-test", "assistant", "Vecchia risposta").unwrap();
        append_response(&c, "model-test", "Nuova risposta", "claude-new-family-10-1").unwrap();
        let messages = detail(&c, "model-test").unwrap().messages;
        assert_eq!(messages[0].content, "Vecchia risposta");
        assert_eq!(messages[0].model, None);
        assert_eq!(messages[1].model.as_deref(), Some("claude-new-family-10-1"));
    }
    #[test]
    fn messages_preserve_send_times_and_leave_legacy_times_unknown() {
        let c = memory();
        create(&c, "time-test", "Call", "2026-10-24T10:00:00Z").unwrap();
        c.execute("INSERT INTO messages(call_id,role,content) VALUES('time-test','user','Legacy')", []).unwrap();
        let before = Utc::now();
        append_message(&c, "time-test", "user", "Domanda").unwrap();
        append_response(&c, "time-test", "Risposta", "test-model").unwrap();
        let after = Utc::now();
        let messages = detail(&c, "time-test").unwrap().messages;
        assert!(messages[0].created_at.is_none());
        for message in &messages[1..] {
            let sent = chrono::DateTime::parse_from_rfc3339(message.created_at.as_ref().unwrap()).unwrap();
            assert!(sent >= before && sent <= after);
        }
        assert_eq!(detail(&c, "time-test").unwrap().messages[1].created_at, messages[1].created_at);
    }
    #[test]
    fn prefix_search_finds_notes_transcripts_and_accents() {
        let mut c = memory();
        create(&c, "prefix", "Call sintetica", "2026-10-17T10:00:00Z").unwrap();
        add_note(&mut c, "prefix", &CallNote {
            id: "note".into(), timestamp_ms: 1500,
            text: "Ricordati il caffè di domani".into(),
        }).unwrap();
        store_segments(&mut c, "prefix", &[Segment {
            start_ms: 0, end_ms: 1000, speaker: "Tu".into(),
            text: "La consegna è confermata".into(), source: "microphone".into(),
            quality: None, original_text: None,
        }]).unwrap();
        let result = page(&c, "ricor", 0, 10).unwrap();
        assert_eq!(result.total, 1);
        assert_eq!(result.search_results["prefix"].count, 1);
        assert_eq!(result.search_results["prefix"].hits[0].source, "note");
        assert_eq!(page(&c, "RICOR", 0, 10).unwrap().total, 1);
        assert_eq!(page(&c, "il caff", 0, 10).unwrap().total, 1);
        assert_eq!(page(&c, "consegna è conf", 0, 10).unwrap().total, 1);
        assert_eq!(page(&c, "cord", 0, 10).unwrap().total, 0);
        assert_eq!(page(&c, "\" OR *", 0, 10).unwrap().total, 0);
    }
    #[test]
    fn date_filter_combines_local_day_search_and_pagination() {
        let c=memory();
        for (id,title,date) in [
            ("before","Cliente","2026-10-16T21:59:59Z"),
            ("start","Cliente","2026-10-16T22:00:00Z"),
            ("end","Cliente","2026-10-17T23:59:59+02:00"),
            ("other","Interno","2026-10-17T14:00:00Z"),
            ("after","Cliente","2026-10-17T22:00:00Z"),
        ] {create(&c,id,title,date).unwrap();}
        let from=Some("2026-10-17T00:00:00+02:00");
        let to=Some("2026-10-18T00:00:00+02:00");
        assert_eq!(page_filtered(&c,"",0,10,from,to,&[]).unwrap().total,3);
        let first=page_filtered(&c,"cliente",0,1,from,to,&[]).unwrap();
        let second=page_filtered(&c,"cliente",1,1,from,to,&[]).unwrap();
        assert_eq!(first.total,2);assert!(first.has_more);assert!(!second.has_more);
        assert_ne!(first.items[0].id,second.items[0].id);
        assert!(first.items.iter().chain(&second.items).all(|c|c.id=="start" || c.id=="end"));
        assert_eq!(page_filtered(&c,"",0,10,from,None,&[]).unwrap().total,4);
        assert_eq!(page_filtered(&c,"",0,10,None,to,&[]).unwrap().total,4);
        assert_eq!(page(&c,"",0,10).unwrap().total,5);
        assert!(page_filtered(&c,"",0,10,to,from,&[]).is_err());
        assert!(page_filtered(&c,"",0,10,Some("invalid"),None,&[]).is_err());
    }
    #[test]
    fn archive_search_and_cascade() {
        let mut c = memory();
        create(&c, "a", "Call ottobre", "2026-10-11T10:00:00+00:00").unwrap();
        store_segments(
            &mut c,
            "a",
            &[Segment {
                start_ms: 0,
                end_ms: 1000,
                speaker: "Speaker A".into(),
                text: "Decidiamo la consegna venerdì".into(),
                source: "system".into(),
                  quality: None, original_text: None,
            }],
        )
        .unwrap();
        append_message(&c, "a", "user", "Riassumi").unwrap();
        assert_eq!(list(&c, "consegna").unwrap().len(), 1);
        assert!(list(&c, "\" OR *").unwrap().is_empty());
        rename(&mut c, "a", "Demo cliente").unwrap();
        assert_eq!(list(&c, "Demo").unwrap().len(), 1);
        delete(&mut c, "a").unwrap();
        assert_eq!(
            c.query_row("SELECT count(*) FROM messages", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert!(list(&c, "consegna").unwrap().is_empty());
    }
    #[test]
    fn retention_leaves_active_recording() {
        let c = memory();
        create(&c, "old", "Old", "2020-01-01T00:00:00+00:00").unwrap();
        assert!(expired(&c, 3).unwrap().is_empty());
        status(&c, "old", "pending", None, None).unwrap();
        assert!(expired(&c, 3).unwrap().is_empty());
        status(&c, "old", "ready", None, None).unwrap();
        assert!(expired(&c, 0).unwrap().is_empty());
        assert_eq!(expired(&c, 3).unwrap(), vec!["old"]);
    }
    #[test]
    fn pages_are_stable_and_search_covers_unloaded_rows() {
        let mut c = memory();
        for i in 0..25 {
            let id = format!("{i:02}");
            create(&c, &id, &format!("Call {i}"), "2026-10-11T10:00:00Z").unwrap();
        }
        let first = page(&c, "", 0, 10).unwrap();
        let second = page(&c, "", 10, 10).unwrap();
        let third = page(&c, "", 20, 10).unwrap();
        assert_eq!(first.total, 25);
        assert_eq!(first.items.len(), 10);
        assert!(first.has_more);
        assert!(first
            .items
            .iter()
            .all(|a| !second.items.iter().any(|b| a.id == b.id)));
        assert_eq!(third.items.len(), 5);
        assert!(!third.has_more);
        store_draft(
            &mut c,
            "00",
            &[Segment {
                start_ms: 0,
                end_ms: 1000,
                speaker: "Tu".into(),
                text: "Parola nascosta nell’ultima pagina".into(),
                source: "microphone".into(),
                  quality: None, original_text: None,
            }],
        )
        .unwrap();
        assert_eq!(detail(&c, "00").unwrap().call.status, "recording");
        assert_eq!(page(&c, "nascosta", 0, 10).unwrap().items[0].id, "00");
    }
    #[test]
    fn committed_silent_transcript_survives_restart_during_finalization() {
        let path = std::env::temp_dir().join(format!(
            "callnotes-recovery-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut conn = open(&path).unwrap();
            create(&conn, &id, "Call silenziosa", &Utc::now().to_rfc3339()).unwrap();
            store_segments(&mut conn, &id, &[]).unwrap();
        }
        {
            let conn = open(&path).unwrap();
            assert_eq!(finalizing(&conn).unwrap(), vec![id.clone()]);
            assert_eq!(detail(&conn, &id).unwrap().call.status, "finalizing");
            assert!(detail(&conn, &id).unwrap().segments.is_empty());
        }
        std::fs::remove_file(path).unwrap();
    }
}


pub fn set_tags(conn: &mut Connection, id: &str, tags: Vec<String>) -> Result<()> {
    if tags.len() > 24 { return Err("Puoi aggiungere al massimo 24 tag per call.".into()); }
    let mut normalized = Vec::new();
    for tag in tags {
        if tag.chars().any(char::is_control) { return Err("Il tag contiene caratteri non validi.".into()); }
        let tag = tag.split_whitespace().collect::<Vec<_>>().join(" ");
        if tag.is_empty() || tag.chars().count() > 40 { return Err("Ogni tag deve contenere da 1 a 40 caratteri.".into()); }
        if !normalized.iter().any(|old: &String| old.to_lowercase() == tag.to_lowercase()) { normalized.push(tag); }
    }
    let tx = conn.transaction().map_err(err)?;
    let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM calls WHERE id=?1)", [id], |r| r.get(0)).map_err(err)?;
    if !exists { return Err("Call non trovata".into()); }
    tx.execute("DELETE FROM call_tags WHERE call_id=?1", [id]).map_err(err)?;
    for tag in normalized { tx.execute("INSERT INTO call_tags(call_id,tag) VALUES(?1,?2)", params![id,tag]).map_err(err)?; }
    tx.commit().map_err(err)
}
