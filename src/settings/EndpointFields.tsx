import { useEffect, useState } from "react";
import ApiKeyField from "../ApiKeyField";
import { api, desktop } from "../api";
import { t } from "../i18n";
import type { Endpoint, EndpointKind } from "../types";
import { Actions, Field, Hint, Switch } from "./controls";

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
  return <>
    {(kind === "stt" || kind === "tts") && <>
      <Hint>{t("Per un server gestito da te puoi usare Speaches. Installa il server, prepara un modello e inserisci qui l’indirizzo e l’ID del modello.")}</Hint>
      <Hint>{guide("speaches_install", t("Installare Speaches"))} · {guide("speaches_models", t("Trovare i modelli"))} · {guide(kind === "stt" ? "speaches_stt" : "speaches_tts", kind === "stt" ? t("Guida trascrizione") : t("Guida voce"))}</Hint>
      <Actions><button type="button" className="text-button" disabled={disabled || working}
        onClick={() => onChange({ ...value, url: "http://localhost:8000/v1" })}>{t("Usa l’indirizzo Speaches su questo computer")}</button></Actions>
      <Hint>{t("Se il server è su un altro computer, sostituisci localhost con il suo indirizzo. Usa il percorso /v1 del server.")}</Hint>
    </>}
    {kind === "chat" && <Hint>{t("Usa un server compatibile con l’API OpenAI per la chat. L’ID del modello è quello indicato dal server.")} {guide("lm_studio", t("Guida LM Studio"))} · {guide("chat_api", t("Documentazione API"))}</Hint>}
    {kind === "ollama" && <Hint>{guide("ollama_server", t("Configurare un server Ollama"))}</Hint>}
    {(kind !== "ollama" || value.url) && <Hint>{contract}</Hint>}
    <Field label={kind === "ollama" ? t("Indirizzo server Ollama") : t("URL base")}
      hint={kind === "ollama" ? t("Lascia vuoto per Ollama sul computer. Per un server usa l’indirizzo base senza /v1.") : t("Includi il prefisso API, per esempio http://localhost:8000/v1.")}>
      <input type="url" value={value.url} placeholder={kind === "ollama" ? "https://ollama.example.com" : "http://localhost:8000/v1"}
        onChange={event => onChange({ ...value, url: event.target.value })} />
    </Field>
    {kind !== "ollama" && <Field label={t("ID modello")} hint={t("Usa il nome esatto del modello disponibile sul server.")}>
      <input value={value.model} onChange={event => onChange({ ...value, model: event.target.value })} />
    </Field>}
    {kind === "tts" && <Field label={t("ID voce")} hint={t("Usa il nome di una voce supportata dal modello sul server.")}>
      <input value={value.voice} onChange={event => onChange({ ...value, voice: event.target.value })} />
    </Field>}
    {kind === "chat" && <Switch label={t("Usa strumenti")}
      hint={t("Richiede un server e un modello compatibili. Con il tuo consenso, PARLO può inviare al modello le fonti testuali abilitate e salvare file TXT, Markdown o CSV sul computer.")}
      checked={!!value.tools_enabled} disabled={disabled || working}
      onChange={tools_enabled => onChange({ ...value, tools_enabled })} />}
    {(kind !== "ollama" || value.url) && <>
    <ApiKeyField label={t("Chiave API facoltativa")} saved={saved} value={key} placeholder={t("Se richiesta dal server")}
      busy={disabled || working} onChange={setKey} />
    <Actions>
      <button className="secondary-button" disabled={disabled || working || !value.url || !key.trim()} onClick={() => void saveKey(key)}>{t("Salva chiave")}</button>
      <button className="text-button" disabled={disabled || working || !saved} onClick={() => void saveKey("")}>{t("Rimuovi chiave")}</button>
      {kind !== "ollama" && <button className="secondary-button" disabled={disabled || working || !value.url || !value.model || (kind === "tts" && !value.voice)}
        onClick={() => void run(async () => {
          await api.testCustomEndpoint(kind, value);
          setMessage(t("Il server ha risposto al test."));
        })}>{t("Prova endpoint")}</button>}
    </Actions>
    {kind !== "ollama" && <Hint>{t("Il test invia una breve richiesta al server usando la chiave salvata. Verifica la risposta del servizio.")}</Hint>}
    {message && <Hint>{message}</Hint>}
    </>}
  </>;
}
