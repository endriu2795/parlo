import { t } from "./i18n";
import { Fragment, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { listen } from "@tauri-apps/api/event";
import { ArrowDown, ArrowUp, FileText, Maximize2, Minimize2, Plug, RefreshCw, RotateCcw, Sparkles, Square, X } from "lucide-react";
import ToolPermissionCard from "./ToolPermissionCard";
import ConfirmDialog from "./ConfirmDialog";
import { api, desktop } from "./api";
import { useAssistantAuth } from "./useAssistantAuth";
import ChatTimeline, { isChatText } from "./ChatTimeline";
import ChatSources, { useChatSources } from "./ChatSources";
import { insertMention, matchingSources, mentionQuery } from "./sourceMentions";
import OllamaContext from "./assistant/OllamaContext";
import ConnectorsPopover from "./assistant/ConnectorsPopover";
import { AssistantReply, ChatEmpty, MentionList, StreamingReply, UserMessage } from "./assistant/ChatMessages";
import { providerConfig } from "./assistant/provider";
import type { AssistantProvider, ChatRun, ChatSource, Connector, Detail, ModelCatalog, Settings, Status, ToolPermission } from "./types";

// Drafts live in memory so switching calls or closing the widget does not discard typing.
const drafts = new Map<string, string>();

export default function AssistantPanel({
  detail,
  settings,
  status,
  answer,
  onSend,
  onModel,
  onProvider,
  onConnector,
  onSettingsChange,
  onReset,
  wide,
  onWide,
  onClose,
  onAccount,
  onContextSettings,
}: {
  detail: Detail;
  settings: Settings | null;
  status: Status;
  answer: string;
  onSend: (text: string) => Promise<void>;
  onModel: (model: string) => Promise<void>;
  onProvider: (provider: AssistantProvider) => Promise<void>;
  onConnector: (name: string, enabled: boolean) => Promise<void>;
  onSettingsChange: (settings: Settings) => void;
  onReset: () => Promise<void>;
  wide: boolean;
  onWide: () => void;
  onClose: () => void;
  onAccount: () => void;
  onContextSettings: () => void;
}) {
  const config = providerConfig(settings);
  const { provider, label: providerLabel, connectorsEnabled, disabledConnectors } = config;
  const { auth, authError, checkAuth } = useAssistantAuth(provider);
  const ollamaServer = provider === "ollama" && !!settings?.ollama_server_url;
  const [prompt, setPrompt] = useState(""),
    [error, setError] = useState(""),
    [sending, setSending] = useState(false);
  const [resetConfirm, setResetConfirm] = useState(false),
    [resetBusy, setResetBusy] = useState(false);
  const [pendingPrompt, setPendingPrompt] = useState<{
    call_id: string;
    text: string;
    index: number;
    created_at: string;
  } | null>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const sources = useChatSources(
    detail.call.id,
    `${provider}:${detail.screenshots?.length || 0}:${detail.notes?.length || 0}:${detail.audio_directory || ""}:${detail.video_path || ""}:${detail.call.status}:${detail.messages.length}:${status.chat_busy}`,
  );
  const [sourcesOpen, setSourcesOpen] = useState(false);
  const [mention, setMention] = useState<ReturnType<typeof mentionQuery>>(null);
  const [mentionIndex, setMentionIndex] = useState(0);
  const mentionOptions = mention
    ? matchingSources(sources.inventory?.sources || [], mention.query)
    : [];
  const chooseMention = (source: ChatSource) => {
    const cursor = composer.current?.selectionStart ?? prompt.length;
    const result = insertMention(
      prompt,
      mention || { start: cursor, end: cursor },
      source,
    );
    setPrompt(result.text);
    draftByCall.current.set(detail.call.id, result.text);
    setMention(null);
    requestAnimationFrame(() => {
      composer.current?.focus();
      composer.current?.setSelectionRange(result.caret, result.caret);
    });
  };
  useEffect(() => {
    setMention(null);
    setMentionIndex(0);
    setSourcesOpen(false);
  }, [detail.call.id]);
  useEffect(() => {
    setMentionIndex(0);
  }, [mention?.query]);
  const [catalog, setCatalog] = useState<ModelCatalog | null>(null),
    [modelsLoading, setModelsLoading] = useState(false),
    [modelsError, setModelsError] = useState("");
  const [connectors, setConnectors] = useState<Connector[]>([]),
    [connectorsOpen, setConnectorsOpen] = useState(false),
    [connectorsLoading, setConnectorsLoading] = useState(false),
    [connectorsError, setConnectorsError] = useState("");
  useEffect(() => {
    if (!sourcesOpen && !connectorsOpen) return;
    const close = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      setSourcesOpen(false); setConnectorsOpen(false);
      composer.current?.focus();
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [sourcesOpen, connectorsOpen]);

  const [permission, setPermission] = useState<ToolPermission | null>(null),
    [permissionBusy, setPermissionBusy] = useState(false),
    [activity, setActivity] = useState<ChatRun | null>(null);
  const [questionAnswers, setQuestionAnswers] = useState<
    Record<string, string>
  >({});
  useEffect(() => setQuestionAnswers({}), [permission?.id]);
  const [atBottom, setAtBottom] = useState(true);
  const feed = useRef<HTMLDivElement>(null),
    pinned = useRef(true),
    draftByCall = useRef(drafts);
  const callRef = useRef(detail.call.id),
    progressEpoch = useRef(0);
  callRef.current = detail.call.id;
  const modelRequest = useRef(0),
    connectorRequest = useRef(0);
  const loadModels = async (refresh = false) => {
    const request = ++modelRequest.current;
    setModelsLoading(true);
    setModelsError("");
    try {
      const value = await api.models(refresh);
      if (request === modelRequest.current) setCatalog(value);
    } catch (e) {
      if (request === modelRequest.current) setModelsError(String(e));
    } finally {
      if (request === modelRequest.current) setModelsLoading(false);
    }
  };
  const loadConnectors = async () => {
    const request = ++connectorRequest.current;
    setConnectorsLoading(true);
    setConnectorsError("");
    try {
      const value = await api.connectors();
      if (request === connectorRequest.current) setConnectors(value);
    } catch (e) {
      if (request === connectorRequest.current) setConnectorsError(String(e));
    } finally {
      if (request === connectorRequest.current) setConnectorsLoading(false);
    }
  };
  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const epoch = progressEpoch.current;
    setActivity(null);
    void checkAuth().catch(() => {});
    const listeners = [
      listen<ToolPermission | null>("chat-permission", (e) =>
        setPermission(e.payload),
      ),
      listen<ChatRun | null>("chat-progress", (e) => {
        if (!disposed && e.payload?.call_id === detail.call.id)
          setActivity(e.payload);
      }),
      listen<{ id: string }>("chat-reset", (e) => {
        if (e.payload.id === detail.call.id) clearLocalChat();
      }),
    ];
    void api
      .permission()
      .then(setPermission)
      .catch((e) => setError(String(e)));
    void Promise.all(listeners)
      .then(() => api.progress())
      .then((run) => {
        if (
          !disposed &&
          epoch === progressEpoch.current &&
          run?.call_id === detail.call.id
        )
          setActivity(run);
      })
      .catch((e) => {
        if (!disposed) setError(String(e));
      });
    return () => {
      disposed = true;
      listeners.forEach((p) => void p.then((stop) => stop()));
    };
  }, [settings?.claude_bin, settings?.codex_bin, settings?.gemini_bin, settings?.ollama_model, settings?.ollama_server_url,
    settings?.custom_chat?.url, settings?.custom_chat?.model, provider, detail.call.id]);
  useEffect(() => {
    setCatalog(null);
    if (desktop && auth?.logged_in) {
      void loadModels();
      if (connectorsEnabled || (provider === "ollama" && settings?.ollama_tools_enabled !== false && (settings?.tts_enabled || settings?.ollama_web_search === true))) void loadConnectors();
    } else {
      setCatalog(null);
      setConnectors([]);
    }
    return () => {
      ++modelRequest.current;
      ++connectorRequest.current;
    };
  }, [
    settings?.claude_bin,
    settings?.codex_bin, settings?.gemini_bin, settings?.ollama_server_url,
    provider,
    connectorsEnabled,
    settings?.tts_enabled, settings?.ollama_web_search, settings?.ollama_tools_enabled,
    auth?.logged_in,
  ]);
  useEffect(() => {
    if (pinned.current && feed.current)
      feed.current.scrollTop = feed.current.scrollHeight;
  }, [detail.messages.length, answer, activity]);
  useEffect(() => {
    setPrompt(draftByCall.current.get(detail.call.id) || "");
    setPendingPrompt(null);
    setError("");
    setResetConfirm(false);
    pinned.current = true;
    setAtBottom(true);
    requestAnimationFrame(() => {
      if (feed.current) feed.current.scrollTop = feed.current.scrollHeight;
    });
  }, [detail.call.id]);
  useEffect(() => {
    if (!prompt && composer.current) composer.current.style.height = "auto";
  }, [prompt]);
  useEffect(() => {
    if (!status.chat_busy) {
      setPermission(null);
    }
  }, [status.chat_busy]);
  const selectedModel = config.model,
    modelInfo = catalog?.models.find((m) => m.value === selectedModel);
  const ready =
    !!auth?.logged_in &&
    (provider !== "ollama" || !!modelInfo) &&
    !status.chat_busy &&
    !resetBusy &&
    !sources.busy &&
    !sending;
  const active = status.chat_busy && status.chat_call === detail.call.id;
  const runningChat = status.chat_busy ? status.chat_call : null;
  const hasTranscript = (status.capture?.id === detail.call.id ? status.live_segments : detail.segments).length > 0;
  const currentPermission =
    permission?.call_id === detail.call.id ? permission : null;
  const connectedCount = connectors.filter(
    (c) => c.status === "connected" && (["parlo_tts", "parlo_web"].includes(c.name) || !disabledConnectors?.includes(c.name)),
  ).length;
  const entriesFor = (index: number) =>
    activity?.user_index === index
      ? activity.entries
      : detail.messages[index]?.activity || [];
  const orderedReply = (index: number) =>
    index > 0 &&
    detail.messages[index - 1].role === "user" &&
    entriesFor(index - 1).some(isChatText);
  const mentionSources = sources.inventory?.sources || [];
  const clearLocalChat = () => {
    ++progressEpoch.current;
    setActivity(null);
    setPermission(null);
    setPendingPrompt(null);
    setPrompt("");
    setMention(null);
    setError("");
    setResetConfirm(false);
    draftByCall.current.delete(detail.call.id);
    pinned.current = true;
    setAtBottom(true);
  };
  const reset = async () => {
    if (active || sending || resetBusy) return;
    setResetBusy(true);
    setError("");
    const callId = detail.call.id;
    try {
      await onReset();
      if (callRef.current === callId) clearLocalChat();
    } catch (e) {
      if (callRef.current === callId) {
        setResetConfirm(false);
        setError(String(e));
      }
    } finally {
      setResetBusy(false);
    }
  };
  const send = async (text: string) => {
    if (!ready || !text.trim()) return;
    const callId = detail.call.id;
    setSending(true);
    setActivity(null);
    setPendingPrompt({
      call_id: detail.call.id,
      text: text.trim(),
      index: detail.messages.length,
      created_at: new Date().toISOString(),
    });
    setError("");
    pinned.current = true;
    setAtBottom(true);
    setPrompt("");
    setMention(null);
    draftByCall.current.delete(detail.call.id);
    try {
      await onSend(text.trim());
      if (callRef.current === callId) await sources.refresh();
    } catch (e) {
      if (callRef.current === callId) {
        setError(String(e));
        setPrompt(text);
      }
      draftByCall.current.set(callId, text);
    } finally {
      setSending(false);
      setPendingPrompt(null);
    }
  };
  const decide = async (allow: boolean, always = false) => {
    if (!currentPermission) return;
    setPermissionBusy(true);
    try {
      onSettingsChange(
        await api.answerPermission(
          currentPermission.id,
          allow,
          always,
          questionAnswers,
        ),
      );
      setPermission(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setPermissionBusy(false);
    }
  };
  const scrollBottom = () => {
    pinned.current = true;
    setAtBottom(true);
    feed.current?.scrollTo({
      top: feed.current.scrollHeight,
      behavior: "smooth",
    });
  };
  return (
    <aside
      className={`assistant-panel chat-widget ${wide ? "wide" : ""}`}
      id="assistant-panel"
      aria-label={t("Chat della call")}
    >
      <header className="widget-header">
        <div className="widget-title">
          <Sparkles size={18} />
          <h2>{t("Assistente")}</h2>
        </div>
        <div className="widget-header-actions">
          <button
            type="button"
            className={`widget-icon widget-sources-button ${sourcesOpen ? "selected" : ""}`}
            aria-label={t("File e fonti della chat")}
            title={sources.inventory?.generated.length
              ? t("File e fonti della chat · {0} file generati", sources.inventory.generated.length)
              : t("File e fonti della chat")}
            aria-expanded={sourcesOpen}
            onClick={() => {
              setSourcesOpen(!sourcesOpen);
              setConnectorsOpen(false);
              setResetConfirm(false);
            }}
          >
            <FileText size={16} />
            <span>{t("Fonti")}</span>
          </button>
          <button
            className={`widget-icon ${resetConfirm ? "selected" : ""}`}
            aria-label={t("Ricomincia chat")}
            title={
              active
                ? t("Interrompi la risposta prima di azzerare la chat")
                : t("Ricomincia chat")
            }
            disabled={
              active ||
              sending ||
              resetBusy ||
              (!detail.messages.length && !activity)
            }
            aria-haspopup="dialog"
            onClick={() => {
              setResetConfirm(!resetConfirm);
              setSourcesOpen(false);
              setConnectorsOpen(false);
            }}
          >
            <RotateCcw size={15} />
          </button>
          <button
            className={`widget-icon ${connectorsOpen ? "selected" : ""}`}
            aria-label={t("Connettori MCP")}
            title={t("Connettori: app e servizi collegati alla chat")}
            aria-expanded={connectorsOpen}
            onClick={() => {
              setConnectorsOpen(!connectorsOpen);
              setSourcesOpen(false);
            }}
          >
            <Plug size={16} />
            {connectedCount > 0 && <i />}
          </button>
          <button
            className="widget-icon"
            aria-label={wide ? t("Riduci assistente") : t("Espandi assistente")}
            title={wide ? t("Riduci assistente") : t("Espandi assistente")}
            onClick={onWide}
          >
            {wide ? <Minimize2 size={16} /> : <Maximize2 size={16} />}
          </button>
          <button type="button" className="widget-icon widget-close" title={t("Chiudi assistente")} aria-label={t("Chiudi assistente")} onClick={onClose}><X size={15}/></button>
        </div>
      </header>
      <div
        className="widget-context-line"
        title={t("Fonti della chat: {0}. Gestiscile dal pulsante Fonti.", detail.call.title)}
      >
        <FileText size={14} />
        <span>{detail.call.title}</span>
        <button type="button" className="context-state" title={t("Gestisci le fonti della chat")} onClick={() => { setSourcesOpen(!sourcesOpen); setConnectorsOpen(false); setResetConfirm(false); }}>
          {detail.call.status === "ready"
            ? sources.inventory
              ? t("{0} fonti", sources.inventory.sources.filter((s) => s.enabled && s.available).length)
              : t("Fonti")
            : hasTranscript ? t("Testo provvisorio") : t("Senza trascrizione")}
        </button>
      </div>
      {(!hasTranscript || detail.call.status !== "ready") && <div className="widget-transcript-notice" role="status">
        {hasTranscript
          ? t("L’AI usa il testo provvisorio disponibile quando invii la domanda. Può essere incompleto o contenere errori.")
          : t("La trascrizione non è ancora disponibile. Puoi comunque usare la chat e le fonti allegate.")}
      </div>}
      {sourcesOpen && (
        <ChatSources
          key={detail.call.id}
          controller={sources}
          disabled={status.chat_busy || sending}
          onMention={(source) => {
            chooseMention(source);
            setSourcesOpen(false);
          }}
          onClose={() => { setSourcesOpen(false); composer.current?.focus(); }}
        />
      )}
      {resetConfirm && createPortal(
        <ConfirmDialog
          title={t("Ricominciare questa chat?")}
          confirmLabel={resetBusy ? t("Pulizia…") : t("Azzera chat")}
          busy={resetBusy || active || sending}
          danger
          onCancel={() => { if (!resetBusy) setResetConfirm(false); }}
          onConfirm={() => void reset()}
        >
          <p>{t("Elimina i messaggi e le attività di questa chat. La trascrizione, le istruzioni di progetto e i consensi degli strumenti restano disponibili.")}</p>
        </ConfirmDialog>,
        document.body,
      )}
      {connectorsOpen && (
        <ConnectorsPopover config={config} settings={settings} connectors={connectors} loading={connectorsLoading} error={connectorsError}
          chatBusy={status.chat_busy} onRefresh={() => void loadConnectors()} onToggle={onConnector} onSettingsChange={onSettingsChange}
          onError={setError} onClose={() => { setConnectorsOpen(false); composer.current?.focus(); }} />
      )}
      <div className="widget-feed-wrap">
        <div
          className="assistant-feed"
          ref={feed}
          onScroll={() => {
            const el = feed.current;
            if (el) {
              const bottom =
                el.scrollHeight - el.scrollTop - el.clientHeight < 70;
              pinned.current = bottom;
              setAtBottom(bottom);
            }
          }}
        >
          {!auth?.logged_in && (
            <div className="widget-signin">
              <strong>{provider === "custom" ? t("Configura l’endpoint") : provider === "ollama" ? (ollamaServer ? t("Verifica il server Ollama") : t("Prepara Ollama sul computer")) : t("Collega il tuo account")}</strong>
              <p>{provider === "custom" ? t("Verifica l’indirizzo e il modello nelle impostazioni dell’assistente.") : provider === "ollama" ? (ollamaServer ? t("Verifica che il server sia raggiungibile e che il modello supporti gli strumenti.") : t("Avvia Ollama e installa un modello con supporto agli strumenti. Non serve un account cloud.")) : t("Accedi nel browser per parlare con l’assistente.")}</p>
              <button onClick={provider === "ollama" ? () => void checkAuth().catch(error => setError(String(error))) : onAccount}>
                {provider === "custom" ? t("Impostazioni endpoint") : provider === "ollama" ? t("Verifica Ollama") : t("Accedi con {0}", providerLabel)} <ArrowUp size={14} />
              </button>
            </div>
          )}
          {(error || authError || auth?.error) && (
            <div role="alert" className="chat-info">
              {error || authError || auth?.error}
            </div>
          )}
          {!detail.messages.length && !active && !sending && <ChatEmpty ready={ready} onSend={prompt => void send(prompt)} />}
          {detail.messages.map((message, i) => (
            <Fragment key={i}>
              {message.role === "user"
                ? <UserMessage text={message.content} sentAt={message.created_at} callId={detail.call.id} sources={mentionSources} />
                : <AssistantReply content={message.content} sentAt={message.created_at} model={message.model} inTimeline={orderedReply(i)} onError={setError} />}
              {message.role === "user" && (
                <ChatTimeline
                  key={
                    activity?.user_index === i ? activity.run_id : `saved-${i}`
                  }
                  entries={
                    activity?.user_index === i
                        ? activity.entries
                        : message.activity || []
                  }
                  live={active && activity?.user_index === i}
                  finalAnswer={
                    detail.messages[i + 1]?.role === "assistant"
                      ? detail.messages[i + 1].content
                      : undefined
                  }
                />
              )}
            </Fragment>
          ))}
          {pendingPrompt?.call_id === detail.call.id &&
            !detail.messages
              .slice(pendingPrompt.index)
              .some(
                (m) => m.role === "user" && m.content === pendingPrompt.text,
              ) && (
              <UserMessage text={pendingPrompt.text} sentAt={pendingPrompt.created_at} callId={detail.call.id} sources={mentionSources} />
            )}
          {activity && !detail.messages[activity.user_index] && (
            <ChatTimeline
              key={activity.run_id}
              entries={activity.entries}
              live={active}
            />
          )}
          {active &&
            answer &&
            !activity?.entries.some(isChatText) && (
              <StreamingReply answer={answer} />
            )}
          {active && !answer && (
            <div className="widget-thinking">
              <span className="thinking-dots">
                <i />
                <i />
                <i />
              </span>
              <span>
                {currentPermission
                  ? (["AskUserQuestion", "request_user_input"].includes(currentPermission.tool) ? t("In attesa delle tue risposte…") : t("In attesa del tuo consenso…"))
                  : t("{0} sta elaborando la richiesta…", providerLabel)}
              </span>
            </div>
          )}
        </div>
        {!atBottom && (
          <button className="scroll-latest" onClick={scrollBottom}>
            <ArrowDown size={13} />
            {t("Ultimi messaggi")}
          </button>
        )}
      </div>
      {currentPermission && (
        <ToolPermissionCard key={currentPermission.id} permission={currentPermission} busy={permissionBusy}
          answers={questionAnswers} onAnswer={(key, answer) => setQuestionAnswers((previous) => ({ ...previous, [key]: answer }))}
          onDecide={(allow, always) => void decide(allow, always)} />
      )}
      <form
        className="assistant-composer"
        onSubmit={(e) => {
          e.preventDefault();
          void send(prompt);
        }}
      >
        {mention && !sourcesOpen && <MentionList options={mentionOptions} active={mentionIndex} onChoose={chooseMention} />}
        <textarea
          ref={composer}
          aria-label={t("Messaggio all’assistente")}
          placeholder={
            !auth?.logged_in
              ? t("Accedi per iniziare…")
              : t("Scrivi una domanda · @ per le fonti…")
          }
          value={prompt}
          disabled={!ready}
          rows={2}
          aria-autocomplete="list"
          aria-controls={mention ? "source-mentions" : undefined}
          aria-activedescendant={
            mentionOptions[mentionIndex]
              ? `source-option-${mentionIndex}`
              : undefined
          }
          onSelect={(e) =>
            setMention(
              mentionQuery(
                e.currentTarget.value,
                e.currentTarget.selectionStart,
              ),
            )
          }
          onChange={(e) => {
            setPrompt(e.target.value);
            setMention(mentionQuery(e.target.value, e.target.selectionStart));
            draftByCall.current.set(detail.call.id, e.target.value);
            e.currentTarget.style.height = "auto";
            e.currentTarget.style.height =
              Math.min(160, e.currentTarget.scrollHeight) + "px";
          }}
          onKeyDown={(e) => {
            if (mention && !e.nativeEvent.isComposing) {
              if (e.key === "Escape") {
                e.preventDefault();
                setMention(null);
                return;
              }
              if (
                mentionOptions.length &&
                !e.shiftKey &&
                ["ArrowDown", "ArrowUp", "Enter", "Tab"].includes(e.key)
              ) {
                e.preventDefault();
                if (e.key === "Enter" || e.key === "Tab")
                  chooseMention(
                    mentionOptions[
                      Math.min(mentionIndex, mentionOptions.length - 1)
                    ],
                  );
                else {
                  const index =
                    (mentionIndex +
                      (e.key === "ArrowDown" ? 1 : -1) +
                      mentionOptions.length) %
                    mentionOptions.length;
                  setMentionIndex(index);
                  document
                    .getElementById(`source-option-${index}`)
                    ?.scrollIntoView({ block: "nearest" });
                }
                return;
              }
            }
            if (
              e.key === "Enter" &&
              !e.shiftKey &&
              !e.nativeEvent.isComposing
            ) {
              e.preventDefault();
              void send(prompt);
            }
          }}
        />
        {provider === "ollama" && !ollamaServer && <OllamaContext key={detail.call.id} id={detail.call.id} settings={settings} detail={detail}
          liveSegments={status.live_segments} revision={sources.inventory} prompt={prompt} active={active} onSettings={onContextSettings} />}
        <div className="composer-toolbar">
          <select
            aria-label={t("Fornitore AI")}
            value={provider}
            disabled={status.chat_busy || sending}
            onChange={(event) =>
              void onProvider(event.target.value as AssistantProvider).catch(
                (e) => setError(String(e)),
              )
            }
          >
            <option value="claude">Claude Code</option>
            <option value="codex">Codex</option>
            <option value="ollama">Ollama</option>
            {settings?.custom_chat?.model && <option value="custom">{t("Endpoint personalizzato")}</option>}
          </select>
          <select
            aria-label={t("Modello AI")}
            title={modelInfo?.description}
            value={selectedModel}
            disabled={status.chat_busy || !catalog}
            onChange={(e) => void onModel(e.target.value).catch(error => setError(String(error)))}
          >
            {provider === "ollama" && <option value="" disabled>{catalog?.models.length ? t("Scegli un modello") : t("Nessun modello installato")}</option>}
            {provider !== "claude" && provider !== "ollama" && provider !== "custom" && (
              <option value="">{t("Automatico ·") + " "}{providerLabel}</option>
            )}
            {!modelInfo && !(provider !== "claude" && !selectedModel) && (
              <option value={selectedModel}>
                {modelsLoading
                  ? t("Caricamento…")
                  : selectedModel || t("Automatico · {0}", providerLabel)}
              </option>
            )}
            {catalog?.models.map((m) => (
              <option key={m.value} value={m.value}>
                {m.displayName}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="model-refresh"
            title={t("Aggiorna modelli")}
            disabled={modelsLoading || status.chat_busy || (provider !== "ollama" && !auth?.logged_in)}
            onClick={() => {
              if (provider === "ollama") void checkAuth().catch(() => {});
              void loadModels(true);
            }}
          >
            <RefreshCw size={12} className={modelsLoading ? "spin" : ""} />
          </button>
          {runningChat ? (
            <button
              type="button"
              className="send-button"
              aria-label={t("Interrompi risposta")}
              onClick={() =>
                void api
                  .cancelChat(runningChat)
                  .catch((e) => setError(String(e)))
              }
            >
              <Square size={13} fill="currentColor" />
            </button>
          ) : (
            <button
              type="submit"
              className="send-button"
              aria-label={t("Invia messaggio")}
              disabled={!ready || !prompt.trim()}
            >
              <ArrowUp size={17} />
            </button>
          )}
        </div>
        {modelsError && (
          <div className="model-error" role="alert">
            {catalog ? t("Aggiornamento modelli non riuscito. Puoi usare l’elenco precedente.") : t("Catalogo non disponibile.")}{" "}
            <span>{modelsError}</span>{" "}
            <button type="button" disabled={modelsLoading} onClick={() => void loadModels(true)}>
              {t("Riprova")}
            </button>
          </div>
        )}
        {provider === "ollama" && catalog && !catalog.models.length && <p className="model-error">{ollamaServer ? t("Nessun modello installato sul server Ollama.") : t("Installa un modello locale in Ollama, poi aggiorna l’elenco. Nessun modello cloud viene selezionato.")}</p>}
      </form>
      <footer className="widget-footer">
        <button onClick={onAccount}>
          <span
            className={
              auth?.logged_in ? "account-dot connected" : "account-dot"
            }
          />
          {provider === "custom" ? t("Impostazioni endpoint") : provider === "ollama" ? (ollamaServer ? t("Ollama su server") : "Ollama") : auth?.logged_in ? t("Account collegato") : t("Collega account")}
        </button>
        <span title={t("Messaggi e trascrizione sono inviati al modello quando usi la chat. I connettori ricevono i dati delle operazioni che autorizzi.")}>
          {t("Invio per inviare · Shift+Invio a capo")}
        </span>
      </footer>
    </aside>
  );
}
