import { t } from "./i18n";
import { useEffect, useMemo, useRef, useState } from "react";
import { Circle, FileText, FileUp, Mic, Moon, RefreshCw, Search, Send, Settings as SettingsIcon, ShieldCheck, Square, Sun, Users } from "lucide-react";
import { HighlightedText, SearchExcerpt } from "./SearchText";
import { FilterPanel, FilterSummary, FilterToggle, filterCount, type ArchiveFilter } from "./ArchiveFilters";
import { archiveDay, archiveHeading, fmtClock, fmtTime, statusLabel } from "./format";
import VirtualArchiveList from "./VirtualArchiveList";
import type { SettingsTab } from "./SettingsModal";
import type { Call, SearchHit, SharedTranscript, TagCount } from "./types";

const parloIcon = new URL("./assets/parlo-app-icon.png", import.meta.url).href;
const parloIcon2x = new URL("./assets/parlo-app-icon@2x.png", import.meta.url).href;


export type ArchiveSidebarProps = {
  calls: Call[];
  total: number;
  hasMore: boolean;
  loading: boolean;
  loadingMore: boolean;
  onLoadMore: () => void;
  selected: string | null;
  onOpen: (call: Call, hit?: SearchHit) => void;
  shared: SharedTranscript[];
  selectedShared?: string;
  onOpenShared: (id: string) => void;
  onCloseShared: () => void;
  query: string;
  onQuery: (query: string) => void;
  filter: ArchiveFilter;
  onFilter: (filter: ArchiveFilter) => void;
  /** Every tag in the archive, offered by the filter menu. */
  tags: TagCount[];
  recording: boolean;
  elapsedMs: number;
  canRecord: boolean;
  onRecord: () => void;
  canImport: boolean;
  importing: boolean;
  onImport: () => void;
  /** Formatted global shortcut, or null when the system refused to register it. */
  shortcut: string | null;
  retentionMonths: number | undefined;
  onSettings: (tab: SettingsTab) => void;
  /** Theme and settings live in the custom titlebar on Windows. */
  preferences: { theme: "dark" | "light"; onTheme: () => void } | null;
  desktop: boolean;
};

function CallStatusIcon({ status, origin }: Pick<Call, "status" | "origin">) {
  const source = origin === "telegram" ? { icon: <Send size={15} />, label: "Telegram" }
    : origin === "file" ? { icon: <FileUp size={16} />, label: t("File importato") }
    : origin === "recording" ? { icon: <Mic size={16} />, label: t("Call registrata") }
    : { icon: <FileText size={16} />, label: "" };
  const icon = status === "processing" || status === "finalizing" ? <RefreshCw size={15} className="spin" />
    : status === "recording" ? <Circle size={11} fill="currentColor" />
    : source.icon;
  const label = [source.label, statusLabel(status)].filter(Boolean).join(" · ");
  return <span className={`archive-file-icon ${status}`} title={label}>{icon}<span className="sr-only">{label}</span></span>;
}

function CallSearchResults({ call, onOpen }: { call: Call; onOpen: (call: Call, hit?: SearchHit) => void }) {
  const search = call.search!;
  return (
    <div className="call-search-results">
      <span className="call-search-count">{search.count ? `${search.count} ${search.count === 1 ? t("passaggio trovato") : t("passaggi trovati")}` : t("Trovato nel titolo o nei tag")}</span>
      {search.hits.map((hit, i) => (
        <button type="button" className="call-search-hit" key={`${hit.source}-${hit.timestamp_ms}-${i}`} onClick={() => onOpen(call, hit)}>
          <span className="call-search-hit-time"><time className="mono">{fmtTime(hit.timestamp_ms)}</time>{hit.source === "note" ? t("Nota") : t("Trascrizione")}</span>
          <span><SearchExcerpt text={hit.excerpt} /></span>
        </button>
      ))}
      {search.count > search.hits.length && <span className="call-search-more">{t("Altri {0} passaggi nella registrazione", search.count - search.hits.length)}</span>}
    </div>
  );
}

const SHARED_PAGE_SIZE = 30;

