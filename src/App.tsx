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
import { noFilter, type ArchiveFilter } from "./ArchiveFilters";
import { archiveDateBounds } from "./archiveDates";
import { defaultShortcut, formatShortcut } from "./shortcuts";
import type { SearchFocus } from "./TranscriptSearch";
import type { SettingsTab } from "./SettingsModal";
import type { Call, Detail, Settings, Status, Health, EngineSetup, SearchHit, TagCount, PublicationStatus } from "./types";

const SettingsModal = lazy(() => import("./SettingsModal"));
const AssistantPanel = lazy(() => import("./AssistantPanel"));
const AccountDialog = lazy(() => import("./AccountDialog"));

const PAGE_SIZE = 10;
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
  const [calls, setCalls] = useState<Call[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [sharedChat, setSharedChat] = useState<{ source: string; id: string } | null>(null);
  const [detail, setDetail] = useState<Detail | null>(null);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<ArchiveFilter>(noFilter);
  const [tags, setTags] = useState<TagCount[]>([]);
  const dateBounds = archiveDateBounds(filter.from, filter.to)!;
  const filterKey = `${query}\n${filter.from}\n${filter.to}\n${filter.tags.join("\n")}`;
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
  const [total, setTotal] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const openDetail = detail && detail.call.id === selected ? detail : null;
  const assistantDetail = sharedId
    ? sharedChat?.source === sharedId && openDetail?.call.id === sharedChat.id ? openDetail : null
    : openDetail?.call.origin === "shared_chat" ? null : openDetail;
  const showAssistant = !!assistantDetail && chatOpen;
  const workspace = useWorkspaceLayout(showAssistant, chatWide);
  const refreshVersion = useRef(0);
  const selectionVersion = useRef(0);
  const visibleCount = useRef(PAGE_SIZE);
  const loadBusy = useRef(false);
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const previousFilter = useRef(filterKey);
  if (previousFilter.current !== filterKey) {
    previousFilter.current = filterKey;
    visibleCount.current = PAGE_SIZE;
  }

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
      ++refreshVersion.current;
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
    setStatus(value);
  }, [closeShared]);

  const refresh = useCallback(async () => {
    if (!desktop) return;
    const version = ++refreshVersion.current;
    try {
      const [pages, tagList, state, config, checks] = await Promise.all([
        Promise.all(Array.from({ length: Math.ceil(visibleCount.current / PAGE_SIZE) },
          (_, page) => api.list(query, page * PAGE_SIZE, PAGE_SIZE, dateBounds.from, dateBounds.to, filter.tags))),
        api.tags(),
        api.status(),
        api.settings(),
        api.health(),
      ]);
      const nextDetail = selected ? await api.detail(selected).catch(() => null) : null;
      if (version !== refreshVersion.current) return;
      setCalls(pages.flatMap(page => page.items));
      setTotal(pages[0]?.total || 0);
      setHasMore(pages.at(-1)?.has_more || false);
      setTags(tagList);
      receiveStatus(state);
      setSettings(config);
      setHealth(checks);
      setDetail(nextDetail);
    } catch (e) {
      if (version === refreshVersion.current) setError(String(e));
    }
  }, [query, filter, selected, receiveStatus]);
  useEffect(() => { void refresh(); }, [refresh]);

  // Engine preparation runs in the backend; poll it and reload once it finishes.
  useEffect(() => {
    if (!desktop) return;
    let disposed = false, checking = false, wasRunning = false;
    const check = async () => {
      if (checking) return;
      checking = true;
      try {
        const current = await api.engineSetupStatus();
        if (disposed) return;
        setEngineSetup(current);
        if (wasRunning && !current.running) await refresh();
        wasRunning = current.running;
      } catch (e) {
        if (!disposed) setError(String(e));
      } finally {
        checking = false;
      }
    };
    void check();
    const timer = setInterval(() => void check(), 1000);
    return () => { disposed = true; clearInterval(timer); };
  }, [refresh]);

  useEffect(() => {
    if (!desktop) return;
    // A minimized WebView may pause JS/event delivery; native inference keeps
    // running. Reconcile with the backend when the window becomes visible.
    const reconcile = () => { if (!document.hidden) void refresh(); };
    window.addEventListener("focus", reconcile);
    document.addEventListener("visibilitychange", reconcile);
    return () => {
      window.removeEventListener("focus", reconcile);
      document.removeEventListener("visibilitychange", reconcile);
    };
  }, [refresh]);

  useEffect(() => {
    if (!desktop) return;
    const listeners = [
      listen("calls-changed", () => void refresh()),
      listen<Status>("status-changed", e => receiveStatus(e.payload)),
      listen("screenshot-saved", () => setCaptureMessage(t("Screenshot salvato nella trascrizione"))),
      listen<"dark" | "light">("theme-changed", e => setSettings(previous => previous ? { ...previous, theme: e.payload } : previous)),
      listen<{ source: "microphone" | "system"; reconnecting: boolean }>("audio-route", e =>
        setReconnecting(previous => previous[e.payload.source] === e.payload.reconnecting
          ? previous : { ...previous, [e.payload.source]: e.payload.reconnecting })),
      listen<string>("open-call", e => { ++selectionVersion.current; closeShared(); setSharedChat(null); setSelected(e.payload); }),
      listen<{ id: string; text: string }>("chat-delta", e => { if (e.payload.id === selected) setLiveAnswer(e.payload.text); }),
      listen<string>("app-error", e => setError(e.payload)),
    ];
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => {
      clearInterval(timer);
      listeners.forEach(stop => void stop.then(unlisten => unlisten()));
    };
  }, [refresh, selected, receiveStatus, closeShared]);

  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    setError("");
    try {
      await fn();
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const run = (fn: () => Promise<unknown>) => void act(fn);

  const loadMore = async () => {
    if (!hasMore || loadBusy.current) return;
    loadBusy.current = true;
    setLoadingMore(true);
    const version = refreshVersion.current;
    try {
      const page = await api.list(query, calls.length, PAGE_SIZE, dateBounds.from, dateBounds.to, filter.tags);
      if (version === refreshVersion.current) {
        setCalls(previous => [...previous, ...page.items.filter(row => !previous.some(call => call.id === row.id))]);
        visibleCount.current = calls.length + page.items.length;
        setTotal(page.total);
        setHasMore(page.has_more);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      loadBusy.current = false;
      setLoadingMore(false);
    }
  };

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
      ++refreshVersion.current;
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
      if (selectedRef.current === selected) await refresh();
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
                    onModel={async model => { if (settings) await act(() => api.chatModel(model)); }}
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
          calls={calls} total={total} hasMore={hasMore} loadingMore={loadingMore} onLoadMore={() => void loadMore()}
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
          query={query} onQuery={setQuery} filter={filter} onFilter={setFilter} tags={tags}
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
                onTags={async tags => { await api.setTags(displayDetail.call.id, tags); await refresh(); }}
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
              onRefresh={refresh}
              onAccount={provider => { setSettingsOpen(false); openAccount(provider); }}
              onClose={() => setSettingsOpen(false)}
              onSave={async value => {
                setBusy(true);
                setError("");
                try {
                  await api.saveSettings(value);
                  await refresh();
                  setSettingsOpen(false);
                } finally {
                  setBusy(false);
                }
              }}
              onCleanup={() => act(api.cleanup)}
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
