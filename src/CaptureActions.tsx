import { t } from "./i18n";
import { useEffect } from "react";
import { Camera, Pencil, StickyNote, Type } from "lucide-react";
import { api, desktop } from "./api";
import { defaultShortcut, formatShortcut } from "./shortcuts";
import type { Settings } from "./types";

/** Ctrl/Cmd+N opens a timestamped note while a recording is running and focus is not in a text field. */
export function useNoteShortcut(recording: boolean, onError: (error: string) => void) {
  useEffect(() => {
    if (!desktop || !recording) return;
    const key = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "n"
        && !(event.target as HTMLElement)?.closest("input,textarea,select,[contenteditable=true]")) {
        event.preventDefault();
        void api.openNote().catch(e => onError(String(e)));
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [recording]);
}

/** Tools available while recording: note, drawing, on-screen text and screenshot. */
export default function CaptureActions({ settings, disabled, run }: {
  settings: Settings | null;
  disabled: boolean;
  run: (action: () => Promise<unknown>) => void;
}) {
  const shortcut = (value: string | undefined, key: string) => formatShortcut(value || defaultShortcut(key));
  const actions = [
    { label: t("Nota"), hint: t("Aggiungi una nota"), icon: StickyNote, keys: shortcut(settings?.note_shortcut, "N"), action: () => api.openNote() },
    { label: t("Disegna"), hint: t("Disegna sullo schermo"), icon: Pencil, keys: shortcut(settings?.drawing_shortcut, "D"), action: () => api.toggleDrawing() },
    { label: t("Testo"), hint: t("Scrivi sullo schermo"), icon: Type, keys: shortcut(settings?.text_shortcut, "T"), action: () => api.toggleDrawing("text") },
    { label: "Screenshot", hint: t("Cattura lo schermo"), icon: Camera, keys: shortcut(settings?.screenshot_shortcut, "S"), action: () => api.screenshot() },
  ];
  return (
    <div className="capture-actions" role="toolbar" aria-label={t("Strumenti durante la registrazione")}>
      {actions.map(({ label, hint, icon: Icon, keys, action }) => (
        <button key={label} type="button" className="text-button" disabled={disabled}
          data-capture-hint={`${hint} (${keys})`} aria-label={`${label} (${keys})`} onClick={() => run(action)}>
          <Icon size={14} /> {label}
        </button>
      ))}
    </div>
  );
}
