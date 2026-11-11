import { t } from "./i18n";
import { useState } from "react";
import { Plus, Tag, X } from "lucide-react";

export default function CallTags({ tags, disabled, onSave }: { tags: string[]; disabled: boolean; onSave: (tags: string[]) => Promise<void> }) {
  const [value, setValue] = useState("");
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");
  const save = async (next: string[]) => {
    setSaving(true); setError("");
    try { await onSave(next); setValue(""); }
    catch (e) { setError(String(e)); }
    finally { setSaving(false); }
  };
  return <div className="call-tags-editor">
    <div className="call-tags-list" aria-label={t("Tag della call")}>
      {tags.map(tag => <span className="call-tag" key={tag}><Tag size={11}/>{tag}<button type="button" aria-label={t("Rimuovi tag {0}", tag)} disabled={disabled || saving} onClick={() => void save(tags.filter(t => t !== tag))}><X size={11}/></button></span>)}
      {editing ? <form onSubmit={e => {
        e.preventDefault();
        const tag = value.trim().replace(/\s+/g, " ");
        if (tag && !tags.some(t => t.toLocaleLowerCase() === tag.toLocaleLowerCase())) void save([...tags, tag]);
      }}><input autoFocus aria-label={t("Nuovo tag")} placeholder={t("Nuovo tag · Invio")} value={value} maxLength={40} disabled={disabled || saving}
        onChange={e => setValue(e.target.value)} onKeyDown={e => { if (e.key === "Escape") { setEditing(false); setValue(""); } }}/><button type="submit" disabled={disabled || saving || !value.trim()} aria-label={t("Aggiungi tag")}><Plus size={13}/></button><button type="button" aria-label={t("Chiudi inserimento tag")} onClick={() => setEditing(false)}><X size={13}/></button></form>
        : <button type="button" className="add-call-tag" disabled={disabled || tags.length >= 24} onClick={() => setEditing(true)}><Plus size={12}/> {t("Aggiungi tag")}</button>}
    </div>
    {error && <p className="source-error" role="alert">{error}</p>}
  </div>;
}
