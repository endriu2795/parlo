import { t } from "./i18n";
import { useEffect, useState } from "react";
import { Check, LogIn, RefreshCw, X } from "lucide-react";
import { api } from "./api";
import { useAssistantAuth } from "./useAssistantAuth";
import { useDialog } from "./useDialog";

type Provider = "claude" | "codex" | "gemini";
const labels: Record<Provider, { name: string; app: string }> = {
  claude: { name: "Claude", app: "Claude Code" },
  codex: { name: "Codex", app: "Codex" },
  gemini: { name: "Antigravity", app: "Antigravity" },
};

export default function AccountDialog({ provider = "claude", onClose }: { provider?: Provider; onClose: () => void }) {
  const { name: label, app } = labels[provider];
  const { auth, authError, checkAuth, loginAuth } = useAssistantAuth(provider);
  const [error, setError] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const dialog = useDialog<HTMLElement>(onClose, ".dialog-close");
  const check = async () => {
    try {
      await checkAuth();
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => { void check(); }, [provider]);
  const attempt = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const message = error || authError || auth?.error;
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <section className="dialog account-dialog" role="dialog" aria-modal="true" aria-labelledby="account-title"
        ref={dialog.ref} onKeyDown={dialog.onKeyDown} onClick={event => event.stopPropagation()}>
        <header>
          <h2 id="account-title">Account {label}</h2>
          <button className="icon-button dialog-close" aria-label={t("Chiudi account")} onClick={onClose}><X size={18} /></button>
        </header>
        <div className="dialog-body account-body">
          <div className={`account-mark ${auth?.logged_in ? "connected" : ""}`}>{auth?.logged_in ? <Check size={22} /> : <LogIn size={22} />}</div>
          <h3>{auth?.logged_in ? t("Account collegato") : provider === "gemini" ? t("Accedi con Google") : t("Accedi al tuo account {0}", label)}</h3>
          <p>
            {auth?.installed === false
              ? t("PARLO installa {0} nella propria cartella, poi apre l’accesso nel browser. Serve Internet, nessun terminale.", app)
              : auth?.logged_in ? t("Il tuo accesso {0} è attivo: puoi usare la chat.", label) : t("Premi il pulsante e completa l’accesso nel browser, poi torna qui.")}
          </p>
          {provider === "gemini" && <p className="dialog-note">{t("PARLO usa Antigravity con un accesso Google dedicato; il vecchio accesso Gemini CLI non viene riutilizzato. Se Google rifiuta un accesso salvato, usa «Rinnova accesso».")}</p>}

          {!auth?.pending && (
            <button className="primary-button" disabled={busy || !auth} onClick={() => void attempt(() => loginAuth(!!auth?.logged_in))}>
              {busy ? t("Preparazione accesso…") : auth?.logged_in ? t("Rinnova accesso") : auth?.installed === false ? t("Installa e accedi con {0}", label) : t("Accedi con {0}", label)}
            </button>
          )}
          {auth?.installing && <p role="status" className="account-progress"><RefreshCw className="spin" size={14} /> {auth.install_message || t("Installazione di {0}…", label)}</p>}
          {auth?.pending && !auth.installing && <>
            <p className="account-progress">
              <RefreshCw className="spin" size={14} />
              {auth.url ? t("Accesso in corso… Completa la pagina nel browser. Se non si è aperta, usa il pulsante qui sotto.") : t("Preparazione accesso… In attesa del servizio.")}
            </p>
            {auth.url && <button className="secondary-button" onClick={() => void api.loginBrowser(provider).catch(e => setError(String(e)))}>{t("Apri la pagina di accesso")}</button>}
            {provider === "claude" && <>
              <label className="field stacked">
                <span>{t("Se il browser mostra un codice, incollalo qui")}</span>
                <input aria-label={t("Codice di accesso")} autoComplete="off" value={code} onChange={e => setCode(e.target.value)} />
              </label>
              <button className="primary-button" disabled={!code.trim() || busy}
                onClick={() => void attempt(async () => { await api.loginCode(code.trim()); setCode(""); await check(); })}>
                {t("Completa accesso")}
              </button>
            </>}
            <button className="text-button" onClick={() => void attempt(async () => { await api.cancelLogin(provider); await check(); })}>{t("Annulla accesso")}</button>
          </>}
          {message && <p role="alert" className="form-error">{message}</p>}
        </div>
        <footer>
          <button className="text-button" disabled={auth?.installing} onClick={() => void check()}><RefreshCw size={13} /> {t("Verifica accesso")}</button>
          <button className="secondary-button" onClick={onClose}>{t("Chiudi")}</button>
        </footer>
      </section>
    </div>
  );
}
