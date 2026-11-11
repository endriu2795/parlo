import { t } from "./i18n";
import { useRef, useState, type KeyboardEvent } from "react";
import { Archive, BookOpen, Keyboard, Mic, Send, Share2, SlidersHorizontal, Sparkles, Volume2, Wrench, X } from "lucide-react";
import { api } from "./api";
import { useDialog } from "./useDialog";
import type { AppUpdates } from "./AppUpdates";
import type { EngineSetup, Health, Settings } from "./types";
import type { EngineControls, SectionProps } from "./settings/controls";
import GeneralSection from "./settings/GeneralSection";
import RecordingSection from "./settings/RecordingSection";
import DictionarySection from "./settings/DictionarySection";
import ShortcutsSection from "./settings/ShortcutsSection";
import ArchiveSection from "./settings/ArchiveSection";
import AssistantSection from "./settings/AssistantSection";
import VoiceSection from "./settings/VoiceSection";
import AdvancedSection from "./settings/AdvancedSection";
import SharingSection from "./settings/SharingSection";
import TelegramSection from "./settings/TelegramSection";

const sections = [
  { id: "general", label: t("Generali"), icon: SlidersHorizontal, description: t("Aggiornamenti, tema e notifiche.") },
  { id: "recording", label: t("Registrazione"), icon: Mic, description: t("Cosa registrare e come trascrivere.") },
  { id: "dictionary", label: t("Dizionario"), icon: BookOpen, description: t("Parole che la trascrizione deve riconoscere.") },
  { id: "shortcuts", label: t("Scorciatoie"), icon: Keyboard, description: t("Combinazioni di tasti per registrare e annotare.") },
  { id: "archive", label: t("Archivio"), icon: Archive, description: t("Quali file tenere e per quanto tempo.") },
  { id: "sharing", label: t("Condivisione"), icon: Share2, description: t("Pubblica trascrizioni in un canale o in una cartella.") },
  { id: "telegram", label: "Telegram", icon: Send, description: t("Importa vocali, audio e video dal tuo bot.") },
  { id: "assistant", label: t("Assistente"), icon: Sparkles, description: t("Servizio AI e istruzioni per la chat.") },
  { id: "voice", label: t("Voce"), icon: Volume2, description: t("Lettura ad alta voce dei testi dalla chat.") },
  { id: "advanced", label: t("Avanzate"), icon: Wrench, description: t("Opzioni per utenti esperti.") },
] as const;
export type SettingsTab = (typeof sections)[number]["id"];
const tabs: SettingsTab[] = sections.map(section => section.id);

/** Saved settings keep an empty prompt when it matches the built-in default. */
function normalize(settings: Settings, defaultPrompt = ""): Settings {
  const prompt = (settings.system_prompt || "").trim();
  return { ...settings, diarization: false, system_prompt: prompt === defaultPrompt.trim() ? "" : prompt };
}

