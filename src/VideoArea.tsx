import { t } from "./i18n";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./video.css";
import { Monitor as MonitorIcon, Video, X } from "lucide-react";
import { api } from "./api";

type Point = { x: number; y: number };
type Rectangle = Point & { width: number; height: number };
type Monitor = { name: string; region: { width: number; height: number } };

export default function VideoArea() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [index, setIndex] = useState(0);
  const [rectangle, setRectangle] = useState<Rectangle | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [recording, setRecording] = useState<{ width: number; height: number; generation: number } | null>(null);
  const starting = useRef(false);
  const start = useRef<Point | null>(null);
  const area = useRef<HTMLDivElement>(null);
  const cancel = () => { if (!starting.current) void invoke("video_cancel_selection").catch(e => setError(String(e))); };
  useEffect(() => {
    document.documentElement.classList.add("video-area-window");
    const reset = () => {
      void api.settings().then(s => { document.documentElement.dataset.theme = s.theme || "dark"; }).catch(() => {});
      start.current = null;
      starting.current = false;
      setRecording(null);
      setRectangle(null); setIndex(0); setError(""); setBusy(false);
      void invoke<Monitor[]>("video_monitors").then(setMonitors).catch(() => {});
    };
    reset();
    const listener = listen("video-selection-reset", reset);
    const started = listen<{ width: number; height: number; generation: number }>("video-recording-started", event => setRecording(event.payload));
    const keys = (event: KeyboardEvent) => { if (event.key === "Escape") cancel(); };
    window.addEventListener("keydown", keys);
    return () => {
      document.documentElement.classList.remove("video-area-window");
      window.removeEventListener("keydown", keys);
      void listener.then(off => off());
      void started.then(off => off());
    };
  }, []);
  useEffect(() => {
    if (!recording) return;
    let second = 0;
    const first = requestAnimationFrame(() => { second = requestAnimationFrame(() => {
      void invoke("video_hint_ready", { generation: recording.generation }).catch(() => {});
    }); });
    return () => { cancelAnimationFrame(first); cancelAnimationFrame(second); };
  }, [recording]);
  const point = (event: React.PointerEvent): Point => {
    const bounds = area.current!.getBoundingClientRect();
    return { x: Math.max(0, Math.min(1, (event.clientX - bounds.left) / bounds.width)), y: Math.max(0, Math.min(1, (event.clientY - bounds.top) / bounds.height)) };
  };
  const record = async (selection: Rectangle) => {
    if (starting.current) return;
    starting.current = true;
    setBusy(true); setError("");
    try { await invoke("video_start_selection", { index, rectangle: selection }); }
    catch (e) {
      // The selector is hidden before audio starts; publish errors to the main window too.
      setError(String(e));
      const { emit } = await import("@tauri-apps/api/event");
      await emit("app-error", String(e));
    } finally { starting.current = false; setBusy(false); }
  };
  if (recording) return <div className="video-recording-hint" role="status"><span>{t("Registrazione avviata ·") + " "}{recording.width} × {recording.height} px</span></div>;
  return <div ref={area} className="video-selection" aria-label={t("Seleziona area da registrare")}
    onPointerDown={event => {
      if (busy || event.button !== 0 || (event.target as HTMLElement).closest(".video-selection-toolbar")) return;
      event.currentTarget.setPointerCapture(event.pointerId);
      start.current = point(event); setRectangle(null);
    }}
    onPointerMove={event => {
      if (busy || !start.current) return;
      const end = point(event), first = start.current;
      setRectangle({ x: Math.min(first.x, end.x), y: Math.min(first.y, end.y), width: Math.abs(end.x - first.x), height: Math.abs(end.y - first.y) });
    }}
    onPointerUp={() => { start.current = null; }} onPointerCancel={() => { start.current = null; }}>
    {rectangle && <div className="video-selection-rectangle" style={{ left: `${rectangle.x * 100}%`, top: `${rectangle.y * 100}%`, width: `${rectangle.width * 100}%`, height: `${rectangle.height * 100}%` }} />}
    <div className="video-selection-toolbar" role="dialog" aria-label={t("Registrazione video")}>
      <header><span className="video-selection-icon"><Video size={19}/></span><div><strong>{t("Registra schermo")}</strong><small>{t("Video con audio · 30 fps")}</small></div><button type="button" className="icon-button" aria-label={t("Annulla selezione")} onClick={cancel} disabled={busy}><X size={18}/></button></header>
      <p>{busy ? t("Preparazione video e audio… La registrazione partirà automaticamente.") : t("Trascina un’area sullo schermo oppure registra lo schermo intero.")}</p>
      <label className="field video-monitor-field"><span><MonitorIcon size={14}/> {t("Schermo da registrare")}</span><select aria-label={t("Schermo da registrare")} value={index} disabled={busy} onChange={async event => {
        const next = Number(event.target.value);
        try { await invoke("video_select_monitor", { index: next }); setIndex(next); setRectangle(null); }
        catch (e) { setError(String(e)); }
      }}>{monitors.map((m, i) => <option key={i} value={i}>{m.name}</option>)}</select></label>
      {error && <p role="alert">{error}</p>}
      <small className="video-selection-size">{rectangle && monitors[index] ? t("Area selezionata: {0} × {1} px", Math.floor(rectangle.width * monitors[index].region.width), Math.floor(rectangle.height * monitors[index].region.height)) : t("Seleziona un’area trascinando il mouse")}</small>
      <div className="video-selection-buttons">
        <button className="text-button" disabled={busy} onClick={cancel}>{t("Annulla · Esc")}</button>
        <button className="secondary-button" disabled={busy} onClick={() => void record({ x: 0, y: 0, width: 1, height: 1 })}>{t("Schermo intero")}</button>
        <button className="primary-button" disabled={busy || !rectangle || !monitors[index] || rectangle.width * monitors[index].region.width < 32 || rectangle.height * monitors[index].region.height < 32}
          onClick={() => rectangle && void record(rectangle)}>{busy ? t("Avvio…") : t("Registra area")}</button>
      </div>
    </div>
  </div>;
}
