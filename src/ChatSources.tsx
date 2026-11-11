import { t } from "./i18n";
import { useEffect, useRef, useState } from "react";
import {
  AtSign,
  FileText,
  Folder,
  FolderOpen,
  Plus,
  RefreshCw,
  Trash2,
  X,
  Play,
} from "lucide-react";
import { api, desktop } from "./api";
import type { ChatSource, SourceInventory } from "./types";
import GeneratedFiles from "./GeneratedFiles";

export function useChatSources(callId: string, revision: string) {
  const [inventory, setInventory] = useState<SourceInventory | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const currentCall = useRef(callId);
  currentCall.current = callId;
  const request = useRef(0);
  const run = async (operation: (id: string) => Promise<SourceInventory>) => {
    const id = callId,
      epoch = ++request.current;
    setBusy(true);
    setError("");
    try {
      const result = await operation(id);
      if (currentCall.current === id && request.current === epoch)
        setInventory(result);
    } catch (e) {
      if (currentCall.current === id && request.current === epoch)
        setError(String(e));
    } finally {
      if (currentCall.current === id && request.current === epoch)
        setBusy(false);
    }
  };
  useEffect(() => {
    setInventory(null);
    setError("");
    if (desktop) void run(api.sources);
    return () => {
      ++request.current;
    };
  }, [callId, revision]);
  return {
    callId,
    inventory,
    busy,
    error,
    run,
    refresh: () => run(api.sources),
  };
}

const kindLabel: Record<ChatSource["kind"], string> = {
  transcript: t("Già nel contesto"),
  notes: t("Già nel contesto"),
  image: t("Immagine"),
  audio: "Audio",
  video: t("Video locale · escluso dall’AI"),
  file: t("File locale"),
  folder: t("Cartella locale"),
};

