import { t } from "../i18n";
import { useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import { Actions, Group, Hint, type SectionProps } from "./controls";

const MAX_TERMS = 100;

export default function DictionarySection({ draft, update }: SectionProps) {
  const [rows, setRows] = useState(() => (draft.dictionary || []).map((entry, index) => ({ id: String(index), term: entry.term })));
  const change = (next: typeof rows) => {
    setRows(next);
    update({ dictionary: next.filter(row => row.term.trim()).map(row => ({ term: row.term.trim() })) });
  };
  return (
    <Group title={t("Parole da riconoscere")}
      description={t("Nomi di persone, aziende, prodotti o sigle che usi spesso. Aiutano la trascrizione a scriverli correttamente, ma non sostituiscono il testo in automatico.")}>
      {rows.length > 0 && <ul className="dictionary-list">
        {rows.map((row, index) => (
          <li key={row.id}>
            <input aria-label={t("Termine {0}", index + 1)} maxLength={80} placeholder={t("Es. PARLO")} value={row.term}
              onChange={e => change(rows.map((r, i) => i === index ? { ...r, term: e.target.value } : r))} />
            <button type="button" className="icon-button" title={t("Rimuovi")} aria-label={t("Rimuovi {0}", row.term || "termine")}
              onClick={() => change(rows.filter((_, i) => i !== index))}><Trash2 size={15} /></button>
          </li>
        ))}
      </ul>}
      {!rows.length && <Hint>{t("Il dizionario è vuoto. Aggiungi solo i termini che ricorrono spesso.")}</Hint>}
      <Actions>
        <button type="button" className="text-button" disabled={rows.length >= MAX_TERMS}
          onClick={() => change([...rows, { id: crypto.randomUUID(), term: "" }])}><Plus size={14} /> {t("Aggiungi termine")}</button>
      </Actions>
      <Hint>{t("Vale per tutte le nuove trascrizioni su questo computer.")}</Hint>
    </Group>
  );
}
