import EndpointFields from "./EndpointFields";
import { t } from "../i18n";
import { useEffect, useState } from "react";
import { CheckCircle2, Download, LoaderCircle } from "lucide-react";
import { api } from "../api";
import ApiKeyField from "../ApiKeyField";
import AudioOutputPicker from "../AudioOutputPicker";
import { Actions, Advanced, Field, Group, Hint, OptionCards, Switch, type EngineControls, type SectionProps } from "./controls";
import { availableModels, calibrationLabel, modelDescription, modelIdFor, modelTitle, previewReady, type WhisperModel } from "./models";

type Props = SectionProps & { engine: EngineControls; updateBusy: boolean; onRefresh: () => Promise<void> };

function EngineStatus({ health, engine }: Pick<Props, "health" | "engine">) {
  const { setup } = engine;
  const state = setup?.running ? "running" : health?.whisper_ready ? "ready" : setup?.error ? "error" : "pending";
  const text = {
    running: setup?.message || t("Download del modello in corso…"),
    ready: t("Pronta. Funziona anche senza Internet."),
    error: setup?.error || t("Download interrotto."),
    pending: t("In preparazione: il modello si scarica automaticamente una sola volta."),
  }[state];
  return (
    <div className={`engine-status ${state}`} role={state === "error" ? "alert" : "status"}>
      {state === "ready" ? <CheckCircle2 size={16} /> : state === "running" ? <LoaderCircle size={16} className="spin" /> : <Download size={16} />}
      <span><strong>{t("Trascrizione sul computer")}</strong>{text}</span>
    </div>
  );
}

