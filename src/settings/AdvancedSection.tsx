import { t } from "../i18n";
import { Actions, Field, Group, Hint, type SectionProps } from "./controls";

export default function AdvancedSection({ draft, update, busy, health }: SectionProps) {
  return <>
    <Hint tone="warning">{t("Queste opzioni servono solo in casi particolari. Se non sai a cosa servono, lasciale come sono.")}</Hint>
    <Group title={t("Istruzioni di base dell’assistente")}>
      <Field stacked label={t("Prompt di sistema dell’assistente")}
        hint={t("Modificare questo testo sostituisce le istruzioni di base dalla prossima domanda. Per aggiungere preferenze su lingua, tono o formato, usa Istruzioni di progetto nella sezione Assistente.")}>
        <textarea rows={10} maxLength={16000} value={draft.system_prompt || ""} onChange={event => update({ system_prompt: event.target.value })} />
      </Field>
      <Actions>
        <button className="secondary-button" disabled={!health?.default_system_prompt || busy}
          onClick={() => update({ system_prompt: health?.default_system_prompt || "" })}>{t("Ripristina prompt predefinito")}</button>
      </Actions>
    </Group>
    <Group title={t("Configurazione del motore")} description={t("Modelli e componenti sono gestiti in automatico. Se la trascrizione smette di funzionare, ripristina i percorsi predefiniti.")}>
      <Actions>
        <button className="secondary-button" disabled={!health?.engine_defaults || busy}
          onClick={() => update({ ...health?.engine_defaults })}>{t("Ripristina configurazione automatica")}</button>
      </Actions>
    </Group>
  </>;
}
