import { useEffect, useState } from "react";
import { ArrowUpRight, CheckCircle2, Copy, LoaderCircle } from "lucide-react";
import { api } from "../api";
import { t } from "../i18n";
import { Actions, Advanced, Field, Group, Hint } from "./controls";

export default function TelegramSection() {
  const [status, setStatus] = useState<Awaited<ReturnType<typeof api.telegramStatus>>>();
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [statusError, setStatusError] = useState("");
  const [copiedCommand, setCopiedCommand] = useState("");

  useEffect(() => {
    if (busy) return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const refresh = async () => {
      try {
        const next = await api.telegramStatus();
        if (active) {
          setStatus(next);
          setStatusError("");
        }
      } catch (e) {
        if (active) setStatusError(String(e));
      } finally {
        if (active) timer = setTimeout(refresh, 1500);
      }
    };
    void refresh();
    return () => { active = false; clearTimeout(timer); };
  }, [busy]);

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError("");
    try {
      await action();
      setToken("");
      setStatus(await api.telegramStatus());
      setStatusError("");
    } catch (e) {
      setError(String(e));
      // Refresh the server cooldown immediately, including after failed pairing.
      try { setStatus(await api.telegramStatus()); } catch { /* Regular polling reports status errors. */ }
    } finally {
      setBusy(false);
    }
  };

  const openChat = (botFather = false) => {
    setError("");
    void api.telegramOpenChat(botFather).catch(e => setError(String(e)));
  };

  const copyCommand = async (command: string) => {
    setError("");
    try {
      await navigator.clipboard.writeText(command);
      setCopiedCommand(command);
    } catch {
      setError(t("Seleziona e copia il comando mostrato qui sopra."));
    }
  };

  const pairing = !!(status?.pairing_code || status?.pairing_expired);
  const title = status?.connected ? t("Bot collegato")
    : pairing ? t("Collega con il codice") : t("Collega un bot");
  const message = error || statusError || status?.error;
  const command = status?.pairing_code ? `/start ${status.pairing_code}` : "";
  const retryAfter = status?.retry_after ?? 0;

  return (
    <Group title={title}>
      {status?.connected ? <>
        <div className="telegram-state" role="status">
          <CheckCircle2 size={20} aria-hidden="true" />
          <div>
            <strong>{t("Collegato a @{0}", status.username)}</strong>
            <p>{t("Invia un vocale, un file audio o un video al bot. La trascrizione comparirà nell’archivio di PARLO.")}</p>
            {status.pending > 0 && <p>{t("File in attesa: {0}", status.pending)}</p>}
          </div>
        </div>
        <Actions>
          <button type="button" className="primary-button" disabled={busy} onClick={() => openChat()}>
            {t("Apri la chat del bot")} <ArrowUpRight size={14} aria-hidden="true" />
          </button>
          <button type="button" className="text-button" disabled={busy}
            onClick={() => void run(api.telegramDisconnect)}>{t("Scollega Telegram")}</button>
        </Actions>
        <Advanced label={t("Limiti di importazione")}>
          <Hint>{t("Se il computer è spento o PARLO è chiuso, i messaggi in attesa vengono recuperati alla riapertura entro il limite di 24 ore di Telegram. I file passano da Telegram e possono pesare fino a 20 MB.")}</Hint>
        </Advanced>
      </> : pairing ? <>
        {command ? <div className="telegram-state" role="status">
          <LoaderCircle size={20} className="spin" aria-hidden="true" />
          <div>
            <strong>{t("In attesa di conferma da @{0}", status?.username)}</strong>
            <p>{t("Apri la chat privata del bot in Telegram. Se vedi «Avvia», premilo, poi invia il comando qui sotto entro 10 minuti.")}</p>
          </div>
        </div> : <Hint tone="warning">{t("Il codice di collegamento è scaduto.")}</Hint>}
        <Actions>
          {command ? <div className="telegram-code">
            <code>{command}</code>
            <button type="button" className="primary-button" disabled={busy} onClick={() => void copyCommand(command)}>
              <Copy size={14} aria-hidden="true" />{t("Copia comando")}
            </button>
          </div> : <button type="button" className="primary-button" disabled={busy}
            onClick={() => void run(api.telegramRenewPairing)}>{t("Genera un nuovo codice")}</button>}
          <button type="button" className="text-button" disabled={busy}
            onClick={() => void run(api.telegramDisconnect)}>{t("Annulla collegamento")}</button>
        </Actions>
        {command && copiedCommand === command && <p className="settings-hint" role="status">{t("Comando copiato.")}</p>}
      </> : status ? <>
        <div className="telegram-steps">
          <div>
            <strong>{t("1. Crea un bot")}</strong>
            <Hint>{t("Apri @BotFather, invia /newbot e segui le istruzioni. Alla fine riceverai il token da copiare.")}</Hint>
            <Actions>
              <button type="button" className="secondary-button" disabled={busy} onClick={() => openChat(true)}>
                {t("Apri BotFather")} <ArrowUpRight size={14} aria-hidden="true" />
              </button>
            </Actions>
          </div>
          <form onSubmit={event => {
            event.preventDefault();
            if (!busy && !retryAfter && token.trim()) void run(() => api.telegramConnect(token));
          }}>
            <Field label={t("2. Incolla il token di BotFather")} stacked>
              <input type="password" autoComplete="off" spellCheck={false}
                value={token} disabled={busy} onChange={event => setToken(event.target.value)} />
            </Field>
            <Actions>
              <button type="submit" className="primary-button" disabled={busy || retryAfter > 0 || !token.trim()}>
                {busy && <LoaderCircle size={14} className="spin" aria-hidden="true" />}
                {busy ? t("Verifica del token…") : retryAfter > 0 ? t("Riprova tra {0} s", retryAfter) : t("Continua")}
              </button>
            </Actions>
          </form>
        </div>
      </> : !message && <p className="settings-hint" role="status">{t("Caricamento…")}</p>}
      {message && <Hint tone="error">{message}</Hint>}
      {retryAfter > 0 && <Hint tone="warning">{t("Collegamento in pausa. Attendi {0} secondi prima di riprovare; non occorre reinserire il token.", retryAfter)}</Hint>}
    </Group>
  );
}
