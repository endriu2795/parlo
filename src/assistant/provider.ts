import type { AssistantProvider, Settings } from "../types";

/** Per-provider settings, so the panel does not repeat the same four-way switch. */
export function providerConfig(settings: Settings | null) {
  const provider: AssistantProvider = settings?.assistant_provider || "claude";
  const pick = <T,>(values: Record<AssistantProvider, T>) => values[provider];
  return {
    provider,
    label: pick({ claude: "Claude", codex: "Codex", gemini: "Antigravity", ollama: "Ollama", custom: "Endpoint" }),
    connectorsEnabled: pick({
      claude: settings?.claude_connectors !== false,
      codex: settings?.codex_connectors !== false,
      gemini: settings?.gemini_connectors !== false,
      ollama: false,
      custom: false,
    }),
    disabledConnectors: pick({
      claude: settings?.disabled_connectors,
      codex: settings?.codex_disabled_connectors,
      gemini: settings?.gemini_disabled_connectors,
      ollama: settings?.disabled_connectors,
      custom: undefined,
    }),
    trustedTools: pick({
      claude: settings?.trusted_tools,
      codex: settings?.codex_trusted_tools,
      gemini: settings?.gemini_trusted_tools,
      ollama: settings?.ollama_server_url ? settings.ollama_server_trusted_tools : settings?.ollama_trusted_tools,
      custom: undefined,
    }),
    model: pick({
      claude: settings?.claude_model || "sonnet",
      codex: settings?.codex_model || "",
      gemini: settings?.gemini_model || "",
      ollama: settings?.ollama_model || "",
      custom: settings?.custom_chat?.model || "",
    }),
  };
}
