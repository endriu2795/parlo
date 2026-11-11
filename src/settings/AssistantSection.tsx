import EndpointFields from "./EndpointFields";
import { t, uiLocale } from "../i18n";
import { useEffect, useRef, useState } from "react";
import { ExternalLink, RefreshCw } from "lucide-react";
import { api } from "../api";
import type { AssistantProvider, ModelCatalog } from "../types";
import { Actions, Advanced, Field, Group, Hint, OptionCards, Switch, type OptionCard, type SectionProps } from "./controls";

const providers: OptionCard<AssistantProvider>[] = [
  { value: "claude", title: "Claude", description: t("Usa il tuo account Claude tramite Claude Code.") },
  { value: "codex", title: "Codex", description: t("Usa il tuo account OpenAI tramite Codex.") },
  { value: "custom", title: t("Endpoint personalizzato"), description: t("Invia la trascrizione e i messaggi al server che configuri.") },
  { value: "ollama", title: "Ollama", description: t("Usa un modello sul computer o su un server Ollama.") },
];

function OllamaModels({ draft, update, busy }: Pick<SectionProps, "draft" | "update" | "busy">) {
  const [catalog, setCatalog] = useState<ModelCatalog | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [maximumContext, setMaximumContext] = useState<number | null>(null);
  const [contextError, setContextError] = useState("");
  const [customContext, setCustomContext] = useState(false);
  const epoch = useRef(0);
  const serverUrl = draft.ollama_server_url?.trim() || "";
  const initialUrl = useRef(serverUrl);
  const load = async () => {
    const current = ++epoch.current;
    setLoading(true);
    setError("");
    try {
      const next = await api.ollamaModels(serverUrl);
      if (current === epoch.current) setCatalog(next);
    } catch (e) {
      if (current === epoch.current) { setCatalog(null); setError(String(e)); }
    } finally {
      if (current === epoch.current) setLoading(false);
    }
  };
  useEffect(() => {
    setCatalog(null);
    setError("");
    setLoading(false);
    if (!serverUrl || serverUrl === initialUrl.current) void load();
    return () => { ++epoch.current; };
  }, [serverUrl]);
  useEffect(() => {
    let active = true;
    setMaximumContext(null); setContextError("");
    if (!serverUrl && draft.ollama_model) void api.ollamaModelContext(draft.ollama_model).then(maximum => {
      if (active) setMaximumContext(maximum);
    }).catch(e => {if (active) setContextError(String(e));});
    return () => {active = false;};
  }, [draft.ollama_model, serverUrl]);
  const context = draft.ollama_context_length || 32768;
  const choices: number[] = [];
  for (let value = 8192; value <= (maximumContext || 131072); value *= 2) choices.push(value);
  if (maximumContext && maximumContext >= 8192 && !choices.includes(maximumContext)) choices.push(maximumContext);
  if (!choices.includes(context)) choices.push(context);
  choices.sort((a, b) => a - b);
  const models = catalog?.models || [];
  return <>
    <EndpointFields kind="ollama" endpoint={{ url: draft.ollama_server_url || "", model: "", voice: "" }}
      onChange={({ url }) => update({ ollama_server_url: url })} disabled={busy} />
    {!serverUrl && <Hint>
      {t("Per iniziare") + " "}<a href="https://ollama.com/download/" onClick={event => {
        event.preventDefault();
        void api.ollamaDownload().catch(e => setError(String(e)));
      }}>{t("scarica Ollama") + " "}<ExternalLink size={11} /></a>{t(", avvialo e installa un modello. Poi aggiorna l’elenco qui sotto.")}
    </Hint>}
    <Field label={t("Modello Ollama")}>
      <select aria-label={t("Modello Ollama")} value={draft.ollama_model || ""} disabled={busy || loading || !models.length}
        onChange={event => update({ ollama_model: event.target.value })}>
        <option value="" disabled>{loading ? t("Caricamento…") : !catalog ? t("Aggiorna elenco modelli") : models.length ? t("Scegli un modello") : t("Nessun modello installato")}</option>
        {draft.ollama_model && !models.some(model => model.value === draft.ollama_model) && <option value={draft.ollama_model}>{draft.ollama_model}{catalog ? " " + t("· non disponibile") : ""}</option>}
        {models.map(model => <option key={model.value} value={model.value}>{model.displayName}</option>)}
      </select>
    </Field>
    <Actions>
      <button type="button" className="text-button" disabled={loading || busy} onClick={() => void load()}><RefreshCw size={13} /> {t("Aggiorna elenco modelli")}</button>
    </Actions>
    {serverUrl ? <Hint>{t("La memoria della conversazione è configurata sul server Ollama.")}</Hint> : <>
    <Field label={t("Memoria della conversazione Ollama")}>
      <select aria-label={t("Memoria della conversazione Ollama")} value={customContext ? "custom" : context} disabled={busy}
        onChange={event => {const custom=event.target.value === "custom"; setCustomContext(custom); if (!custom) update({ ollama_context_length: Number(event.target.value) });}}>
        {choices.map(value => <option key={value} value={value} disabled={!!maximumContext && value > maximumContext}>{value.toLocaleString(uiLocale)} {t("token")}{value === 32768 ? " " + t("· predefinito") : ""}{value === maximumContext ? " " + t("· massimo del modello") : maximumContext && value > maximumContext ? " " + t("· non supportato") : ""}</option>)}
        <option value="custom">{t("Valore personalizzato…")}</option>
      </select>
    </Field>
    {customContext && <Field label={t("Contesto personalizzato (token)")}><input type="number" min={8192} max={maximumContext || 4294967295} step={1024} value={draft.ollama_context_length ?? 32768} disabled={busy} onChange={event => update({ollama_context_length:Number(event.target.value)})} /></Field>}
    {maximumContext && <Hint>{t("Il modello dichiara un massimo di") + " "}{maximumContext.toLocaleString(uiLocale)} {t("token. Questo limite non indica quanta RAM è disponibile. Il contesto comprende trascrizione, cronologia, istruzioni, strumenti e spazio per la risposta.")}</Hint>}
    {contextError && <Hint tone="error">{t("Contesto massimo non rilevato:") + " "}{contextError}</Hint>}
    <Hint>{t("Un contesto più ampio permette di leggere più testo, ma usa più memoria e può rallentare. PARLO prepara una configurazione locale del modello scelto senza riscaricare i pesi né cambiare le impostazioni globali di Ollama. I modelli molto piccoli possono comunque sbagliare risposte e strumenti.")}</Hint>
    </>}
    {error && <Hint tone="error">{error}</Hint>}
    {catalog && !models.length && <Hint>{serverUrl ? t("Nessun modello installato sul server Ollama.") : t("Nessun modello locale per la chat. Scaricalo da Ollama e aggiorna l’elenco (i modelli cloud sono esclusi).")}</Hint>}
    <Switch label={t("Usa strumenti con Ollama")} checked={draft.ollama_tools_enabled !== false} disabled={busy}
      onChange={ollama_tools_enabled => update({ollama_tools_enabled})}
      hint={t("Se disattivi gli strumenti, l’assistente usa trascrizione e note in chat. Lettura di allegati, file generati, ricerca web e voce richiedono gli strumenti.")} />
    {!serverUrl && draft.ollama_tools_enabled !== false && <Switch
      label={t("Set completo di strumenti")}
      hint={t("Aggiunge shell e strumenti avanzati. Il set essenziale include ricerca, lettura e modifica di file e richiede meno tempo prima della risposta.")}
      checked={draft.ollama_full_tools === true} disabled={busy}
      onChange={ollama_full_tools => update({ ollama_full_tools })} />}
    <Switch label={t("Strumenti web con Ollama")} checked={draft.ollama_tools_enabled !== false && draft.ollama_web_search === true} disabled={busy || draft.ollama_tools_enabled === false}
      onChange={ollama_web_search => update({ollama_web_search})}
      hint={t("Disattivati per impostazione predefinita. Attivandoli, le ricerche vengono inviate a DuckDuckGo e le pagine richieste vengono contattate. Prima di aprire una pagina, WebFetch invia il solo dominio ad Anthropic (api.anthropic.com) per un controllo di sicurezza.")} />
    {draft.ollama_tools_enabled !== false && (serverUrl || draft.ollama_full_tools) && <Hint>{t("Gli strumenti completi della CLI occupano parte del contesto: per call e ricerche prova almeno 32k token, se il modello e la memoria lo consentono.")}</Hint>}
  </>;
}

