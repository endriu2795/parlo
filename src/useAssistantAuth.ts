import { useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, desktop } from "./api";
import type { AuthStatus, AssistantProvider } from "./types";

type Provider = AssistantProvider;

function createStore(provider: Provider) {
  let snapshot: { auth: AuthStatus | null; error: string } = { auth: null, error: "" };
  let revision = 0;
  let started = false;
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((listener) => listener());
  const publish = (auth: AuthStatus) => {
    ++revision;
    snapshot = { auth, error: "" };
    notify();
  };
  const run = async (action: () => Promise<AuthStatus>) => {
    const request = ++revision;
    try {
      const auth = await action();
      // A newer login event must win over an older status request or response.
      if (request === revision) {
        snapshot = { auth, error: "" };
        notify();
      }
      return auth;
    } catch (error) {
      if (request === revision) {
        snapshot = { auth: null, error: String(error) };
        notify();
      }
      throw error;
    }
  };
  const refresh = () => run(() => api.auth(provider));
  return {
    getSnapshot: () => snapshot,
    refresh,
    login: (force: boolean) => (provider === "ollama" || provider === "custom") ? refresh() : run(() => api.login(force, provider)),
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      if (desktop && !started) {
        started = true;
        // One subscription per provider, shared by Account and every chat.
        void listen<AuthStatus>(`${provider}-auth-changed`, (event) => publish(event.payload))
          .then(() => { void refresh().catch(() => {}); })
          .catch((error) => {
            snapshot = { auth: null, error: String(error) };
            notify();
          });
        window.addEventListener("focus", () => {
          if (listeners.size && !snapshot.auth?.pending) void refresh().catch(() => {});
        });
      }
      return () => { listeners.delete(listener); };
    },
  };
}

const stores = {
  claude: createStore("claude"),
  codex: createStore("codex"),
  gemini: createStore("gemini"),
  ollama: createStore("ollama"),
  custom: createStore("custom"),
};

export function useAssistantAuth(provider: Provider) {
  const store = stores[provider];
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot);
  return { auth: snapshot.auth, authError: snapshot.error, checkAuth: store.refresh, loginAuth: store.login };
}