export default function RecordingSection({ draft, update, busy, health, setError, engine, updateBusy, onRefresh }: Props) {
  const models = availableModels(health, draft);
  const modelId = (path: string) => modelIdFor(path, models, health?.engine_root);
  const finalId = modelId(draft.whisper_model);
  const liveId = modelId(draft.live_whisper_model);
  const local = !draft.stt_provider || draft.stt_provider === "local";
  const groq = draft.stt_provider === "groq";
  const engineBusy = busy || engine.working;
  const calibration = calibrationLabel(health, finalId, draft.compute_mode);

  const [sttKey, setSttKey] = useState("");
  const [sttKeySaved, setSttKeySaved] = useState<boolean | null>(null);
  const [keyBusy, setKeyBusy] = useState(false);
  const [permissionsBusy, setPermissionsBusy] = useState(false);
  useEffect(() => {
    let active = true;
    api.sttKeyStatus().then(value => { if (active) setSttKeySaved(value); }).catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, []);
  const changeKey = async (action: () => Promise<void>, saved: boolean) => {
    setKeyBusy(true);
    setError("");
    try {
      await action();
      setSttKey("");
      setSttKeySaved(saved);
      await onRefresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setKeyBusy(false);
    }
  };

  const chooseFinal = (model: WhisperModel) => {
    update({ whisper_model: model.path });
    if (!model.ready) void engine.download(model.id, false);
  };
  const preparePreview = (id: string, mode = draft.live_compute_mode || "auto") => {
    const model = models.find(model => model.id === id);
    if (model && (mode !== "cpu" || !previewReady(model, mode))) void engine.download(id, true, mode);
  };
  const sizeBadge = (model: WhisperModel, ready: boolean) => ready ? undefined : t("Da scaricare{0}", model.download_size ? ` · ${model.download_size}` : "");

  return <>
    <Group title={t("Cosa registrare")}>
      <Switch label={t("Microfono")} hint={t("La tua voce.")} checked={draft.microphone} onChange={microphone => update({ microphone })} />
      <Switch label={t("Audio del computer")} hint={t("Le voci degli altri partecipanti alla call.")} checked={draft.system_audio} onChange={system_audio => update({ system_audio })} />
      <Field label={t("Lingua della call")}>
        <select value={draft.language} onChange={event => update({ language: event.target.value })}>
          <option value="auto">{t("Rileva automaticamente")}</option>
          <option value="it">{t("Italiano")}</option>
          <option value="en">{t("Inglese")}</option>
        </select>
      </Field>
    </Group>

    <Group title={t("Dove trascrivere")}>
      <OptionCards label={t("Dove trascrivere")} value={draft.stt_provider || "local"} disabled={engineBusy}
        onChange={stt_provider => update({ stt_provider, live_transcription: stt_provider === "local" ? draft.live_transcription : false })}
        options={[
          { value: "local", title: t("Su questo computer"), description: t("Privato e gratuito, funziona offline. Usa il processore o la scheda grafica.") },
          { value: "custom", title: t("Endpoint personalizzato"), description: t("Invia l’audio al server che configuri.") },
          { value: "groq", title: t("Online con Groq"), description: t("Veloce anche su computer lenti. L’audio viene inviato a Groq con il tuo account (piano gratuito disponibile).") },
        ]} />
      {groq ? <>
        <ApiKeyField label={t("Chiave API Groq")} saved={sttKeySaved} value={sttKey} placeholder={t("Incolla la chiave da console.groq.com/keys")}
          busy={busy || keyBusy} onChange={setSttKey} />
        <Actions>
          <button className="secondary-button" disabled={busy || keyBusy || !sttKey.trim()} onClick={() => void changeKey(() => api.sttSaveKey(sttKey), true)}>{t("Salva chiave Groq")}</button>
          <button className="text-button" disabled={busy || keyBusy || sttKeySaved !== true} onClick={() => void changeKey(api.sttDeleteKey, false)}>{t("Rimuovi chiave")}</button>
        </Actions>
        <Hint>{t("Se Groq non risponde o la quota finisce, l’audio resta disponibile: scegli «Su questo computer», salva e usa «Riprova la trascrizione» dalla registrazione.")}</Hint>
        <Hint>{t("Se la connessione si interrompe, l’app riprova automaticamente fino a due volte solo la parte interessata. Le richieste interrotte potrebbero essere conteggiate da Groq.")}</Hint>
      </> : local ? <>
        <EngineStatus health={health} engine={engine} />
        {engine.setup?.error && (
          <Actions>
            <button className="secondary-button" disabled={engineBusy}
              onClick={() => void engine.download(engine.setup?.model || (finalId === "custom" ? "turbo" : finalId), engine.setup?.preview)}>{t("Riprova download")}</button>
          </Actions>
        )}
        <OptionCards label={t("Qualità della trascrizione")} value={finalId} disabled={engineBusy}
          onChange={id => { const model = models.find(model => model.id === id); if (model) chooseFinal(model); }}
          options={[
            ...models.map(model => ({ value: model.id, title: modelTitle(model.id), description: modelDescription(model.id), badge: sizeBadge(model, model.ready) })),
            ...(finalId === "custom" ? [{ value: "custom", title: t("Modello personalizzato"), description: draft.whisper_model, disabled: true }] : []),
          ]} />
        <Switch label={t("Mostra il testo mentre registri")} hint={t("Anteprima provvisoria durante la call, poi sostituita dalla trascrizione definitiva.")}
          checked={draft.live_transcription} disabled={engineBusy}
          onChange={live_transcription => { update({ live_transcription }); if (live_transcription) preparePreview(liveId); }} />
      </> : <EndpointFields kind="stt" endpoint={draft.custom_stt} onChange={custom_stt => update({ custom_stt })} disabled={engineBusy} />}
    </Group>

    <Group>
      <Advanced>
        <Switch label={t("Migliora la voce bassa del microfono")} hint={t("Aumenta il volume del microfono quando parli piano.")}
          checked={draft.microphone_voice_boost !== false} onChange={microphone_voice_boost => update({ microphone_voice_boost })} />
        {health?.windows_capture && draft.system_audio && <AudioOutputPicker value={draft.system_audio_output || ""}
          onChange={system_audio_output => update({ system_audio_output })} />}
        {local && <>
          <Field label={t("Elaborazione")} hint={t("{0}Automatica sceglie la scheda grafica o il processore, in base a cosa è più veloce sul tuo computer.", calibration ? t("Rilevato: {0}.", calibration) + " " : "")}>
            <select value={draft.compute_mode || "auto"} onChange={event => update({ compute_mode: event.target.value as "auto" | "cpu" })}>
              <option value="auto">{t("Automatica · consigliata")}</option>
              <option value="cpu">{t("Solo processore (CPU)")}</option>
            </select>
          </Field>
          {draft.live_transcription && <>
            <Field label={t("Qualità dell’anteprima")}>
              <select value={liveId} disabled={engineBusy} onChange={event => {
                const model = models.find(model => model.id === event.target.value);
                if (!model) return;
                update({ live_whisper_model: model.path });
                preparePreview(model.id);
              }}>
                {liveId === "custom" && <option value="custom">{t("Modello personalizzato")}</option>}
                {models.map(model => <option key={model.id} value={model.id}>{modelTitle(model.id)}{previewReady(model, draft.live_compute_mode) ? "" : ` · ${sizeBadge(model, false)?.toLowerCase()}`}</option>)}
              </select>
            </Field>
            <Field label={t("Elaborazione dell’anteprima")} hint={t("Se la scheda grafica non è disponibile l’anteprima si ferma, ma la registrazione continua.")}>
              <select value={draft.live_compute_mode || "auto"} disabled={engineBusy} onChange={event => {
                const mode = event.target.value;
                update({ live_compute_mode: mode });
                if (liveId !== "custom") void engine.download(liveId, true, mode);
              }}>
                <option value="auto">{t("Scheda grafica automatica · consigliata")}</option>
                <option value="cpu">{t("Solo processore · più carico sul computer")}</option>
              </select>
            </Field>
          </>}
          {!!engine.setup?.log.length && (
            <details className="settings-log">
              <summary>{t("Dettagli della preparazione")}</summary>
              <pre>{engine.setup.log.join("\n")}</pre>
            </details>
          )}
        </>}
        <Hint>{t("Durante la trascrizione finale l’app evita lo standby automatico. Se chiudi il portatile o lo sospendi a mano il lavoro può interrompersi: attendi lo stato «Pronta».")}</Hint>
        {health?.capture_platform === "macos" && <>
          <Hint>{t("Se dopo un aggiornamento la registrazione resta bloccata anche con i permessi attivi, ripristinali: PARLO si riavvia e macOS chiederà di nuovo il consenso per microfono e schermo.")}</Hint>
          <Actions>
            <button type="button" className="secondary-button" disabled={busy || updateBusy || permissionsBusy} onClick={async () => {
              setPermissionsBusy(true);
              setError("");
              try { await api.resetCapturePermissions(); }
              catch (e) { setError(String(e)); setPermissionsBusy(false); }
            }}>{permissionsBusy ? t("Ripristino e riavvio…") : t("Ripristina permessi e riavvia")}</button>
          </Actions>
        </>}
      </Advanced>
    </Group>
  </>;
}
