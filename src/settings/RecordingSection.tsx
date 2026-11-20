import EndpointFields from "./EndpointFields";
import { t } from "../i18n";
import { useCallback, useEffect, useState } from "react";
import { CheckCircle2, Download, LoaderCircle } from "lucide-react";
import { api } from "../api";
import ApiKeyField from "../ApiKeyField";
import AudioOutputPicker from "../AudioOutputPicker";
import { Actions, Advanced, Field, Group, Hint, Switch, type EngineControls, type SectionProps } from "./controls";
import { availableModels, calibrationLabel, modelDescription, modelIdFor, modelTitle, previewReady } from "./models";

type Props = SectionProps & { engine: EngineControls; updateBusy: boolean; onRefresh: () => Promise<void> };

function DefaultMicrophonePicker({ value, disabled, onChange }: { value: string; disabled: boolean; onChange: (id: string) => void }) {
  const [inputs, setInputs] = useState<Awaited<ReturnType<typeof api.audioInputs>> | null>(null);
  const [error, setError] = useState("");
  const refresh = useCallback(() => {
    void api.audioInputs().then(devices => {
      setInputs(devices);
      setError("");
    }).catch(e => setError(String(e)));
  }, []);
  useEffect(refresh, [refresh]);

  const selectionMissing = value && !inputs?.some(input => input.id === value);
  return <>
    <Field label={t("Microfono predefinito")} hint={t("Usato nelle nuove registrazioni.")}>
      <select value={value} disabled={disabled} onFocus={refresh} onChange={event => onChange(event.target.value)}>
        <option value="">{t("Predefinito di Windows")}</option>
        {inputs?.map(input => <option value={input.id} key={input.id}>{input.name}</option>)}
        {selectionMissing && <option value={value}>
          {inputs && !error ? t("Microfono selezionato non disponibile") : t("Microfono selezionato")}
        </option>}
      </select>
    </Field>
    {error && <Hint tone="error">{error}</Hint>}
  </>;
}

function EngineStatus({ setup, ready, waitingForPreview }: {
  setup: EngineControls["setup"];
  ready: boolean;
  waitingForPreview: boolean;
}) {
  const state = setup?.running ? "running" : ready && !waitingForPreview ? "ready" : setup?.error ? "error" : "pending";
  const message = {
    running: setup?.message || t("Download del modello in corso…"),
    ready: t("Modello pronto. Puoi trascrivere senza Internet."),
    error: setup?.error || t("Download interrotto."),
    pending: waitingForPreview && ready
      ? t("Salva per preparare il modello del testo in diretta.")
      : t("Salva per preparare il modello sul computer."),
  }[state];
  return (
    <div className={`engine-status ${state}`} role={state === "error" ? "alert" : "status"}>
      {state === "ready" ? <CheckCircle2 size={16} aria-hidden="true" />
        : state === "running" ? <LoaderCircle size={16} className="spin" aria-hidden="true" />
        : <Download size={16} aria-hidden="true" />}
      <span>{message}</span>
    </div>
  );
}

