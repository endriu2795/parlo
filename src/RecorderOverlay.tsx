import { t } from "./i18n";
import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ChevronLeft, Circle, Maximize2, Mic, Minus, Monitor, Square } from "lucide-react";
import { api } from "./api";
import CaptureActions, { useNoteShortcut } from "./CaptureActions";
import { DockButton } from "./EdgeDock";
import { fmtTime } from "./format";
import type { DockEdge, Settings, Status } from "./types";

type Source = "microphone" | "system";
const silent = { microphone: { level: 0, dbfs: -60 }, system: { level: 0, dbfs: -60 } };
const connected = { microphone: false, system: false };

function SourceMeters({ microphone, systemAudio, reconnecting }: {
  microphone: boolean;
  systemAudio: boolean;
  reconnecting: typeof connected;
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
    {sources.map(({ key, label, icon: Icon, active }) => (
      <span key={key}>
        <Icon size={14} />
        {label}
        <meter min="0" max="1" value={levels[key].level} aria-label={t("Livello {0}", label.toLowerCase())} aria-valuetext={`${Math.round(levels[key].dbfs)} dBFS`} />
        <small>{active ? (reconnecting[key] ? t("Riconnessione…") : t("Attivo")) : t("Spento")}</small>
      </span>
    ))}
  </div>;
}

/** Floating recording indicator, rendered in its own always-on-top window. */
export default function RecorderOverlay() {
  const [capture, setCapture] = useState<Status["capture"]>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [reconnecting, setReconnecting] = useState(connected);
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
  }, [captureId]);
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
      listen("calls-changed", reload),
      listen<Status>("status-changed", e => receiveStatus(e.payload)),
      listen<boolean>("recorder-expanded", e => setExpanded(e.payload)),
      listen<DockEdge | null>("recorder-docked", e => { dockRevision.current++; setDock(e.payload); }),
      listen("screenshot-saved", () => setMessage(t("Screenshot salvato nella trascrizione"))),
      listen<"dark" | "light">("theme-changed", e => setSettings(previous => previous ? { ...previous, theme: e.payload } : previous)),
      listen<string>("app-error", e => setError(e.payload)),
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

  // Wait for the native placement before rendering the panel into the small tab window.
  if (dock === undefined) return null;
  if (dock) {
    return (
      <button className="recorder-edge-tab" aria-label={t("Registrazione in corso: mostra controlli")} onClick={() => void api.dockRecorder(null).catch(e => setError(String(e)))}>
        <span className="recorder-tab-dot" /><span className="recorder-tab-label">REC</span><ChevronLeft size={15} />
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
    <div className={`recorder ${expanded ? "expanded" : "compact"}`}>
      <div className="recorder-header">
        <div className="recorder-drag" data-tauri-drag-region="deep">
          <Circle size={8} className="record-dot" fill="currentColor" />
          <strong>
            {anyReconnecting && !expanded ? t("Riconnessione…") : expanded ? "PARLO" : t("Registrazione")}
          </strong>
          <span className="mono">{fmtTime(elapsed)}</span>
        </div>
        <DockButton nativeTooltip={false} onClick={() => void api.dockRecorder("bottom").catch(e => setError(String(e)))} />
        <button className="icon-button" aria-label={expanded ? t("Riduci indicatore") : t("Espandi indicatore")} onClick={() => resize(!expanded)}>
          {expanded ? <Minus size={15} /> : <Maximize2 size={14} />}
        </button>
      </div>
      {expanded && <>
        <SourceMeters key={capture.id} microphone={capture.microphone} systemAudio={capture.system_audio} reconnecting={reconnecting} />
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
