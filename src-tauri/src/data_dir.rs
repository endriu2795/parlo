//! Per-user data folder, including compatibility with earlier bundle identifiers.
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const LEGACY_IDENTIFIER: &str = "local.callnotes.desktop";
const PREVIOUS_IDENTIFIER: &str = "com.presentio.transcript";
// SQLite keeps committed transactions in the WAL until a checkpoint: the three move together.
const DATABASE: [&str; 3] = ["archive.sqlite", "archive.sqlite-wal", "archive.sqlite-shm"];

/// Returns the folder to use for this session. If the previous folder cannot be moved
/// (an older build still holds the archive open), it keeps using it and retries next start.
pub fn resolve(root: PathBuf, app_id: &str) -> PathBuf {
    if app_id != crate::APP_ID || root.join(DATABASE[0]).exists() {
        return root;
    }
    let Some(parent) = root.parent() else {
        return root;
    };
    // Archived media and assistant sessions can contain absolute paths.
    // Keep the existing installation's data at those paths.
    let previous = parent.join(PREVIOUS_IDENTIFIER);
    if previous.join(DATABASE[0]).is_file() {
        return previous;
    }
    let legacy = parent.join(LEGACY_IDENTIFIER);
    if legacy == root || !legacy.join(DATABASE[0]).is_file() {
        return root;
    }
    match migrate(&legacy, &root) {
        Ok(()) => root,
        Err(e) => {
            eprintln!("Spostamento dei dati non completato, uso la cartella precedente: {e}");
            legacy
        }
    }
}

fn migrate(legacy: &Path, root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root)?;
    // The archive moves first and as a unit: if any part is locked, nothing has changed.
    let mut moved = Vec::new();
    for name in DATABASE {
        let from = legacy.join(name);
        if !from.exists() {
            continue;
        }
        if let Err(e) = fs::rename(&from, root.join(name)) {
            for name in moved.iter().rev() {
                let _ = fs::rename(root.join(name), legacy.join(name));
            }
            return Err(e);
        }
        moved.push(name);
    }
    // Engine, models and temporary audio follow. An entry already present in the new
    // folder (the WebView profile created with the window) stays where it is.
    for entry in fs::read_dir(legacy)? {
        let entry = entry?;
        let target = root.join(entry.file_name());
        if target.exists() {
            continue;
        }
        if let Err(e) = fs::rename(entry.path(), &target) {
            eprintln!("Spostamento di {} non riuscito: {e}", entry.path().display());
        }
    }
    let _ = fs::remove_dir(legacy);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent() -> PathBuf {
        let parent = std::env::temp_dir().join(format!("parlo-data-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(parent.join(LEGACY_IDENTIFIER)).unwrap();
        parent
    }

    #[test]
    fn previous_folder_moves_once_with_its_archive() {
        let parent = parent();
        let legacy = parent.join(LEGACY_IDENTIFIER);
        fs::write(legacy.join("archive.sqlite"), b"db").unwrap();
        fs::write(legacy.join("archive.sqlite-wal"), b"wal").unwrap();
        fs::create_dir_all(legacy.join("engine/models")).unwrap();
        fs::write(legacy.join("engine/models/model.bin"), b"model").unwrap();
        fs::create_dir_all(legacy.join("EBWebView")).unwrap();
        let root = parent.join(crate::APP_ID);
        fs::create_dir_all(root.join("EBWebView")).unwrap();

        assert_eq!(resolve(root.clone(), crate::APP_ID), root);
        assert_eq!(fs::read(root.join("archive.sqlite")).unwrap(), b"db");
        assert_eq!(fs::read(root.join("archive.sqlite-wal")).unwrap(), b"wal");
        assert_eq!(fs::read(root.join("engine/models/model.bin")).unwrap(), b"model");
        assert!(!legacy.join("archive.sqlite").exists());
        // A second start finds the archive in place and touches nothing.
        fs::write(legacy.join("archive.sqlite"), b"stale").unwrap();
        assert_eq!(resolve(root.clone(), crate::APP_ID), root);
        assert_eq!(fs::read(root.join("archive.sqlite")).unwrap(), b"db");
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn rebranded_app_reuses_existing_archive_at_its_original_path() {
        let parent = parent();
        let previous = parent.join(PREVIOUS_IDENTIFIER);
        fs::create_dir(&previous).unwrap();
        fs::write(previous.join(DATABASE[0]), b"existing").unwrap();
        let root = parent.join(crate::APP_ID);
        assert_eq!(resolve(root.clone(), crate::APP_ID), previous);
        assert!(!root.exists());
        fs::create_dir(&root).unwrap();
        fs::write(root.join(DATABASE[0]), b"new").unwrap();
        assert_eq!(resolve(root.clone(), crate::APP_ID), root);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn fork_keeps_the_official_legacy_archive_in_place() {
        let parent = parent();
        let legacy = parent.join(LEGACY_IDENTIFIER);
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join(DATABASE[0]), b"db").unwrap();
        let fork = parent.join("org.example.transcript");
        assert_eq!(resolve(fork.clone(), "org.example.transcript"), fork);
        assert!(legacy.join(DATABASE[0]).is_file());
        assert!(!fork.exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn new_installation_without_previous_data_is_unchanged() {
        let parent = parent();
        let root = parent.join(crate::APP_ID);
        assert_eq!(resolve(root.clone(), crate::APP_ID), root);
        assert!(!root.exists());
        fs::remove_dir_all(parent).unwrap();
    }
}
