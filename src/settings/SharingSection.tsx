import { useState } from "react";
import { Folder } from "lucide-react";
import { api } from "../api";
import { t } from "../i18n";
import type { Settings } from "../types";
import { Actions, Field, Group, Hint, Switch, type SectionProps } from "./controls";

export default function SharingSection({ draft, update, busy, setError }: SectionProps) {
  const sharing: NonNullable<Settings["sharing"]> = draft.sharing ?? { name: "", folder: "", slack: false, teams: false };
  const [slackUrl, setSlackUrl] = useState("");
  const [teamsUrl, setTeamsUrl] = useState("");
  const [message, setMessage] = useState("");
  const [working, setWorking] = useState(false);
  const run = async (task: () => Promise<void>) => {
    setError("");
    setMessage("");
    setWorking(true);
    try { await task(); }
    catch (error) { setError(String(error)); }
    finally { setWorking(false); }
  };
  return <>
    <Group title={t("Chi pubblica")} description={t("Condividi solo la trascrizione testuale. Audio, video, allegati e chat restano fuori.")}>
      <Field label={t("Nome da mostrare")}>
        <input maxLength={80} value={sharing.name} placeholder={t("Facoltativo")} onChange={event => update({ sharing: { ...sharing, name: event.target.value } })} />
      </Field>
    </Group>
    <Group title={t("Cartella condivisa")} description={t("Usa una cartella di Google Drive o OneDrive a cui hanno accesso anche i colleghi.")}>
      <div className="sharing-folder">
        <Folder size={18} aria-hidden="true" />
        <div>
          <strong>{sharing.folder ? t("Cartella selezionata") : t("Nessuna cartella selezionata")}</strong>
          <p className="sharing-folder-path">{sharing.folder || t("Scegli la cartella sincronizzata sul computer.")}</p>
        </div>
      </div>
      <Actions>
        <button className="secondary-button" disabled={busy || working} onClick={() => void run(async () => {
          const folder = await api.sharingPickFolder();
          if (folder && folder !== sharing.folder) update({ sharing: { ...sharing, folder, auto_folder: false, read_shared: false, expire_with_archive: false } });
        })}>{sharing.folder ? t("Cambia cartella") : t("Scegli cartella")}</button>
        {sharing.folder && <button className="text-button" disabled={busy || working}
          onClick={() => update({ sharing: { ...sharing, folder: "", auto_folder: false, read_shared: false, expire_with_archive: false } })}>{t("Scollega cartella")}</button>}
      </Actions>
      <Hint>{t("PARLO salva una copia del testo in formato Markdown (.md). Drive o OneDrive la rende disponibile agli altri quando la sincronizzazione è attiva.")}</Hint>
      {sharing.folder && <>
        <Switch label={t("Mostra le trascrizioni dei colleghi")} hint={t("Le trascrizioni trovate in questa cartella compaiono nella barra laterale di PARLO.")} checked={!!sharing.read_shared}
          onChange={read_shared => update({ sharing: { ...sharing, read_shared } })} />
        <Switch label={t("Condividi automaticamente le nuove trascrizioni")} hint={t("Salva una copia in questa cartella appena una trascrizione è completata. Se disattivato, puoi pubblicarla dal menu della call.")}
          checked={!!sharing.auto_folder} onChange={auto_folder => update({ sharing: { ...sharing, auto_folder } })} />
        <div className="sharing-retention">
          <Switch label={t("Elimina le copie quando scadono nell’archivio")}
            hint={t("Usa la durata impostata in Archivio. Vale anche per le copie già pubblicate da questo PARLO: dopo la sincronizzazione, vengono rimosse anche per i colleghi.")}
            checked={!!sharing.expire_with_archive} disabled={busy}
            onChange={expire_with_archive => update({ sharing: { ...sharing, expire_with_archive } })} />
        </div>
      </>}
    </Group>
    <Group title={t("Canali")}>
      {([
        ["slack", "auto_slack", "Slack", slackUrl, setSlackUrl],
        ["teams", "auto_teams", "Teams", teamsUrl, setTeamsUrl],
      ] as const).map(([kind, automatic, label, url, setUrl]) => <div className="sharing-channel" key={kind}>
        <Switch label={<>{label} <span className="sharing-channel-state" aria-hidden="true">
          {sharing[kind] ? t("Attivo") : t("Disattivato")}
        </span></>} checked={sharing[kind]}
          onChange={enabled => update({ sharing: { ...sharing, [kind]: enabled, [automatic]: enabled && !!sharing[automatic] } })} />
        {sharing[kind] && <>
          <Switch label={t("Pubblica automaticamente")} hint={t("Invia ogni nuova trascrizione completata a questa destinazione.")}
            checked={!!sharing[automatic]}
            onChange={value => update({ sharing: { ...sharing, [automatic]: value } })} />
          <Field label={t("URL webhook {0}", label)} hint={kind === "teams" ? t("Usa il webhook di un flusso Workflows configurato per il canale.") : undefined}>
            <input type="password" autoComplete="off" value={url} onChange={event => setUrl(event.target.value)} placeholder="https://" />
          </Field>
          <Actions>
            <button className="secondary-button" disabled={busy || working || !url.trim()} onClick={() => void run(async () => {
              await api.sharingWebhook(kind, url);
              setUrl("");
              setMessage(t("Webhook {0} salvato nel portachiavi.", label));
            })}>{t("Salva webhook")}</button>
          </Actions>
        </>}
      </div>)}
      {message && <Hint>{message}</Hint>}
    </Group>
  </>;
}
