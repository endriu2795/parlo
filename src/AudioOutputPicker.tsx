import { t } from "./i18n";
import { useEffect, useState } from "react";
import { api } from "./api";

export default function AudioOutputPicker({ value, disabled = false, onChange }: { value: string; disabled?: boolean; onChange: (value: string) => void }) {
  const [outputs, setOutputs] = useState<{ id: string; name: string }[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const refresh = async () => {
    setBusy(true); setError("");
    try { setOutputs(await api.audioOutputs()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  useEffect(() => { void refresh(); }, []);
  const missing = value && value !== "@communications" && !outputs.some(output => output.id === value);
  return <div>
    <label className="field">
      <span>{t("Uscita da registrare · audio del computer")}</span>
      <select value={value} disabled={disabled} onChange={event => onChange(event.target.value)}>
        <option value="">{t("Automatica · uscita predefinita Windows")}</option>
        <option value="@communications">{t("Automatica · uscita comunicazioni Windows")}</option>
        {outputs.map(output => <option value={output.id} key={output.id}>{output.name}</option>)}
        {missing && <option value={value}>{t("Uscita selezionata non disponibile")}</option>}
      </select>
    </label>
    <button type="button" className="text-button" disabled={busy} onClick={() => void refresh()}>{busy ? t("Aggiornamento…") : t("Aggiorna dispositivi")}</button>
    {error && <p className="settings-hint" role="alert">{error}</p>}
    <p className="settings-hint">{t("Per registrare chi ti parla, scegli la stessa uscita usata da Teams o Zoom. Le opzioni automatiche seguono i cambi di Windows; un dispositivo scelto resta fisso e viene riconnesso se lo ricolleghi. Vale dalle nuove registrazioni.")}</p>
  </div>;
}
