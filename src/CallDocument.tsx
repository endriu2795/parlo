import { t } from "./i18n";
import { useEffect, useRef, useState, type FocusEvent, type KeyboardEvent, type ReactNode } from "react";
import { ArrowUp, AudioLines, Check, ChevronDown, ChevronRight, Circle, Columns3, Copy, Download, MoreHorizontal, RefreshCw, Share2, Sparkles, StickyNote, Trash2 } from "lucide-react";
import AudioPlayer from "./AudioPlayer";
import CallTags from "./CallTags";
import ScreenshotCard from "./ScreenshotCard";
import TranscriptSearch, { type SearchFocus } from "./TranscriptSearch";
import { HighlightedText } from "./SearchText";
import { quietLegacyAudioDiagnostic } from "./audioWarning";
import { fmtClock, fmtDate, fmtTime, statusLabel } from "./format";
import { useDetailsMenu } from "./useDialog";
import { passageAtTime } from "./transcriptNavigation";
import type { CallNote, Detail, Screenshot, Segment, Settings, ShareDestination, PublicationStatus } from "./types";

type TimelineEntry = { time: number; key: string } & (
  | { kind: "segment"; segment: Segment }
  | { kind: "image"; image: Screenshot }
  | { kind: "note"; note: CallNote }
);

function buildTimeline(segments: Segment[], detail: Detail): TimelineEntry[] {
  return [
    ...segments.map((segment, i): TimelineEntry => ({ kind: "segment", time: segment.start_ms, key: `text-${i}`, segment })),
    ...(detail.screenshots || []).map((image): TimelineEntry => ({ kind: "image", time: image.timestamp_ms, key: image.id, image })),
    ...(detail.notes || []).map((note): TimelineEntry => ({ kind: "note", time: note.timestamp_ms, key: note.id, note })),
  ].sort((a, b) => a.time - b.time);
}

export type CallDocumentProps = {
  detail: Detail;
  /** Live preview segments while recording, otherwise the saved transcript. */
  segments: Segment[];
  live: boolean;
  liveStage: string;
  shared?: { publisher: string; loading: boolean; error?: string; approximateDuration: boolean };
  busy: boolean;
  query: string;
  searchFocus?: SearchFocus;
  onClearQuery: () => void;
  captureActions: ReactNode;
  captureMessage: string;
  exportMessage: string;
  retryQueued: boolean;
  canChangeService: boolean;
  chatOpen: boolean;
  onToggleChat: () => void;
  onRename: (title: string) => void;
  onTags: (tags: string[]) => Promise<void>;
  onRetry: () => void;
  onChangeService: () => void;
  onExport: () => void;
  onCopy: () => void;
  sharing?: Settings["sharing"];
  publication?: { result?: PublicationStatus; error?: string } | null;
  onPublish: (destination: ShareDestination) => void;
  onResetPanels: () => void;
  onDelete: () => void;
};