function SharedCallList({ items, query, selected, onOpen }: {
  items: SharedTranscript[]; query: string; selected?: string; onOpen: (id: string) => void;
}) {
  const [limit, setLimit] = useState(SHARED_PAGE_SIZE);
  const matches = useMemo(() => {
    const term = query.trim().toLocaleLowerCase();
    return items.filter(item => item.id === selected || [item.title, item.publisher].some(text => text.toLocaleLowerCase().includes(term)));
  }, [items, query, selected]);

  const rows = useMemo(() => matches.slice(0, limit), [matches, limit]);
  return <VirtualArchiveList label={t("Condivise")} rows={rows} estimateSize={65} renderRow={item => (
      <article className={`call-item ${selected === item.id ? "selected" : ""}`}>
        <button type="button" className="call-select" aria-current={selected === item.id ? "page" : undefined} onClick={() => onOpen(item.id)}>
          <span className="archive-file-icon"><Users size={16} /></span>
          <div className="archive-file-content">
            <strong><HighlightedText text={item.title} query={query} substring /></strong>
            <div className="call-meta"><HighlightedText text={item.publisher} query={query} substring /></div>
          </div>
        </button>
      </article>
    )}>
    {matches.length === 0 && <div className="sidebar-empty">{t("Nessuna trascrizione condivisa trovata.")}</div>}
    {matches.length > limit && <button type="button" className="load-more" onClick={() => setLimit(value => value + SHARED_PAGE_SIZE)}>{t("Mostra altre")}</button>}
  </VirtualArchiveList>;
}

