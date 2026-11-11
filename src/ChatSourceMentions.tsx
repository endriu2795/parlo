import { t } from "./i18n";
import { Fragment, useState } from "react";
import { api, desktop } from "./api";
import { splitSourceMentions } from "./sourceMentions";
import type { ChatSource } from "./types";

function SourceLink({ callId, source, text }: { callId: string; source: ChatSource; text: string }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return <>
    <a className="chat-source-mention" href={`#source-${encodeURIComponent(source.id)}`}
      title={t("Mostra nella cartella: {0}", source.path)} aria-label={t("Mostra {0} nella cartella", source.name)}
      aria-disabled={busy || !desktop} aria-busy={busy}
      onClick={async event => {
        event.preventDefault();
        if (busy || !desktop) return;
        setBusy(true); setError("");
        try { await api.revealSource(callId, source.id); }
        catch (e) { setError(String(e)); }
        finally { setBusy(false); }
      }}>{text}</a>
    {error && <span role="alert" className="chat-file-error">{error}</span>}
  </>;
}

export default function SourceMentions({ text, callId, sources }: { text: string; callId: string; sources: ChatSource[] }) {
  return <>{splitSourceMentions(text, sources).map((part, index) => part.source
    ? <SourceLink key={`${callId}:${part.source.id}:${index}`} callId={callId} source={part.source} text={part.text}/>
    : <Fragment key={index}>{part.text}</Fragment>)}</>;
}
