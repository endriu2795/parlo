import EndpointFields from "./EndpointFields";
import { t, uiLocale } from "../i18n";
import { useEffect, useRef, useState } from "react";
import { RefreshCw } from "lucide-react";
import { api } from "../api";
import ApiKeyField from "../ApiKeyField";
import ChoicePicker, { type ChoiceOption } from "../ChoicePicker";
import type { KokoroStatus, PiperStatus, QwenTtsStatus, Settings, TtsCatalog, TtsVoice } from "../types";
import { Actions, Advanced, Field, Group, Hint, OptionCards, Progress, Switch, type SectionProps } from "./controls";

type Provider = NonNullable<Settings["tts_provider"]>;
type CatalogProvider = "edge" | "elevenlabs";

const suggestedEdge: TtsVoice[] = [
  { id: "it-IT-IsabellaNeural", name: "Isabella", locale: "it-IT", gender: "Female" },
  { id: "it-IT-DiegoNeural", name: "Diego", locale: "it-IT", gender: "Male" },
  { id: "it-IT-ElsaNeural", name: "Elsa", locale: "it-IT", gender: "Female" },
  { id: "it-IT-GiuseppeMultilingualNeural", name: "Giuseppe", locale: "it-IT", gender: "Male" },
];
const piperChoices: TtsVoice[] = [
  { id: "it_IT-serena-medium", name: "Serena · Standard", locale: "it-IT", gender: "Female", download_bytes: 63521305 },
  { id: "it_IT-serena-high", name: t("Serena · Alta qualità"), locale: "it-IT", gender: "Female", download_bytes: 114209276 },
];
const suggestedElevenModels = [
  { id: "eleven_multilingual_v2", name: "Multilingual v2" },
  { id: "eleven_v3", name: "Eleven v3" },
  { id: "eleven_flash_v2_5", name: "Flash v2.5" },
];
const qwenLanguages = [["Italian", t("Italiano")], ["English", t("Inglese")], ["French", t("Francese")], ["German", t("Tedesco")], ["Spanish", t("Spagnolo")],
  ["Portuguese", t("Portoghese")], ["Chinese", t("Cinese")], ["Japanese", t("Giapponese")], ["Korean", t("Coreano")], ["Russian", t("Russo")]];
const DEFAULT_PIPER = "it_IT-serena-medium";

function voiceChoice(voice: TtsVoice, local = false): ChoiceOption {
  let language = voice.locale || voice.labels?.language || "";
  try { if (language) language = new Intl.DisplayNames([uiLocale], { type: "language" }).of(language) || language; } catch { /* Keep the provider's language label. */ }
  const gender = voice.gender === "Female" ? t("Femminile") : voice.gender === "Male" ? t("Maschile") : voice.labels?.gender;
  return {
    value: voice.id, label: voice.name.replace(/Multilingual$/, ""),
    description: [language, gender, local && voice.download_bytes ? `${Math.round(voice.download_bytes / 1e6)} MB` : ""].filter(Boolean).join(" · "),
    badge: local ? voice.ready ? t("Scaricata") : t("Da scaricare") : voice.id.includes("Multilingual") ? t("Multilingua") : undefined,
    searchText: [voice.id, voice.locale, voice.labels?.accent].filter(Boolean).join(" "),
  };
}

/** Polls a setup status while the section is visible. */
function usePolling<T>(load: () => Promise<T>, enabled: boolean, interval: number, onError: (error: string) => void) {
  const [value, setValue] = useState<T | null>(null);
  useEffect(() => {
    if (!enabled) return;
    let active = true;
    const refresh = async () => {
      try { const next = await load(); if (active) setValue(next); }
      catch (e) { if (active) onError(String(e)); }
    };
    void refresh();
    const timer = setInterval(() => void refresh(), interval);
    return () => { active = false; clearInterval(timer); };
  }, [enabled]);
  return [value, setValue] as const;
}

