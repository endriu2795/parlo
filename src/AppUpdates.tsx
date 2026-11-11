import { t } from "./i18n";
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RefreshCw, X } from "lucide-react";

interface UpdateStatus {
  current_version: string;
  version: string | null;
  notes: string | null;
  phase: string;
  downloaded: number;
  total: number | null;
  error: string | null;
  notice: string | null;
}

export function useAppUpdates(enabled: boolean) {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [error, setError] = useState("");
  const [dismissed, setDismissed] = useState("");
  const inFlight = useRef(false), lastAttempt = useRef(0), retried = useRef(false);
  const latest = useRef<UpdateStatus | null>(null);
  latest.current = status;
  const check = useCallback(async () => {
    if (!enabled || !latest.current || latest.current.phase === "disabled" || inFlight.current) return;
    inFlight.current = true;
    lastAttempt.current = Date.now();
    setError("");
    try {
      setStatus(await invoke<UpdateStatus>("check_update"));
    } catch {
      setError(t("Impossibile verificare gli aggiornamenti. Riprova tra poco."));
    } finally {
      inFlight.current = false;
    }
  }, [enabled]);
  const refreshIfStale = useCallback(() => {
    const age = Date.now() - lastAttempt.current;
    if (enabled && !document.hidden && age >= (latest.current?.notice ? 30_000 : 5 * 60_000)) void check();
  }, [enabled, check]);
  useEffect(() => {
    if (!enabled) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    let timer: ReturnType<typeof setInterval> | undefined;
    void (async () => {
      try {
        const unlisten = await listen<UpdateStatus>("update-status", ({ payload }) => {
          if (!disposed) setStatus(payload);
        });
        if (disposed) { unlisten(); return; }
        stop = unlisten;
        const initial = await invoke<UpdateStatus>("update_status");
        if (disposed) return;
        latest.current = initial;
        setStatus(initial);
        if (initial.phase === "disabled") return;
        await check();
        if (!disposed) timer = setInterval(() => void check(), 6 * 60 * 60 * 1000);
      } catch {
        if (!disposed) setError(t("Impossibile caricare lo stato degli aggiornamenti. Riprova tra poco."));
      }
    })();
    return () => { disposed = true; stop?.(); clearInterval(timer); };
  }, [enabled, check]);
  useEffect(() => {
    if (!enabled) return;
    window.addEventListener("focus", refreshIfStale);
    window.addEventListener("online", refreshIfStale);
    document.addEventListener("visibilitychange", refreshIfStale);
    return () => {
      window.removeEventListener("focus", refreshIfStale);
      window.removeEventListener("online", refreshIfStale);
      document.removeEventListener("visibilitychange", refreshIfStale);
    };
  }, [enabled, refreshIfStale]);
  useEffect(() => {
    if (!enabled) return;
    if (!status?.notice) {
      if (status?.phase === "current" || status?.phase === "available") retried.current = false;
      return;
    }
    if (retried.current) return;
    const timer = setTimeout(() => { retried.current = true; void check(); }, 15_000);
    return () => clearTimeout(timer);
  }, [enabled, status?.notice, status?.phase, check]);
  const install = async () => {
    setError("");
    try {
      await invoke("install_update");
    } catch (error) {
      setError(String(error));
    }
  };
  const working = !!status && ["checking", "downloading", "installing"].includes(status.phase);
  const available = !!status?.version;
  return {
    status, error: error || status?.error || "", working, available,
    visible: available && (dismissed !== status?.version || working),
    check, refreshIfStale, install, dismiss: () => setDismissed(status?.version || ""),
  };
}

export type AppUpdates = ReturnType<typeof useAppUpdates>;

function message(updates: AppUpdates, busy: boolean) {
  const status = updates.status;
  if (!status) return t("Controllo aggiornamenti…");
  if (status.phase === "disabled") return t("Questa build non ha un canale di aggiornamento.");
  if (status.phase === "checking") return t("Verifica aggiornamenti in corso…");
  if (status.phase === "downloading") {
    const amount = status.total
      ? `${Math.min(100, Math.round(status.downloaded / status.total * 100))}%`
      : `${Math.round(status.downloaded / 1048576)} MB`;
    return t("Download aggiornamento · {0}", amount);
  }
  if (status.phase === "installing") return t("Installazione e riavvio dell’app…");
  if (updates.available && busy) return t("Potrai aggiornare al termine delle attività in corso.");
  if (status.phase === "ready") return t("Aggiornamento scaricato. Puoi installarlo e riavviare l’app.");
  if (updates.available) return t("Installa l’aggiornamento e riavvia l’app.");
  if (status.phase === "waiting_platform") return t("L’aggiornamento per questo sistema non è ancora pronto.");
  if (status.phase === "current") return t("Stai usando l’ultima versione disponibile.");
  return t("Gli aggiornamenti vengono cercati automaticamente all’avvio e ogni 6 ore.");
}

export function UpdateControls({ updates, busy }: { updates: AppUpdates; busy: boolean }) {
  useEffect(() => { updates.refreshIfStale(); }, [updates.refreshIfStale]);
  return <div className="app-updates" aria-live="polite">
    <p>{t("Versione installata:") + " "}<strong>{updates.status?.current_version || "…"}</strong></p>
    {updates.available && <p>{t("Nuova versione:") + " "}<strong>{updates.status?.version}</strong></p>}
    <p>{message(updates, busy)}</p>
    {updates.status?.notice && <p className="settings-hint">{updates.status.notice}</p>}
    {updates.error && <p className="model-error" role="alert">{updates.error}</p>}
    {updates.status && updates.status.phase !== "disabled" && <div className="update-actions">
      <button className="secondary-button" disabled={updates.working || updates.status?.phase === "ready"} onClick={() => void updates.check()}>
        <RefreshCw size={15} /> {t("Verifica aggiornamenti")}
      </button>
      {updates.available && <button className="primary-button" disabled={updates.working || busy} onClick={() => void updates.install()}>
        {updates.status?.phase === "ready" ? t("Installa e riavvia") : t("Aggiorna e riavvia")}
      </button>}
    </div>}
  </div>;
}

export function UpdateBanner({ updates, busy }: { updates: AppUpdates; busy: boolean }) {
  if (!updates.visible) return null;
  return <div className="banner engine-progress update-banner" role="status">
    <div>
      <strong>PARLO {updates.status?.version} {t("disponibile")}</strong>
      <p>{message(updates, busy)}</p>
      {updates.status?.notice && <p className="settings-hint">{updates.status.notice}</p>}
      {updates.error && <p className="model-error">{updates.error}</p>}
    </div>
    <button className="secondary-button" disabled={updates.working || busy} onClick={() => void updates.install()}>
      {updates.working ? t("Aggiornamento…") : t("Aggiorna e riavvia")}
    </button>
    {!updates.working && <button className="icon-button" aria-label={t("Ricordamelo più tardi")} onClick={updates.dismiss}><X size={16} /></button>}
  </div>;
}
