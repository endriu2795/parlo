import { t } from "./i18n";
import { RefreshCw, X } from "lucide-react";
import type { EngineSetup, Status } from "./types";

/** Model download progress, shown while the local transcription engine is prepared. */
export function EngineBanner({ setup, busy, onRetry }: { setup: EngineSetup; busy: boolean; onRetry: () => void }) {
  return (
    <div className="banner engine-progress" role={setup.error ? "alert" : "status"}>
      <div>
        <strong>{setup.error ? t("Download da completare") : t("Preparazione della trascrizione")}</strong>
        <p>{setup.error || setup.message}</p>
        {!setup.error && <p>{t("Il modello si scarica una sola volta, poi funziona anche offline.")}</p>}
      </div>
      {setup.error && <button className="secondary-button" disabled={busy} onClick={onRetry}>{t("Riprova download")}</button>}
    </div>
  );
}

export function ErrorBanner({ message, onClose }: { message: string; onClose: () => void }) {
  return (
    <div role="alert" className="banner error-banner">
      <span>{message}</span>
      <button className="icon-button" aria-label={t("Chiudi errore")} onClick={onClose}><X size={16} /></button>
    </div>
  );
}

export function ProcessingBanner({ status, busy, onCancel }: { status: Status; busy: boolean; onCancel: () => void }) {
  const stopping = status.stage.startsWith("Interruzione");
  return (
    <div className="banner processing-banner" role="status">
      <RefreshCw size={15} className="spin" />
      <span>{stopping ? t("Interruzione dell’elaborazione…") : status.stage || t("Trascrizione in corso…")}</span>
      {status.queued.length > 0 && <span>{t("· {0} in coda", status.queued.length)}</span>}
      <span className="processing-note">{t("Puoi già registrare un’altra call")}</span>
      <button type="button" className="processing-cancel" disabled={busy || stopping} onClick={onCancel}>
        <X size={13} /> {t("Interrompi")}
      </button>
    </div>
  );
}
