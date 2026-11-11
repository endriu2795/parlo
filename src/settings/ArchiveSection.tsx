import { t } from "../i18n";
import { Actions, Field, Group, Hint, OptionCards, type SectionProps } from "./controls";

type Kept = "none" | "audio" | "video";

export default function ArchiveSection({ draft, update, busy, health, onCleanup }: SectionProps & { onCleanup: () => Promise<void> }) {
  const kept: Kept = draft.keep_video ? "video" : draft.keep_audio ? "audio" : "none";
  return <>
    <Group title={t("Cosa conservare")} description={t("Vale per le nuove registrazioni.")}>
      <OptionCards<Kept> label={t("File da conservare")} value={kept}
        onChange={value => update({ keep_audio: value === "audio", keep_video: value === "video" })}
        options={[
          { value: "none", title: t("Solo il testo"), description: t("L’audio viene eliminato quando la trascrizione è pronta. Occupa meno spazio.") },
          { value: "audio", title: t("Testo e audio"), description: t("Puoi riascoltare la call e cliccare gli orari per saltare al punto giusto.") },
          {
            value: "video", title: t("Testo e video"), disabled: !health?.windows_capture,
            description: health?.windows_capture ? t("Registra anche lo schermo (o un’area) con le annotazioni. Richiede più spazio e risorse.") : t("Disponibile su Windows."),
          },
        ]} />
    </Group>
    <Group title={t("Per quanto tempo")} description={t("Trascrizioni, note, screenshot e file restano sul computer. Quando usi l’assistente, i contenuti necessari vengono inviati al servizio AI scelto.")}>
      <Field label={t("Elimina automaticamente dopo")} hint={draft.retention_months === 0 ? t("Le registrazioni restano finché non le elimini tu.") : t("Include trascrizione, chat, screenshot e audio o video conservati.")}>
        <select value={draft.retention_months} onChange={event => update({ retention_months: Number(event.target.value) })}>
          {[1, 3, 6, 12, 24].map(months => <option value={months} key={months}>{months} {months === 1 ? t("mese") : t("mesi")}</option>)}
          <option value={0}>{t("Mai")}</option>
        </select>
      </Field>
      {draft.retention_months !== 0 && <Actions>
        <button type="button" className="text-button settings-cleanup" disabled={busy} onClick={() => void onCleanup()}>
          {t("Elimina ora le registrazioni scadute")}
        </button>
      </Actions>}
      {kept !== "none" && draft.retention_months !== 0 && <Hint>{t("Audio e video conservati seguono la stessa scadenza della registrazione.")}</Hint>}
    </Group>
  </>;
}
