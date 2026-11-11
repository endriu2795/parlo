import { useEffect, useRef, useState, type CSSProperties } from "react";

export type WorkspacePanel = "archive" | "assistant";
const defaults = { archive: 260, assistant: 392 };
const storageKey = "parlo.workspace-layout.v1";
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

function readWidths() {
  try {
    const value = JSON.parse(localStorage.getItem(storageKey) ?? localStorage.getItem("presentio.workspace-layout.v1") ?? "null");
    return {
      archive: Number.isFinite(value?.archive) ? clamp(value.archive, 180, 380) : defaults.archive,
      assistant: Number.isFinite(value?.assistant) ? clamp(value.assistant, 280, 900) : defaults.assistant,
    };
  } catch { return defaults; }
}

export function useWorkspaceLayout(assistantVisible: boolean, wide: boolean) {
  const root = useRef<HTMLDivElement>(null);
  const [preferred, setPreferred] = useState(readWidths);
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    if (!root.current) return;
    const observer = new ResizeObserver(([entry]) => setWidth(entry.contentRect.width));
    observer.observe(root.current);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    try { localStorage.setItem(storageKey, JSON.stringify(preferred)); } catch { /* Optional preference. */ }
  }, [preferred]);

  const centerMin = width >= 1050 ? 380 : width >= 900 ? 320 : 200;
  const assistantMin = width >= 900 ? 300 : 260;
  const archiveMin = width >= 900 ? 200 : 180;
  const archiveMax = Math.max(archiveMin, Math.min(380, width - centerMin - (assistantVisible ? assistantMin + 12 : 6)));
  const archive = clamp(preferred.archive, archiveMin, archiveMax);
  const detailWidth = Math.max(0, width - archive - 6);
  const assistantMax = Math.max(assistantMin, Math.min(900, detailWidth - centerMin - 6));
  const assistant = clamp(wide ? Math.max(preferred.assistant, detailWidth * 0.62) : preferred.assistant, assistantMin, assistantMax);
  const preview = (panel: WorkspacePanel, value: number) => {
    root.current?.style.setProperty(`--${panel}-width`, `${value}px`);
    if (panel === "archive") {
      const remaining = Math.max(0, width - value - 6);
      const max = Math.max(assistantMin, Math.min(900, remaining - centerMin - 6));
      const next = clamp(wide ? Math.max(preferred.assistant, remaining * 0.62) : preferred.assistant, assistantMin, max);
      root.current?.style.setProperty("--assistant-width", `${next}px`);
    }
  };
  const commit = (panel: WorkspacePanel, value: number) => {
    preview(panel, value);
    setPreferred(previous => ({ ...previous, [panel]: value }));
  };
  return {
    root,
    style: { "--archive-width": `${archive}px`, "--assistant-width": `${assistant}px` } as CSSProperties,
    archive: { value: archive, min: archiveMin, max: archiveMax, initial: defaults.archive },
    assistant: { value: assistant, min: assistantMin, max: assistantMax, initial: defaults.assistant },
    preview, commit,
    reset: () => setPreferred({ ...defaults }),
  };
}
