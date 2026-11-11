import { t } from "./i18n";
import { useEffect, useRef, useState } from "react";

export default function ApiKeyField({ label, saved, value, placeholder, busy, onChange }: {
  label: string;
  saved: boolean | null;
  value: string;
  placeholder: string;
  busy: boolean;
  onChange: (value: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { if (!value) setEditing(false); }, [saved, value]);
  const masked = saved === true && !editing && !value;
  return <div>
    <label className="field">
      <span>{label}{saved === true ? " " + t("· salvata") : saved === false ? " " + t("· non configurata") : " " + t("· verifica…")}</span>
      <input ref={input} type="password" autoComplete="off" spellCheck={false}
        aria-label={label} readOnly={masked} disabled={busy}
        value={masked ? "••••••••••••" : value}
        placeholder={placeholder}
        onChange={event => onChange(event.target.value)} />
      <small>{masked
        ? t("Chiave salvata nel portachiavi del sistema. Premi Sostituisci per cambiarla.")
        : t("La chiave viene salvata nel portachiavi del sistema e non viene passata all’assistente.")}</small>
    </label>
    {masked && <button type="button" className="text-button" disabled={busy} onClick={() => {
      setEditing(true);
      requestAnimationFrame(() => input.current?.focus());
    }}>{t("Sostituisci chiave")}</button>}
  </div>;
}
