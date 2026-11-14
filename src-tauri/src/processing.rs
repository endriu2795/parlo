use std::{collections::VecDeque, sync::Arc};

use tauri::{AppHandle, Emitter, State};

use crate::{
    archive_changed, changed, db, engine_child::EngineChild, finalize_audio, locale::tr,
    model::{Settings, Status}, temp_dir, AppState, LockExt, Result, Shared,
};

const CANCELLED: &str = "Elaborazione interrotta. Puoi riprovare dalla call.";
type Job = (String, Settings);

struct Active {
    id: String,
    cancelled: bool,
    child: Option<Arc<EngineChild>>,
}

#[derive(Default)]
pub struct Queue {
    waiting: VecDeque<Job>,
    active: Option<Active>,
    worker_running: bool,
}

impl Queue {
    fn contains(&self, id: &str) -> bool {
        self.active.as_ref().is_some_and(|job| job.id == id)
            || self.waiting.iter().any(|(queued, _)| queued == id)
    }

    fn push(&mut self, job: Job) -> bool {
        self.waiting.push_back(job);
        !std::mem::replace(&mut self.worker_running, true)
    }

    fn next(&mut self) -> Option<Job> {
        let job = self.waiting.pop_front();
        self.active = job.as_ref().map(|(id, _)| Active {
            id: id.clone(),
            cancelled: false,
            child: None,
        });
        self.worker_running = job.is_some();
        job
    }

    fn update_status(&self, status: &mut Status) {
        status.processing = self.active.as_ref().map(|job| job.id.clone());
        status.queued = self.waiting.iter().map(|(id, _)| id.clone()).collect();
    }
}

pub fn ensure_active(state: &AppState) -> Result<()> {
    if state.processing.guard().active.as_ref().is_some_and(|job| !job.cancelled) {
        Ok(())
    } else {
        Err(CANCELLED.into())
    }
}

pub fn report_stage(app: &AppHandle, state: &AppState, stage: String) {
    {
        let queue = state.processing.guard();
        if !queue.active.as_ref().is_some_and(|job| !job.cancelled) {
            return;
        }
        state.status.guard().stage = stage;
    }
    changed(app, state);
}

pub fn attach_child(state: &AppState, child: Arc<EngineChild>) {
    let mut queue = state.processing.guard();
    if let Some(job) = &mut queue.active {
        if job.cancelled { child.cancel(); }
        job.child = Some(child);
    }
}

pub fn detach_child(state: &AppState) {
    if let Some(job) = &mut state.processing.guard().active {
        job.child = None;
    }
}

// The caller holds operation while preparing and enqueuing a recording or import.
pub fn enqueue(app: &AppHandle, state: &Shared, id: &str) -> Result<()> {
    let settings = db::get_settings(&state.db.guard())?;
    let dir = temp_dir(state, id)?;
    if !["microphone.wav", "system.wav", "registrazione.wav"].iter().any(|name| dir.join(name).is_file()) {
        return Err("Non è disponibile audio temporaneo da elaborare.".into());
    }
    crate::stt::ensure_ready(&settings)?;
    if state.processing.guard().contains(id) {
        return Err("Questa call è già in coda.".into());
    }
    {
        let conn = state.db.guard();
        let previous = db::detail(&conn, id)?.call.error;
        db::status(&conn, id, "pending", None, previous.as_deref())?;
    }
    let spawn = {
        let mut queue = state.processing.guard();
        let spawn = queue.push((id.into(), settings));
        queue.update_status(&mut state.status.guard());
        spawn
    };
    if spawn {
        let app = app.clone();
        let state = state.clone();
        std::thread::spawn(move || run(&app, &state));
    }
    Ok(())
}

fn run(app: &AppHandle, state: &Shared) {
    let _power = crate::power::ProcessingPower::new();
    loop {
        let job = {
            let _operation = state.operation.guard();
            let mut queue = state.processing.guard();
            let job = queue.next();
            if job.is_none() { state.engine.guard().take(); }
            let mut status = state.status.guard();
            queue.update_status(&mut status);
            status.stage = job.as_ref().map_or("", |(_, settings)| {
                if settings.remote_stt() {
                    tr("Preparazione della trascrizione tramite endpoint…", "Preparing endpoint transcription…")
                } else {
                    tr("Preparazione del motore locale…", "Preparing the local engine…")
                }
            }).into();
            job
        };
        changed(app, state);
        let Some((id, settings)) = job else { break };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            transcribe(app, state, &id, &settings)
        })).unwrap_or_else(|panic| {
            detach_child(state);
            state.engine.guard().take();
            Err(format!("Errore interno durante l’elaborazione: {}", crate::crash::message(&*panic)))
        });
        if let Err(error) = result {
            let _operation = state.operation.guard();
            let cancelled = ensure_active(state).is_err();
            let reason = if cancelled { CANCELLED } else { &error };
            if let Err(error) = db::status(&state.db.guard(), &id, "failed", None, Some(reason)) {
                let _ = app.emit("app-error", error);
            }
            if !cancelled { let _ = app.emit("app-error", error); }
        } else {
            crate::notifications::completed(app, state, &id);
            crate::sharing::auto_publish(app, state, &id);
        }
        archive_changed(app, Some(&id));
    }
}

