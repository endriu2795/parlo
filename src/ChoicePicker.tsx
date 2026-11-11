import { t } from "./i18n";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Check, ChevronDown, LoaderCircle, Search, Settings2 } from "lucide-react";

export type ChoiceOption = { value: string; label: string; description?: string; badge?: string; searchText?: string };

export default function ChoicePicker({ label, value, options, onChange, disabled = false, loading = false,
  allowCustom = false, onOpen, hint, error, searchPlaceholder = "Cerca per nome o lingua…" }: {
  label: string; value: string; options: ChoiceOption[]; onChange: (value: string) => void;
  disabled?: boolean; loading?: boolean; allowCustom?: boolean; onOpen?: () => void; hint?: string; error?: string;
  searchPlaceholder?: string;
}) {
  const id = useId();
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [custom, setCustom] = useState(false);
  const [customValue, setCustomValue] = useState("");
  const normalize = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLocaleLowerCase("it");
  const filtered = useMemo(() => {
    const terms = normalize(query).trim().split(/\s+/).filter(Boolean);
    return options.filter(option => terms.every(term => normalize([option.label, option.description, option.value, option.searchText].join(" ")).includes(term)));
  }, [options, query]);
  const selected = options.find(option => option.value === value);
  const close = (restore = false) => { setOpen(false); if (restore) trigger.current?.focus(); };
  const choose = (next: string) => { onChange(next); close(true); };
  const show = () => {
    setQuery(""); setCustom(false); setOpen(true);
    setActive(Math.max(0, options.findIndex(option => option.value === value)));
    onOpen?.();
  };
  useEffect(() => {
    if (!open) return;
    search.current?.focus();
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) close(); };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  useEffect(() => { setActive(index => Math.min(index, Math.max(0, filtered.length - 1))); }, [filtered]);
  useEffect(() => {
    if (open && !custom) document.getElementById(`${id}-option-${active}`)?.scrollIntoView?.({ block: "nearest" });
  }, [active, open, custom, id]);

  return <div className="field choice-field" ref={root} onBlur={event => {
    if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget as Node)) close();
  }} onKeyDown={event => {
    if (!open) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); close(true); }
    if (custom || event.target !== search.current) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      setActive(index => filtered.length ? (index + (event.key === "ArrowDown" ? 1 : -1) + filtered.length) % filtered.length : 0);
    } else if (event.key === "Home" && event.ctrlKey) { event.preventDefault(); setActive(0); }
    else if (event.key === "End" && event.ctrlKey) { event.preventDefault(); setActive(Math.max(0, filtered.length - 1)); }
    else if (event.key === "Enter") { event.preventDefault(); if (filtered[active]) choose(filtered[active].value); }
  }}>
    <span id={`${id}-label`}>{label}</span>
    <div className="choice-control">
      <button type="button" ref={trigger} className="choice-trigger" disabled={disabled}
        aria-labelledby={`${id}-label ${id}-selected`} aria-haspopup="listbox" aria-expanded={open} aria-controls={`${id}-list`}
        onClick={() => open ? close() : show()} onKeyDown={event => {
          if (!open && (event.key === "ArrowDown" || event.key === "ArrowUp")) { event.preventDefault(); show(); }
        }}>
        <span className="choice-selected"><strong id={`${id}-selected`}>{selected?.label || value || t("Scegli…")}</strong>
          {selected?.description && <small>{selected.description}</small>}</span>
        <ChevronDown size={16} aria-hidden="true" className={open ? "choice-chevron open" : "choice-chevron"} />
      </button>
      {open && <div className="choice-menu">
        {!custom ? <>
          <div className="choice-search"><Search size={15} aria-hidden="true" />
            <input ref={search} role="combobox" aria-label={t("Cerca: {0}", label)} aria-expanded="true" aria-controls={`${id}-list`}
              aria-autocomplete="list" aria-activedescendant={filtered[active] ? `${id}-option-${active}` : undefined}
              value={query} onChange={event => { setQuery(event.target.value); setActive(0); }}
              placeholder={searchPlaceholder} autoComplete="off" spellCheck={false} />
            {loading && <LoaderCircle size={15} className="choice-spinner" aria-label={t("Caricamento elenco")} />}
          </div>
          <div className="choice-list" role="listbox" aria-labelledby={`${id}-label`} id={`${id}-list`}>
            {filtered.map((option, index) => <div role="option" aria-selected={option.value === value} key={option.value}
              id={`${id}-option-${index}`} className={`choice-option${index === active ? " active" : ""}`}
              onPointerMove={() => setActive(index)} onMouseDown={event => event.preventDefault()} onClick={() => choose(option.value)}>
              <span><strong>{option.label}</strong>{option.description && <small>{option.description}</small>}</span>
              <span className="choice-option-end">{option.badge && <span className="choice-badge">{option.badge}</span>}
                {option.value === value && <Check size={16} aria-label={t("Selezionata")} />}</span>
            </div>)}
            {!filtered.length && <p className="choice-empty" role="status">{loading ? t("Caricamento delle opzioni…") : t("Nessun risultato. Prova un altro nome o una lingua.")}</p>}
          </div>
          {(hint || error) && <p className="choice-note" role="status">{error || hint}</p>}
          {allowCustom && <button type="button" className="choice-custom-toggle" onClick={() => { setCustom(true); setCustomValue(value); }}>
            <Settings2 size={14} aria-hidden="true" /> {t("Inserisci un ID personalizzato")}
          </button>}
        </> : <div className="choice-custom">
          <label><span>{t("ID personalizzato")}</span><input autoFocus value={customValue} onChange={event => setCustomValue(event.target.value)}
            autoComplete="off" spellCheck={false} onKeyDown={event => { if (event.key === "Enter" && /^[\w.-]{1,200}$/.test(customValue.trim())) { event.preventDefault(); choose(customValue.trim()); } }} /></label>
          <div><button type="button" className="text-button" onClick={() => { setCustom(false); requestAnimationFrame(() => search.current?.focus()); }}>{t("Torna all’elenco")}</button>
            <button type="button" className="secondary-button" disabled={!/^[\w.-]{1,200}$/.test(customValue.trim())} onClick={() => choose(customValue.trim())}>{t("Usa questo ID")}</button></div>
        </div>}
      </div>}
    </div>
  </div>;
}
