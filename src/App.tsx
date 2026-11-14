import { t, uiLocale } from "./i18n";
import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import PanelDivider from "./PanelDivider";
import { useWorkspaceLayout } from "./useWorkspaceLayout";
import { api, desktop } from "./api";
import { useAppUpdates, UpdateBanner } from "./AppUpdates";
import AppTitlebar from "./AppTitlebar";
import ArchiveSidebar from "./ArchiveSidebar";
import { useSharedTranscripts } from "./useSharedTranscripts";
import CallDocument from "./CallDocument";
import CaptureActions, { useNoteShortcut } from "./CaptureActions";
import ConfirmDialog from "./ConfirmDialog";
import WelcomeScreen from "./WelcomeScreen";
import { EngineBanner, ErrorBanner, ProcessingBanner } from "./StatusBanners";
import { useArchive } from "./useArchive";
import { defaultShortcut, formatShortcut } from "./shortcuts";
import type { SearchFocus } from "./TranscriptSearch";
import type { SettingsTab } from "./SettingsModal";
import type { Call, Detail, Settings, Status, Health, EngineSetup, SearchHit, PublicationStatus } from "./types";

const SettingsModal = lazy(() => import("./SettingsModal"));
const AssistantPanel = lazy(() => import("./AssistantPanel"));
const AccountDialog = lazy(() => import("./AccountDialog"));

const initialStatus: Status = {
  capture: null,
  processing: null,
  stage: "",
  chat_busy: false,
  chat_call: null,
  queued: [],
  live_segments: [],
  live_stage: "",
};
const noReconnect = { microphone: false, system: false };

