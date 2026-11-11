import { t } from "./i18n";
import { useState } from "react";
import {
  AtSign,
  Download,
  FileText,
  FolderOpen,
  RefreshCw,
} from "lucide-react";
import { api } from "./api";
import type { GeneratedFile } from "./types";

export default function GeneratedFiles({
  callId,
  files,
  disabled,
  onRefresh,
  onMention,
  mentionable,
}: {
  callId: string;
  files: GeneratedFile[];
  disabled: boolean;
  onRefresh: () => Promise<void>;
  onMention: (id: string) => void;
  mentionable: string[];
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <section className="generated-files" aria-label={t("File generati nella call")}>
      <div className="source-section-title">
        <strong>{t("File generati") + " "}{files.length ? `(${files.length})` : ""}</strong>
        <button
          type="button"
          aria-label={t("Aggiorna file generati")}
          title={t("Aggiorna file generati")}
          disabled={busy}
          onClick={() => void run(onRefresh)}
        >
          <RefreshCw size={13} />
        </button>
      </div>
      {!files.length && (
        <p>
          {t("Chiedi in chat di creare un file: lo troverai qui, pronto da scaricare.")}
        </p>
      )}
      {files.map((file) => (
        <article key={file.id} className="generated-file">
          <header>
            <FileText size={17} />
            <div>
              <strong title={file.path}>{file.name}</strong>
              <small>
                {t("Salvato nella call ·")}{" "}
                {file.size < 1024
                  ? `${file.size} B`
                  : file.size < 1024 * 1024
                    ? `${Math.ceil(file.size / 1024)} KB`
                    : `${(file.size / (1024 * 1024)).toFixed(1)} MB`}
              </small>
            </div>
          </header>
          <div className="generated-file-actions">
            <button
              type="button"
              disabled={busy || disabled}
              onClick={() =>
                void run(async () => {
                  if (await api.saveGenerated(callId, file.id))
                    setNotice(t("File salvato nella cartella scelta"));
                })
              }
            >
              <Download size={13} /> {t("Scarica")}
            </button>
            <button
              type="button"
              disabled={
                busy || disabled || !mentionable.includes(`output:${file.id}`)
              }
              title={t("Usa questo file come reference nella chat")}
              onClick={() => onMention(file.id)}
            >
              <AtSign size={13} /> {t("Cita")}
            </button>
            <button
              type="button"
              disabled={busy}
              title={t("Mostra nella cartella")}
              aria-label={t("Mostra {0} nella cartella", file.name)}
              onClick={() =>
                void run(() => api.revealGenerated(callId, file.id))
              }
            >
              <FolderOpen size={13} />
            </button>
          </div>
        </article>
      ))}
      {error && (
        <p role="alert" className="source-error">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
    </section>
  );
}
