import { t } from "../i18n";
import ShortcutField from "../ShortcutField";
import { defaultShortcut, primaryModifier } from "../shortcuts";
import { Group, type SectionProps } from "./controls";

export default function ShortcutsSection({ draft, update }: SectionProps) {
  return <>
    <Group title={t("Registrazione")}>
      <ShortcutField label={t("Avvia registrazione / mostra indicatore")} value={draft.shortcut} onChange={shortcut => update({ shortcut })} />
    </Group>
    <Group title={t("Durante la registrazione")} description={t("Funzionano solo mentre stai registrando. Clicca una combinazione per cambiarla.")}>
      <ShortcutField label={t("Nota nella trascrizione")} value={draft.note_shortcut || defaultShortcut("N")} defaultValue={defaultShortcut("N")}
        description={t("Aggiunge una nota con l’orario. {0}+Invio salva; nell’app funziona anche {1}+N.", primaryModifier, primaryModifier)}
        onChange={note_shortcut => update({ note_shortcut })} />
      <ShortcutField label="Screenshot" value={draft.screenshot_shortcut || defaultShortcut("S")} defaultValue={defaultShortcut("S")}
        description={t("Seleziona un’area o lo schermo intero: lo screenshot finisce nella trascrizione.")}
        onChange={screenshot_shortcut => update({ screenshot_shortcut })} />
      <ShortcutField label={t("Disegna sullo schermo")} value={draft.drawing_shortcut || defaultShortcut("D")} defaultValue={defaultShortcut("D")}
        description={t("Apre il pennello. Esc torna alle altre app senza cancellare i disegni.")}
        onChange={drawing_shortcut => update({ drawing_shortcut })} />
      <ShortcutField label={t("Scrivi sullo schermo")} value={draft.text_shortcut || defaultShortcut("T")} defaultValue={defaultShortcut("T")}
        description={t("Apre l’editor con lo strumento testo già selezionato.")}
        onChange={text_shortcut => update({ text_shortcut })} />
    </Group>
  </>;
}
