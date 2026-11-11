import { t } from "../i18n";
import { useState } from "react";
import { RefreshCw, X } from "lucide-react";
import { api } from "../api";
import { toolLabel } from "../ChatActivity";
import type { Connector, Settings } from "../types";
import type { providerConfig } from "./provider";

const connectorStatus: Record<string, string> = {
  configured: t("Configurato in Antigravity"),
  "needs-auth": t("Accesso da rinnovare"),
  pending: t("Connessione in corso"),
  disabled: t("Disattivato"),
  unsupported: t("Configurazione non supportata: verifica le impostazioni MCP"),
};

/** MCP connectors of the current provider, plus the tools trusted "always". */
export default function ConnectorsPopover({ config, settings, connectors, loading, error, chatBusy, onRefresh, onToggle, onSettingsChange, onError, onClose }: {
  config: ReturnType<typeof providerConfig>;
  settings: Settings | null;
  connectors: Connector[];
  loading: boolean;
  error: string;
  chatBusy: boolean;
  onRefresh: () => void;
  onToggle: (name: string, enabled: boolean) => Promise<void>;
  onSettingsChange: (settings: Settings) => void;
  onError: (error: string) => void;
  onClose: () => void;
}) {
  const { provider, label, connectorsEnabled: enabled, disabledConnectors, trustedTools } = config;
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState("");
  const save = async (action: () => Promise<void>) => {
    setSaving(true);
    setSaveError("");
    try { await action(); }
    catch (e) { setSaveError(String(e)); }
    finally { setSaving(false); }
  };
  const shownError = saveError || error;

  return (
    <section className="connectors-popover" aria-label={t("Elenco connettori")}>
      <header>
        <strong>{t("Connettori") + " "}{label}</strong>
        <button className="widget-icon" title={t("Aggiorna connettori")} disabled={loading} onClick={onRefresh}>
          <RefreshCw size={14} className={loading ? "spin" : ""} />
        </button>
        <button className="widget-icon connectors-close" title={t("Torna alla chat")} aria-label={t("Torna alla chat")} onClick={onClose}>
          <X size={16} />
        </button>
      </header>
      <div className="connectors-body">
        <p>{provider === "ollama" ? t("Strumenti dell’app collegati alla chat.") : t("Scegli gli MCP da usare nelle chat di PARLO.")}</p>
        {loading && <p>{t("Verifica dei connettori…")}</p>}
        {shownError && <p role="alert">{shownError}</p>}
        {!enabled && (
          <p>{provider === "ollama" ? t("File, Bash e PowerShell su Windows sono strumenti della CLI. Ricerca e apertura delle pagine sono disattivate di default e si abilitano in Impostazioni → Assistente → Strumenti web con Ollama, con il dettaglio dei dati inviati online. La voce si configura separatamente. I connettori degli account cloud sono esclusi.")
            : t("I connettori sono disattivati nelle impostazioni.")}</p>
        )}
        {!loading && !connectors.length && !shownError && enabled && (
          <p>
            {provider === "gemini"
              ? t("Nessun MCP configurato in Antigravity. I connettori di Claude e Codex restano separati.")
              : provider === "codex"
              ? t("Nessun connettore trovato nella configurazione MCP di Codex.")
              : t("Claude non ha restituito connettori per questo accesso. Verifica di aver collegato e autorizzato i servizi sullo stesso account Claude, poi premi Aggiorna.")}
          </p>
        )}
        <ul>
          {connectors.map(c => {
            const tts = c.name === "parlo_tts";
            const web = c.name === "parlo_web";
            const integrated = tts || web;
            const name = tts ? t("Voce dell’app (TTS)") : web ? t("Ricerca web") : c.name.replace(/^claude\.ai /, "");
            return (
              <li key={c.name}>
                <label className="connector-choice">
                  <input type="checkbox" aria-label={integrated ? name : t("Usa {0}", name)}
                    checked={c.status !== "unsupported" && c.status !== "disabled" && (integrated || !disabledConnectors?.includes(c.name))}
                    disabled={integrated || saving || chatBusy || !enabled || (provider === "gemini" && ["unsupported", "disabled"].includes(c.status))}
                    onChange={e => void save(() => onToggle(c.name, e.target.checked))} />
                  <span className={`connector-dot ${c.status}`} />
                  <div>
                    <strong>{name}</strong>
                    <small>{c.status === "connected" ? t("{0} strumenti disponibili", c.tools) : connectorStatus[c.status] ?? t("Connessione non riuscita")}</small>
                  </div>
                </label>
              </li>
            );
          })}
        </ul>
        {provider === "claude" && !loading && !connectors.length && (
          <p className="connector-note">
            {t("Gli MCP configurati solo in un progetto di VS Code o su un altro PC non vengono trasferiti dal login. Anche credenziali API o criteri dell’organizzazione possono impedire il caricamento dei connettori dell’account.")}
          </p>
        )}
        {provider === "claude" && (
          <button className="widget-link" onClick={() => void api.connectorsBrowser().catch(e => onError(String(e)))}>
            {t("Gestisci i connettori su Claude ↗")}
          </button>
        )}
        <p className="connector-note">
          {provider === "gemini"
            ? t("La scelta viene ricordata. Le operazioni che richiedono consenso mostrano una conferma nella chat.")
            : t("La scelta viene ricordata. Gli strumenti richiedono il tuo consenso, salvo quelli che hai consentito sempre.")}
        </p>
        {provider !== "gemini" && (
          <div className="trusted-tools">
            <strong>{t("Strumenti consentiti sempre")}</strong>
            {!trustedTools?.length && <p>{t("Nessuno. Puoi autorizzarli dalla richiesta nella chat.")}</p>}
            <ul>
              {trustedTools?.map(tool => (
                <li key={tool}>
                  <span title={tool}>{toolLabel(tool)}</span>
                  <button aria-label={t("Revoca {0}", toolLabel(tool))} disabled={saving}
                    onClick={() => void save(async () => onSettingsChange(await api.revokeToolTrust(tool)))}>
                    {t("Revoca")}
                  </button>
                </li>
              ))}
            </ul>
            {!!trustedTools?.length && <p>{t("Valgono nelle chat di PARLO. I connettori disattivati restano esclusi.")}</p>}
          </div>
        )}
        {provider === "gemini" && (
          <p className="connector-note">
            {t("Le conferme richieste da Antigravity valgono una volta. I server con OAuth dedicato, variabili, cartelle di avvio o esclusioni di singoli strumenti richiedono una configurazione compatibile; quelli non gestibili sono indicati nell’elenco.")}
          </p>
        )}
      </div>
    </section>
  );
}
