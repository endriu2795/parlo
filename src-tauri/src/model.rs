use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

fn platform_shortcut(key: &str) -> String {
    format!("{}+Shift+{key}", if cfg!(target_os = "macos") { "Super" } else { "Ctrl" })
}

pub fn engine_dir() -> PathBuf {
    std::env::var_os("CALLNOTES_ENGINE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap_or_default()
                .parent()
                .unwrap_or(Path::new("."))
                .join("engine")
        })
}
pub fn bundled_model_id(path: &str) -> Option<&'static str> {
    bundled_model_id_at(&engine_dir(), path)
}
fn bundled_model_id_at(root: &Path, path: &str) -> Option<&'static str> {
    ["turbo", "tiny", "small"].into_iter().find(|id| Path::new(path) == root.join("models").join(format!("whisper-{id}")))
}
pub fn model_ready(path: &Path, preview: bool) -> bool {
    model_ready_at(&engine_dir(), path, preview)
}
pub fn preview_model_ready(path: &Path, cpu: bool) -> bool {
    if cpu { return model_ready(path, true); }
    let root = engine_dir();
    if root.join("cuda").is_dir() && model_ready(path, true) { return true; }
    let Some(id) = path.to_str().and_then(bundled_model_id) else { return false; };
    let ov = root.join("models").join(format!("whisper-{id}-openvino"));
    (ov.join("presentio-model.json").is_file() && ["openvino_encoder_model.xml", "openvino_encoder_model.bin", "openvino_decoder_model.xml", "openvino_decoder_model.bin"].iter().all(|name| ov.join(name).is_file()))
        || (root.join("models").join(format!("ggml-{id}.bin")).is_file()
            && root.join("whisper-cpp").join(if cfg!(windows) { "whisper.dll" } else { "libwhisper.dylib" }).is_file())
}
fn model_ready_at(root: &Path, path: &Path, preview: bool) -> bool {
    if ["model.bin", "config.json", "tokenizer.json"].iter().all(|file| path.join(file).is_file()) { return true; }
    if preview { return false; }
    let Some(id) = path.to_str().and_then(|path| bundled_model_id_at(root, path)) else { return false; };
    let ov = root.join("models").join(format!("whisper-{id}-openvino"));
    (ov.join("presentio-model.json").is_file() && ["openvino_encoder_model.xml", "openvino_encoder_model.bin", "openvino_decoder_model.xml", "openvino_decoder_model.bin"].iter().all(|file| ov.join(file).is_file()))
        || (root.join("whisper-cpp/runtime.json").is_file() && root.join("whisper-cpp").join(if cfg!(windows) { "whisper-cli.exe" } else { "whisper-cli" }).is_file() && root.join("models").join(format!("ggml-{id}.bin")).is_file())
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DictionaryEntry {
    pub term: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub theme: String,
    pub custom_stt: crate::custom_endpoint::Endpoint,
    pub custom_tts: crate::custom_endpoint::Endpoint,
    pub custom_chat: crate::custom_endpoint::Endpoint,
    pub sharing: crate::sharing::Sharing,
    pub titlebar_color: String,
    pub retention_months: u32,
    pub keep_audio: bool,
    pub keep_video: bool,
    pub dictionary: Vec<DictionaryEntry>,
    pub microphone: bool,
    pub microphone_voice_boost: bool,
    pub system_audio: bool,
    pub system_audio_output: String,
    pub shortcut: String,
    pub drawing_shortcut: String,
    pub screenshot_shortcut: String,
    pub text_shortcut: String,
    pub note_shortcut: String,
    #[serde(default)]
    pub shortcut_platform: String,
    pub brush_color: String,
    pub brush_size: u32,
    pub language: String,
    pub stt_provider: String,
    pub whisper_model: String,
    pub compute_mode: String,
    pub python_bin: String,
    pub segmentation_model: String,
    pub embedding_model: String,
    pub diarization: bool,
    pub claude_bin: String,
    pub claude_model: String,
    pub assistant_provider: String,
    pub ollama_model: String,
    pub ollama_server_url: String,
    pub ollama_server_trusted_tools: Vec<String>,
    pub ollama_context_length: u32,
    pub ollama_context_default_version: u8,
    pub ollama_web_search: bool,
    pub ollama_tools_enabled: bool,
    pub ollama_full_tools: bool,
    pub ollama_web_policy_version: u8,
    pub ollama_trusted_tools: Vec<String>,
    pub codex_bin: String,
    pub codex_model: String,
    pub codex_connectors: bool,
    pub codex_disabled_connectors: Vec<String>,
    pub codex_trusted_tools: Vec<String>,
    pub gemini_bin: String,
    pub gemini_model: String,
    pub gemini_connectors: bool,
    pub gemini_disabled_connectors: Vec<String>,
    pub gemini_trusted_tools: Vec<String>,
    pub live_transcription: bool,
    pub live_whisper_model: String,
    pub live_compute_mode: String,
    pub live_pipeline_version: u32,
    pub claude_connectors: bool,
    pub disabled_connectors: Vec<String>,
    pub trusted_tools: Vec<String>,
    pub project_instructions: String,
    pub system_prompt: String,
    pub completion_notifications: bool,
    pub tts_enabled: bool,
    pub tts_provider: String,
    pub tts_edge_voice: String,
    pub tts_piper_voice: String,
    pub tts_kokoro_voice: String,
    pub tts_qwen_voice: String,
    pub tts_qwen_device: String,
    pub tts_qwen_language: String,
    pub tts_eleven_voice: String,
    pub tts_eleven_model: String,
}
impl Default for Settings {
    fn default() -> Self {
        let engine = engine_dir();
        let path = |p: &str| engine.join(p).to_string_lossy().to_string();
        Self {
            theme: "dark".into(),
            custom_stt: Default::default(),
            custom_tts: Default::default(),
            custom_chat: Default::default(),
            sharing: Default::default(),
            titlebar_color: "theme".into(),
            retention_months: 3,
            keep_audio: false,
            keep_video: false,
            dictionary: vec![],
            microphone: true,
            microphone_voice_boost: true,
            system_audio: true,
            system_audio_output: String::new(),
            shortcut: platform_shortcut("Space"),
            drawing_shortcut: platform_shortcut("D"),
            screenshot_shortcut: platform_shortcut("S"),
            text_shortcut: platform_shortcut("T"),
            note_shortcut: platform_shortcut("N"),
            shortcut_platform: std::env::consts::OS.into(),
            brush_color: "#ff4040".into(),
            brush_size: 5,
            language: "auto".into(),
            stt_provider: "local".into(),
            whisper_model: path("models/whisper-turbo"),
            compute_mode: "auto".into(),
            python_bin: crate::python_runtime::prepared_installed(&engine).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| path(if cfg!(windows) {
                "venv/Scripts/python.exe"
            } else {
                "venv/bin/python"
            })),
            segmentation_model: path("models/segmentation/model.onnx"),
            embedding_model: path("models/embedding.onnx"),
            diarization: false,
            claude_model: "sonnet".into(),
            assistant_provider: "claude".into(),
            ollama_model: String::new(),
            ollama_server_url: String::new(),
            ollama_server_trusted_tools: vec![],
            ollama_context_length: 32768,
            ollama_context_default_version: 1,
            ollama_web_search: false,
            ollama_tools_enabled: true,
            ollama_full_tools: false,
            ollama_web_policy_version: 1,
            ollama_trusted_tools: vec![],
            codex_bin: crate::codex_runtime::installed(&engine).map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(||"codex".into()),
            codex_model: String::new(),
            codex_connectors: true,
            codex_disabled_connectors: vec![],
            codex_trusted_tools: vec![],
            gemini_bin: crate::gemini_runtime::installed(&engine).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| "agy_acp_server".into()),
            gemini_model: String::new(),
            gemini_connectors: true,
            gemini_disabled_connectors: vec![],
            gemini_trusted_tools: vec![],
            live_transcription: false,
            live_whisper_model: path("models/whisper-tiny"),
            live_compute_mode: "auto".into(),
            live_pipeline_version: 2,
            claude_connectors: true,
            disabled_connectors: vec![],
            trusted_tools: vec![],
            project_instructions: String::new(),
            system_prompt: String::new(),
            completion_notifications: true,
            tts_enabled: false,
            tts_provider: "edge".into(),
            tts_edge_voice: "it-IT-IsabellaNeural".into(),
            tts_piper_voice: "it_IT-serena-medium".into(),
            tts_kokoro_voice: "if_sara".into(),
            tts_qwen_voice: "ryan".into(),
            tts_qwen_device: "cuda".into(),
            tts_qwen_language: "Italian".into(),
            tts_eleven_voice: String::new(),
            tts_eleven_model: "eleven_multilingual_v2".into(),
            claude_bin: if let Some(binary) = crate::claude_runtime::installed(&engine) {
                binary.to_string_lossy().into_owned()
            } else if engine.join("claude.exe").is_file() {
                path("claude.exe")
            } else {
                "claude".into()
            },
        }
    }
}
impl Settings {
    pub fn upgrade_screen_shortcuts(&mut self, saved: &serde_json::Value) {
        let same=|a:&str,b:&str| a.parse::<tauri_plugin_global_shortcut::Shortcut>().ok()==b.parse::<tauri_plugin_global_shortcut::Shortcut>().ok();
        if saved.get("note_shortcut").is_none() {
            for candidate in ["Ctrl+Shift+N", "Ctrl+Alt+Shift+N", "Ctrl+Alt+Shift+F12"] {
                if [&self.shortcut,&self.drawing_shortcut,&self.screenshot_shortcut,&self.text_shortcut].iter().all(|value| !same(candidate,value)) { self.note_shortcut = candidate.into(); break; }
            }
        }
        if saved.get("drawing_shortcut").is_none() {
            for candidate in ["Ctrl+Shift+D","Ctrl+Alt+Shift+D","Ctrl+Alt+Shift+F6","Ctrl+Alt+Shift+F9"] {
                if !same(candidate,&self.shortcut) && !same(candidate,&self.screenshot_shortcut) && !same(candidate,&self.text_shortcut) {self.drawing_shortcut=candidate.into();break;}
            }
        }
        if saved.get("screenshot_shortcut").is_none() {
            for candidate in ["Ctrl+Shift+S","Ctrl+Alt+Shift+S","Ctrl+Alt+Shift+F7","Ctrl+Alt+Shift+F10"] {
                if !same(candidate,&self.shortcut) && !same(candidate,&self.drawing_shortcut) && !same(candidate,&self.text_shortcut) {self.screenshot_shortcut=candidate.into();break;}
            }
        }
        if saved.get("text_shortcut").is_none() {
            for candidate in ["Ctrl+Shift+T","Ctrl+Alt+Shift+T","Ctrl+Alt+Shift+F8","Ctrl+Alt+Shift+F11"] {
                if !same(candidate,&self.shortcut) && !same(candidate,&self.drawing_shortcut) && !same(candidate,&self.screenshot_shortcut) {self.text_shortcut=candidate.into();break;}
            }
        }
    }
    pub fn upgrade_platform_shortcuts(&mut self) {
        #[cfg(target_os = "macos")]
        if self.shortcut_platform != "macos" {
            let same = |a: &str, b: &str| a.parse::<tauri_plugin_global_shortcut::Shortcut>().ok() == b.parse::<tauri_plugin_global_shortcut::Shortcut>().ok();
            // Convert only old defaults; preserve custom chords and avoid collisions.
            let mut values = [self.shortcut.clone(), self.drawing_shortcut.clone(), self.screenshot_shortcut.clone(), self.text_shortcut.clone(), self.note_shortcut.clone()];
            for (i, key) in ["Space", "D", "S", "T", "N"].iter().enumerate() {
                let old = format!("Ctrl+Shift+{key}");
                let next = platform_shortcut(key);
                if same(&values[i], &old) && values.iter().enumerate().all(|(j, value)| i == j || !same(value, &next)) { values[i] = next; }
            }
            [self.shortcut, self.drawing_shortcut, self.screenshot_shortcut, self.text_shortcut, self.note_shortcut] = values;
        }
        self.shortcut_platform = std::env::consts::OS.into();
    }
    pub fn upgrade_ollama_context(&mut self, saved: &serde_json::Value) {
        if saved["ollama_context_default_version"].as_u64().unwrap_or(0) < 1 && self.ollama_context_length == 16384 {
            self.ollama_context_length = 32768;
        }
        self.ollama_context_default_version = 1;
    }
    pub fn upgrade_ollama_web_policy(&mut self, saved: &serde_json::Value) {
        // The previous default enabled search without consent to WebFetch's domain check.
        if saved["ollama_web_policy_version"].as_u64().unwrap_or(0) < 1 {
            self.ollama_web_search = false;
        }
        self.ollama_web_policy_version = 1;
    }
    pub fn remote_stt(&self) -> bool { self.stt_provider != "local" }
    pub fn provider_label(&self) -> &'static str { match self.assistant_provider.as_str() { "custom" => "Endpoint", "ollama" => "Ollama", "codex" => "Codex", "gemini" => "Antigravity", _ => "Claude" } }
    pub fn connector_settings(&self) -> Self {
        let mut effective = self.clone();
        if self.assistant_provider == "codex" { effective.claude_connectors=self.codex_connectors; effective.disabled_connectors=self.codex_disabled_connectors.clone(); effective.trusted_tools=self.codex_trusted_tools.clone(); }
        if self.assistant_provider == "gemini" { effective.claude_connectors=self.gemini_connectors; effective.disabled_connectors=self.gemini_disabled_connectors.clone(); effective.trusted_tools=self.gemini_trusted_tools.clone(); }
        if self.assistant_provider == "ollama" { effective.claude_connectors=false; effective.disabled_connectors.clear(); effective.trusted_tools=if self.ollama_server_url.trim().is_empty() { self.ollama_trusted_tools.clone() } else { self.ollama_server_trusted_tools.clone() }; }
        effective
    }
    pub fn active_disabled_mut(&mut self) -> &mut Vec<String> { match self.assistant_provider.as_str() { "codex" => &mut self.codex_disabled_connectors, "gemini" => &mut self.gemini_disabled_connectors, _ => &mut self.disabled_connectors } }
    pub fn active_trusted_mut(&mut self) -> &mut Vec<String> { match self.assistant_provider.as_str() { "ollama" if !self.ollama_server_url.trim().is_empty() => &mut self.ollama_server_trusted_tools, "ollama" => &mut self.ollama_trusted_tools, "codex" => &mut self.codex_trusted_tools, "gemini" => &mut self.gemini_trusted_tools, _ => &mut self.trusted_tools } }
    fn paths_mut(&mut self) -> [&mut String; 8] {
        [
            &mut self.python_bin,
            &mut self.whisper_model,
            &mut self.live_whisper_model,
            &mut self.segmentation_model,
            &mut self.embedding_model,
            &mut self.claude_bin,
            &mut self.codex_bin,
            &mut self.gemini_bin,
        ]
    }
    pub fn portable_at(&self, root: &Path) -> Self {
        let mut portable = self.clone();
        for value in portable.paths_mut() {
            if let Ok(relative) = Path::new(value.as_str()).strip_prefix(root) {
                if relative
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                {
                    *value = format!("@engine/{}", relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        portable
    }
    pub fn resolve_at(&mut self, root: &Path) -> Result<(), String> {
        for value in self.paths_mut() {
            let normalized = value.replace('\\', "/");
            let legacy = normalized
                .split_once("/.tools/engine/")
                .map(|(_, tail)| tail);
            let known = [
                "venv/Scripts/python.exe",
                "venv/bin/python",
                "models/whisper-turbo",
                "models/whisper-tiny",
                "models/whisper-small",
                "models/segmentation/model.onnx",
                "models/embedding.onnx",
                "claude.exe",
            ];
            let relative = normalized
                .strip_prefix("@engine/")
                .or_else(|| legacy.filter(|path| known.contains(path)));
            if let Some(relative) = relative {
                let path = Path::new(relative);
                if relative.is_empty()
                    || !path.components().all(|c| matches!(c, Component::Normal(_)))
                {
                    return Err("Percorso del motore locale non valido.".into());
                }
                *value = root.join(path).to_string_lossy().into_owned();
            }
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.stt_provider == "custom" { self.custom_stt.validate("stt")?; }
        if self.tts_provider == "custom" { self.custom_tts.validate("tts")?; }
        if self.assistant_provider == "custom" { self.custom_chat.validate("chat")?; }
        if self.sharing.name.chars().count() > 80 || self.sharing.name.chars().any(char::is_control) {
            return Err("Il nome può contenere al massimo 80 caratteri su una riga.".into());
        }
        if !["local", "groq", "custom"].contains(&self.stt_provider.as_str()) {
            return Err("Fornitore della trascrizione non valido.".into());
        }
        if self.remote_stt() && self.live_transcription {
            return Err("Con Groq l’anteprima locale deve essere disattivata.".into());
        }
        if !["edge", "piper", "kokoro", "qwen", "elevenlabs", "custom"].contains(&self.tts_provider.as_str()) {
            return Err("Fornitore della voce non valido.".into());
        }
        if !crate::piper::known_voice(&self.tts_piper_voice) { return Err("Voce Piper non disponibile.".into()); }
        if !crate::kokoro::known_voice(&self.tts_kokoro_voice) { return Err("Voce Kokoro non disponibile.".into()); }
        if !crate::qwen_tts::known_voice(&self.tts_qwen_voice) || !["cpu","cuda"].contains(&self.tts_qwen_device.as_str())
            || !crate::qwen_tts::LANGUAGES.contains(&self.tts_qwen_language.as_str()) {return Err("Voce, lingua o dispositivo Qwen3-TTS non valido.".into());}
        for value in [&self.tts_edge_voice, &self.tts_eleven_voice, &self.tts_eleven_model] {
            if value.len() > 200 || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
                return Err("Identificativo voce o modello TTS non valido.".into());
            }
        }
        if self.system_audio_output.len() > 4096 || self.system_audio_output.chars().any(char::is_control) {
            return Err("Uscita audio non valida.".into());
        }
        if self.dictionary.len() > 100 || self.dictionary.iter().any(|entry| {
            entry.term.trim().is_empty() || entry.term.chars().count() > 80 || entry.term.chars().any(char::is_control)
        }) { return Err("Dizionario non valido: massimo 100 termini di 80 caratteri.".into()); }
        if self.keep_audio && self.keep_video {
            return Err("Scegli se conservare il video con audio oppure il solo audio.".into());
        }
        let mut shortcuts = Vec::new();
        for value in [&self.shortcut, &self.drawing_shortcut, &self.screenshot_shortcut, &self.text_shortcut, &self.note_shortcut] {
            let shortcut: tauri_plugin_global_shortcut::Shortcut = value.parse().map_err(|_| "Combinazione di tasti non valida.")?;
            if shortcut.mods.is_empty() || shortcuts.contains(&shortcut) { return Err("Le scorciatoie devono essere diverse e includere Ctrl, Alt, Shift o Cmd.".into()); }
            shortcuts.push(shortcut);
        }
        if !(1..=30).contains(&self.brush_size) || !crate::screen::valid_color(&self.brush_color) {
            return Err("Pennello non valido: scegli un colore e uno spessore da 1 a 30.".into());
        }
        if !["claude","codex","ollama","custom"].contains(&self.assistant_provider.as_str()) { return Err("Fornitore AI non disponibile.".into()); }
        if !self.ollama_server_url.trim().is_empty() { crate::ollama::server_base(&self.ollama_server_url)?; }
        if !self.ollama_model.is_empty() && !crate::ollama::valid_model(&self.ollama_model) { return Err("Identificativo del modello Ollama non valido.".into()); }
        if self.ollama_context_length < 8192 { return Err("Il contesto Ollama deve essere di almeno 8192 token.".into()); }
        for model in [&self.codex_model, &self.gemini_model] {
            if model.len() > 200 || model.chars().any(|c| c.is_control() || c.is_whitespace()) { return Err("Identificativo modello AI non valido.".into()); }
        }
        if !["auto", "cpu"].contains(&self.compute_mode.as_str()) {
            return Err("Modalità di elaborazione non valida.".into());
        }
        if !["auto", "cpu"].contains(&self.live_compute_mode.as_str()) {
            return Err("Accelerazione dell’anteprima non valida.".into());
        }
        if self.titlebar_color != "theme"
            && self.titlebar_color != "system"
            && !(self.titlebar_color.len() == 7
                && self.titlebar_color.starts_with('#')
                && self.titlebar_color[1..]
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit()))
        {
            return Err("Colore della barra del titolo non valido.".into());
        }
        if !["dark", "light"].contains(&self.theme.as_str()) {
            return Err("Tema non valido.".into());
        }
        if self.project_instructions.chars().count() > 8000
            || self.project_instructions.contains('\0')
        {
            return Err(
                "Le istruzioni di progetto possono contenere al massimo 8.000 caratteri.".into(),
            );
        }
        if self.system_prompt.chars().count() > 16000 || self.system_prompt.contains('\0') {
            return Err("Il prompt di sistema può contenere al massimo 16.000 caratteri.".into());
        }
        if self.retention_months > 24 {
            return Err("Conservazione consentita: da 1 a 24 mesi oppure senza scadenza.".into());
        }
        if !self.microphone && !self.system_audio {
            return Err("Attiva almeno una sorgente audio.".into());
        }
        if !["it", "en", "auto"].contains(&self.language.as_str()) {
            return Err("Lingua non supportata.".into());
        }
        let shortcut: tauri_plugin_global_shortcut::Shortcut = self
            .shortcut
            .parse()
            .map_err(|_| "Combinazione di tasti non valida.")?;
        if shortcut.mods.is_empty() {
            return Err(
                "La shortcut richiede almeno un modificatore: Ctrl, Alt, Shift o Cmd.".into(),
            );
        }
        if self.claude_model.trim().is_empty()
            || self.claude_model.len() > 200
            || self
                .claude_model
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
        {
            return Err("Identificativo modello Claude non valido.".into());
        }
        Ok(())
    }
    pub fn whisper_ready(&self) -> bool {
        !engine_dir().join("venv-installing.json").exists()
            && Path::new(&self.python_bin).is_file()
            && model_ready(Path::new(&self.whisper_model), false)
    }
    pub fn preview_ready(&self) -> bool {
        !self.live_transcription || (Path::new(&self.python_bin).is_file() && preview_model_ready(Path::new(&self.live_whisper_model), self.live_compute_mode == "cpu"))
    }
    pub fn diarization_ready(&self) -> bool {
        Path::new(&self.python_bin).is_file()
            && Path::new(&self.segmentation_model).is_file()
            && Path::new(&self.embedding_model).is_file()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Call {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub duration_ms: i64,
    pub status: String,
    pub error: Option<String>,
    pub preview: String,
    pub origin: String,
    pub tags: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker: String,
    pub text: String,
    pub source: String,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub original_text: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub model: Option<String>,
    pub activity: Vec<crate::activity::Activity>,
    #[serde(default)]
    pub created_at: Option<String>,
}
#[derive(Serialize)]
pub struct Detail {
    pub call: Call,
    pub segments: Vec<Segment>,
    pub messages: Vec<Message>,
    pub audio_directory: Option<String>,
    pub video_path: Option<String>,
    pub screenshots: Vec<Screenshot>,
    pub notes: Vec<CallNote>,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CallNote {
    pub id: String,
    pub timestamp_ms: i64,
    pub text: String,
}

pub fn markdown_detail(detail: &Detail) -> String {
    let mut text = markdown(&detail.call, &detail.segments);
    if !detail.notes.is_empty() {
        text.push_str("\n\n## Note manuali\n\n");
        for note in &detail.notes {
            text.push_str(&format!("**[{:02}:{:02}] Nota manuale**\n\n{}\n\n", note.timestamp_ms / 60000, (note.timestamp_ms / 1000) % 60, note.text));
        }
    }
    text
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Screenshot {
    pub id: String,
    pub timestamp_ms: i64,
    pub created_at: String,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Serialize, Debug)]
pub struct Capture {
    pub id: String,
    pub started_at: String,
    pub microphone: bool,
    pub system_audio: bool,
}
#[derive(Clone, Serialize, Default)]
pub struct Status {
    pub capture: Option<Capture>,
    pub processing: Option<String>,
    pub stage: String,
    pub chat_busy: bool,
    pub chat_call: Option<String>,
    pub queued: Vec<String>,
    pub live_segments: Vec<Segment>,
    pub live_stage: String,
}

#[derive(Serialize)]
pub struct TagCount {
    pub tag: String,
    pub count: usize,
}
#[derive(Serialize)]
pub struct CallPage {
    pub items: Vec<Call>,
    pub total: usize,
    pub has_more: bool,
    pub search_results: std::collections::BTreeMap<String, CallSearchResult>,
}
#[derive(Serialize)]
pub struct SearchHit {
    pub source: String,
    pub timestamp_ms: i64,
    pub excerpt: String,
}
#[derive(Serialize)]
pub struct CallSearchResult {
    pub count: usize,
    pub hits: Vec<SearchHit>,
}

pub fn markdown(call: &Call, segments: &[Segment]) -> String {
    let mut text = format!(
        "# {}\n\nData: {}\nDurata: {} minuti\n\n",
        call.title,
        call.created_at,
        call.duration_ms / 60000
    );
    for s in segments {
        text.push_str(&format!(
            "**[{:02}:{:02}] {}**\n\n{}\n\n",
            s.start_ms / 60000,
            (s.start_ms / 1000) % 60,
            s.speaker,
            s.text
        ));
          if let Some(warning) = &s.quality {text.push_str(&format!("*Da verificare: {warning}*\n\n"));}
    }
    text
}

#[cfg(test)]
mod tests {
    #[test]
    fn piper_provider_defaults_and_voice_validation_roundtrip() {
        let mut settings:super::Settings=serde_json::from_str("{}").unwrap();
        assert_eq!(settings.tts_piper_voice,"it_IT-serena-medium");
        settings.tts_provider="piper".into();settings.tts_enabled=true;
        settings.validate().unwrap();
        let restored:super::Settings=serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.tts_provider,"piper");
        settings.tts_piper_voice="../arbitrary".into();assert!(settings.validate().is_err());
    }
    #[test]
    fn cloud_transcription_is_opt_in_and_disables_local_preview() {
        let mut settings: super::Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.stt_provider, "local");
        settings.stt_provider = "groq".into();
        settings.validate().unwrap();
        let restored: super::Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.stt_provider, "groq");
        settings.live_transcription = true;
        assert!(settings.validate().is_err());
        settings.stt_provider = "invalid".into();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn screenshot_settings_upgrade_and_validate_distinct_shortcuts() {
        let mut settings:super::Settings=serde_json::from_str("{}").unwrap();
        assert_eq!(settings.brush_size,5);assert_eq!(settings.drawing_shortcut,"Ctrl+Shift+D");
        settings.validate().unwrap();
        settings.screenshot_shortcut=settings.drawing_shortcut.clone();assert!(settings.validate().is_err());
        settings.screenshot_shortcut="Ctrl+Shift+S".into();settings.brush_size=31;assert!(settings.validate().is_err());
        settings.brush_size=5;settings.brush_color="#invalid".into();assert!(settings.validate().is_err());
        let legacy=serde_json::json!({"shortcut":"Ctrl+Shift+D"});
        let mut settings:super::Settings=serde_json::from_value(legacy.clone()).unwrap();
        settings.upgrade_screen_shortcuts(&legacy);settings.validate().unwrap();
        assert_eq!(settings.shortcut,"Ctrl+Shift+D");assert_eq!(settings.drawing_shortcut,"Ctrl+Alt+Shift+D");
        let legacy=serde_json::json!({"shortcut":"Ctrl+Shift+T"});
        let mut settings:super::Settings=serde_json::from_value(legacy.clone()).unwrap();
        settings.upgrade_screen_shortcuts(&legacy);settings.validate().unwrap();
        assert_eq!(settings.text_shortcut,"Ctrl+Alt+Shift+T");
    }
    #[test]
    fn audio_retention_is_opt_in_and_round_trips() {
        assert!(!super::Settings::default().keep_audio);
        let mut settings:super::Settings=serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!settings.keep_audio);
        settings.keep_audio=true;
        let restored:super::Settings=serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(restored.keep_audio);
    }
    #[test]
    fn video_retention_is_opt_in_and_exclusive_with_audio() {
        let mut settings: super::Settings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!settings.keep_video);
        settings.keep_video = true;
        settings.validate().unwrap();
        let restored: super::Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(restored.keep_video);
        settings.keep_audio = true;
        assert!(settings.validate().is_err());
    }
    #[test]
    fn dictionary_is_optional_and_contains_only_hint_terms() {
        let mut settings: super::Settings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(settings.dictionary.is_empty());
        settings.dictionary.push(super::DictionaryEntry { term: "PARLO".into() });
        settings.validate().unwrap();
        let saved: super::Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(saved.dictionary[0].term, "PARLO");
        settings.dictionary.push(super::DictionaryEntry { term: "  ".into() });
        assert!(settings.validate().is_err());
    }
    #[test]
    fn audio_output_is_backward_compatible_and_persists_the_selected_route() {
        let mut settings: super::Settings = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(settings.system_audio_output.is_empty());
        for route in ["@communications", "synthetic-endpoint"] {
            settings.system_audio_output = route.into();
            settings.validate().unwrap();
            let saved: super::Settings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(saved.system_audio_output, route);
        }
        settings.system_audio_output = "invalid\nendpoint".into();
        assert!(settings.validate().is_err());
    }
    #[cfg(windows)]
    #[test]
    fn windows_vulkan_waits_for_cpu_adapter_before_reporting_model_ready() {
        let root=std::env::temp_dir().join(format!("parlo-vulkan-ready-{}",uuid::Uuid::new_v4()));
        let model=root.join("models/whisper-turbo");
        std::fs::create_dir_all(root.join("whisper-cpp")).unwrap(); std::fs::create_dir_all(&model).unwrap();
        for file in ["runtime.json","whisper-cli.exe"] {std::fs::write(root.join("whisper-cpp").join(file),b"fixture").unwrap();}
        std::fs::write(root.join("models/ggml-turbo.bin"),b"fixture").unwrap();
        assert!(!super::model_ready_at(&root,&model,false));
        for file in ["model.bin","config.json","tokenizer.json"]{std::fs::write(model.join(file),b"fixture").unwrap();}
        assert!(super::model_ready_at(&root,&model,false));
        assert!(root.starts_with(std::env::temp_dir()));std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn native_model_is_ready_without_an_unrequested_cpu_format_but_not_for_preview() {
        let root = std::env::temp_dir().join(format!("parlo-model-test-{}", uuid::Uuid::new_v4()));
        let model = root.join("models/whisper-turbo");
        let native = root.join("models/whisper-turbo-openvino");
        std::fs::create_dir_all(&native).unwrap();
        for file in ["presentio-model.json", "openvino_encoder_model.xml", "openvino_encoder_model.bin", "openvino_decoder_model.xml", "openvino_decoder_model.bin"] { std::fs::write(native.join(file), b"fixture").unwrap(); }
        assert!(super::model_ready_at(&root, &model, false));
        assert!(!super::model_ready_at(&root, &model, true));
        std::fs::remove_file(native.join("openvino_encoder_model.bin")).unwrap();
        assert!(!super::model_ready_at(&root, &model, false));
        std::fs::remove_dir_all(root).unwrap();
    }
    use super::*;
    #[test]
    fn old_installations_keep_claude_and_codex_paths_relocate() {
        let settings:Settings=serde_json::from_value(serde_json::json!({"claude_model":"old-user-choice"})).unwrap();assert_eq!(settings.assistant_provider,"claude");assert_eq!(settings.claude_model,"old-user-choice");
        let first=std::env::temp_dir().join("parlo-first");let second=std::env::temp_dir().join("parlo-second");let mut settings=settings;settings.codex_bin=first.join("codex-code/0.162.1/bin/codex.exe").to_string_lossy().into_owned();let mut portable=settings.portable_at(&first);assert_eq!(portable.codex_bin,"@engine/codex-code/0.162.1/bin/codex.exe");portable.resolve_at(&second).unwrap();assert_eq!(Path::new(&portable.codex_bin),second.join("codex-code/0.162.1/bin/codex.exe"));
    }
    #[test]
    fn settings_move_between_installations_without_personal_paths() {
        let first = std::env::temp_dir()
            .join("PARLO first installation")
            .join("engine");
        let second = std::env::temp_dir()
            .join("PARLO second installation")
            .join("engine");
        let mut settings = Settings::default();
        settings.whisper_model = first
            .join("models/whisper-turbo")
            .to_string_lossy()
            .into_owned();
        settings.python_bin = first
            .join("venv/Scripts/python.exe")
            .to_string_lossy()
            .into_owned();
        settings.claude_bin = "claude".into();
        let mut moved = settings.portable_at(&first);
        assert_eq!(moved.whisper_model, "@engine/models/whisper-turbo");
        assert_eq!(moved.python_bin, "@engine/venv/Scripts/python.exe");
        moved.resolve_at(&second).unwrap();
        assert_eq!(
            Path::new(&moved.whisper_model),
            second.join("models/whisper-turbo")
        );
        assert_eq!(moved.claude_bin, "claude");
        moved.whisper_model = "@engine/../outside".into();
        assert!(moved.resolve_at(&second).is_err());
    }
    #[test]
    fn legacy_project_paths_relocate_and_custom_paths_are_preserved() {
        let mut settings = Settings::default();
        settings.whisper_model =
            "C:/Users/AnotherUser/PARLO/.tools/engine/models/whisper-small".into();
        settings.embedding_model = "C:/Custom models/speaker.onnx".into();
        let root = Path::new("New installation/engine");
        settings.resolve_at(root).unwrap();
        assert_eq!(
            Path::new(&settings.whisper_model),
            root.join("models/whisper-small")
        );
        assert_eq!(settings.embedding_model, "C:/Custom models/speaker.onnx");
    }
    #[test]
    fn preview_is_optional_and_shortcuts_and_title_colors_are_validated() {
        let mut settings = Settings::default();
        assert!(!settings.live_transcription);
        assert_eq!(settings.compute_mode, "auto");
        settings.compute_mode = "cpu".into();
        assert!(settings.validate().is_ok());
        settings.compute_mode = "unknown".into();
        assert!(settings.validate().is_err());
        settings.compute_mode = "auto".into();
        settings.shortcut = "Space".into();
        assert!(settings.validate().is_err());
        settings.shortcut = "Ctrl+Shift+F8".into();
        assert!(settings.validate().is_ok());
        settings.shortcut = "Ctrl+NotAKey".into();
        assert!(settings.validate().is_err());
        settings.shortcut = "Ctrl+Shift+Space".into();
        settings.titlebar_color = "#00ffaa".into();
        assert!(settings.validate().is_ok());
        settings.titlebar_color = "red".into();
        assert!(settings.validate().is_err());
    }
    #[test]
    fn reject_no_sources_and_invalid_retention() {
        let mut s = Settings::default();
        s.microphone = false;
        s.system_audio = false;
        assert!(s.validate().is_err());
        s.microphone = true;
        s.retention_months = 25;
        assert!(s.validate().is_err());
    }
    #[test]
    fn retention_defaults_to_three_months_and_supports_no_expiry() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.retention_months, 3);
        settings.retention_months = 0; // No automatic expiry.
        assert!(settings.validate().is_ok());
        let saved = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&saved).unwrap().retention_months, 0);
    }
    #[test]
    fn old_settings_default_to_dark_and_theme_round_trips() {
        let mut settings: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings.theme, "dark");
        settings.theme = "light".into();
        assert!(settings.validate().is_ok());
        let saved = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<Settings>(&saved).unwrap().theme,
            "light"
        );
        settings.theme = "unknown".into();
        assert!(settings.validate().is_err());
    }
}
