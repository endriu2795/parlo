import { invoke, isTauri } from "@tauri-apps/api/core";
import english from "./messages.en.json";

type Message = keyof typeof english;

function languageOf(locale: string | undefined) {
  return locale?.split(/[-_]/)[0]?.toLowerCase();
}

export let uiLocale = "en";

// All windows load this before their view modules, including recording overlays.
export async function initializeLocale() {
  const browserLocale = navigator.language;
  const systemLanguage = isTauri()
    ? await invoke<"it" | "en">("ui_language").catch(() => undefined)
    : undefined;
  const browserLanguage = languageOf(browserLocale);
  const language = systemLanguage ?? (browserLanguage === "it" ? "it" : "en");
  uiLocale = browserLanguage === language ? browserLocale : language;
  document.documentElement.lang = language;
}

// Italian source messages are the keys. Numbered placeholders can be reordered.
export function t(message: Message, ...values: unknown[]): string {
  const text = languageOf(uiLocale) === "it" ? message : english[message];
  return values.length ? text.replace(/\{(\d+)\}/g, (_, index) => String(values[Number(index)])) : text;
}
