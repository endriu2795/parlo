use crate::{db, AppState, LockExt, Shared};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Component, Path, PathBuf},
};
use tauri::State;
type Result<T> = std::result::Result<T, String>;

#[derive(Serialize)]
pub struct GeneratedFile {
    pub id: String,
    pub name: String,
    pub path: String,
    pub size: u64,
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

pub fn directory(state: &AppState, call: &str, create: bool) -> Result<PathBuf> {
    let call = uuid::Uuid::parse_str(call)
        .map_err(|_| "Identificatore call non valido")?
        .to_string();
    let mut path = state.data_dir.clone();
    for part in ["attachments", &call, "generated"] {
        path.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&path) {
            if is_link(&metadata) || !metadata.is_dir() {
                return Err("Cartella di output non valida".into());
            }
        }
    }
    if create {
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    }
    // Canonical paths are required by the tools; strip Windows' extended prefix.
    if path.exists() {
        crate::sources::canonical_path(&path)
    } else {
        Ok(path)
    }
}

fn visible(relative: &Path) -> bool {
    relative.components().all(|part| match part {
        Component::Normal(name) => {
            let name = name.to_string_lossy();
            !name.starts_with('.') && name != "node_modules" && !name.contains(['\\', ':'])
        }
        _ => false,
    }) && !relative.as_os_str().is_empty()
}

fn resolve(state: &AppState, call: &str, file: &str) -> Result<PathBuf> {
    let relative = Path::new(file);
    if !visible(relative) {
        return Err("Percorso file non valido".into());
    }
    let root = directory(state, call, false)?;
    let mut path = root.clone();
    for part in relative.components() {
        path.push(part);
        let metadata = fs::symlink_metadata(&path).map_err(|_| "File non trovato")?;
        if is_link(&metadata) {
            return Err("Collegamento a file esterno non consentito".into());
        }
    }
    if !path.is_file()
        || !path
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(root.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("File non trovato nella cartella della call".into());
    }
    Ok(path)
}

pub fn list(state: &AppState, call: &str) -> Result<Vec<GeneratedFile>> {
    let root = directory(state, call, false)?;
    if !root.exists() {
        return Ok(vec![]);
    }
    // Preserve readable names of files created by the removed quiz/slides shortcuts.
    let legacy: HashMap<String, String> = {
        let conn = state.db.guard();
        let mut statement = conn.prepare("SELECT id,name,extension FROM generated_files WHERE call_id=?1 AND extension IS NOT NULL").map_err(|e|e.to_string())?;
        let rows = statement
            .query_map([call], |r| {
                let id: String = r.get(0)?;
                let name: String = r.get(1)?;
                let extension: String = r.get(2)?;
                Ok((
                    format!("{id}.{extension}"),
                    format!(
                        "{}.{}",
                        name.trim_end_matches(&format!(".{extension}")),
                        extension
                    ),
                ))
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(|e| e.to_string())?
    };
    let mut result = vec![];
    let mut pending = vec![(root.clone(), 0)];
    let mut budget = 1000;
    while let Some((directory, depth)) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            if budget == 0 {
                break;
            }
            budget -= 1;
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let Ok(relative) = path.strip_prefix(&root) else {
                continue;
            };
            if !visible(relative) {
                continue;
            }
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if is_link(&metadata) {
                continue;
            }
            if metadata.is_dir() && depth < 8 {
                pending.push((path, depth + 1));
            } else if metadata.is_file() {
                let id = relative.to_string_lossy().replace('\\', "/");
                result.push(GeneratedFile {
                    name: legacy.get(&id).cloned().unwrap_or_else(|| id.clone()),
                    id,
                    path: path.to_string_lossy().into_owned(),
                    size: metadata.len(),
                });
            }
        }
        if budget == 0 {
            break;
        }
    }
    result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(result)
}

#[tauri::command]
pub async fn save_generated(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: State<'_, Shared>,
    id: String,
    file_id: String,
) -> Result<bool> {
    db::detail(&state.db.guard(), &id)?;
    let file = resolve(&state, &id, &file_id)?;
    let name = list(&state, &id)?
        .into_iter()
        .find(|f| f.id == file_id)
        .ok_or("File non trovato")?
        .name;
    // A nested relative path is shown in the list; Save As only needs its basename.
    let name = Path::new(&name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let cleaned = crate::export::filename(&name);
    let name = cleaned.strip_suffix(".md").unwrap_or(&cleaned).to_owned();
    crate::export::save_file(app, window, name, file).await
}

#[tauri::command]
pub fn reveal_generated(state: State<'_, Shared>, id: String, file_id: String) -> Result<()> {
    db::detail(&state.db.guard(), &id)?;
    crate::sources::reveal_path(&resolve(&state, &id, &file_id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[test]
    fn output_files_are_listed_downloadable_scoped_and_kept_as_references() {
        let root = std::env::temp_dir().join(format!("parlo-output-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let state = AppState {
            db: Mutex::new(db::open(Path::new(":memory:")).unwrap()),
            status: Default::default(),
            recorder: Mutex::new(None),
            operation: Mutex::new(()),
            data_dir: root.clone(),
            processing: Default::default(),
            engine: Mutex::new(None),
            live_engine: Mutex::new(None),
            live_process: Mutex::new(None),
            chat_control: Default::default(),
            shortcut_capture: Default::default(),
            provision: Default::default(),
            auth: Default::default(),
            codex_auth: Default::default(),gemini_auth:Default::default(),
            catalog: Mutex::new(None),
        };
        let a = uuid::Uuid::new_v4().to_string();
        let b = uuid::Uuid::new_v4().to_string();
        {
            let conn = state.db.guard();
            db::create(&conn, &a, "Call sintetica", "").unwrap();
            db::create(&conn, &b, "Altra call", "").unwrap();
        }
        let output = directory(&state, &a, true).unwrap();
        fs::write(output.join("quiz.json"), b"[{\"question\":\"Esempio\"}]").unwrap();
        fs::write(output.join("slide.pptx"), [0, 255, 17, 33]).unwrap();
        fs::create_dir(output.join(".work")).unwrap();
        fs::write(output.join(".work/temp.py"), "temporaneo").unwrap();
        let legacy = uuid::Uuid::new_v4().to_string();
        fs::write(output.join(format!("{legacy}.json")), "[]").unwrap();
        state.db.guard().execute("INSERT INTO generated_files(id,call_id,kind,name,created_at,extension) VALUES(?1,?2,'quiz','Quiz precedente','','json')",rusqlite::params![legacy,a]).unwrap();
        let files = list(&state, &a).unwrap();
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|f| f.name == "Quiz precedente.json"));
        assert_eq!(
            fs::read(resolve(&state, &a, "slide.pptx").unwrap()).unwrap(),
            vec![0, 255, 17, 33]
        );
        assert!(list(&state, &b).unwrap().is_empty());
        for path in [
            "../quiz.json",
            "/quiz.json",
            r"C:\outside.txt",
            ".work/temp.py",
            "missing.json",
        ] {
            assert!(resolve(&state, &a, path).is_err());
        }
        let detail = db::detail(&state.db.guard(), &a).unwrap();
        let inventory = crate::sources::inventory(&state, &detail).unwrap();
        let serialized = serde_json::to_value(inventory).unwrap();
        assert!(serialized["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "output:quiz.json" && s["enabled"] == true));
        // Removing the call's attachments removes its outputs; shared originals are elsewhere.
        crate::screen::remove(&state, &a).unwrap();
        assert!(!output.exists());
        assert!(list(&state, &a).unwrap().is_empty());
        fs::remove_dir(root.join("attachments")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