export default function CallDocument(props: CallDocumentProps) {
  const { detail, segments, live, query, shared } = props;
  const call = detail.call;
  const scroll = useRef<HTMLDivElement>(null);
  const player = useRef<HTMLDivElement>(null);
  const position = useRef<number | null>(null);
  const [pinned, setPinned] = useState(false);
  const [following, setFollowing] = useState(false);
  const [showTop, setShowTop] = useState(false);
  const [activePassage, setActivePassage] = useState<{ callId: string; index: number | null } | null>(null);
  const menu = useDetailsMenu(call.id);
  const shareMenu = useDetailsMenu(call.id);
  const [seek, setSeek] = useState<{ callId: string; time: number; id: number } | null>(null);
  const timeline = buildTimeline(segments, detail);
  const hasContent = shared ? !shared.loading && !shared.error : !!(detail.segments.length || detail.notes?.length);
  const shareDestinations = ([
    ["slack", props.sharing?.slack, t("Invia trascrizione a Slack")],
    ["teams", props.sharing?.teams, t("Invia trascrizione a Teams")],
    ["folder", !!props.sharing?.folder, t("Pubblica trascrizione nella cartella")],
  ] as const).filter(([, enabled]) => enabled);
  const canSeek = !!(detail.audio_directory || detail.video_path) && ["ready", "failed"].includes(call.status);
  const seekTo = (time: number) => { if (canSeek) setSeek({ callId: call.id, time, id: performance.now() }); };
  const activeIndex = activePassage?.callId === call.id ? activePassage.index : null;
  const positionChanged = (time: number) => {
    position.current = time;
    const index = passageAtTime(segments, time);
    setActivePassage(current => current?.callId === call.id && current.index === index ? current : { callId: call.id, index });
  };
  const scrollToPassage = (index: number, onlyIfHidden = false) => {
    const container = scroll.current;
    const target = container?.querySelector<HTMLElement>(`[data-passage-index="${index}"]`);
    if (!container || !target) return;
    const bounds = container.getBoundingClientRect();
    const search = container.querySelector<HTMLElement>(".transcript-search");
    const offset = (search?.offsetHeight || 0) + (pinned ? player.current?.offsetHeight || 0 : 0) + 16;
    const targetBounds = target.getBoundingClientRect();
    if (onlyIfHidden && targetBounds.top >= bounds.top + offset && targetBounds.bottom <= bounds.bottom) return;
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    container.scrollTo({ top: Math.max(0, container.scrollTop + targetBounds.top - bounds.top - offset), behavior: reducedMotion ? "auto" : "smooth" });
  };
  const jumpToCurrent = () => {
    if (position.current === null) return;
    const index = passageAtTime(segments, position.current, true);
    if (index !== null) scrollToPassage(index);
  };
  useEffect(() => {
    position.current = null;
    setActivePassage(null);
    setFollowing(false);
    const container = scroll.current;
    setShowTop(!!container && container.scrollTop > container.clientHeight);
  }, [call.id]);
  useEffect(() => {
    if (following && activeIndex !== null) scrollToPassage(activeIndex, true);
  }, [following, activeIndex, call.id, pinned]);
  useEffect(() => {
    if (query.trim()) setFollowing(false);
  }, [query]);
  useEffect(() => {
    const container = scroll.current;
    const pinnedPlayer = player.current;
    if (!container || !pinned || !pinnedPlayer) return;
    const update = () => container.style.setProperty("--pinned-player-height", `${pinnedPlayer.offsetHeight + (query.trim() ? 48 : 0) + 16}px`);
    update();
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(update);
    observer?.observe(pinnedPlayer);
    return () => { observer?.disconnect(); container.style.removeProperty("--pinned-player-height"); };
  }, [pinned, query, canSeek, call.id]);
  const quietDiagnostic = call.status === "ready" && quietLegacyAudioDiagnostic(call.error);
  const needsAttention = !quietDiagnostic && (call.error || call.status === "pending");
  const menuAction = (action: () => void) => () => { menu.close(); action(); };
  const finishRename = (event: FocusEvent<HTMLInputElement>) => {
    const title = event.currentTarget.value.trim();
    event.currentTarget.value = title || call.title;
    if (title && title !== call.title) props.onRename(title);
  };
  const renameKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") event.currentTarget.blur();
    if (event.key === "Escape") { event.currentTarget.value = call.title; event.currentTarget.blur(); }
  };

  return (
    <section className="transcript" id="transcript-panel" aria-label={shared ? t("Trascrizione condivisa") : t("Documento della registrazione")}>
      <div className="document-toolbar">
        <div className="document-breadcrumb">
          <span>{shared ? t("Condivise") : t("Registrazioni")}</span><ChevronRight size={12} />
          {shared ? <span className="breadcrumb-title-input read-only">{call.title}</span> : <input className="breadcrumb-title-input" aria-label={t("Titolo della call nella barra superiore")} title={t("Clicca per rinominare")}
            key={`${call.id}-${call.title}`} defaultValue={call.title} maxLength={200} size={Math.min(Math.max(call.title.length, 8), 50)}
            onBlur={finishRename} onKeyDown={renameKeyDown} />}
        </div>
        <div className="detail-actions">
          <button className="text-button" title={t("Salva la trascrizione come file Markdown")} aria-label={t("Esporta la trascrizione in Markdown")}
            onClick={props.onExport} disabled={!hasContent || props.busy}>
            <Download size={15} /><span className="action-label">{t("Esporta")}</span>
          </button>
          <button className={`text-button chat-toggle ${props.chatOpen ? "selected" : ""}`} title={props.chatOpen ? t("Chiudi assistente") : t("Apri assistente")}
            aria-label={props.chatOpen ? t("Chiudi assistente") : t("Apri assistente")} aria-pressed={props.chatOpen} onClick={props.onToggleChat}>
            <Sparkles size={15} /><span>{t("Assistente")}</span>
          </button>
          <details className="document-menu" ref={menu.ref}>
            <summary aria-label={t("Altre opzioni")} title={t("Altre opzioni")}><MoreHorizontal size={18} /></summary>
            <div className="document-menu-content">
              <button type="button" onClick={menuAction(props.onCopy)} disabled={!hasContent || props.busy}><Copy size={14} />{t("Copia come testo Markdown")}</button>
              <button type="button" onClick={menuAction(props.onResetPanels)}><Columns3 size={14} />{t("Ripristina disposizione pannelli")}</button>
              {!shared && <><hr />
              <button type="button" className="document-delete" disabled={call.status === "recording" || call.status === "processing"} onClick={menuAction(props.onDelete)}>
                <Trash2 size={14} />{t("Elimina registrazione")}
              </button></>}
            </div>
          </details>
        </div>
      </div>

      <div className="document-scroll" ref={scroll} onScroll={event => {
        const container=event.currentTarget;
        setShowTop(container.scrollTop > container.clientHeight);
      }} onWheel={event => {
        if (event.deltaY && !(event.target as Element).closest(".call-audio-player")) setFollowing(false);
      }} onTouchMove={event => {
        if (!(event.target as Element).closest(".call-audio-player")) setFollowing(false);
      }}>
        {showTop && <div className={`document-top-float ${query.trim() && !pinned ? "has-search" : ""}`}>
          <button type="button" title={t("Torna all’inizio della registrazione")} aria-label={t("Torna all’inizio della registrazione")} onClick={() => {
            setFollowing(false);
            const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
            scroll.current?.scrollTo({ top: 0, behavior: reducedMotion ? "auto" : "smooth" });
          }}><ArrowUp size={15}/>{t("Torna su")}</button>
        </div>}
        <div className="document-surface">
          {props.exportMessage && <div className="document-export-message" role="status"><Check size={13} />{props.exportMessage}</div>}
          <header className="transcript-header">
            <div className="transcript-title-row">
              {shared ? <h1 className="title-input read-only">{call.title}</h1> : <input className="title-input" aria-label={t("Titolo della call")} title={t("Clicca per rinominare")}
                key={`${call.id}-${call.title}`} defaultValue={call.title} maxLength={200}
                onBlur={finishRename} onKeyDown={renameKeyDown} />}
              {!shared && call.status === "ready" && detail.segments.length > 0 && shareDestinations.length > 0 && (
                <details className="document-menu document-share" ref={shareMenu.ref}>
                  <summary><Share2 size={14} />{t("Condividi")}<ChevronDown size={13} /></summary>
                  <div className="document-menu-content">
                    {shareDestinations.map(([destination, , label]) => (
                      <button type="button" key={destination} disabled={props.busy} onClick={() => {
                        shareMenu.close();
                        props.onPublish(destination);
                      }}>{label}{props.publication?.result?.destinations.includes(destination) ? ` · ${t("Già condivisa")}` : ""}</button>
                    ))}
                  </div>
                </details>
              )}
            </div>
            <div className="detail-eyebrow">
              <span>{fmtDate(call.created_at)}</span><span aria-hidden>·</span>
              <span>{t("Inizio")} <time dateTime={call.created_at}>{fmtClock(call.created_at)}</time></span>
              {(!shared || hasContent) && <span className="call-duration">{t("Durata")} {shared?.approximateDuration ? "≈ " : ""}{fmtTime(call.duration_ms)}</span>}
              {shared ? <span>{t("Pubblicata da {0}", shared.publisher)}</span> : <span className={`document-state ${call.status}`}><i />{statusLabel(call.status)}</span>}
            </div>
            {!shared && props.publication?.result && (props.publication.result.destinations.length > 0 || props.publication.result.folder_unavailable) && <div className="publication-status">
              {props.publication.result.destinations.length > 0 && <span><Share2 size={13} /> {t("Condivisa su {0}", props.publication.result.destinations.map(destination => destination === "folder" ? t("cartella Drive / OneDrive") : destination === "slack" ? "Slack" : "Teams").join(", "))}</span>}
              {props.publication.result.folder_unavailable && <span>{t("Stato della cartella condivisa non disponibile.")}</span>}
            </div>}
            {!shared && props.publication?.error && <div className="publication-status" role="status">{props.publication.error}</div>}
            {!shared && <CallTags key={call.id} tags={call.tags || []} disabled={props.busy} onSave={props.onTags} />}
          </header>

          {query.trim() && (!shared || hasContent) && <TranscriptSearch query={query.trim()} callId={call.id} scope={scroll} content={detail} focus={props.searchFocus} onClear={props.onClearQuery} />}
          {live && props.captureActions}
          {canSeek && <div ref={player} className={`media-player-wrap ${pinned ? "is-pinned" : ""} ${query.trim() ? "has-search" : ""}`}>
            <AudioPlayer key={call.id} callId={call.id} video={!!detail.video_path} seekRequest={seek?.callId === call.id ? seek : undefined}
              pinned={pinned} following={following} hasPassages={segments.length > 0} onPinChange={setPinned}
              onFollowChange={value => { setFollowing(value); if (value) jumpToCurrent(); }}
              onJumpToPassage={jumpToCurrent} onPositionChange={positionChanged} />
          </div>}
          {props.captureMessage && <p role="status" className="export-message">{props.captureMessage}</p>}

          {quietDiagnostic && (
            <details className="audio-diagnostic">
              <summary>{t("Dettagli tecnici sull’audio")}</summary>
              <p>{call.error}</p>
            </details>
          )}
          {needsAttention && (
            <div className="call-error">
              <p>{call.error || t("Questa registrazione non è ancora stata trascritta.")}</p>
              {["failed", "pending"].includes(call.status) && <>
                <div className="call-error-actions">
                  <button className="secondary-button" disabled={props.busy || props.retryQueued} onClick={props.onRetry}>
                    <RefreshCw size={14} /> {t("Riprova la trascrizione")}
                  </button>
                  <button className="text-button" disabled={props.busy || !props.canChangeService} onClick={props.onChangeService}>
                    {t("Usa un altro servizio")}
                  </button>
                </div>
                <small>{t("L’audio è conservato: puoi cambiare servizio di trascrizione nelle impostazioni e riprovare.")}</small>
              </>}
            </div>
          )}
          {(live || (call.status !== "ready" && segments.length > 0)) && (
            <div className="live-banner" role="status">
              <Circle size={7} fill="currentColor" />
              <span>{live ? props.liveStage || t("Anteprima · in attesa del parlato") : call.status === "failed"
                ? t("Trascrizione incompleta · ultimo passaggio a {0}. Premi Riprova per completarla.", fmtTime(segments.reduce((last, segment) => Math.max(last, segment.end_ms), 0)))
                : t("Trascrizione in elaborazione · testo provvisorio")}</span>
            </div>
          )}

          {shared?.error ? <p role="alert">{shared.error}</p> : shared?.loading ? <p role="status">{t("Caricamento…")}</p> : <>
          <div className="transcript-section-label">
            <AudioLines size={13} /><span>{t("Trascrizione")}</span>
            <span className="transcript-section-count">{segments.length} {segments.length === 1 ? "passaggio" : "passaggi"}</span>
          </div>
          <div className="segments">
            {timeline.map(entry => {
              const timestamp = (
                <button type="button" className="timestamp-link mono" disabled={!canSeek} onClick={() => seekTo(entry.time)}
                  title={canSeek ? t("Ascolta da questo punto") : t("Audio non conservato")}>{fmtTime(entry.time)}</button>
              );
              if (entry.kind === "note") return (
                <article key={entry.key} className="segment manual-note" data-search-source="note" data-search-start={entry.time} data-search-end={entry.time}>
                  {timestamp}
                  <div><strong><StickyNote size={13} /> {t("Nota")}</strong><p><HighlightedText text={entry.note.text} query={query} passage /></p></div>
                </article>
              );
              if (entry.kind === "image") return <ScreenshotCard key={entry.key} callId={call.id} image={entry.image} onSeek={canSeek ? seekTo : undefined} />;
              const { segment } = entry;
              const you = !shared && segment.speaker === "Tu";
              return (
                <article key={entry.key} className={`segment ${activeIndex === Number(entry.key.slice(5)) ? "is-current" : ""}`}
                  data-passage-index={Number(entry.key.slice(5))} data-search-source="text" data-search-start={segment.start_ms} data-search-end={segment.end_ms}>
                  {timestamp}
                  <div>
                    <strong className={you ? "speaker-you" : ""} title={shared ? undefined : you ? t("Voce dal tuo microfono (stima)") : t("Voce dall’audio del computer (stima)")}>{you ? t("Tu") : segment.speaker}</strong>
                    <p><HighlightedText text={segment.text} query={query} passage /></p>
                    {segment.quality && <small className="segment-quality">{segment.quality}</small>}
                    {segment.original_text && <details className="segment-original"><summary>{t("Mostra testo originale")}</summary><p>{segment.original_text}</p></details>}
                  </div>
                </article>
              );
            })}
            {!timeline.length && (
              <div className="transcript-empty">
                <AudioLines size={28} />
                <p>{call.status === "recording" ? t("Registrazione in corso.") : call.status === "ready" ? t("Nessun parlato rilevato.") : t("Il testo apparirà a fine elaborazione.")}</p>
              </div>
            )}
          </div>
          </>}
          {!shared && call.status === "ready" && (
            <footer className="transcript-footer">
              <Check size={14} />
              {detail.video_path ? t("Trascrizione pronta · video e audio conservati") : detail.audio_directory ? t("Trascrizione pronta · audio conservato") : t("Trascrizione pronta · audio temporaneo eliminato")}
            </footer>
          )}
        </div>
      </div>
    </section>
  );
}
