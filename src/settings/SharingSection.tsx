import { useState } from "react";
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
    <Group title={t("Cartella Drive o OneDrive")}>
      <Hint>{sharing.folder || t("Scegli una cartella sincronizzata da Google Drive per computer o OneDrive, visibile in Esplora file o nel Finder.")}</Hint>
      <Actions>
        <button className="secondary-button" disabled={busy || working} onClick={() => void run(async () => {
          const folder = await api.sharingPickFolder();
          if (folder && folder !== sharing.folder) update({ sharing: { ...sharing, folder, auto_folder: false, read_shared: false, expire_with_archive: false } });
        })}>{t("Scegli cartella")}</button>
        {sharing.folder && <button className="text-button" disabled={busy || working}
          onClick={() => update({ sharing: { ...sharing, folder: "", auto_folder: false, read_shared: false, expire_with_archive: false } })}>{t("Scollega cartella")}</button>}
      </Actions>
      <Hint>{t("PARLO salva qui il file .md. Google Drive o OneDrive lo carica quando la sincronizzazione è attiva.")}</Hint>
      {sharing.folder && <>
        <Switch label={t("Mostra le trascrizioni degli altri nella barra laterale")} checked={!!sharing.read_shared}
          onChange={read_shared => update({ sharing: { ...sharing, read_shared } })} />
        <Switch label={t("Pubblica automaticamente")} hint={t("Invia ogni nuova trascrizione completata a questa destinazione.")}
          checked={!!sharing.auto_folder} onChange={auto_folder => update({ sharing: { ...sharing, auto_folder } })} />
        <Switch label={t("Elimina le mie copie condivise alla scadenza della call")}
          hint={t("Segue la scadenza dell’archivio e si applica anche ai file già pubblicati da questo PARLO. Dopo la sincronizzazione spariscono anche per i colleghi.")}
          checked={!!sharing.expire_with_archive} disabled={busy}
          onChange={expire_with_archive => update({ sharing: { ...sharing, expire_with_archive } })} />
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
