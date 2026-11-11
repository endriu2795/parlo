import { t, uiLocale } from "../i18n";
import { useState } from "react";
import { ArrowUpRight, Check, ChevronDown, Copy, FileText, ListChecks, ListTree, MessageCircle, Sparkles } from "lucide-react";
import AssistantIdentity from "../AssistantIdentity";
import ChatMarkdown from "../ChatMarkdown";
import SourceMentions from "../ChatSourceMentions";
import type { ChatSource } from "../types";

function modelLabel(id: string) {
  return id
    .replace(/^claude-/, "")
    .replace(/-(\d+)-(\d+)(?:-\d{8})?$/, " $1.$2")
    .replace(/^\w/, c => c.toUpperCase());
}

export function MessageTime({ sentAt }: { sentAt?: string | null }) {
  if (!sentAt) return null;
  const date = new Date(sentAt);
  if (Number.isNaN(date.getTime())) return null;
  const fullDate = date.toLocaleString(uiLocale);
  return <time className="message-time" dateTime={sentAt} title={fullDate} aria-label={t("Inviato il {0}", fullDate)}>
    {date.toLocaleTimeString(uiLocale, { hour: "2-digit", minute: "2-digit", hourCycle: "h23" })}
  </time>;
}

export function UserMessage({ text, sentAt, callId, sources }: { text: string; sentAt?: string | null; callId: string; sources: ChatSource[] }) {
  return (
    <article className="assistant-message user">
      <div className="user-bubble"><SourceMentions text={text} callId={callId} sources={sources} /></div>
      <MessageTime sentAt={sentAt} />
    </article>
  );
}

/** A saved reply. When the text already appears in the activity timeline, only the actions are shown. */
export function AssistantReply({ content, sentAt, model, inTimeline, onError }: {
  content: string;
  sentAt?: string | null;
  model?: string | null;
  inTimeline: boolean;
  onError: (error: string) => void;
}) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      onError(t("Impossibile copiare il testo. Puoi selezionarlo nella risposta."));
    }
  };
  return (
    <article className={`assistant-message assistant ${inTimeline ? "timeline-actions" : ""}`}>
      {!inTimeline && <>
        <AssistantIdentity />
        <div className="message-markdown"><ChatMarkdown>{content}</ChatMarkdown></div>
      </>}
      <div className="message-actions">
        <button title={t("Copia risposta")} aria-label={t("Copia risposta")} onClick={() => void copy()}>
          {copied ? <Check size={14} /> : <Copy size={14} />}
          <span>{copied ? t("Copiata") : t("Copia")}</span>
        </button>
        <MessageTime sentAt={sentAt} />
        {model && (
          <details className="response-model">
            <summary>{modelLabel(model)}<ChevronDown size={10} /></summary>
            <code>{model}</code>
          </details>
        )}
      </div>
    </article>
  );
}

export function StreamingReply({ answer }: { answer: string }) {
  return (
    <article className="assistant-message assistant streaming">
      <AssistantIdentity streaming />
      <div className="message-markdown"><ChatMarkdown>{answer}</ChatMarkdown></div>
    </article>
  );
}

const starters = [
  { label: t("Riassumi"), icon: ListTree, prompt: t("Riassumi i punti principali"), hint: t("Punti principali della conversazione") },
  { label: t("Estrai decisioni e attività"), icon: ListChecks, prompt: t("Estrai decisioni e prossime azioni"), hint: t("Azioni, responsabili e scadenze citate") },
  { label: t("Trova le domande aperte"), icon: MessageCircle, prompt: t("Quali domande sono rimaste aperte?"), hint: t("Questioni rimaste senza risposta") },
];

export function ChatEmpty({ ready, onSend }: { ready: boolean; onSend: (prompt: string) => void }) {
  return (
    <div className="widget-empty">
      <div className="widget-empty-symbol"><Sparkles size={24} strokeWidth={1.5} /></div>
      <h3>{t("Lavora sulla registrazione")}</h3>
      <p>{t("Trova un dettaglio, riordina le idee o prepara i prossimi passi.")}</p>
      <div className="widget-starters">
        {starters.map(item => (
          <button key={item.prompt} disabled={!ready} aria-label={item.prompt} onClick={() => onSend(item.prompt)}>
            <item.icon size={18} strokeWidth={1.6} />
            <span><strong>{item.label}</strong><small>{item.hint}</small></span>
            <ArrowUpRight size={15} />
          </button>
        ))}
      </div>
    </div>
  );
}

/** Autocomplete for "@" mentions in the composer, grouped by source scope. */
export function MentionList({ options, active, onChoose }: { options: ChatSource[]; active: number; onChoose: (source: ChatSource) => void }) {
  return (
    <div className="source-mentions" role="listbox" id="source-mentions" aria-label={t("Menziona una fonte")}>
      {(["chat", "shared"] as const).map(scope => {
        const group = options.filter(source => source.scope === scope);
        return group.length ? (
          <div key={scope}>
            <strong>{scope === "chat" ? t("Questa chat") : t("Cartella condivisa")}</strong>
            {group.map(source => {
              const index = options.indexOf(source);
              return (
                <button key={source.id} id={`source-option-${index}`} type="button" role="option" aria-selected={index === active}
                  onMouseDown={e => e.preventDefault()} onClick={() => onChoose(source)}>
                  <FileText size={13} />
                  <span>{source.name}<small>{source.path || t("Nel contesto della call")}</small></span>
                </button>
              );
            })}
          </div>
        ) : null;
      })}
      {!options.length && <p>{t("Nessuna fonte disponibile. Aggiungi file dal pulsante Fonti.")}</p>}
    </div>
  );
}
