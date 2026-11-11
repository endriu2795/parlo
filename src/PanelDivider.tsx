import { t } from "./i18n";
import { useEffect, useRef, type KeyboardEvent, type PointerEvent } from "react";

type Props = {
  label: string;
  controls: string;
  value: number;
  min: number;
  max: number;
  initial: number;
  direction?: 1 | -1;
  onPreview: (value: number) => void;
  onChange: (value: number) => void;
};

export default function PanelDivider({ label, controls, value, min, max, initial, direction = 1, onPreview, onChange }: Props) {
  const drag = useRef<{ id: number; x: number; initial: number; current: number } | null>(null);
  const element = useRef<HTMLDivElement>(null);
  const clamp = (next: number) => Math.round(Math.min(max, Math.max(min, next)));
  const finish = (cancel = false) => {
    const current = drag.current;
    if (!current) return;
    drag.current = null;
    document.documentElement.classList.remove("workspace-resizing");
    element.current?.removeAttribute("data-resizing");
    if (cancel) {
      onPreview(value);
      element.current?.setAttribute("aria-valuenow", String(Math.round(value)));
      element.current?.setAttribute("aria-valuetext", `${Math.round(value)} pixel`);
    }
    else onChange(current.current);
  };
  const cancel = useRef(() => finish(true));
  cancel.current = () => finish(true);
  useEffect(() => {
    const key = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape" && drag.current) { event.preventDefault(); cancel.current(); }
    };
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("keydown", key);
      if (drag.current) cancel.current();
    };
  }, []);
  const start = (event: PointerEvent<HTMLDivElement>) => {
    if (!event.isPrimary || event.button !== 0) return;
    event.preventDefault();
    event.currentTarget.focus();
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { id: event.pointerId, x: event.clientX, initial: value, current: value };
    event.currentTarget.dataset.resizing = "true";
    document.documentElement.classList.add("workspace-resizing");
  };
  const move = (event: PointerEvent<HTMLDivElement>) => {
    const current = drag.current;
    if (!current || event.pointerId !== current.id) return;
    current.current = clamp(current.initial + (event.clientX - current.x) * direction);
    onPreview(current.current);
    event.currentTarget.setAttribute("aria-valuenow", String(current.current));
    event.currentTarget.setAttribute("aria-valuetext", `${current.current} pixel`);
  };
  const key = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 48 : 16;
    const next = event.key === "ArrowLeft" ? value - step * direction
      : event.key === "ArrowRight" ? value + step * direction
      : event.key === "Home" ? min : event.key === "End" ? max
      : event.key === "Enter" ? initial : null;
    if (next === null) return;
    event.preventDefault();
    onChange(clamp(next));
  };
  return <div ref={element} className="panel-divider" role="separator" tabIndex={0}
    aria-label={label} aria-controls={controls} aria-orientation="vertical"
    aria-valuemin={min} aria-valuemax={max} aria-valuenow={Math.round(value)} aria-valuetext={`${Math.round(value)} pixel`}
    title={t("{0} · trascina per ridimensionare · doppio clic per ripristinare", label)}
    onPointerDown={start} onPointerMove={move} onPointerUp={() => finish()}
    onPointerCancel={() => finish(true)} onLostPointerCapture={() => finish(true)}
    onDoubleClick={() => onChange(clamp(initial))} onKeyDown={key} />;
}