function useCatalogs(keySaved: boolean | null) {
  const [catalogs, setCatalogs] = useState<Partial<Record<CatalogProvider, TtsCatalog>>>({});
  const [loading, setLoading] = useState<CatalogProvider | null>(null);
  const [errors, setErrors] = useState<Partial<Record<CatalogProvider, string>>>({});
  const requests = useRef(new Set<CatalogProvider>());
  const epoch = useRef(0);
  const load = async (provider: CatalogProvider, refresh = false) => {
    if (requests.current.has(provider) || (!refresh && catalogs[provider]) || (provider === "elevenlabs" && keySaved !== true)) return;
    requests.current.add(provider);
    setLoading(provider);
    const current = epoch.current;
    setErrors(previous => ({ ...previous, [provider]: "" }));
    try {
      const catalog = await api.ttsCatalog(provider);
      if (current === epoch.current) setCatalogs(previous => ({
        ...previous,
        [provider]: { voices: Array.isArray(catalog.voices) ? catalog.voices : [], models: Array.isArray(catalog.models) ? catalog.models : [] },
      }));
    } catch {
      if (current === epoch.current) setErrors(previous => ({ ...previous, [provider]: t("Elenco completo non disponibile. Riprova o inserisci un ID personalizzato.") }));
    } finally {
      requests.current.delete(provider);
      setLoading(previous => previous === provider ? null : previous);
    }
  };
  /** Drops the ElevenLabs catalog after the key changes, ignoring in-flight results. */
  const resetElevenLabs = () => {
    epoch.current++;
    setCatalogs(previous => ({ ...previous, elevenlabs: undefined }));
    setErrors(previous => ({ ...previous, elevenlabs: "" }));
  };
  return { catalogs, loading, errors, load, resetElevenLabs };
}

