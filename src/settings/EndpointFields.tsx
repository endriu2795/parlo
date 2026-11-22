import { useEffect, useState } from "react";
import ApiKeyField from "../ApiKeyField";
import { api, desktop } from "../api";
import { t } from "../i18n";
import type { Endpoint, EndpointKind } from "../types";
import { Actions, Advanced, Field, Hint, Switch } from "./controls";

const guideUrls = {
  speaches_install: "https://github.com/speaches-ai/speaches/blob/master/docs/installation.md",
  speaches_models: "https://github.com/speaches-ai/speaches/blob/master/docs/usage/model-discovery.md",
  speaches_stt: "https://github.com/speaches-ai/speaches/blob/master/docs/usage/speech-to-text.md",
  speaches_tts: "https://github.com/speaches-ai/speaches/blob/master/docs/usage/text-to-speech.md",
  chat_api: "https://platform.openai.com/docs/api-reference/chat/create",
  lm_studio: "https://lmstudio.ai/docs/developer/openai-compat",
  ollama_server: "https://docs.ollama.com/faq",
} as const;
type Guide = keyof typeof guideUrls;

export default function EndpointFields({ kind, endpoint, onChange, disabled }: {
  kind: EndpointKind | "ollama";
  endpoint?: Endpoint;
  onChange: (endpoint: Endpoint) => void;
  disabled: boolean;
}) {
  const value = endpoint ?? { url: "", model: "", voice: "" };
  const [key, setKey] = useState("");
  const [saved, setSaved] = useState<boolean | null>(null);
  const [working, setWorking] = useState(false);
  const [message, setMessage] = useState("");
  useEffect(() => {
    let active = true;
    setSaved(null);
    setKey("");
    setMessage("");
    if (value.url) api.customEndpointKey(kind, value.url).then(result => { if (active) setSaved(result); })
      .catch(() => { if (active) setSaved(null); });
    return () => { active = false; };
  }, [kind, value.url]);
  const run = async (task: () => Promise<void>) => {
    setMessage("");
    setWorking(true);
    try { await task(); }
    catch (error) { setMessage(String(error)); }
    finally { setWorking(false); }
  };
  const saveKey = (text: string) => run(async () => {
    setSaved(await api.customEndpointKey(kind, value.url, text));
    setKey("");
  });
  const guide = (id: Guide, label: string) => <a href={guideUrls[id]} target="_blank" rel="noreferrer"
    onClick={event => {
      if (!desktop) return;
      event.preventDefault();
      void run(() => api.openEndpointGuide(id));
    }}>{label}</a>;
  const contract = {
    stt: t("POST audio/transcriptions, audio WAV multipart e risposta verbose_json con timestamp."),
    tts: t("POST audio/speech, input JSON con modello e voce, risposta MP3."),
    chat: t("POST chat/completions, messaggi JSON e risposta testuale progressiva se il server la supporta."),
    ollama: t("Sul server vengono inviati trascrizione, cronologia e risultati degli strumenti usati. Gli strumenti si eseguono sul tuo computer."),
  }[kind];
  const introduction = {
    stt: t("Collega un server di trascrizione compatibile con l’API OpenAI. PARLO gli invia l’audio e riceve il testo."),
    tts: t("Collega un server di sintesi vocale compatibile con l’API OpenAI. PARLO gli invia il testo e riceve un MP3."),
    chat: t("Collega un server di chat compatibile con l’API OpenAI, ad esempio LM Studio. Riceverà la trascrizione e i messaggi della chat."),
  };
  return <div className="endpoint-fields">
    {kind !== "ollama" && <Hint>{introduction[kind]}</Hint>}
    <div className="endpoint-section">
    <Field stacked label={kind === "ollama" ? t("Indirizzo server Ollama") : t("Indirizzo API del server")}
      hint={kind === "ollama" ? t("Lascia vuoto per Ollama sul computer. Per un server usa l’indirizzo base senza /v1.") : t("Copia l’indirizzo API indicato dal server. /v1 identifica la versione dell’API: mantienilo se fa parte dell’indirizzo. PARLO aggiunge da sé il percorso della richiesta.")}>
      <input type="url" value={value.url} placeholder={kind === "ollama" ? "https://ollama.example.com" : "http://localhost:8000/v1"}
        onChange={event => onChange({ ...value, url: event.target.value })} />
    </Field>
    {(kind === "stt" || kind === "tts") && <Actions><button type="button" className="text-button" disabled={disabled || working}
      onClick={() => onChange({ ...value, url: "http://localhost:8000/v1" })}>{t("Compila con l’indirizzo di Speaches su questo computer")}</button></Actions>}
    {kind !== "ollama" && <Field stacked label={t("Modello (ID)")} hint={t("Copia l’identificativo esatto del modello dall’elenco del server. Il nome visualizzato può essere diverso.")}>
      <input value={value.model} onChange={event => onChange({ ...value, model: event.target.value })} />
    </Field>}
    {kind === "tts" && <Field stacked label={t("Voce (ID)")} hint={t("Copia l’identificativo di una voce supportata dal modello scelto. Lo trovi nella configurazione del server.")}>
      <input value={value.voice} onChange={event => onChange({ ...value, voice: event.target.value })} />
    </Field>}
    </div>
    {(kind !== "ollama" || value.url) && <div className="endpoint-section endpoint-auth">
    <ApiKeyField label={t("Chiave API (solo se richiesta)")} saved={value.url ? saved : false} value={key} placeholder={t("Lascia vuoto se il server non richiede una chiave")}
      busy={disabled || working} onChange={setKey} />
    <Actions>
      <button className="secondary-button" disabled={disabled || working || !value.url || !key.trim()} onClick={() => void saveKey(key)}>{t("Salva chiave")}</button>
      <button className="text-button" disabled={disabled || working || !saved} onClick={() => void saveKey("")}>{t("Rimuovi chiave")}</button>
    </Actions>
    </div>}
    {kind === "chat" && <Switch label={t("Consenti gli strumenti all’assistente")}
      hint={t("Richiede un server e un modello compatibili. Con il tuo consenso, PARLO può inviare al modello le fonti testuali abilitate e salvare file TXT, Markdown o CSV sul computer.")}
      checked={!!value.tools_enabled} disabled={disabled || working}
      onChange={tools_enabled => onChange({ ...value, tools_enabled })} />}
    {kind !== "ollama" && <div className="endpoint-section">
      <Actions><button className="secondary-button" disabled={disabled || working || !value.url || !value.model || (kind === "tts" && !value.voice)}
        onClick={() => void run(async () => {
          await api.testCustomEndpoint(kind, value);
          setMessage(t("Il server ha risposto al test."));
        })}>{working ? t("Operazione in corso…") : t("Verifica collegamento")}</button></Actions>
      <Hint>{t("Invia una breve richiesta con questi dati e con la chiave salvata, se presente.")}</Hint>
    </div>}
    {message && <div role="status"><Hint>{message}</Hint></div>}
    <Advanced label={t("Guide e requisiti del server")}>
      {(kind === "stt" || kind === "tts") && <>
        <Hint>{t("Se devi ancora configurare un server, puoi usare Speaches. Installalo e prepara un modello prima di collegarlo a PARLO.")}</Hint>
        <Hint>{guide("speaches_install", t("Installare Speaches"))} · {guide("speaches_models", t("Trovare i modelli"))} · {guide(kind === "stt" ? "speaches_stt" : "speaches_tts", kind === "stt" ? t("Guida trascrizione") : t("Guida voce"))}</Hint>
        <Hint>{t("localhost indica questo computer. Se Speaches è su un altro computer, sostituiscilo con il suo indirizzo e mantieni la porta e il percorso API configurati sul server.")}</Hint>
      </>}
      {kind === "chat" && <Hint>{guide("lm_studio", t("Guida LM Studio"))} · {guide("chat_api", t("Documentazione API"))}</Hint>}
      {kind === "ollama" && <Hint>{guide("ollama_server", t("Configurare un server Ollama"))}</Hint>}
      {(kind !== "ollama" || value.url) && <Hint>{contract}</Hint>}
    </Advanced>
  </div>;
}