fn transcribe(app: &AppHandle, state: &Shared, id: &str, settings: &Settings) -> Result<()> {
    let previous = db::detail(&state.db.guard(), id)?.call.error;
    let warning = previous.filter(|e| e.starts_with("Registrazione incompleta:") || e.starts_with("Discontinuità audio:") || e.starts_with("Discontinuità registrazione:"));
    {
        let _operation = state.operation.guard();
        ensure_active(state)?;
        db::status(&state.db.guard(), id, "processing", None, warning.as_deref())?;
    }
    archive_changed(app, Some(id));
    let directory = temp_dir(state, id)?;
    if directory.join("keep-video").is_file() {
        ensure_active(state)?;
        report_stage(app, state, tr("Salvataggio video con audio…", "Saving video with audio…").into());
        crate::video::finalize(state, id, settings)?;
        crate::detail_changed(app, id);
    }
    ensure_active(state)?;
    let segments = if settings.remote_stt() {
        crate::stt::transcribe(app, state, settings, &directory)?
    } else {
        crate::process::transcribe(app, state, settings, &directory)?
    };
    if segments.is_empty() {
        return Err("Trascrizione vuota. Audio temporaneo conservato: verifica il microfono e riprova, oppure elimina la call.".into());
    }
    let _operation = state.operation.guard();
    ensure_active(state)?;
    db::store_segments(&mut state.db.guard(), id, &segments)?;
    finalize_audio(state, id)?;
    db::status(&state.db.guard(), id, "ready", None, warning.as_deref())
}

#[tauri::command]
pub fn cancel_processing(app: AppHandle, state: State<Shared>, id: String) -> Result<()> {
    let operation = state.operation.guard();
    let detail = db::detail(&state.db.guard(), &id)?;
    if !["pending", "processing"].contains(&detail.call.status.as_str()) {
        return Err("L’elaborazione di questa call è già terminata.".into());
    }
    let mut queue = state.processing.guard();
    let waiting = queue.waiting.iter().position(|(queued, _)| queued == &id);
    if let Some(index) = waiting {
        db::status(&state.db.guard(), &id, "failed", None, Some(CANCELLED))?;
        queue.waiting.remove(index);
        queue.update_status(&mut state.status.guard());
    } else if let Some(job) = queue.active.as_mut().filter(|job| job.id == id) {
        if !job.cancelled {
            job.cancelled = true;
            state.status.guard().stage = "Interruzione dell’elaborazione…".into();
            if let Some(child) = &job.child { child.cancel(); }
        }
    } else {
        return Err("Questa call non è più in elaborazione.".into());
    }
    drop(queue);
    drop(operation);
    changed(&app, &state);
    if waiting.is_some() { archive_changed(&app, Some(&id)); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelling_before_pickup_preserves_the_worker_and_the_next_job() {
        let mut queue = Queue::default();
        assert!(queue.push(("first".into(), Settings::default())));
        let mut status = Status::default();
        queue.update_status(&mut status);
        assert!(status.processing.is_none());
        assert_eq!(status.queued, ["first"]);

        queue.waiting.remove(0);
        let settings = Settings { language: "en".into(), ..Settings::default() };
        assert!(!queue.push(("second".into(), settings)));
        let (id, settings) = queue.next().unwrap();
        assert_eq!((id.as_str(), settings.language.as_str()), ("second", "en"));
        queue.update_status(&mut status);
        assert_eq!(status.processing.as_deref(), Some("second"));
        assert!(status.queued.is_empty());
        assert!(queue.contains("second"));
        assert!(!queue.contains("first"));
        status.capture = Some(crate::model::Capture {
            id: "new-call".into(),
            started_at: String::new(),
            microphone: true,
            system_audio: true,
        });
        assert!(queue.next().is_none());
        queue.update_status(&mut status);
        assert!(status.processing.is_none());
        assert_eq!(status.capture.unwrap().id, "new-call");
        assert!(queue.push(("third".into(), Settings::default())));
    }
}