export default function ArchiveSidebar(props: ArchiveSidebarProps) {
  const { calls, query, selected, filter, onOpen } = props;
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [showShared, setShowShared] = useState(false);
  const viewingShared = showShared && props.shared.length > 0;
  useEffect(() => { if (selected) setShowShared(false); }, [selected]);
  const searchInput = useRef<HTMLInputElement>(null);
  const mac = navigator.platform.toLowerCase().includes("mac");
  // Cmd/Ctrl+K jumps to the archive search from anywhere in the window.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey && event.key.toLowerCase() === "k") {
        event.preventDefault();
        const input = searchInput.current;
        input?.focus({ preventScroll: true });
        input?.setSelectionRange(0, input.value.length);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const activeFilters = filterCount(filter);
  const searching = !!query.trim();
  const months = props.retentionMonths ?? 3;
  const retention = months === 0 ? t("senza scadenza") : months === 1
    ? t("per un mese") : t("per {0} mesi", months);
  return (
    <aside className="sidebar" id="archive-panel" aria-label={t("Archivio delle registrazioni")}>
      <div className="brand">
        <img src={parloIcon} srcSet={`${parloIcon2x} 2x`} width={28} height={28} alt="" />
        <span>parlo</span>
      </div>

      <div className="sidebar-actions">
        <button className={`record-button ${props.recording ? "recording" : ""}`} disabled={!props.canRecord} onClick={props.onRecord}>
          {props.recording ? <Square size={14} fill="currentColor" /> : <Mic size={17} />}
          <span>{props.recording ? t("Termina · {0}", fmtTime(props.elapsedMs)) : t("Nuova registrazione")}</span>
        </button>
        <div className="shortcut">
          {props.shortcut === null
            ? <>{t("Scorciatoia non disponibile ·") + " "}<button type="button" className="link-button" onClick={() => props.onSettings("shortcuts")}>{t("cambiala")}</button></>
            : <>{t("Oppure premi") + " "}<kbd>{props.shortcut}</kbd></>}
        </div>
        <button className="import-media-button" type="button" disabled={!props.canImport} onClick={props.onImport}>
          <FileUp size={15} /> {props.importing ? t("Estrazione audio…") : t("Importa audio o video")}
        </button>
      </div>

      <div className="archive-search">
        <label className="search">
          <Search size={15} />
          <input ref={searchInput} aria-label={t("Cerca nelle trascrizioni")} aria-keyshortcuts={mac ? "Meta+K" : "Control+K"} placeholder={t("Cerca nelle trascrizioni…")} value={query}
            onChange={e => props.onQuery(e.target.value)} onKeyDown={e => { if (e.key === "Escape" && query) { e.preventDefault(); props.onQuery(""); } }} />
          {!query && <kbd className="search-shortcut" aria-hidden>{mac ? "⌘K" : "Ctrl K"}</kbd>}
        </label>
        {!viewingShared && <FilterToggle open={filtersOpen} count={activeFilters} onToggle={() => setFiltersOpen(!filtersOpen)} />}
      </div>
      {!viewingShared && filtersOpen && <FilterPanel value={filter} tags={props.tags} onChange={props.onFilter} onClose={() => setFiltersOpen(false)} />}
      {!viewingShared && activeFilters > 0 && !filtersOpen && <FilterSummary value={filter} onChange={props.onFilter} />}

      {props.shared.length ? <div className="archive-label archive-switch">
        <button type="button" aria-pressed={!viewingShared} onClick={() => { setShowShared(false); props.onCloseShared(); }}>{t("Le mie")}</button>
        <button type="button" aria-pressed={viewingShared} onClick={() => setShowShared(true)}>
          {t("Condivise")} <span className="archive-count">{props.shared.length}</span>
        </button>
      </div> : <div className="archive-label">
        <span>{t("Le mie trascrizioni")}</span>
        <span className="archive-count">{props.total}</span>
      </div>}
      {viewingShared ? <SharedCallList key={query} items={props.shared} query={query} selected={props.selectedShared} onOpen={props.onOpenShared} /> : <VirtualArchiveList
        label={t("Le mie trascrizioni")} rows={calls} estimateSize={searching ? 220 : 105}
        onScroll={e => {
          const el = e.currentTarget;
          if (el.scrollHeight - el.scrollTop - el.clientHeight < 100) props.onLoadMore();
        }}
        renderRow={(call, index) => (
          <>
            {(index === 0 || archiveDay(calls[index - 1].created_at) !== archiveDay(call.created_at)) && <div className={index === 0 ? "archive-group first" : "archive-group"}>{archiveHeading(call.created_at)}</div>}
            <article className={`call-item ${selected === call.id ? "selected" : ""}`}>
              <button type="button" className="call-select" aria-current={selected === call.id ? "page" : undefined} onClick={() => onOpen(call, call.search?.hits[0])}>
                <CallStatusIcon status={call.status} origin={call.origin} />
                <div className="archive-file-content">
                  <strong><HighlightedText text={call.title} query={query} substring /></strong>
                  <div className="call-meta">
                    <span>{t("Inizio")} <time dateTime={call.created_at}>{fmtClock(call.created_at)}</time></span>
                    <span className="call-duration">{t("Durata")} {fmtTime(call.duration_ms)}</span>
                    {call.status !== "ready" && <span className={`archive-file-status ${call.status}`}>{statusLabel(call.status)}</span>}
                  </div>
                  {!searching && call.preview && <p>{call.preview}</p>}
                </div>
              </button>
              {!!call.tags?.length && (
                <div className="call-tags-list compact call-item-tags">
                  {call.tags.map(tag => {
                    const on = filter.tags.some(t => t.toLocaleLowerCase() === tag.toLocaleLowerCase());
                    return (
                      <button type="button" className={`call-tag ${on ? "selected" : ""}`} key={tag} aria-pressed={on}
                        title={on ? t("Rimuovi il filtro “{0}”", tag) : t("Mostra solo le registrazioni con il tag “{0}”", tag)}
                        onClick={() => props.onFilter({ ...filter, tags: on ? filter.tags.filter(value => value.toLocaleLowerCase() !== tag.toLocaleLowerCase()) : [...filter.tags, tag] })}>{tag}</button>
                    );
                  })}
                </div>
              )}
              {searching && call.search && <CallSearchResults call={call} onOpen={onOpen} />}
            </article>
          </>
        )}>
        {props.loading && <div className="sidebar-empty">{t("Caricamento…")}</div>}
        {!props.loading && calls.length === 0 && (
          <div className="sidebar-empty">
            {searching || activeFilters ? t("Nessuna registrazione trovata.") : t("Ancora nessuna registrazione. Inizia dal pulsante qui sopra.")}
          </div>
        )}
        {props.hasMore && (
          <button className="load-more" disabled={props.loadingMore} onClick={props.onLoadMore}>
            {props.loadingMore ? t("Caricamento…") : t("Mostra altre")}
          </button>
        )}
      </VirtualArchiveList>}

      <div className="sidebar-bottom">
        <div className="local-storage">
          <ShieldCheck size={15} />
          <span>
            {t("Archivio salvato sul computer")}
            <small>
              <button type="button" className="link-button" disabled={!props.desktop} aria-haspopup="dialog" title={t("Cambia per quanto tempo conservare le registrazioni")}
                onClick={() => props.onSettings("archive")}>{t("Conservate") + " "}{retention}</button>
            </small>
          </span>
        </div>
        {props.preferences && (
          <div className="sidebar-preferences">
            <button type="button" onClick={() => props.onSettings("general")} disabled={!props.desktop}>
              <SettingsIcon size={16} /> {t("Impostazioni")}
            </button>
            <button type="button" className="theme-toggle" aria-label={t("Cambia tema")} title={props.preferences.theme === "light" ? t("Tema scuro") : t("Tema chiaro")}
              disabled={!props.desktop} onClick={props.preferences.onTheme}>
              {props.preferences.theme === "light" ? <Moon size={16} /> : <Sun size={16} />}
            </button>
          </div>
        )}
      </div>
    </aside>
  );
}
