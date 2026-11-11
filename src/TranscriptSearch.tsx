import { t } from "./i18n";
import { useEffect, useRef, useState, type RefObject } from "react";
import { ChevronUp, ChevronDown, Search, X } from "lucide-react";

export type SearchFocus = {callId: string; query: string; source: string; timestamp_ms: number; id: number};

export default function TranscriptSearch({query, callId, scope, content, focus, onClear}: {
  query: string; callId: string; scope: RefObject<HTMLElement | null>; content: unknown; focus?: SearchFocus; onClear: () => void;
}) {
  const marks = useRef<HTMLElement[]>([]);
  const current = useRef(0);
  const previous = useRef("");
  const handledFocus = useRef<number | null>(null);
  const [position, setPosition] = useState({index: 0, count: 0});
  const activate = (index: number, scroll = true) => {
    marks.current.forEach(mark => mark.removeAttribute("data-search-current"));
    const count = marks.current.length;
    current.current = count ? (index + count) % count : 0;
    const mark = marks.current[current.current];
    mark?.setAttribute("data-search-current", "true");
    if (scroll) mark?.scrollIntoView({block: "center", behavior: "smooth"});
    setPosition({index: current.current, count});
  };
  useEffect(() => {
    marks.current = Array.from(scope.current?.querySelectorAll<HTMLElement>(".segments mark[data-search-hit]") || []);
    const key = `${callId}:${query}`;
    const changed = key !== previous.current;
    let index = changed ? 0 : Math.min(current.current, Math.max(0, marks.current.length - 1));
    const requested = focus && focus.id !== handledFocus.current;
    if (requested) {
      const found = marks.current.findIndex(mark => {
        const parent = mark.closest<HTMLElement>("[data-search-source]");
        if (!parent || parent.dataset.searchSource !== focus.source) return false;
        const start = Number(parent.dataset.searchStart), end = Number(parent.dataset.searchEnd);
        return focus.timestamp_ms >= start && focus.timestamp_ms <= end;
      });
      if (found >= 0) index = found;
      handledFocus.current = focus.id;
    }
    previous.current = key;
    activate(index, changed || !!requested);
    return () => marks.current.forEach(mark => mark.removeAttribute("data-search-current"));
  }, [query, callId, scope, content, focus]);
  return <div className="transcript-search" role="search" aria-label={t("Risultati nella trascrizione")}>
    <Search size={14}/><span className="transcript-search-query">{query}</span>
    <span role="status">{position.count ? t("{0} di {1}", position.index + 1, position.count) : t("Nessuna corrispondenza nel testo")}</span>
    <button type="button" className="icon-button" aria-label={t("Risultato precedente")} title={t("Risultato precedente")} disabled={!position.count} onClick={() => activate(current.current - 1)}><ChevronUp size={16}/></button>
    <button type="button" className="icon-button" aria-label={t("Risultato successivo")} title={t("Risultato successivo")} disabled={!position.count} onClick={() => activate(current.current + 1)}><ChevronDown size={16}/></button>
    <button type="button" className="icon-button" aria-label={t("Chiudi ricerca")} title={t("Chiudi ricerca")} onClick={onClear}><X size={15}/></button>
  </div>;
}
