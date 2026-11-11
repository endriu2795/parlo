import { t } from "./i18n";
import { primaryModifier } from "./shortcuts";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { StickyNote, X } from "lucide-react";
import { api } from "./api";
import { fmtTime } from "./format";

type Draft = {call_id: string; timestamp_ms: number};
export default function NoteComposer() {
  const input = useRef<HTMLTextAreaElement>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  useEffect(() => {
    let alive = true;
    const open = (value: Draft | null) => {
      if (!alive || !value) return;
      setDraft(value); setText(""); setError("");
      requestAnimationFrame(() => input.current?.focus());
    };
    const stop = listen<Draft>("note-opened", event => open(event.payload));
    void stop.then(() => api.noteDraft()).then(open).catch(e => {if (alive) setError(String(e));});
    return () => { alive = false; void stop.then(unlisten => unlisten()); };
  }, []);
  const cancel = () => { if (!savingRef.current) void api.cancelNote().catch(e => setError(String(e))); };
  const save = async () => {
    if (!text.trim() || !draft || savingRef.current) return;
    savingRef.current = true; setSaving(true); setError("");
    try { await api.saveNote(text); setText(""); setDraft(null); }
    catch (e) { setError(String(e)); }
    finally { savingRef.current = false; setSaving(false); }
  };
  const ms = draft?.timestamp_ms || 0;
  return <main className="note-composer" onKeyDown={event => {
    if (event.key === "Escape") { event.preventDefault(); cancel(); }
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {event.preventDefault(); void save();}
  }}>
    <header><strong><StickyNote size={17}/> {t("Nota") + " "}<time>{fmtTime(ms)}</time></strong><button className="icon-button" aria-label={t("Annulla nota")} disabled={saving} onClick={cancel}><X size={17}/></button></header>
    <textarea ref={input} autoFocus placeholder={t("Scrivi una nota per questo punto della registrazione…")} aria-label={t("Nota da aggiungere alla trascrizione")} maxLength={4000} value={text} disabled={saving} onChange={event => setText(event.target.value)}/>
    {error && <p role="alert">{error}</p>}
    <footer><small>{primaryModifier}{t("+Invio per salvare · Esc annulla")}</small><button className="primary-button" disabled={!text.trim() || !draft || saving} onClick={() => void save()}>{saving ? t("Salvataggio…") : t("Salva nota")}</button></footer>
  </main>;
}
