import { t } from "./i18n";
import { FileUp, Mic, Monitor, Play, Square } from "lucide-react";
import type { Settings } from "./types";

const parloIcon = new URL("./assets/parlo-welcome-icon.png", import.meta.url).href;
const parloIcon2x = new URL("./assets/parlo-welcome-icon@2x.png", import.meta.url).href;

export default function WelcomeScreen({ settings, recording, canRecord, onRecord, canImport, onImport, preparing, shortcut }: {
  settings: Settings | null;
  recording: boolean;
  canRecord: boolean;
  onRecord: () => void;
  canImport: boolean;
  onImport: () => void;
  /** The local transcription model is still being prepared. */
  preparing: boolean;
  shortcut: string | null;
}) {
  const sources = [
    { label: t("Microfono"), icon: Mic, on: settings?.microphone !== false },
    { label: t("Audio del computer"), icon: Monitor, on: settings?.system_audio !== false },
  ];
  return (
    <div className="welcome">
      <img className="welcome-mark" src={parloIcon} srcSet={`${parloIcon2x} 2x`} width={56} height={56} alt="" />
      <h1>{t("Registra e trascrivi le tue call")}</h1>
      <p>{t("Registra la tua voce e quella degli altri partecipanti, poi crea la trascrizione.")}</p>
      <div className="welcome-actions">
        <button className={`primary-button${recording ? " recording" : ""}`} onClick={onRecord} disabled={!canRecord}>
          {recording ? <Square size={15} /> : <Play size={15} />}
          {recording ? t("Termina registrazione") : t("Avvia registrazione")}
        </button>
        <button className="secondary-button" onClick={onImport} disabled={!canImport}>
          <FileUp size={15} /> {t("Importa un file")}
        </button>
      </div>
      {shortcut && <p className="welcome-shortcut">{t("Puoi avviare la registrazione anche con") + " "}<kbd>{shortcut}</kbd></p>}
      <ul className="welcome-sources" aria-label={t("Sorgenti audio")}>
        {sources.map(({ label, icon: Icon, on }) => (
          <li key={label} className={on ? "on" : ""}>
            <Icon size={14} aria-hidden="true" />
            {on ? t("{0} attivo", label) : t("{0} disattivato", label)}
          </li>
        ))}
      </ul>
      {preparing && <p className="setup-note" role="status">{t("Preparazione della trascrizione in corso…")}</p>}
    </div>
  );
}