export default function RecordingSection({ draft, update, busy, health, setError, engine, updateBusy, onRefresh }: Props) {
  const models = availableModels(health, draft);
  const finalId = modelIdFor(draft.whisper_model, models, health?.engine_root);
  const liveId = modelIdFor(draft.live_whisper_model, models, health?.engine_root);
  const finalModel = models.find(model => model.id === finalId);
  const liveModel = models.find(model => model.id === liveId);
  const provider = draft.stt_provider || "local";
  const local = provider === "local";
  const groq = provider === "groq";
  const engineBusy = busy || engine.working;
  const calibration = calibrationLabel(health, finalId, draft.compute_mode);
  const setup = engine.setup;
  const selectedSetup = setup?.preview
    ? (draft.live_transcription && setup.model === liveId ? setup : null)
    : (setup?.model === finalId ? setup : null);
  const finalReady = finalModel ? !!health?.python_available && finalModel.ready : !!health?.whisper_ready;
  const waitingForPreview = draft.live_transcription && (liveModel
    ? !previewReady(liveModel, draft.live_compute_mode)
    : health?.preview_ready === false);
  const retryModel = selectedSetup?.error && (!finalReady || waitingForPreview) ? selectedSetup.model : null;
  const providerHint = {
    local: t("PARLO trascrive sul computer, anche offline."),
    groq: t("L’audio viene inviato a Groq per la trascrizione."),
    custom: t("L’audio viene inviato al server che configuri."),
  }[provider];

  const [sttKey, setSttKey] = useState("");
  const [sttKeySaved, setSttKeySaved] = useState<boolean | null>(null);
  const [keyBusy, setKeyBusy] = useState(false);
  const [permissionsBusy, setPermissionsBusy] = useState(false);
  useEffect(() => {
    if (!groq) return;
    let active = true;
    api.sttKeyStatus()
      .then(value => { if (active) setSttKeySaved(value); })
      .catch(e => { if (active) setError(String(e)); });
    return () => { active = false; };
  }, [groq]);
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

  const modelLabel = (id: string) => `Whisper ${id.charAt(0).toUpperCase()}${id.slice(1)} · ${modelTitle(id)}`;

  return <>
    <Group title={t("Audio da registrare")}>
      <Switch label={t("Microfono")} hint={t("La tua voce.")} checked={draft.microphone} onChange={microphone => update({ microphone })} />
      {health?.windows_capture && draft.microphone && <DefaultMicrophonePicker value={draft.microphone_input || ""} disabled={busy}
        onChange={microphone_input => update({ microphone_input })} />}
      <Switch label={t("Audio del computer")} hint={t("L’audio che senti dal computer, incluse le voci della call.")} checked={draft.system_audio} onChange={system_audio => update({ system_audio })} />
      <Advanced label={t("Opzioni audio e permessi")}>
        <Switch label={t("Migliora la voce bassa del microfono")} hint={t("Aumenta il volume del microfono quando parli piano.")}
          checked={draft.microphone_voice_boost !== false} onChange={microphone_voice_boost => update({ microphone_voice_boost })} />
        {health?.windows_capture && draft.system_audio && <AudioOutputPicker value={draft.system_audio_output || ""}
          onChange={system_audio_output => update({ system_audio_output })} />}
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

    <Group title={t("Trascrizione")}>
      <Field label={t("Dove elaborare l’audio")} hint={providerHint}>
        <select value={provider} disabled={busy} onChange={event => {
          const next = event.target.value as NonNullable<typeof draft.stt_provider>;
          update({ stt_provider: next, live_transcription: next === "local" ? draft.live_transcription : false });
        }}>
          <option value="local">{t("Su questo computer")}</option>
          <option value="groq">{t("Online con Groq")}</option>
          <option value="custom">{t("Endpoint personalizzato")}</option>
        </select>
      </Field>
      <Field label={t("Lingua della call")}>
        <select value={draft.language} onChange={event => update({ language: event.target.value })}>
          <option value="auto">{t("Rileva automaticamente")}</option>
          <option value="it">{t("Italiano")}</option>
          <option value="en">{t("Inglese")}</option>
        </select>
      </Field>
    </Group>

    {local && <Group title={t("Trascrizione sul computer")}>
      <Field label={t("Modello Whisper")} hint={modelDescription(finalId) || undefined}>
        <select value={finalId} disabled={engineBusy || !health} onChange={event => {
          const model = models.find(model => model.id === event.target.value);
          if (model) update({ whisper_model: model.path });
        }}>
          {finalId === "custom" && <option value="custom">{t("Modello personalizzato")}</option>}
          {models.map(model => <option key={model.id} value={model.id}>{modelLabel(model.id)}</option>)}
        </select>
      </Field>
      {health && <EngineStatus setup={selectedSetup} ready={finalReady} waitingForPreview={waitingForPreview} />}
      {retryModel && retryModel !== "custom" && (
        <Actions>
          <button type="button" className="secondary-button" disabled={engineBusy}
            onClick={() => void engine.download(retryModel, selectedSetup?.preview)}>{t("Riprova download")}</button>
        </Actions>
      )}
      <Switch label={t("Mostra il testo mentre registri")}
        hint={t("L’anteprima appare durante la call. Il testo definitivo arriva quando termini la registrazione.")}
        checked={draft.live_transcription} disabled={engineBusy}
        onChange={live_transcription => update({ live_transcription })} />
      <Advanced label={t("Opzioni del modello")}>
        <Field label={t("Elaborazione")} hint={t("{0}Automatica sceglie la scheda grafica o il processore, in base a cosa è più veloce sul tuo computer.", calibration ? t("Rilevato: {0}.", calibration) + " " : "")}>
          <select value={draft.compute_mode || "auto"} disabled={engineBusy} onChange={event => update({ compute_mode: event.target.value as "auto" | "cpu" })}>
            <option value="auto">{t("Automatica · consigliata")}</option>
            <option value="cpu">{t("Solo processore (CPU)")}</option>
          </select>
        </Field>
        {draft.live_transcription && <>
          <Field label={t("Modello per il testo in diretta")}>
            <select value={liveId} disabled={engineBusy || !health} onChange={event => {
              const model = models.find(model => model.id === event.target.value);
              if (model) update({ live_whisper_model: model.path });
            }}>
              {liveId === "custom" && <option value="custom">{t("Modello personalizzato")}</option>}
              {models.map(model => <option key={model.id} value={model.id}>{modelLabel(model.id)}</option>)}
            </select>
          </Field>
          <Field label={t("Elaborazione dell’anteprima")} hint={t("Se la scheda grafica non è disponibile l’anteprima si ferma, ma la registrazione continua.")}>
            <select value={draft.live_compute_mode || "auto"} disabled={engineBusy}
              onChange={event => update({ live_compute_mode: event.target.value })}>
              <option value="auto">{t("Scheda grafica automatica · consigliata")}</option>
              <option value="cpu">{t("Solo processore · più carico sul computer")}</option>
            </select>
          </Field>
        </>}
        {!!selectedSetup?.log.length && (
          <details className="settings-log">
            <summary>{t("Dettagli della preparazione")}</summary>
            <pre>{selectedSetup.log.join("\n")}</pre>
          </details>
        )}
        <Hint>{t("Durante la trascrizione finale l’app evita lo standby automatico. Se chiudi il portatile o lo sospendi a mano il lavoro può interrompersi: attendi lo stato «Pronta».")}</Hint>
      </Advanced>
    </Group>}

    {groq && <Group title={t("Configurazione Groq")}>
      <ApiKeyField label={t("Chiave API Groq")} saved={sttKeySaved} value={sttKey} placeholder={t("Incolla la chiave da console.groq.com/keys")}
        busy={busy || keyBusy} onChange={setSttKey} />
      <Actions>
        <button type="button" className="secondary-button" disabled={busy || keyBusy || !sttKey.trim()} onClick={() => void changeKey(() => api.sttSaveKey(sttKey), true)}>{t("Salva chiave Groq")}</button>
        <button type="button" className="text-button" disabled={busy || keyBusy || sttKeySaved !== true} onClick={() => void changeKey(api.sttDeleteKey, false)}>{t("Rimuovi chiave")}</button>
      </Actions>
    </Group>}

    {provider === "custom" && <Group title={t("Configurazione del server")}>
      <EndpointFields kind="stt" endpoint={draft.custom_stt} onChange={custom_stt => update({ custom_stt })} disabled={busy} />
    </Group>}
  </>;
}
