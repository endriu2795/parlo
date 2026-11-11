import { t } from "./i18n";
import { useEffect, useRef, useState } from "react";
import { api, desktop } from "./api";
import { defaultShortcut, shortcutKeyLabel } from "./shortcuts";

const modifiers = ["Ctrl", "Alt", "Shift", "Super"];
const modifierKey = (key: string) => ({ Control: "Ctrl", Alt: "Alt", Shift: "Shift", Meta: "Super" } as Record<string,string>)[key];
const keyCode = (code: string) => code === "Space" ? "Space" : /^(Key[A-Z]|Digit[0-9])$/.test(code) ? code.replace(/^(Key|Digit)/, "") : /^F([1-9]|1[0-2])$/.test(code) ? code : null;

export default function ShortcutField({ value, onChange, label = "Shortcut globale", defaultValue = defaultShortcut("Space"), description = "Avvia una call o mostra l’indicatore. Non interrompe la registrazione." }: { value: string; onChange: (value: string) => void; label?: string; defaultValue?: string; description?: string }) {
  const [listening, setListening] = useState(false), [hint, setHint] = useState(""), [preview, setPreview] = useState<string[]>([]);
  const chord = useRef({ held: new Set<string>(), mods: new Set<string>(), key: "", invalid: false });
  const capture = useRef(false), alive = useRef(true), starting = useRef(false);
  const end = () => {
    capture.current = false;
    setListening(false); setPreview([]);
    chord.current = { held: new Set(), mods: new Set(), key: "", invalid: false };
    if (desktop) void api.shortcutCapture(false).catch(error => { if (alive.current) setHint(String(error)); });
  };
  useEffect(() => {
    alive.current = true;
    window.addEventListener("blur", end);
    return () => { alive.current = false; capture.current = false; window.removeEventListener("blur", end); if (desktop) void api.shortcutCapture(false).catch(() => {}); };
  }, []);
  return <div className="shortcut-field">
    <span className="settings-label">{label}</span>
    <div className="shortcut-preview">
      <button type="button" className={`shortcut-capture ${listening ? "listening" : ""}`} aria-label={t("{0}: cambia combinazione", label)} aria-pressed={listening}
        onClick={async event => {
          event.currentTarget.focus();
          if (capture.current) { end(); return; }
          if (starting.current) return;
          starting.current = true;
          try {
            if (desktop) await api.shortcutCapture(true);
            if (!alive.current || !document.hasFocus()) { if (desktop) await api.shortcutCapture(false); return; }
            chord.current = { held: new Set(), mods: new Set(), key: "", invalid: false };
            capture.current = true; setHint(""); setPreview([]); setListening(true);
          } catch (error) { setHint(String(error)); }
          finally { starting.current = false; }
        }}
        onBlur={() => { if (capture.current) end(); }}
        onKeyDown={event => {
          if (!capture.current) return;
          event.preventDefault(); event.stopPropagation();
          if (event.key === "Escape") { setHint(t("Scelta annullata.")); end(); return; }
          if (event.repeat) return;
          const current = chord.current;
          current.held.add(event.code);
          const mod = modifierKey(event.key);
          if (mod) current.mods.add(mod);
          if (event.ctrlKey) current.mods.add("Ctrl");
          if (event.altKey) current.mods.add("Alt");
          if (event.shiftKey) current.mods.add("Shift");
          if (event.metaKey) current.mods.add("Super");
          if (!mod) {
            const key = keyCode(event.code);
            if (!key || (current.key && current.key !== key) || event.getModifierState("AltGraph")) current.invalid = true;
            else current.key = key;
          }
          setPreview([...modifiers.filter(m => current.mods.has(m)), ...(current.key ? [current.key] : [])]);
          setHint(t("Rilascia tutti i tasti per confermare la combinazione."));
        }}
        onKeyUp={event => {
          if (!capture.current) return;
          event.preventDefault(); event.stopPropagation();
          const current = chord.current; current.held.delete(event.code);
          // macOS may omit letter keyup events while Command is held.
          if (!event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) current.held.clear();
          if (current.held.size) return;
          if (!current.key) { current.mods.clear(); setPreview([]); setHint(t("Aggiungi una lettera, numero, Spazio o F1–F12 ai modificatori.")); return; }
          if (current.invalid || !current.mods.size) {
            chord.current = { held: new Set(), mods: new Set(), key: "", invalid: false };
            setPreview([]); setHint(t("Usa uno o più modificatori (Ctrl, Alt, Shift, Cmd) e un solo tasto principale.")); return;
          }
          onChange([...modifiers.filter(m => current.mods.has(m)), current.key].join("+")); setHint(""); end();
        }}>
        {(listening ? preview : value.split("+")).map((key, index) => <kbd key={index}>{shortcutKeyLabel(key)}</kbd>)}
        <span>{listening ? (preview.length ? t("Rilascia i tasti") : t("Premi la combinazione…")) : t("Modifica")}</span>
      </button>
      {listening && <button type="button" className="text-button" onClick={end}>{t("Annulla")}</button>}
      <button type="button" className="text-button" onClick={() => { end(); onChange(defaultValue); }}>{t("Ripristina")}</button>
    </div>
    <small role="status">{hint || (listening ? t("Premi insieme tutti i tasti. Esc per annullare.") : description)}</small>
  </div>;
}
