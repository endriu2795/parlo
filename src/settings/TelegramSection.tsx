import { useEffect, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import { Field, Group, Hint } from "./controls";

export default function TelegramSection() {
  const [status, setStatus] = useState<Awaited<ReturnType<typeof api.telegramStatus>>>();
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try {
        const next = await api.telegramStatus();
        if (active) setStatus(next);
      } catch (e) {
        if (active) setError(String(e));
      } finally {
        if (active) timer = setTimeout(refresh, 1500);
      }
    };
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, []);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError("");
    try {
      await action();
      setToken("");
      setStatus(await api.telegramStatus());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Group title={t("Ricevi da Telegram")}>
      <Hint>{t("Invia vocali, file audio o video al tuo bot. PARLO li trascrive usando le impostazioni di registrazione attuali.")}</Hint>
      <Hint>{t("Se il computer è spento o PARLO è chiuso, i messaggi in attesa vengono recuperati alla riapertura entro il limite di 24 ore di Telegram. I file passano da Telegram e possono pesare fino a 20 MB.")}</Hint>
      {status?.connected ? <>
        <p role="status">{t("Collegato a @{0}", status.username)} · {t("File in attesa: {0}", status.pending)}</p>
        <div><button type="button" className="secondary-button" disabled={busy}
          onClick={() => void run(api.telegramDisconnect)}>{t("Scollega Telegram")}</button></div>
      </> : status?.pairing_code ? <>
        <p role="status">{t("Invia questo codice in un messaggio privato a @{0} entro 10 minuti.", status.username)}</p>
        <p><code>{status.pairing_code}</code></p>
        <div><button type="button" className="secondary-button" disabled={busy}
          onClick={() => void run(api.telegramDisconnect)}>{t("Annulla")}</button></div>
      </> : <>
        <Hint>{t("Su Telegram apri @BotFather, usa /newbot e incolla qui il token del bot dedicato a PARLO.")}</Hint>
        <Field label={t("Token del bot Telegram")}>
          <input aria-label={t("Token del bot Telegram")} type="password" autoComplete="off" spellCheck={false}
            value={token} disabled={busy} onChange={event => setToken(event.target.value)} />
        </Field>
        <div><button type="button" className="primary-button" disabled={busy || !token.trim()}
          onClick={() => void run(() => api.telegramConnect(token))}>{t("Collega Telegram")}</button></div>
      </>}
      {(error || status?.error) && <p className="form-error" role="alert">{error || status?.error}</p>}
    </Group>
  );
}