export default function AssistantSection({ draft, update, busy, onManageAccount }: SectionProps & { onManageAccount: () => void }) {
  const provider = draft.assistant_provider || "claude";
  const ollama = provider === "ollama";
  const codex = provider === "codex";
  const options = provider === "gemini"
    ? [...providers, { value: "gemini" as const, title: "Antigravity", description: t("Accesso Google dedicato.") }]
    : providers;
  return <>
    <Group title={t("Servizio AI")} description={t("L’assistente lavora sulle tue trascrizioni. Il modello si sceglie direttamente nella chat.")}>
      <OptionCards label={t("Fornitore AI nelle impostazioni")} value={provider} onChange={assistant_provider => update({ assistant_provider })} options={options} />
      {provider === "custom" ? <EndpointFields kind="chat" endpoint={draft.custom_chat} onChange={custom_chat => update({ custom_chat })} disabled={busy} /> : ollama ? <OllamaModels draft={draft} update={update} busy={busy} /> : (
        <Actions>
          <button type="button" className="secondary-button" disabled={busy} onClick={onManageAccount}>
            {t("Salva e gestisci account") + " "}{codex ? "Codex" : provider === "gemini" ? "Antigravity" : "Claude"}
          </button>
        </Actions>
      )}
    </Group>

    <Group title={t("Istruzioni")}>
      <Field stacked label={t("Istruzioni di progetto")} hint={t("Aggiungi contesto e preferenze su lingua, tono o formato alle istruzioni di base. Valgono per tutte le registrazioni, anche nelle chat già avviate.")}>
        <textarea aria-label={t("Istruzioni di progetto")} rows={6} maxLength={8000} value={draft.project_instructions || ""}
          placeholder={t("Es. Rispondi in modo sintetico, con elenchi puntati e i prossimi passi alla fine.")}
          onChange={event => update({ project_instructions: event.target.value })} />
      </Field>
    </Group>

    {provider !== "custom" && <Group>
      <Advanced label={t("Strumenti e collegamenti")}>
        {ollama ? <>
          <Hint>{draft.ollama_tools_enabled !== false ? t("Gli strumenti per i file chiedono conferma in chat. Ricerca e apertura delle pagine dipendono dall’interruttore Strumenti web con Ollama. La voce usa il servizio scelto nella sezione Voce.") : t("Gli strumenti sono disattivati per Ollama.")}</Hint>
          <Hint>{t("La chat usa Claude Code senza account Claude. Se l’istanza Ollama scelta non risponde, la richiesta si ferma.")}</Hint>
        </> : <>
          <Switch label={t("Usa i connettori di {0}", codex ? "Codex" : "Claude Code")}
            hint={t("Collegamenti ad altre app (es. Drive, CRM). Scegli quali usare dal pulsante Connettori nella chat.")}
            checked={codex ? draft.codex_connectors !== false : draft.claude_connectors !== false}
            onChange={enabled => update(codex ? { codex_connectors: enabled } : { claude_connectors: enabled })} />
          <Hint>{t("Gli strumenti di base (web, shell, lettura e modifica file) restano disponibili anche senza connettori. Le operazioni sensibili chiedono sempre conferma nella chat.")}</Hint>
        </>}
      </Advanced>
    </Group>}
  </>;
}
