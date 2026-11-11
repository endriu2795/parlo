import { t } from "./i18n";
import { useState } from "react";
import { Check, ListFilter, Search, Tag, X } from "lucide-react";
import { archiveDateBounds } from "./archiveDates";
import { fmtIsoDay } from "./format";
import type { TagCount } from "./types";

export type ArchiveFilter = { from: string; to: string; tags: string[] };
export const noFilter: ArchiveFilter = { from: "", to: "", tags: [] };
export const filterCount = (filter: ArchiveFilter) => filter.tags.length + (filter.from || filter.to ? 1 : 0);

const sameTag = (a: string, b: string) => a.toLocaleLowerCase() === b.toLocaleLowerCase();

export function FilterToggle({ open, count, onToggle }: { open: boolean; count: number; onToggle: () => void }) {
  return (
    <button type="button" className={`filter-toggle ${count ? "active" : ""}`} aria-label={t("Filtra per tag e date")} title={t("Filtra per tag e date")}
      aria-expanded={open} aria-controls="archive-filters" onClick={onToggle}>
      <ListFilter size={16} />{count > 0 && <i>{count}</i>}
    </button>
  );
}

const isoDay = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
const daysAgo = (days: number) => {
  const date = new Date();
  date.setDate(date.getDate() - days);
  return isoDay(date);
};
const presets = [
  { label: t("Sempre"), range: () => ({ from: "", to: "" }) },
  { label: t("Oggi"), range: () => ({ from: daysAgo(0), to: "" }) },
  { label: t("Ultimi 7 giorni"), range: () => ({ from: daysAgo(6), to: "" }) },
  { label: t("Ultimi 30 giorni"), range: () => ({ from: daysAgo(29), to: "" }) },
];
const presetFor = (from: string, to: string) => presets.find(p => { const r = p.range(); return r.from === from && r.to === to; });

/** Everything applies immediately; a custom range applies once it is valid. */
export function FilterPanel({ value, tags, onChange, onClose }: {
  value: ArchiveFilter;
  tags: TagCount[];
  onChange: (filter: ArchiveFilter) => void;
  onClose: () => void;
}) {
  const [dates, setDates] = useState({ from: value.from, to: value.to });
  const [custom, setCustom] = useState(!presetFor(value.from, value.to));
  const [tagQuery, setTagQuery] = useState("");
  const validDates = !!archiveDateBounds(dates.from, dates.to);
  const changeDates = (next: typeof dates) => {
    setDates(next);
    if (archiveDateBounds(next.from, next.to)) onChange({ ...value, ...next });
  };
  const toggleTag = (tag: string) => onChange({
    ...value,
    tags: value.tags.some(t => sameTag(t, tag)) ? value.tags.filter(t => !sameTag(t, tag)) : [...value.tags, tag],
  });
  const needle = tagQuery.trim().toLocaleLowerCase();
  const visibleTags = needle ? tags.filter(t => t.tag.toLocaleLowerCase().includes(needle)) : tags;
  const activePreset = custom ? undefined : presetFor(value.from, value.to);

  return (
    <section className="archive-filters" id="archive-filters" aria-label={t("Filtri dell’archivio")}
      onKeyDown={e => { if (e.key === "Escape") { e.stopPropagation(); onClose(); } }}>
      <header>
        <strong>{t("Filtri")}</strong>
        {filterCount(value) > 0 && (
          <button type="button" className="link-button" onClick={() => { setDates({ from: "", to: "" }); setCustom(false); onChange(noFilter); }}>{t("Azzera")}</button>
        )}
        <button type="button" className="filter-close" aria-label={t("Chiudi filtri")} onClick={onClose}><X size={14} /></button>
      </header>

      <div className="filter-section" role="group" aria-label={t("Periodo")}>
        <h3>{t("Periodo")}</h3>
        <div className="filter-options">
          {presets.map(preset => (
            <button type="button" key={preset.label} className={`filter-option ${activePreset === preset ? "selected" : ""}`} aria-pressed={activePreset === preset}
              onClick={() => { setCustom(false); changeDates(preset.range()); }}>{preset.label}</button>
          ))}
          <button type="button" className={`filter-option ${custom ? "selected" : ""}`} aria-pressed={custom} onClick={() => setCustom(true)}>{t("Scegli date…")}</button>
        </div>
        {custom && (
          <div className="filter-dates">
            <label>{t("Dal")}<input type="date" className={dates.from ? "" : "empty"} max={dates.to || undefined} value={dates.from} onChange={e => changeDates({ ...dates, from: e.target.value })} /></label>
            <label>{t("Al")}<input type="date" className={dates.to ? "" : "empty"} min={dates.from || undefined} value={dates.to} onChange={e => changeDates({ ...dates, to: e.target.value })} /></label>
          </div>
        )}
        {!validDates && <p role="alert" className="filter-warning">{t("La data iniziale deve precedere quella finale.")}</p>}
      </div>

      <div className="filter-section" role="group" aria-label="Tag">
        <h3>Tag</h3>
        {tags.length === 0
          ? <p className="filter-empty">{t("Nessun tag ancora. Aggiungili dal documento di una registrazione.")}</p>
          : <>
            {tags.length > 8 && (
              <label className="filter-tag-search">
                <Search size={13} />
                <input aria-label={t("Cerca un tag")} placeholder={t("Cerca un tag…")} value={tagQuery} onChange={e => setTagQuery(e.target.value)} />
              </label>
            )}
            <div className="filter-options">
              {visibleTags.map(({ tag, count }) => {
                const on = value.tags.some(t => sameTag(t, tag));
                return (
                  <button type="button" key={tag} className={`filter-option ${on ? "selected" : ""}`} aria-pressed={on} onClick={() => toggleTag(tag)}>
                    {on && <Check size={12} />}{tag}<small>{count}</small>
                  </button>
                );
              })}
              {visibleTags.length === 0 && <p className="filter-empty">{t("Nessun tag corrisponde.")}</p>}
            </div>
            {value.tags.length > 1 && <p className="filter-note">{t("Mostra le registrazioni con almeno uno dei tag scelti.")}</p>}
          </>}
      </div>
    </section>
  );
}

/** Active filters as removable chips under the search box. */
export function FilterSummary({ value, onChange }: { value: ArchiveFilter; onChange: (filter: ArchiveFilter) => void }) {
  const hasDates = !!(value.from || value.to);
  return (
    <div className="filter-summary" aria-label={t("Filtri attivi")}>
      {value.tags.map(tag => (
        <span className="filter-chip" key={tag}>
          <Tag size={11} />{tag}
          <button type="button" aria-label={t("Rimuovi filtro tag {0}", tag)} onClick={() => onChange({ ...value, tags: value.tags.filter(t => t !== tag) })}><X size={11} /></button>
        </span>
      ))}
      {hasDates && (
        <span className="filter-chip">
          {presetFor(value.from, value.to)?.label
            ?? (value.from && value.to ? t("Dal {0} al {1}", fmtIsoDay(value.from), fmtIsoDay(value.to))
              : value.from ? t("Dal {0} in poi", fmtIsoDay(value.from)) : t("Fino al {0}", fmtIsoDay(value.to)))}
          <button type="button" aria-label={t("Rimuovi filtro date")} onClick={() => onChange({ ...value, from: "", to: "" })}><X size={11} /></button>
        </span>
      )}
    </div>
  );
}
