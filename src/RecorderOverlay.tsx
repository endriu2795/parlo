import { t } from "./i18n";
import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChevronLeft, Circle, Maximize2, Mic, MicOff, Minus, Monitor, Square, TriangleAlert } from "lucide-react";
import { api } from "./api";
import CaptureActions, { useNoteShortcut } from "./CaptureActions";
import { DockButton } from "./EdgeDock";
import { fmtTime } from "./format";
import type { DockEdge, Settings, Status } from "./types";

type Source = "microphone" | "system";
const silent = { microphone: { level: 0, dbfs: -60 }, system: { level: 0, dbfs: -60 } };
const connected = { microphone: false, system: false };
type MicHealth = { state: "ok" | "silent" | "unavailable"; device: string; selected: string };
const healthy: MicHealth = { state: "ok", device: "", selected: "" };

/** Switches the recording microphone in place; the recording continues. */
function MicrophonePicker({ health, onError }: { health: MicHealth; onError: (error: string) => void }) {
  const [inputs, setInputs] = useState<{ id: string; name: string }[]>([]);
  useEffect(() => {
    // Refresh when the device state changes, e.g. after reconnecting a headset.
    void api.audioInputs().then(setInputs).catch(e => onError(String(e)));
  }, [health.state, onError]);
  if (!inputs.length) return null;
  const missing = health.selected && !inputs.some(input => input.id === health.selected);
  return <select className="recorder-mic" value={health.selected} title={health.device} aria-label={t("Microfono in uso")}
    onChange={event => void api.setRecordingMicrophone(event.target.value).catch(e => onError(String(e)))}>
    <option value="">{health.device && !health.selected ? t("Predefinito · {0}", health.device) : t("Predefinito di Windows")}</option>
    {inputs.map(input => <option value={input.id} key={input.id}>{input.name}</option>)}
    {missing && <option value={health.selected}>{health.device || t("Microfono selezionato")}</option>}
  </select>;
}

function MicWarning({ health, onDismiss, onError }: { health: MicHealth; onDismiss: () => void; onError: (error: string) => void }) {
  const name = health.device || t("microfono");
  return <div className="recorder-mic-warning" role="alert">
    <p>
      <TriangleAlert size={14} />
      {health.state === "silent"
        ? t("Nessun segnale da {0}. Se stai parlando, controlla le cuffie o cambia microfono.", name)
        : t("{0} non risponde. Ricollega le cuffie o scegli un altro microfono.", name)}
    </p>
    <div>
      <MicrophonePicker health={health} onError={onError} />
      <button className="text-button" onClick={onDismiss}>{t("Ignora")}</button>
    </div>
  </div>;
}

function SourceMeters({ microphone, systemAudio, reconnecting, health, onError }: {
  microphone: boolean;
  systemAudio: boolean;
  reconnecting: typeof connected;
  health: MicHealth;
  onError: (error: string) => void;
}) {
  const [levels, setLevels] = useState(silent);
  useEffect(() => {
    // Subscribe only while the meters are mounted, never for the REC tab or
    // compact controls. Meter events must not redraw the entire recorder.
    const listener = listen<{ source: Source; level: number; dbfs: number }>("audio-level", e => {
      if (!document.hidden) {
        setLevels(previous => ({ ...previous, [e.payload.source]: { level: e.payload.level, dbfs: e.payload.dbfs } }));
      }
    });
    return () => { void listener.then(unlisten => unlisten()); };
  }, []);
  const sources = [
    { key: "microphone" as const, label: t("Microfono"), icon: Mic, active: microphone },
    { key: "system" as const, label: t("Audio PC"), icon: Monitor, active: systemAudio },
  ];
  return <div className="recorder-body">
    {sources.map(({ key, label, icon: Icon, active }) => {
      const picker = key === "microphone" && active && !reconnecting[key];
      return <span key={key}>
        <Icon size={14} />
        {!picker && label}
        <meter min="0" max="1" value={levels[key].level} aria-label={t("Livello {0}", label.toLowerCase())} aria-valuetext={`${Math.round(levels[key].dbfs)} dBFS`} />
        {picker
          ? <MicrophonePicker health={health} onError={onError} />
          : <small>{active ? (reconnecting[key] ? t("Riconnessione…") : t("Attivo")) : t("Spento")}</small>}
      </span>;
    })}
  </div>;
}

