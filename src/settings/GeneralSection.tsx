import { t } from "../i18n";
import { UpdateControls, type AppUpdates } from "../AppUpdates";
import { Advanced, Field, Group, OptionCards, Switch, type SectionProps } from "./controls";

export default function GeneralSection({ draft, update, updates, updateBusy }: SectionProps & { updates: AppUpdates; updateBusy: boolean }) {
  const customTitlebar = draft.titlebar_color?.startsWith("#");
  return <>
    <Group title={t("Aggiornamenti")}>
      <UpdateControls updates={updates} busy={updateBusy} />
    </Group>
    <Group title={t("Aspetto")}>
      <OptionCards label={t("Tema dell’app")} compact value={draft.theme || "dark"} onChange={theme => update({ theme })}
        options={[{ value: "light", title: t("Chiaro") }, { value: "dark", title: t("Scuro") }]} />
      <Advanced label={t("Barra del titolo")}>
        <Field label={t("Colore della barra")} hint={t("Su Windows la barra fa parte dell’app e di solito segue il tema.")}>
          <select aria-label={t("Barra del titolo")} value={customTitlebar ? "custom" : "theme"}
            onChange={event => update({ titlebar_color: event.target.value === "custom" ? "#111518" : event.target.value })}>
            <option value="theme">{t("Segue il tema")}</option>
            <option value="custom">{t("Colore personalizzato")}</option>
          </select>
        </Field>
        {customTitlebar && (
          <Field label={t("Colore personalizzato")}>
            <input aria-label={t("Colore della barra")} type="color" value={draft.titlebar_color} onChange={event => update({ titlebar_color: event.target.value })} />
          </Field>
        )}
      </Advanced>
    </Group>
    <Group title={t("Notifiche")}>
      <Switch label={t("Avvisami quando una trascrizione o una risposta è pronta")}
        hint={t("Utile quando l’app è in secondo piano.")}
        checked={draft.completion_notifications !== false} onChange={completion_notifications => update({ completion_notifications })} />
    </Group>
  </>;
}
