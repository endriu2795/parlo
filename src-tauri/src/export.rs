use std::path::PathBuf;

pub fn filename(title: &str) -> String {
    let cleaned: String = title.chars().take(120).map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { '-' } else { c }).collect();
    let cleaned = cleaned.trim().trim_end_matches(['.', ' ']);
    format!("{}.md", if cleaned.is_empty() { "call" } else { cleaned })
}

#[cfg(windows)]
fn choose(name: &str, owner: usize) -> Result<Option<PathBuf>, String> {
    use windows::{core::{GUID, HSTRING, HRESULT, PCWSTR}, Win32::{Foundation::HWND,
        System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED},
        UI::Shell::{FileSaveDialog, IFileSaveDialog, SIGDN_FILESYSPATH, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, Common::COMDLG_FILTERSPEC}}};
    const SAVE_DIALOG_STATE: GUID = GUID::from_u128(0x15fac265_8cf0_4536_baba_9d87a5326d9f);
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|e| e.to_string())?;
        let result = (|| -> windows::core::Result<Option<PathBuf>> {
            let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
            // Use fresh dialog state so Windows does not restore the old oversized layout.
            dialog.SetClientGuid(&SAVE_DIALOG_STATE)?;
            dialog.SetTitle(&HSTRING::from(crate::locale::tr("Salva file", "Save file")))?;
            let extension=std::path::Path::new(name).extension().and_then(|s|s.to_str()).unwrap_or("md");
            let label=HSTRING::from(format!("File {extension}"));
            let filter=HSTRING::from(format!("*.{extension}"));
            dialog.SetFileTypes(&[COMDLG_FILTERSPEC { pszName: PCWSTR(label.as_ptr()), pszSpec: PCWSTR(filter.as_ptr()) }])?;
            dialog.SetDefaultExtension(&HSTRING::from(extension))?;
            dialog.SetFileName(&HSTRING::from(name))?;
            dialog.SetOptions(dialog.GetOptions()? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT)?;
            if let Err(error) = dialog.Show(Some(HWND(owner as *mut _))) {
                if error.code() == HRESULT(0x800704c7u32 as i32) { return Ok(None); }
                return Err(error);
            }
            let value = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = value.to_string();
            CoTaskMemFree(Some(value.0.cast()));
            Ok(Some(PathBuf::from(path?)))
        })();
        CoUninitialize();
        result.map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "macos")]
fn choose(name: &str) -> Result<Option<PathBuf>, String> {
    use std::ffi::{c_char, CStr, CString};
    extern "C" { fn parlo_save_markdown(name: *const c_char) -> *mut c_char; fn parlo_string_free(value: *mut c_char); }
    let name = CString::new(name).map_err(|e| e.to_string())?;
    unsafe {
        let value = parlo_save_markdown(name.as_ptr());
        if value.is_null() { return Ok(None); }
        let path = PathBuf::from(CStr::from_ptr(value).to_string_lossy().into_owned());
        parlo_string_free(value);
        Ok(Some(path))
    }
}

fn write(path: Option<PathBuf>, text: &str) -> Result<bool, String> {
    let Some(path) = path else { return Ok(false); };
    std::fs::write(path, text.as_bytes()).map_err(|e| format!("Impossibile salvare il file: {e}. Puoi usare Copia Markdown."))?;
    Ok(true)
}

pub async fn save(app: tauri::AppHandle, window: tauri::WebviewWindow, name: String, text: String) -> Result<bool, String> {
    save_bytes(app,window,name,text.into_bytes()).await
}
pub async fn save_bytes(app: tauri::AppHandle, window: tauri::WebviewWindow, name: String, bytes: Vec<u8>) -> Result<bool, String> {
    let write=move |path:Option<PathBuf>| -> Result<bool,String> {
        let Some(path)=path else{return Ok(false)};
        std::fs::write(path,&bytes).map_err(|e|format!("Impossibile salvare il file: {e}"))?;
        Ok(true)
    };
    save_with(app, window, name, write).await
}

pub async fn save_file(app: tauri::AppHandle, window: tauri::WebviewWindow, name: String, source: PathBuf) -> Result<bool, String> {
    let reader = std::fs::File::open(&source).map_err(|e|e.to_string())?;
    let source = source.canonicalize().map_err(|e|e.to_string())?;
    save_with(app, window, name, move |path| copy_file(reader, &source, path)).await
}
fn copy_file(mut reader: std::fs::File, source: &std::path::Path, path: Option<PathBuf>) -> Result<bool,String> {
    let Some(path) = path else {return Ok(false)};
    if path.canonicalize().is_ok_and(|p|p == source) {return Ok(true);}
    let mut destination = std::fs::File::create(path).map_err(|e|format!("Impossibile salvare il file: {e}"))?;
    std::io::copy(&mut reader, &mut destination).map_err(|e|format!("Impossibile copiare il file: {e}"))?;
    Ok(true)
}

async fn save_with<F>(app: tauri::AppHandle, window: tauri::WebviewWindow, name: String, write: F) -> Result<bool, String>
where F: FnOnce(Option<PathBuf>) -> Result<bool,String> + Send + 'static {
    #[cfg(windows)] {
        let owner = window.hwnd().map_err(|e| e.to_string())?.0 as usize;
        let _ = app;
        tauri::async_runtime::spawn_blocking(move || write(choose(&name, owner)?)).await.map_err(|e| e.to_string())?
    }
    #[cfg(target_os = "macos")] {
        let _ = window;
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || { let _ = tx.send(choose(&name).and_then(write)); }).map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn_blocking(move || rx.recv().map_err(|e| e.to_string())?).await.map_err(|e| e.to_string())?
    }
    #[cfg(not(any(windows, target_os = "macos")))] {
        let _ = (app, window, name, write);
        Err("Usa Copia Markdown per esportare la trascrizione su questa piattaforma.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn downloading_local_outputs_preserves_binary_bytes_and_handles_cancel_and_same_file() {
        let root=std::env::temp_dir().join(format!("parlo-download-{}",uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let source=root.join("slide.pptx"); let destination=root.join("copia.pptx");
        let bytes=[0,255,19,0,128,10]; std::fs::write(&source,bytes).unwrap();
        let canonical=source.canonicalize().unwrap();
        let reader=||std::fs::File::open(&source).unwrap();
        assert!(!copy_file(reader(),&canonical,None).unwrap());
        assert!(copy_file(reader(),&canonical,Some(source.clone())).unwrap());
        assert_eq!(std::fs::read(&source).unwrap(),bytes);
        assert!(copy_file(reader(),&canonical,Some(destination.clone())).unwrap());
        assert_eq!(std::fs::read(&destination).unwrap(),bytes);
        std::fs::remove_file(source).unwrap(); std::fs::remove_file(destination).unwrap(); std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn cancel_preserves_files_and_export_writes_utf8_transcript() {
        let path = std::env::temp_dir().join(format!("parlo-export-{}.md", uuid::Uuid::new_v4()));
        std::fs::write(&path, "precedente").unwrap();
        assert!(!write(None, "nuovo").unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "precedente");
        assert!(write(Some(path.clone()), "# Riunione\n\n**[00:01] Tu**\n\nCaffè pronto.").unwrap());
        assert!(std::fs::read_to_string(&path).unwrap().contains("Caffè pronto."));
        std::fs::remove_file(path).unwrap();
    }
}