export default function VoiceSection({ draft, update, busy, setError }: SectionProps) {
  const provider: Provider = draft.tts_provider || "edge";
  const enabled = draft.tts_enabled === true;
  const [ttsKey, setTtsKey] = useState("");
  const [keySaved, setKeySaved] = useState<boolean | null>(null);
  const [keyBusy, setKeyBusy] = useState(false);
  const [requesting, setRequesting] = useState(false);
  const catalog = useCatalogs(keySaved);
  const [piper, setPiper] = usePolling<PiperStatus>(api.piperStatus, enabled && provider === "piper", 1500, setError);
  const [kokoro, setKokoro] = usePolling<KokoroStatus>(api.kokoroStatus, enabled && provider === "kokoro", 1500, setError);
  const [qwen, setQwen] = usePolling<QwenTtsStatus>(api.qwenTtsStatus, enabled && provider === "qwen", 2000, setError);
  useEffect(() => {
    let active = true;
    api.ttsKeyStatus().then(value => { if (active) setKeySaved(value); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, []);

  const request = async (action: () => Promise<void>) => {
    setRequesting(true);
    setError("");
    try { await action(); }
    catch (e) { setError(String(e)); }
    finally { setRequesting(false); }
  };
  const changeKey = async (action: () => Promise<void>, saved: boolean) => {
    setKeyBusy(true);
    setError("");
    try {
      await action();
      catalog.resetElevenLabs();
      setTtsKey("");
      setKeySaved(saved);
    } catch (e) {
      setError(String(e));
    } finally {
      setKeyBusy(false);
    }
  };

  const piperVoice = draft.tts_piper_voice || DEFAULT_PIPER;
  const qwenDevice = draft.tts_qwen_device || "cuda";
  const qwenAvailable = qwen && (qwenDevice === "cpu" ? qwen.cpu_available : qwen.cuda_available);
  const qwenPrepared = qwen && (qwenDevice === "cpu" ? qwen.cpu_ready : qwen.cuda_ready);

  return <>
    <Group title={t("Voce nella chat")} description={t("Chiedi all’assistente di leggere un testo o un copione: ricevi un file audio MP3.")}>
      <Switch label={t("Abilita la sintesi vocale")} checked={enabled} onChange={tts_enabled => update({ tts_enabled })} />
      {enabled && <OptionCards<Provider> label={t("Servizio voce")} value={provider} onChange={tts_provider => update({ tts_provider })} options={[
        { value: "edge", title: "Edge", description: t("Gratuito e online, senza chiave. Il testo viene inviato a Microsoft.") },
        { value: "piper", title: "Piper", description: t("Sul computer e offline. Voce italiana leggera da scaricare.") },
        { value: "kokoro", title: "Kokoro", description: t("Sul computer e offline. Voce diversa da Piper, generalmente più lenta e più pesante.") },
        { value: "qwen", title: "Qwen3-TTS", description: t("Sul computer, voce più espressiva ma lenta. Richiede un computer potente.") },
        { value: "custom", title: t("Endpoint personalizzato"), description: t("Invia il testo al server che configuri e ricevi un MP3.") },
        { value: "elevenlabs", title: "ElevenLabs", description: t("Online, alta qualità. Serve una chiave API e consuma i tuoi crediti.") },
      ]} />}
    </Group>

    {enabled && provider === "custom" && <Group title={t("Endpoint personalizzato")}>
      <EndpointFields kind="tts" endpoint={draft.custom_tts} onChange={custom_tts => update({ custom_tts })} disabled={busy} />
    </Group>}

    {enabled && provider === "edge" && <Group title={t("Voce Edge")}>
      <ChoicePicker label={t("Voce Edge")} value={draft.tts_edge_voice ?? "it-IT-IsabellaNeural"} disabled={busy}
        options={(catalog.catalogs.edge?.voices.length ? catalog.catalogs.edge.voices : suggestedEdge).map(voice => voiceChoice(voice))}
        onChange={tts_edge_voice => update({ tts_edge_voice })} allowCustom loading={catalog.loading === "edge"}
        onOpen={() => void catalog.load("edge")} error={catalog.errors.edge} hint={t("Cerca per nome, lingua o paese.")} />
      <Advanced>
        <Actions><button type="button" className="text-button" disabled={catalog.loading === "edge"} onClick={() => void catalog.load("edge", true)}><RefreshCw size={13} /> {t("Aggiorna elenco voci")}</button></Actions>
        <Hint>{t("Richiede Internet e non esegue modelli sul computer. Disponibilità e limiti del servizio non sono garantiti.")}</Hint>
      </Advanced>
    </Group>}

    {enabled && provider === "piper" && <Group title={t("Voce Piper")}>
      <ChoicePicker label={t("Voce e qualità")} value={piperVoice} disabled={busy}
        options={(piper?.voices || piperChoices).map(voice => voiceChoice(voice, true))}
        onChange={tts_piper_voice => update({ tts_piper_voice })}
        hint={t("Standard è leggera; Alta qualità occupa più spazio.")} />
      {!piper?.voices.some(voice => voice.id === piperVoice && voice.ready) && (
        <Actions>
          <button type="button" className="secondary-button" disabled={busy || requesting || piper?.download.running || !piper}
            onClick={() => void request(async () => { await api.piperDownload(piperVoice); setPiper(await api.piperStatus()); })}>
            {piper?.download.running ? t("Download in corso…") : t("Scarica la voce")}
          </button>
        </Actions>
      )}
      {piper?.download.running && <Progress label={t("Download voce Piper")} value={piper.download.downloaded} max={piper.download.total}
        text={t("{0} di {1} MB", Math.round(piper.download.downloaded / 1e6), Math.round(piper.download.total / 1e6))} />}
      {piper?.download.error && <Hint tone="error">{piper.download.error}</Hint>}
      <Advanced>
        <Hint>{t("Si scarica una sola volta, poi funziona offline senza account. Usa due thread del processore e resta caricata tra una scena e l’altra.")}</Hint>
        <Hint>{t("Fornisce l’MP3 con la durata reale; tempi delle singole parole e sottotitoli non sono disponibili. Motore Piper: GPL-3.0; voce Serena: CC-BY-4.0.")}</Hint>
      </Advanced>
    </Group>}

    {enabled && provider === "kokoro" && <Group title={t("Voce Kokoro")} description={t("Voci italiane Sara e Nicola. Può risultare più naturale di Piper, ma richiede più tempo e memoria: ascolta entrambe per scegliere.")}>
      <ChoicePicker label={t("Voce italiana")} value={draft.tts_kokoro_voice || "if_sara"} disabled={busy}
        options={(kokoro?.voices || [{ id: "if_sara", name: "Sara", locale: "it-IT", gender: "Female" }, { id: "im_nicola", name: "Nicola", locale: "it-IT", gender: "Male" }]).map(voice => voiceChoice(voice))}
        onChange={tts_kokoro_voice => update({ tts_kokoro_voice })} />
      {!kokoro?.ready && <Actions><button type="button" className="secondary-button" disabled={busy || requesting || kokoro?.download.running || !kokoro}
        onClick={() => void request(async () => { await api.kokoroDownload(); setKokoro(await api.kokoroStatus()); })}>
        {kokoro?.download.running ? t("Download in corso…") : t("Scarica Kokoro · circa 354 MB")}</button></Actions>}
      {kokoro?.download.running && <Progress label={t("Download modello Kokoro")} value={kokoro.download.downloaded} max={kokoro.download.total}
        text={t("{0} di {1} MB", Math.round(kokoro.download.downloaded / 1e6), Math.round(kokoro.download.total / 1e6))} />}
      {kokoro?.download.error && <Hint tone="error">{kokoro.download.error}</Hint>}
      {kokoro?.ready && <Hint>{t("Kokoro è pronto. Salva le impostazioni per usarlo nella chat.")}</Hint>}
      <Advanced><Hint>{t("Dopo il download funziona offline, senza account. Usa il processore; non serve una scheda grafica. Genera MP3 senza tempi delle singole parole.")}</Hint></Advanced>
    </Group>}

    {enabled && provider === "qwen" && <Group title={t("Voce Qwen3-TTS")} description={t("Voci più naturali ed espressive, ma più lente di Piper. Prova un breve campione prima di un copione lungo.")}>
      <Field label={t("Elaborazione")} hint={t("La scheda grafica NVIDIA è molto più veloce.")}>
        <select value={qwenDevice} disabled={busy || qwen?.setup.running} onChange={event => update({ tts_qwen_device: event.target.value as "cpu" | "cuda" })}>
          <option value="cuda">{t("Scheda grafica NVIDIA · consigliata (8 GB)")}</option>
          <option value="cpu">{t("Solo processore · molto lenta (32 GB RAM)")}</option>
        </select>
      </Field>
      {qwen && !qwen.supported && <Hint tone="warning">{t("Qwen3-TTS è disponibile per ora solo su Windows a 64 bit. Gli altri servizi voce restano disponibili.")}</Hint>}
      {qwen?.supported && !qwenAvailable && <Hint tone="warning">{t("Questo computer non sembra soddisfare i requisiti per l’elaborazione scelta. Verifica i requisiti qui sotto o scegli il processore.")}</Hint>}
      <ChoicePicker label={t("Voce Qwen3-TTS")} value={draft.tts_qwen_voice || "ryan"} disabled={busy}
        options={(qwen?.voices || [{ id: "ryan", name: "Ryan", gender: "Male" }]).map(voice => voiceChoice(voice))}
        onChange={tts_qwen_voice => update({ tts_qwen_voice })}
        hint={t("Voci multilingue: Ryan e Aiden sono in origine inglesi.")} />
      <Actions>
        <button type="button" className="secondary-button" disabled={busy || requesting || qwen?.setup.running || !qwenAvailable}
          onClick={() => void request(async () => { await api.qwenTtsPrepare(qwenDevice, !!qwenPrepared); setQwen(await api.qwenTtsStatus()); })}>
          {qwen?.setup.running ? t("Preparazione in corso…") : qwenPrepared ? t("Ripara e verifica il modello") : t("Scarica e prepara (circa 4,5 GB)")}
        </button>
      </Actions>
      {qwen?.setup.running && <div role="status">
        <Hint>{qwen.setup.stage}</Hint>
        {qwen.setup.downloaded > 0 && <Progress label={t("Download modello Qwen3-TTS")} value={qwen.setup.downloaded} max={qwen.setup.total}
          text={t("{0} di {1} GB", (qwen.setup.downloaded / 1e9).toFixed(2), (qwen.setup.total / 1e9).toFixed(2))} />}
      </div>}
      {qwen?.setup.error && <Hint tone="error">{qwen.setup.error}</Hint>}
      {qwen?.model_ready && qwenPrepared && <Hint>{t("Qwen3-TTS è pronto. Salva le impostazioni per usarlo dalla chat.")}</Hint>}
      <Advanced>
        <Field label={t("Lingua della voce")}>
          <select value={draft.tts_qwen_language || "Italian"} onChange={event => update({ tts_qwen_language: event.target.value })}>
            {qwenLanguages.map(([value, label]) => <option key={value} value={value}>{label}</option>)}
          </select>
        </Field>
        {qwen?.supported && <Hint>
          {t("Rilevato su questo computer · RAM:") + " "}{qwen.hardware.ram_bytes ? `${Math.round(qwen.hardware.ram_bytes / 1024 ** 3)} GB` : t("non disponibile")} ·
          GPU: {qwen.hardware.gpu || t("NVIDIA non rilevata")}{qwen.hardware.vram_bytes ? ` · ${(qwen.hardware.vram_bytes / 1024 ** 3).toFixed(1)} GB VRAM` : ""}
        </Hint>}
        <Hint><strong>{t("Requisiti:")}</strong> {t("Windows 64 bit, almeno 16 GB di RAM e 15 GB liberi. Consigliata una GPU NVIDIA CUDA con almeno 8 GB di VRAM e driver compatibile con CUDA 12.8. Ollama e altri programmi possono occupare la stessa VRAM. Su processore la generazione può richiedere molti minuti (consigliati 32 GB di RAM). Queste soglie non garantiscono una velocità precisa.")}</Hint>
        <Hint>{t("Il download (circa 4,5 GB più i componenti PyTorch) parte solo con il pulsante, in un ambiente separato. Poi funziona offline, senza chiavi. Non usa il modello della chat né Ollama.")}</Hint>
        <Hint>{t("Fornisce l’MP3 con la durata reale, senza tempi delle parole o sottotitoli. Se manca memoria la richiesta si ferma, senza passare al processore o al cloud. Modello e codice Qwen: Apache-2.0.")}</Hint>
      </Advanced>
    </Group>}

    {enabled && provider === "elevenlabs" && <Group title={t("Voce ElevenLabs")} description={t("Ogni generazione usa i crediti del tuo account.")}>
      <ApiKeyField label={t("Chiave API ElevenLabs")} saved={keySaved} value={ttsKey} placeholder={t("Incolla la chiave API")} busy={busy || keyBusy} onChange={setTtsKey} />
      <Actions>
        <button className="secondary-button" disabled={keyBusy || !ttsKey.trim()} onClick={() => void changeKey(() => api.ttsSaveKey(ttsKey), true)}>{t("Salva chiave")}</button>
        <button className="text-button" disabled={keyBusy || keySaved !== true} onClick={() => void changeKey(api.ttsDeleteKey, false)}>{t("Rimuovi chiave")}</button>
      </Actions>
      <ChoicePicker label={t("Voce ElevenLabs")} value={draft.tts_eleven_voice || ""} disabled={busy}
        options={(catalog.catalogs.elevenlabs?.voices || []).map(voice => voiceChoice(voice))}
        onChange={tts_eleven_voice => update({ tts_eleven_voice })} allowCustom loading={catalog.loading === "elevenlabs"}
        onOpen={() => void catalog.load("elevenlabs")} error={catalog.errors.elevenlabs}
        hint={keySaved ? t("Voci disponibili nel tuo account.") : t("Salva la chiave per vedere le tue voci, oppure inserisci un ID.")} />
      <Advanced>
        <ChoicePicker label={t("Modello ElevenLabs")} value={draft.tts_eleven_model ?? "eleven_multilingual_v2"} disabled={busy} searchPlaceholder={t("Cerca un modello…")}
          options={(catalog.catalogs.elevenlabs?.models.length ? catalog.catalogs.elevenlabs.models : suggestedElevenModels).map(model => ({ value: model.id, label: model.name, searchText: model.id }))}
          onChange={tts_eleven_model => update({ tts_eleven_model })} allowCustom loading={catalog.loading === "elevenlabs"}
          onOpen={() => void catalog.load("elevenlabs")} error={catalog.errors.elevenlabs} hint={t("Il modello cambia solo se lo selezioni tu.")} />
        {keySaved && <Actions><button type="button" className="text-button" disabled={catalog.loading === "elevenlabs"} onClick={() => void catalog.load("elevenlabs", true)}><RefreshCw size={13} /> {t("Aggiorna voci e modelli")}</button></Actions>}
        <Hint>{t("L’audio invariato viene riutilizzato; puoi chiedere una nuova interpretazione. Non cambia modello né taglia il testo in automatico.")}</Hint>
      </Advanced>
    </Group>}
  </>;
}