export default function ChatSources({
  controller,
  disabled,
  onMention,
  onClose,
}: {
  controller: ReturnType<typeof useChatSources>;
  disabled: boolean;
  onMention: (source: ChatSource) => void;
  onClose: () => void;
}) {
  const { inventory, busy, error, run, refresh } = controller;
  const [search, setSearch] = useState("");
  const blocked = disabled || busy;
  const sources = inventory?.sources || [];
  const matches = (source: ChatSource) =>
    `${source.name} ${source.path || ""}`
      .toLocaleLowerCase()
      .includes(search.toLocaleLowerCase());
  const row = (source: ChatSource) => (
    <div
      className={`source-row ${source.enabled ? "" : "excluded"}`}
      key={source.id}
    >
      <input
        type="checkbox"
        checked={source.enabled}
        aria-label={t("Usa {0}", source.name)}
        disabled={
          blocked ||
          !source.available ||
          !!source.unsupported_reason ||
          !!(
            source.parent_id &&
            !sources.find((s) => s.id === source.parent_id)?.enabled
          )
        }
        onChange={(e) =>
          void run((id) => api.setSource(id, source.id, e.target.checked))
        }
      />
      <span className="source-type">
        {source.kind === "folder" ? (
          <Folder size={15} />
        ) : (
          <FileText size={15} />
        )}
      </span>
      <div className="source-description">
        <span title={source.path || source.name}>{source.name}</span>
        <small>
          {!source.available
            ? t("Non disponibile nel percorso originale")
            : source.unsupported_reason
              ? source.unsupported_reason
              : source.enabled
                ? source.read_hint || kindLabel[source.kind]
                : t("Esclusa dalle prossime richieste")}
        </small>
      </div>
      <button
        type="button"
        title={t("Menziona {0}", source.name)}
        aria-label={t("Menziona {0}", source.name)}
        disabled={
          blocked ||
          !source.enabled ||
          !source.available ||
          !!source.unsupported_reason
        }
        onClick={() => onMention(source)}
      >
        <AtSign size={14} />
      </button>
      {source.kind === "video" && <button type="button" title={t("Apri video locale")} aria-label={t("Apri video locale")} onClick={() => void api.openVideo(controller.callId).catch(e => setRevealError(String(e)))}><Play size={14}/></button>}
      {source.path && (
        <button
          type="button"
          title={t("Mostra nella cartella")}
          aria-label={t("Mostra {0} nella cartella", source.name)}
          disabled={!source.available}
          onClick={() =>
            void api
              .revealSource(controller.callId, source.id)
              .catch((e) => setRevealError(String(e)))
          }
        >
          <FolderOpen size={14} />
        </button>
      )}
      {source.removable && (
        <button
          type="button"
          className="source-remove"
          title={
            source.scope === "shared"
              ? t("Scollega da tutte le chat; l’originale rimane sul disco")
              : t("Rimuovi da questa chat; l’originale rimane sul disco")
          }
          aria-label={t("Rimuovi {0} {1}", source.name, source.scope === "shared" ? "da tutte le chat" : "da questa chat")}
          disabled={blocked}
          onClick={() => void run((id) => api.removeSource(id, source.id))}
        >
          <Trash2 size={14} /> {t("Rimuovi")}
        </button>
      )}
    </div>
  );
  // File manager errors stay in the sources panel.
  const [revealError, setRevealError] = useState("");
  const group = (scope: "chat" | "shared") =>
    sources
      .filter(
        (source) =>
          source.scope === scope &&
          !source.parent_id &&
          !source.id.startsWith("output:"),
      )
      .map((source) => {
        const children = sources.filter(
          (child) => child.parent_id === source.id && matches(child),
        );
        if (!matches(source) && !children.length) return null;
        return source.kind === "folder" ? (
          <div key={source.id}>
            {row(source)}
            {!!children.length && (
              <details
                className="source-folder"
                open={search ? true : undefined}
              >
                <summary>
                  {children.length} file{search ? " trovati" : " disponibili"}
                </summary>
                <div>{children.map(row)}</div>
              </details>
            )}
          </div>
        ) : (
          row(source)
        );
      });
  return (
    <section className="chat-sources" aria-label={t("File e fonti della chat")}>
      <header>
        <strong>{t("File e fonti")}</strong>
        <button
          type="button"
          aria-label={t("Aggiorna fonti")}
          title={t("Aggiorna elenco file")}
          disabled={blocked}
          onClick={() => void refresh()}
        >
          <RefreshCw size={14} />
        </button>
        <button type="button" aria-label={t("Torna alla chat")} onClick={onClose}>
          <X size={15} /> {t("Torna alla chat")}
        </button>
      </header>
      <div className="chat-sources-body">
      <p>
        {t("Trascrizione e note abilitate entrano nel contesto. Gli altri file sono disponibili all’AI quando servono. Usa") + " "}<b>@</b> {t("per richiamarli. Audio, video e formati incompatibili sono esclusi. I template Office richiedono strumenti locali di lettura o conversione.")}
      </p>
      <input
        className="source-search"
        aria-label={t("Cerca nelle fonti")}
        placeholder={t("Cerca file…")}
        value={search}
        onChange={(e) => setSearch(e.target.value)}
      />
      <GeneratedFiles
        key={controller.callId}
        callId={controller.callId}
        files={inventory?.generated || []}
        disabled={disabled}
        onRefresh={refresh}
        onMention={(fileId) => {
          const source = sources.find((s) => s.id === `output:${fileId}`);
          if (source) onMention(source);
        }}
        mentionable={sources
          .filter((s) => s.enabled && s.available && !s.unsupported_reason)
          .map((s) => s.id)}
      />
      <div className="source-section-title">
        <strong>{t("Questa chat")}</strong>
      </div>
      <div className="source-actions">
        <button
          type="button"
          disabled={blocked}
          onClick={() => void run((id) => api.addSources(id))}
        >
          <Plus size={13} /> {t("Aggiungi file")}
        </button>
        <button
          type="button"
          disabled={blocked}
          onClick={() => void run((id) => api.addSources(id, true))}
        >
          <Folder size={13} /> {t("Aggiungi cartella")}
        </button>
      </div>
      {group("chat")}
      <div className="source-section-title">
        <strong>{t("File e cartelle condivisi")}</strong>
        <small>{t("Fra tutte le chat")}</small>
      </div>
      <p>
        {t("La spunta decide cosa usare in questa chat. Rimuovi scollega la fonte da tutte le chat.")}
      </p>
      <div className="source-actions">
        <button
          type="button"
          disabled={blocked || !inventory}
          onClick={() => void run((id) => api.addSources(id, false, true))}
        >
          <Plus size={13} /> {t("Aggiungi file condivisi")}
        </button>
        <button
          type="button"
          disabled={blocked || !inventory}
          onClick={() => void run((id) => api.addSources(id, true, true))}
        >
          <Folder size={13} /> {t("Aggiungi cartella condivisa")}
        </button>
      </div>
      {group("shared")}
      {busy && <p role="status">{t("Aggiornamento fonti…")}</p>}
      {inventory?.truncated && (
        <p>
          {t("Elenco limitato a 1.000 elementi e 12 livelli. L’AI può esplorare le cartelle abilitate.")}
        </p>
      )}
      {(error || revealError) && (
        <p className="source-error" role="alert">
          {error || revealError}
        </p>
      )}
      <p className="source-footnote">
        {t("I file rimangono nel percorso originale. Escludere una fonte non rimuove ciò che è già nella cronologia.")}
      </p>
      </div>
    </section>
  );
}
