import { t } from "./i18n";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { Camera, X } from "lucide-react";
import { api } from "./api";
import "./screenshot.css";

type Point = { x: number; y: number };
type Rectangle = Point & { width: number; height: number };
type CaptureMonitor = {
  name: string;
  drawing: { overlay_png: string | null; strokes: { color: string; width: number; points: [number, number][] }[] };
};
const fullScreen: Rectangle = { x: 0, y: 0, width: 1, height: 1 };
const point = (event: React.PointerEvent): Point => ({
  x: Math.max(0, Math.min(1, event.clientX / innerWidth)),
  y: Math.max(0, Math.min(1, event.clientY / innerHeight)),
});
const rectangleBetween = (a: Point, b: Point): Rectangle => ({
  x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.abs(a.x - b.x), height: Math.abs(a.y - b.y),
});

export default function ScreenshotArea() {
  const [monitors, setMonitors] = useState<CaptureMonitor[]>([]);
  const [index, setIndex] = useState(0);
  const [rectangle, setRectangle] = useState<Rectangle | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [dragging, setDragging] = useState(false);
  const start = useRef<Point | null>(null);
  const pending = useRef(false);
  const surface = useRef<HTMLDivElement>(null);
  const cancel = () => {
    if (!pending.current) {
      start.current = null; setDragging(false); setRectangle(null);
      void invoke("screenshot_cancel_selection").catch(e => setError(String(e)));
    }
  };
  useEffect(() => {
    document.documentElement.classList.add("screenshot-area-window");
    let alive = true;
    const reset = async () => {
      start.current = null; pending.current = false;
      setRectangle(null); setIndex(0); setError(""); setBusy(false); setDragging(false);
      try {
        const list = await invoke<CaptureMonitor[]>("screenshot_monitors");
        if (alive) { setMonitors(list); surface.current?.focus(); }
        const settings = await api.settings();
        if (alive) document.documentElement.dataset.theme = settings.theme || "dark";
      } catch (e) { if (alive) setError(String(e)); }
    };
    // Subscribe before querying, so the lazily loaded window also sees its first selection.
    const subscription = listen("screenshot-selection-reset", () => void reset());
    void subscription.then(() => { if (alive) void reset(); });
    const keys = (event: KeyboardEvent) => {
      if (event.key === "Escape") { event.preventDefault(); cancel(); }
    };
    window.addEventListener("keydown", keys);
    return () => {
      alive = false;
      document.documentElement.classList.remove("screenshot-area-window");
      window.removeEventListener("keydown", keys);
      void subscription.then(off => off());
    };
  }, []);
  const capture = async (selection: Rectangle) => {
    if (pending.current) return;
    pending.current = true; setBusy(true); setError("");
    try { await invoke("screenshot_finish_selection", { rectangle: selection }); }
    catch (e) {
      // Native capture hides this window. Keep failures visible in the main app.
      await emit("app-error", String(e));
      setError(String(e));
    } finally { pending.current = false; setBusy(false); }
  };
  return <div ref={surface} className={`screenshot-selection ${dragging ? "is-dragging" : ""}`} tabIndex={-1}
    aria-label={t("Seleziona un’area per lo screenshot")}
    onPointerDown={event => {
      if (pending.current || event.button !== 0 || (event.target as HTMLElement).closest(".screenshot-toolbar")) return;
      event.currentTarget.setPointerCapture(event.pointerId);
      start.current = point(event); setRectangle(null); setDragging(true); setError("");
    }}
    onPointerMove={event => {
      if (start.current && !pending.current) setRectangle(rectangleBetween(start.current, point(event)));
    }}
    onPointerUp={event => {
      const first = start.current;
      start.current = null; setDragging(false);
      if (!first || pending.current) return;
      const selection = rectangleBetween(first, point(event));
      if (selection.width * innerWidth < 4 || selection.height * innerHeight < 4) { setRectangle(null); return; }
      setRectangle(selection); void capture(selection);
    }}
    onPointerCancel={() => { start.current = null; setDragging(false); setRectangle(null); }}
    onLostPointerCapture={() => { start.current = null; setDragging(false); }}>
    {rectangle && <div className="screenshot-rectangle" style={{ left: `${rectangle.x * 100}%`, top: `${rectangle.y * 100}%`, width: `${rectangle.width * 100}%`, height: `${rectangle.height * 100}%` }} />}
    <svg className="screenshot-drawing" aria-hidden="true" width="100%" height="100%">
      {monitors[index]?.drawing.strokes.map((stroke, i) => <polyline key={i}
        points={stroke.points.map(([x, y]) => `${x * innerWidth},${y * innerHeight}`).join(" ")}
        fill="none" stroke={stroke.color} strokeWidth={stroke.width * innerWidth} strokeLinecap="round" strokeLinejoin="round" />)}
    </svg>
    {monitors[index]?.drawing.overlay_png && <img className="screenshot-drawing" src={monitors[index].drawing.overlay_png!} alt="" draggable={false} />}
    <div className="screenshot-toolbar" role="dialog" aria-label="Screenshot">
      <Camera size={18} aria-hidden="true"/>
      <span role="status">{busy ? t("Salvataggio…") : t("Trascina un’area · rilascia per salvare")}</span>
      <select aria-label={t("Schermo da catturare")} value={index} disabled={busy} onChange={async event => {
        if (pending.current) return;
        const next = Number(event.target.value);
        pending.current = true; setBusy(true);
        try { await invoke("screenshot_select_monitor", { index: next }); setIndex(next); setRectangle(null); }
        catch (e) { setError(String(e)); }
        finally { pending.current = false; setBusy(false); }
      }}>{monitors.map((m, i) => <option key={i} value={i}>{m.name}</option>)}</select>
      <button type="button" disabled={busy || !monitors.length} onClick={() => void capture(fullScreen)}>{t("Schermo intero")}</button>
      <button type="button" disabled={busy} onClick={cancel} aria-label={t("Annulla screenshot (Esc)")}><X size={17}/> Esc</button>
      {error && <p role="alert">{error}</p>}
    </div>
  </div>;
}