export default function SettingsModal({ settings, initialTab = "general", updates, updateBusy, health, engineSetup, onEngineSetup, onRefresh, onAccount, onClose, onSave, onCleanup, busy }: {
  settings: Settings;
  initialTab?: SettingsTab;
  updates: AppUpdates;
  updateBusy: boolean;
  health: Health | null;
  engineSetup: EngineSetup | null;
  onEngineSetup: (setup: EngineSetup) => void;
  onRefresh: () => Promise<void>;
  onAccount: (provider: "claude" | "codex" | "gemini") => void;
  onClose: () => void;
  onSave: (settings: Settings) => Promise<void>;
  onCleanup: () => Promise<void>;
  busy: boolean;
}) {
  const defaultPrompt = health?.default_system_prompt || "";
  const [draft, setDraft] = useState<Settings>(() => ({ ...settings, system_prompt: settings.system_prompt || defaultPrompt }));
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  const [formError, setFormError] = useState("");
  const [requestingModel, setRequestingModel] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const dialog = useDialog<HTMLElement>(onClose, ".settings-close");
  const section = sections.find(section => section.id === tab)!;
  const toSave = normalize(draft, defaultPrompt);
  const hasChanges = JSON.stringify(toSave) !== JSON.stringify(normalize(settings, defaultPrompt));
  const engineWorking = requestingModel || !!engineSetup?.running;

  const showError = (message: string) => {
    setFormError(message);
    if (message) body.current?.scrollTo({ top: 0 });
  };
  const engine: EngineControls = {
    setup: engineSetup,
    working: engineWorking,
    download: async (model, preview = false, previewMode = draft.live_compute_mode || "auto") => {
      setRequestingModel(true);
      setFormError("");
      try {
        await api.setupEngine(model, preview, previewMode);
        onEngineSetup(await api.engineSetupStatus());
      } catch (e) {
        showError(String(e));
      } finally {
        setRequestingModel(false);
      }
    },
  };
  const save = async () => {
    if (draft.shortcut.split("+").length < 2) {
      setTab("shortcuts");
      showError(t("La scorciatoia per registrare deve includere almeno un tasto modificatore (Ctrl, Alt, Shift o Cmd)."));
      return;
    }
    setFormError("");
    try {
      await onSave(toSave);
    } catch (e) {
      showError(String(e));
    }
  };
  const manageAccount = async () => {
    try {
      await onSave(toSave);
      const provider = draft.assistant_provider || "claude";
      if (provider !== "ollama" && provider !== "custom") onAccount(provider);
    } catch (e) {
      showError(String(e));
    }
  };
  const navigateTabs = (event: KeyboardEvent<HTMLDivElement>) => {
    const keys = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const index = tabs.indexOf(tab);
    const next = event.key === "Home" ? 0
      : event.key === "End" ? tabs.length - 1
      : (index + (["ArrowUp", "ArrowLeft"].includes(event.key) ? -1 : 1) + tabs.length) % tabs.length;
    setTab(tabs[next]);
    document.getElementById(`settings-tab-${tabs[next]}`)?.focus();
  };

  const common: SectionProps = {
    draft,
    update: patch => setDraft(previous => ({ ...previous, ...patch })),
    busy,
    health,
    setError: showError,
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <section className="settings-modal" ref={dialog.ref} onKeyDown={dialog.onKeyDown} role="dialog" aria-modal="true"
        aria-labelledby="settings-title" onClick={event => event.stopPropagation()}>
        <div className="settings-layout">
          <nav className="settings-nav">
            <h2 id="settings-title">{t("Impostazioni")}</h2>
            <div className="settings-tabs" role="tablist" aria-label={t("Sezioni impostazioni")} aria-orientation="vertical" onKeyDown={navigateTabs}>
              {sections.map(({ id, label, icon: Icon }) => (
                <button key={id} id={`settings-tab-${id}`} role="tab" aria-selected={tab === id} tabIndex={tab === id ? 0 : -1}
                  aria-controls={`settings-panel-${id}`} onClick={() => { setTab(id); setFormError(""); }}>
                  <Icon size={16} strokeWidth={1.8} />
                  {label}
                </button>
              ))}
            </div>
          </nav>
          <div className="settings-main">
            <header className="settings-header">
              <div>
                <h3>{section.label}</h3>
                <p>{section.description}</p>
              </div>
              <button className="icon-button settings-close" aria-label={t("Chiudi impostazioni")} onClick={onClose}><X size={18} /></button>
            </header>
            <div className="settings-body" ref={body} key={tab} role="tabpanel" id={`settings-panel-${tab}`} aria-labelledby={`settings-tab-${tab}`}>
              {formError && <p className="form-error" role="alert">{formError}</p>}
              {tab === "general" && <GeneralSection {...common} updates={updates} updateBusy={updateBusy} />}
              {tab === "recording" && <RecordingSection {...common} engine={engine} updateBusy={updateBusy} onRefresh={onRefresh} />}
              {tab === "dictionary" && <DictionarySection {...common} />}
              {tab === "shortcuts" && <ShortcutsSection {...common} />}
              {tab === "archive" && <ArchiveSection {...common} onCleanup={onCleanup} />}
              {tab === "sharing" && <SharingSection {...common} />}
              {tab === "telegram" && <TelegramSection />}
              {tab === "assistant" && <AssistantSection {...common} onManageAccount={() => void manageAccount()} />}
              {tab === "voice" && <VoiceSection {...common} />}
              {tab === "advanced" && <AdvancedSection {...common} />}
            </div>
            <footer className="settings-footer">
              <span className={`settings-save-state ${hasChanges ? "dirty" : ""}`} role="status">{hasChanges ? t("Modifiche non salvate") : t("Nessuna modifica")}</span>
              <button className="secondary-button" onClick={onClose}>{t("Annulla")}</button>
              <button className="primary-button" disabled={busy || engineWorking} onClick={() => void save()}>{t("Salva modifiche")}</button>
            </footer>
          </div>
        </div>
      </section>
    </div>
  );
}