export default function App() {
  const customTitlebar = desktop && /Windows/i.test(navigator.userAgent);
  const [selected, setSelected] = useState<string | null>(null);
  const [sharedChat, setSharedChat] = useState<{ source: string; id: string } | null>(null);
  const [detail, setDetail] = useState<Detail | null>(null);
  const [searchFocus, setSearchFocus] = useState<SearchFocus | undefined>();
  const [settings, setSettings] = useState<Settings | null>(null);
  const shared = useSharedTranscripts(settings?.sharing);
  const closeShared = shared.close;
  const sharedItem = shared.active;
  const sharedDocument = shared.document?.result;
  const sharedId = sharedItem?.id;
  const sharedLoaded = !!sharedDocument;
  const displayedId = sharedId ?? selected;
  const [health, setHealth] = useState<Health | null>(null);
  const [status, setStatus] = useState<Status>(initialStatus);
  const [settingsOpen, setSettingsOpen] = useState<false | SettingsTab>(false);
  const [engineSetup, setEngineSetup] = useState<EngineSetup | null>(null);
  const updates = useAppUpdates(desktop);
  const [error, setError] = useState("");
  const [settingsWarnings, setSettingsWarnings] = useState<{ cleanup: string | null; engine: string | null }>();
  const archive = useArchive(setError);
  const { query, setQuery, refresh: refreshArchive } = archive;
  const [busy, setBusy] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exportMessage, setExportMessage] = useState("");
  const [publication, setPublication] = useState<{ id: string; result?: PublicationStatus; error?: string } | null>(null);
  const [publicationRevision, setPublicationRevision] = useState(0);
  const [captureMessage, setCaptureMessage] = useState("");
  const [chatOpen, setChatOpen] = useState(true);
  const [chatWide, setChatWide] = useState(false);
  const [accountProvider, setAccountProvider] = useState<"claude" | "codex" | "gemini" | null>(null);
  const [liveAnswer, setLiveAnswer] = useState("");
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [reconnecting, setReconnecting] = useState(noReconnect);
  const [now, setNow] = useState(Date.now());
  const openDetail = detail && detail.call.id === selected ? detail : null;
  const assistantDetail = sharedId
    ? sharedChat?.source === sharedId && openDetail?.call.id === sharedChat.id ? openDetail : null
    : openDetail?.call.origin === "shared_chat" ? null : openDetail;
  const showAssistant = !!assistantDetail && chatOpen;
  const workspace = useWorkspaceLayout(showAssistant, chatWide);
  const detailVersion = useRef(0);
  const statusVersion = useRef(0);
  const configVersion = useRef(0);
  const selectionVersion = useRef(0);
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  useEffect(() => {
    if (!desktop || sharedId || !selected || openDetail?.call.origin === "shared_chat") {
      setPublication(null);
      return;
    }
    const callId = selected;
    let alive = true;
    let revision = 0;
    const refreshPublication = () => {
      const current = ++revision;
      void api.publicationStatus(callId).then(result => {
        if (alive && current === revision) setPublication({ id: callId, result });
      }).catch(() => {
        if (alive && current === revision) setPublication({ id: callId, error: t("Stato della condivisione non disponibile.") });
      });
    };
    setPublication(null);
    refreshPublication();
    const listener = listen<string>("publication-changed", event => {
      if (event.payload === callId) refreshPublication();
    });
    const timer = window.setInterval(() => { if (!document.hidden) refreshPublication(); }, 30000);
    window.addEventListener("focus", refreshPublication);
    return () => {
      alive = false;
      void listener.then(stop => stop());
      window.clearInterval(timer);
      window.removeEventListener("focus", refreshPublication);
    };
  }, [selected, sharedId, openDetail?.call.origin, settings?.sharing?.folder, settings?.sharing?.slack, settings?.sharing?.teams, publicationRevision]);

  useNoteShortcut(!!status.capture, setError);
  useEffect(() => {
    document.documentElement.dataset.theme = settings?.theme || "dark";
  }, [settings?.theme]);
  useEffect(() => {
    if (!captureMessage) return;
    const timer = setTimeout(() => setCaptureMessage(""), 3500);
    return () => clearTimeout(timer);
  }, [captureMessage]);
  useEffect(() => {
    setExportMessage("");
    setLiveAnswer("");
    setDeleteConfirm(false);
  }, [displayedId]);

  useEffect(() => {
    if (!sharedId || !sharedLoaded || !chatOpen || sharedChat?.source === sharedId) return;
    const version = selectionVersion.current;
    let active = true;
    void api.openSharedAssistant(sharedId).then(async id => {
      const next = await api.detail(id);
      if (!active || version !== selectionVersion.current) return;
      ++detailVersion.current;
      setDetail(next);
      setSharedChat({ source: sharedId, id });
      setSelected(id);
    }).catch(error => {
      if (active && version === selectionVersion.current) setError(String(error));
    });
    return () => { active = false; };
  }, [sharedId, sharedLoaded, chatOpen, sharedChat?.source]);

  // A new capture opens its document and clears the previous route state.
  const captureSeen = useRef<string | null>(null);
  const receiveStatus = useCallback((value: Status) => {
    const id = value.capture?.id || null;
    if (id !== captureSeen.current) {
      captureSeen.current = id;
      if (id) { ++selectionVersion.current; closeShared(); setSharedChat(null); setSelected(id); }
      setReconnecting(noReconnect);
    }
    ++statusVersion.current;
    setStatus(value);
  }, [closeShared]);

  const refreshConfiguration = useCallback(async () => {
    if (!desktop) return;
    const version = ++configVersion.current;
    try {
      const [config, checks] = await Promise.all([api.settings(), api.health()]);
      if (version === configVersion.current) {
        setSettings(config);
        setHealth(checks);
      }
    } catch (e) {
      if (version === configVersion.current) setError(String(e));
    }
  }, []);
  const refreshStatus = useCallback(async () => {
    if (!desktop) return;
    const version = ++statusVersion.current;
    try {
      const current = await api.status();
      if (version === statusVersion.current) receiveStatus(current);
    } catch (e) {
      if (version === statusVersion.current) setError(String(e));
    }
  }, [receiveStatus]);
  const refreshDetail = useCallback(async (id: string | null) => {
    if (!desktop || !id || id !== selectedRef.current) return;
    const version = ++detailVersion.current;
    const next = await api.detail(id).catch(() => null);
    if (version === detailVersion.current && id === selectedRef.current) setDetail(next);
  }, []);
  useEffect(() => { void refreshDetail(selected); }, [refreshDetail, selected]);
  useEffect(() => { void refreshConfiguration(); }, [refreshConfiguration]);
  useEffect(() => { void refreshStatus(); }, [refreshStatus]);

  // Engine preparation runs in the backend; poll its progress.
  useEffect(() => {
    if (!desktop) return;
    let disposed = false, checking = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try {
        const current = await api.engineSetupStatus();
        if (disposed) return;
        setEngineSetup(current);
      } catch (e) {
        if (!disposed) setError(String(e));
      } finally {
        checking = false;
      }
    };
    void check();
    const timer = setInterval(() => void check(), 1000);
    return () => { disposed = true; clearInterval(timer); };
  }, []);

  useEffect(() => {
    if (!desktop) return;
    // A minimized WebView may miss notifications while native inference continues.
    const reconcile = () => {
      if (document.hidden) return;
      void refreshStatus();
      void refreshDetail(selectedRef.current);
    };
    const onVisibility = () => {
      if (document.hidden) return;
      reconcile();
      refreshArchive();
      void refreshConfiguration();
    };
    window.addEventListener("focus", reconcile);
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", reconcile);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [refreshArchive, refreshConfiguration, refreshDetail, refreshStatus]);

  useEffect(() => {
    if (!desktop) return;
    const listeners = [
      listen<string>("call-detail-changed", e => void refreshDetail(e.payload)),
      listen<null>("calls-changed", e => { if (!e.payload) void refreshDetail(selectedRef.current); }),
      listen("configuration-changed", () => void refreshConfiguration()),
      listen<Status>("status-changed", e => receiveStatus(e.payload)),
      listen("screenshot-saved", () => setCaptureMessage(t("Screenshot salvato nella trascrizione"))),
      listen<"dark" | "light">("theme-changed", e => setSettings(previous => previous ? { ...previous, theme: e.payload } : previous)),
      listen<{ source: "microphone" | "system"; reconnecting: boolean }>("audio-route", e =>
        setReconnecting(previous => previous[e.payload.source] === e.payload.reconnecting
          ? previous : { ...previous, [e.payload.source]: e.payload.reconnecting })),
      listen<string>("open-call", e => { ++selectionVersion.current; closeShared(); setSharedChat(null); setSelected(e.payload); }),
      listen<{ id: string; text: string }>("chat-delta", e => { if (e.payload.id === selectedRef.current) setLiveAnswer(e.payload.text); }),
      listen<string>("app-error", e => setError(e.payload)),
    ];
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => {
      clearInterval(timer);
      listeners.forEach(stop => void stop.then(unlisten => unlisten()));
    };
  }, [refreshConfiguration, refreshDetail, receiveStatus, closeShared]);

  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const run = (fn: () => Promise<unknown>) => void act(fn);

  const toggleTheme = () => {
    void api.theme(settings?.theme === "light" ? "dark" : "light").then(setSettings).catch(e => setError(String(e)));
  };
  const openAccount = (provider = settings?.assistant_provider || "claude") => {
    if (provider === "ollama" || provider === "custom") setSettingsOpen("assistant");
    else setAccountProvider(provider);
  };
  const openSearchResult = (call: Call, hit?: SearchHit) => {
    closeShared();
    setSharedChat(null);
    const version = ++selectionVersion.current;
    const previous = selectedRef.current;
    const focus = hit ? { callId: call.id, query, source: hit.source, timestamp_ms: hit.timestamp_ms, id: performance.now() } : undefined;
    if (previous === call.id) { setSearchFocus(focus); return; }
    // Keep the current document in place until the new one is ready. Ignore
    // stale requests when another call or an incoming recording takes over.
    void api.detail(call.id).then(next => {
      if (version !== selectionVersion.current || selectedRef.current !== previous) return;
      ++detailVersion.current;
      setDetail(next);
      setSelected(call.id);
      setSearchFocus(focus);
    }).catch(error => {
      if (version === selectionVersion.current && selectedRef.current === previous) setError(String(error));
    });
  };
  const importMedia = () => run(async () => {
    setImporting(true);
    try {
      const id = await api.importMedia();
      if (id) { ++selectionVersion.current; closeShared(); setSharedChat(null); setSelected(id); }
    } finally {
      setImporting(false);
    }
  });
  const exportMarkdown = (copy: boolean) => {
    const call = sharedDocument?.transcript ?? openDetail?.call;
    if (!call || (sharedId && !sharedDocument)) return;
    const markdown = () => sharedDocument ? Promise.resolve(sharedDocument.markdown.trim()) : api.markdown(call.id);
    setExportMessage("");
    run(async () => {
      if (copy) {
        await navigator.clipboard.writeText(await markdown());
        setExportMessage(t("Markdown copiato"));
      } else if (desktop) {
        if (await api.saveMarkdown(call.id, !!sharedDocument)) setExportMessage(t("Markdown salvato"));
      } else {
        const url = URL.createObjectURL(new Blob([await markdown()], { type: "text/markdown;charset=utf-8" }));
        const link = document.createElement("a");
        link.href = url;
        link.download = `${call.title.replace(/[<>:"/\\|?*]/g, "-") || "call"}.md`;
        link.click();
        setTimeout(() => URL.revokeObjectURL(url), 2000);
      }
    });
  };
  const publishTranscript = (destination: import("./types").ShareDestination) => {
    if (!selected || !detail || selected !== detail.call.id) return;
    setExportMessage("");
    run(async () => {
      await api.publishTranscript(selected, destination);
      setPublicationRevision(value => value + 1);
      setExportMessage(destination === "folder" ? t("Trascrizione salvata nella cartella") : t("Trascrizione inviata al canale"));
    });
  };
  const send = async (text: string) => {
    if (!selected || !text.trim()) return;
    setLiveAnswer("");
    setError("");
    try {
      await api.chat(selected, text);
    } finally {
      setLiveAnswer("");
    }
  };

  const recording = !!status.capture;
  const elapsed = status.capture ? now - new Date(status.capture.started_at).getTime() : 0;
  const transcriptionReady = !!(health?.transcription_ready ?? health?.whisper_ready);
  const captureSupported = !!(health?.windows_capture || health?.capture_platform === "macos");
  const canRecord = desktop && !busy && captureSupported && (recording || (!engineSetup?.running && transcriptionReady));
  const canImport = desktop && !busy && !recording && !engineSetup?.running && transcriptionReady;
  const toggleRecording = () => run(recording ? api.stop : api.start);
  const shortcut = health && !health.shortcut_ready ? null : formatShortcut(settings?.shortcut || defaultShortcut("Space"));
  const updateBusy = !!(status.capture || status.processing || status.queued.length || status.chat_busy || engineSetup?.running);
  const usesLocalEngine = !settings?.stt_provider || settings.stt_provider === "local";
  const liveSelected = status.capture?.id === selected;
  const recoveringSources = status.capture ? [
    status.capture.microphone && reconnecting.microphone ? t("Microfono") : "",
    status.capture.system_audio && reconnecting.system ? t("Audio PC") : "",
  ].filter(Boolean) : [];
  const sharedDetail = useMemo<Detail | null>(() => sharedItem ? {
    call: { ...sharedItem, duration_ms: sharedDocument?.duration_ms ?? 0, status: "ready", error: null, preview: "" },
    segments: sharedDocument?.segments ?? [], messages: [],
  } : null, [sharedItem, sharedDocument]);
  const displayDetail = sharedDetail ?? assistantDetail;
  const retryEngine = () => run(async () => {
    const model = engineSetup?.model || health?.models?.find(model =>
      model.path.replace(/\\/g, "/").toLowerCase() === settings?.whisper_model.replace(/\\/g, "/").toLowerCase())?.id || "turbo";
    await api.setupEngine(model, engineSetup?.preview);
    setEngineSetup(await api.engineSetupStatus());
  });

  const assistantPanel = assistantDetail && chatOpen && <>
                <PanelDivider label={t("Larghezza assistente")} controls="assistant-panel" {...workspace.assistant} direction={-1}
                  onPreview={value => workspace.preview("assistant", value)}
                  onChange={value => { workspace.commit("assistant", value); setChatWide(false); }} />
                <Suspense fallback={null}>
                  <AssistantPanel
                    key={settings?.assistant_provider || "claude"}
                    onAccount={() => openAccount()}
                    onContextSettings={() => setSettingsOpen("assistant")}
                    onProvider={async provider => { setSettings(await api.provider(provider)); setHealth(await api.health()); }}
                    detail={assistantDetail}
                    settings={settings}
                    status={status}
                    answer={liveAnswer}
                    onSend={send}
                    onModel={async model => { if (settings) await act(async () => { await api.chatModel(model); await refreshConfiguration(); }); }}
                    onConnector={async (name, enabled) => setSettings(await api.connectorEnabled(name, enabled))}
                    onSettingsChange={setSettings}
                    onReset={async () => {
                      const id = assistantDetail.call.id;
                      const cleared = await api.resetChat(id);
                      setDetail(current => current?.call.id === id ? cleared : current);
                      if (selectedRef.current === id) setLiveAnswer("");
                    }}
                    wide={chatWide}
                    onWide={() => setChatWide(!chatWide)}
                    onClose={() => setChatOpen(false)}
                  />
                </Suspense>
              </>;

  return (
    <div className="app-frame">
      {customTitlebar && <AppTitlebar color={settings?.titlebar_color} theme={settings?.theme}
        onSettings={() => setSettingsOpen("general")} onTheme={toggleTheme} onError={setError} />}
      <div className="app-shell workspace-shell" ref={workspace.root} style={workspace.style}>
        <ArchiveSidebar
          calls={archive.calls} total={archive.total} hasMore={!!archive.cursor} loading={archive.loading}
          loadingMore={archive.loadingMore} onLoadMore={archive.loadMore}
          selected={shared.active ? null : selected} onOpen={openSearchResult}
          shared={shared.items} selectedShared={shared.active?.id} onCloseShared={() => {
            ++selectionVersion.current;
            closeShared();
            if (sharedChat?.id === selectedRef.current) setSelected(null);
            setSharedChat(null);
          }}
          onOpenShared={id => {
            if (id === sharedId) return;
            ++selectionVersion.current;
            setSearchFocus(undefined);
            if (sharedChat?.source !== id) {
              if (sharedChat?.id === selectedRef.current) setSelected(null);
              setSharedChat(null);
            }
            shared.open(id);
          }}
          query={query} onQuery={setQuery} filter={archive.filter} onFilter={archive.setFilter} tags={archive.tags}
          recording={recording} elapsedMs={elapsed} canRecord={canRecord} onRecord={toggleRecording}
          canImport={canImport} importing={importing} onImport={importMedia}
          shortcut={shortcut} retentionMonths={settings?.retention_months}
          onSettings={setSettingsOpen}
          preferences={customTitlebar ? null : { theme: settings?.theme || "dark", onTheme: toggleTheme }}
          desktop={desktop}
        />
        <PanelDivider label={t("Larghezza archivio")} controls="archive-panel" {...workspace.archive}
          onPreview={value => workspace.preview("archive", value)} onChange={value => workspace.commit("archive", value)} />
        <main className="main">
          {!desktop && <div className="banner notice">{t("Questa è un’anteprima dell’interfaccia: registrazione e archivio funzionano nell’app desktop.")}</div>}
          {desktop && <UpdateBanner updates={updates} busy={busy || updateBusy} />}
          {desktop && usesLocalEngine && engineSetup && (engineSetup.running || engineSetup.error) && <EngineBanner setup={engineSetup} busy={busy} onRetry={retryEngine} />}
          {error && <ErrorBanner message={error} onClose={() => setError("")} />}
          {settingsWarnings?.cleanup && <ErrorBanner message={t("Impostazioni salvate. Pulizia dell’archivio incompleta. {0}", settingsWarnings.cleanup)}
            onClose={() => setSettingsWarnings({ ...settingsWarnings, cleanup: null })} />}
          {settingsWarnings?.engine && <ErrorBanner message={t("Impostazioni salvate. Preparazione del motore incompleta. {0}", settingsWarnings.engine)}
            onClose={() => setSettingsWarnings({ ...settingsWarnings, engine: null })} />}
          {status.processing && <ProcessingBanner status={status} busy={busy} onCancel={() => run(() => api.cancelProcessing(status.processing!))} />}
          {recoveringSources.length > 0 && (
            <div className="banner audio-route-notice" role="status">
              {t("{0}: riconnessione al dispositivo in corso. La registrazione continua con le altre sorgenti.", new Intl.ListFormat(uiLocale).format(recoveringSources))}
            </div>
          )}
          {displayDetail ? (
            <div className={`detail-layout ${showAssistant ? "with-chat" : ""} ${chatWide && showAssistant ? "assistant-wide" : ""}`}>
              <CallDocument
                key={sharedId ? `shared:${sharedId}` : "local"}
                detail={displayDetail}
                segments={!sharedItem && liveSelected ? status.live_segments : displayDetail.segments}
                shared={sharedItem && {
                  publisher: sharedItem.publisher,
                  loading: !sharedLoaded && !shared.document?.error,
                  error: shared.document?.error,
                  approximateDuration: sharedDocument?.transcript.duration_ms == null,
                }}
                live={!sharedItem && liveSelected}
                liveStage={status.live_stage}
                busy={busy}
                query={query}
                searchFocus={searchFocus?.callId === displayDetail.call.id && searchFocus.query.trim() === query.trim() ? searchFocus : undefined}
                onClearQuery={() => setQuery("")}
                captureActions={<CaptureActions settings={settings} disabled={busy} run={run} />}
                captureMessage={sharedItem ? "" : captureMessage}
                exportMessage={exportMessage}
                retryQueued={status.queued.includes(displayDetail.call.id)}
                canChangeService={!status.processing && !status.capture && status.queued.length === 0}
                chatOpen={chatOpen}
                onToggleChat={() => setChatOpen(!chatOpen)}
                onRename={title => run(() => api.rename(displayDetail.call.id, title))}
                onTags={tags => api.setTags(displayDetail.call.id, tags)}
                onRetry={() => run(() => api.retry(displayDetail.call.id))}
                onChangeService={() => setSettingsOpen("recording")}
                onExport={() => exportMarkdown(false)}
                onCopy={() => exportMarkdown(true)}
                sharing={desktop ? settings?.sharing : undefined}
                publication={publication?.id === displayDetail.call.id ? publication : null}
                onPublish={publishTranscript}
                onResetPanels={() => { workspace.reset(); setChatWide(false); }}
                onDelete={() => setDeleteConfirm(true)}
              />
              {assistantPanel}
            </div>
          ) : (
            <WelcomeScreen settings={settings} recording={recording} canRecord={canRecord} onRecord={toggleRecording}
              canImport={canImport} onImport={importMedia} shortcut={shortcut}
              preparing={!!health && usesLocalEngine && !health.whisper_ready && !engineSetup?.running && !engineSetup?.error} />
          )}
        </main>
        {accountProvider && <Suspense fallback={null}><AccountDialog key={accountProvider} provider={accountProvider} onClose={() => setAccountProvider(null)} /></Suspense>}
        {settingsOpen && settings && (
          <Suspense fallback={null}>
            <SettingsModal
              settings={settings}
              initialTab={settingsOpen}
              updates={updates}
              updateBusy={busy || updateBusy}
              health={health}
              engineSetup={engineSetup}
              onEngineSetup={setEngineSetup}
              onRefresh={refreshConfiguration}
              onAccount={provider => { setSettingsOpen(false); openAccount(provider); }}
              onClose={() => setSettingsOpen(false)}
              onSave={async value => {
                setBusy(true);
                setError("");
                try {
                  const saved = await api.saveSettings(value);
                  setSettings(saved.settings);
                  setSettingsOpen(false);
                  setSettingsWarnings({ cleanup: saved.cleanup_error, engine: saved.engine_error });
                } finally {
                  setBusy(false);
                }
              }}
              onCleanup={() => act(async () => {
                await api.cleanup();
                setSettingsWarnings(current => current && { ...current, cleanup: null });
              })}
              busy={busy}
            />
          </Suspense>
        )}
        {deleteConfirm && openDetail && (
          <ConfirmDialog title={t("Eliminare questa registrazione?")} confirmLabel={t("Elimina")} danger busy={busy}
            onCancel={() => setDeleteConfirm(false)}
            onConfirm={() => run(async () => {
              await api.remove(openDetail.call.id);
              setSelected(null);
              setDetail(null);
              setDeleteConfirm(false);
            })}>
            <p>{t("Verranno eliminati trascrizione, note, screenshot, chat ed eventuali file audio o video. L’operazione non si può annullare.")}</p>
          </ConfirmDialog>
        )}
      </div>
    </div>
  );
}
