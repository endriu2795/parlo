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
  const [copiedCode, setCopiedCode] = useState("");

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
    } finally {
      setBusy(false);
    }
  };

  const openChat = (botFather = false) => {
    setError("");
    void api.telegramOpenChat(botFather).catch(e => setError(String(e)));
  };

  const copyCode = async (code: string) => {
    setError("");
    try {
      await navigator.clipboard.writeText(code);
      setCopiedCode(code);
    } catch {
      setError(t("Seleziona e copia il codice mostrato qui sopra."));
    }
  };

  const title = status?.connected ? t("Bot collegato")
    : status?.pairing_code ? t("Conferma in Telegram") : t("Collega un bot");
  const message = error || statusError || status?.error;
  const code = status?.pairing_code;

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
      </> : code ? <>
        <div className="telegram-state" role="status">
          <LoaderCircle size={20} className="spin" aria-hidden="true" />
          <div>
            <strong>{t("In attesa di conferma da @{0}", status.username)}</strong>
            <p>{t("Apri il bot e premi «Avvia» in Telegram. Il collegamento si completa qui da solo.")}</p>
          </div>
        </div>
        <Actions>
          <button type="button" className="primary-button" disabled={busy} onClick={() => openChat()}>
            {t("Apri il bot in Telegram")} <ArrowUpRight size={14} aria-hidden="true" />
          </button>
          <button type="button" className="text-button" disabled={busy}
            onClick={() => void run(api.telegramDisconnect)}>{t("Annulla collegamento")}</button>
        </Actions>
        <Advanced label={t("Collega con il codice")}>
          <Hint>{t("Invia il codice qui sotto nella chat privata di @{0} entro 10 minuti.", status.username)}</Hint>
          <div className="telegram-code">
            <code>{code}</code>
            <button type="button" className="secondary-button" onClick={() => void copyCode(code)}>
              <Copy size={14} aria-hidden="true" />{t("Copia codice")}
            </button>
          </div>
          {copiedCode === code && <p className="settings-hint" role="status">{t("Codice copiato.")}</p>}
        </Advanced>
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
            if (!busy && token.trim()) void run(() => api.telegramConnect(token));
          }}>
            <Field label={t("2. Incolla il token di BotFather")} stacked>
              <input type="password" autoComplete="off" spellCheck={false}
                value={token} disabled={busy} onChange={event => setToken(event.target.value)} />
            </Field>
            <Actions>
              <button type="submit" className="primary-button" disabled={busy || !token.trim()}>
                {busy && <LoaderCircle size={14} className="spin" aria-hidden="true" />}
                {busy ? t("Verifica del token…") : t("Continua")}
              </button>
            </Actions>
          </form>
        </div>
      </> : !message && <p className="settings-hint" role="status">{t("Caricamento…")}</p>}
      {message && <Hint tone="error">{message}</Hint>}
    </Group>
  );
}