/** Floating recording indicator, rendered in its own always-on-top window. */
export default function RecorderOverlay() {
  const [capture, setCapture] = useState<Status["capture"]>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [reconnecting, setReconnecting] = useState(connected);
  const [health, setHealth] = useState(healthy);
  const [dismissed, setDismissed] = useState(false);
  const [expanded, setExpanded] = useState(true);
  const [dock, setDock] = useState<DockEdge | null>();
  const dockRevision = useRef(0);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [now, setNow] = useState(Date.now());
  const [visible, setVisible] = useState(!document.hidden);
  const captureId = capture?.id;
  // The recorder needs only capture identity, not streamed transcript/status
  // updates that would otherwise also rerender the small docked REC tab.
  const receiveStatus = useCallback((value: Status) => {
    setCapture(previous => previous?.id === value.capture?.id ? previous : value.capture);
  }, []);
  useNoteShortcut(!!captureId, setError);

  useEffect(() => {
    document.documentElement.dataset.theme = settings?.theme || "dark";
  }, [settings?.theme]);
  useEffect(() => {
    setReconnecting(connected);
    setHealth(healthy);
  }, [captureId]);
  useEffect(() => {
    // "Ignora" hides one episode; a new loss of signal is shown again.
    if (health.state === "ok") setDismissed(false);
  }, [health.state]);
  useEffect(() => {
    // Status and settings arrive by event; reload them when events may have been missed.
    const reload = () => {
      setVisible(!document.hidden);
      void api.status().then(receiveStatus).catch(e => setError(String(e)));
      if (!document.hidden) void api.settings().then(setSettings).catch(e => setError(String(e)));
      const revision = dockRevision.current;
      void api.recorderDockStatus().then(value => { if (revision === dockRevision.current) setDock(value); }).catch(e => setError(String(e)));
    };
    reload();
    window.addEventListener("focus", reload);
    document.addEventListener("visibilitychange", reload);
    const listeners = [
      listen("configuration-changed", reload),
      listen<Status>("status-changed", e => receiveStatus(e.payload)),
      listen<boolean>("recorder-expanded", e => setExpanded(e.payload)),
      listen<DockEdge | null>("recorder-docked", e => { dockRevision.current++; setDock(e.payload); }),
      listen("screenshot-saved", () => setMessage(t("Screenshot salvato nella trascrizione"))),
      listen<"dark" | "light">("theme-changed", e => setSettings(previous => previous ? { ...previous, theme: e.payload } : previous)),
      listen<string>("app-error", e => setError(e.payload)),
      // Repeated every tick: keep the same object to avoid redrawing the recorder.
      listen<MicHealth>("microphone-health", e => setHealth(previous =>
        previous.state === e.payload.state && previous.device === e.payload.device && previous.selected === e.payload.selected
          ? previous : e.payload)),
      listen<{ source: Source; reconnecting: boolean }>("audio-route", e =>
        setReconnecting(previous => previous[e.payload.source] === e.payload.reconnecting
          ? previous : { ...previous, [e.payload.source]: e.payload.reconnecting })),
    ];
    return () => {
      window.removeEventListener("focus", reload);
      document.removeEventListener("visibilitychange", reload);
      listeners.forEach(stop => void stop.then(unlisten => unlisten()));
    };
  }, [receiveStatus]);
  useEffect(() => {
    if (!dock || !captureId) return;
    let second = 0;
    const first = requestAnimationFrame(() => {
      second = requestAnimationFrame(() => {
        void api.recorderFrameReady(captureId).catch(e => setError(String(e)));
      });
    });
    return () => {
      cancelAnimationFrame(first);
      cancelAnimationFrame(second);
    };
  }, [captureId, dock]);
  useEffect(() => {
    if (!captureId || dock || !visible) return;
    const tick = () => setNow(Date.now());
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => clearInterval(timer);
  }, [captureId, dock, visible]);
  useEffect(() => {
    if (!message) return;
    const timer = setTimeout(() => setMessage(""), 3500);
    return () => clearTimeout(timer);
  }, [message]);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await action();
      receiveStatus(await api.status());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const resize = (value: boolean) => {
    setExpanded(value);
    void api.recorderSize(value).catch(e => setError(String(e)));
  };

  const micWarning = capture?.microphone && health.state !== "ok" && !dismissed;
  // Wait for the native placement before rendering the panel into the small tab window.
  if (dock === undefined) return null;
  if (dock) {
    return (
      <button className={`recorder-edge-tab${micWarning ? " mic-warning" : ""}`}
        aria-label={micWarning ? t("Problema al microfono: mostra controlli") : t("Registrazione in corso: mostra controlli")}
        title={micWarning ? t("Nessun segnale dal microfono") : undefined}
        onClick={() => void api.dockRecorder(null).catch(e => setError(String(e)))}>
        {micWarning
          ? <><MicOff size={14} /><span className="recorder-tab-label">MIC</span></>
          : <><span className="recorder-tab-dot" /><span className="recorder-tab-label">REC</span></>}
        <ChevronLeft size={15} />
      </button>
    );
  }

  if (!capture) return null;
  const elapsed = now - new Date(capture.started_at).getTime();
  const anyReconnecting = reconnecting.microphone || reconnecting.system;
  const footer = error || message || (anyReconnecting
    ? t("Cambio dispositivo · la registrazione continua")
    : settings?.live_transcription ? t("Il testo appare nell’app mentre parli") : t("Registrazione in corso"));

  return (
    <div className={`recorder ${expanded ? "expanded" : "compact"}${micWarning ? " mic-warning" : ""}`}>
      <div className="recorder-header">
        <div className="recorder-drag" data-tauri-drag-region="deep">
          <Circle size={8} className="record-dot" fill="currentColor" />
          {micWarning && !expanded && <MicOff size={14} className="recorder-mic-alert" />}
          <strong>
            {micWarning && !expanded
              ? (health.state === "silent" ? t("Mic muto") : t("Mic non disponibile"))
              : anyReconnecting && !expanded ? t("Riconnessione…") : expanded ? "PARLO" : t("Registrazione")}
          </strong>
          <span className="mono">{fmtTime(elapsed)}</span>
        </div>
        <DockButton nativeTooltip={false} onClick={() => void api.dockRecorder("bottom").catch(e => setError(String(e)))} />
        <button className="icon-button" aria-label={expanded ? t("Riduci indicatore") : t("Espandi indicatore")} onClick={() => resize(!expanded)}>
          {expanded ? <Minus size={15} /> : <Maximize2 size={14} />}
        </button>
      </div>
      {expanded && <>
        {micWarning
          ? <MicWarning health={health} onDismiss={() => setDismissed(true)} onError={setError} />
          : <SourceMeters key={capture.id} microphone={capture.microphone} systemAudio={capture.system_audio} reconnecting={reconnecting} health={health} onError={setError} />}
        <CaptureActions settings={settings} disabled={busy} run={action => void run(action)} />
        <div className="recorder-footer">
          <span role={error ? "alert" : "status"}>{footer}</span>
          <button onClick={() => void run(api.stop)} disabled={busy} aria-label={t("Termina registrazione")}>
            <Square size={11} fill="currentColor" />
            {t("Termina")}
          </button>
        </div>
      </>}
    </div>
  );
}
