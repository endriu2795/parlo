// UI smoke test against the Vite dev server (npm run dev) with a mocked Tauri backend.
// Usage: node scripts/ui-check.cjs [--screenshots]   ·   UI_CHECK_CHANNEL=msedge to use Edge.
const { chromium } = require("playwright");
const assert = require("node:assert/strict");
const fs = require("node:fs");

const URL = process.env.UI_CHECK_URL || "http://127.0.0.1:1420";
const screenshots = process.argv.includes("--screenshots");

// Runs in the page before the app loads: a minimal in-memory backend.
function mockTauri() {
  let counter = 0;
  const callbacks = new Map(), listeners = new Map();
  const emit = (event, payload) => {
    for (const [id, l] of listeners) if (l.event === event) callbacks.get(l.handler)?.({ event, id, payload });
  };
  const calls = Array.from({ length: 14 }, (_, i) => ({
    id: "test" + i,
    title: "Conversazione " + i,
    created_at: new Date(Date.UTC(2026, 9, 20 - i, 10)).toISOString(),
    duration_ms: 65000 + i * 1000,
    status: i === 1 ? "failed" : "ready",
    error: i === 1 ? "Trascrizione non riuscita." : null,
    preview: "Un testo di prova.",
    tags: i % 3 === 0 ? ["Cliente"] : i % 3 === 1 ? ["Interno", "Da rivedere"] : [],
  }));
  const segments = [
    { start_ms: 1000, end_ms: 3000, speaker: "Tu", text: "Proviamo il salvataggio della call.", source: "microphone" },
    { start_ms: 4000, end_ms: 8000, speaker: "Speaker A", text: "La demo sarà pronta venerdì.", source: "system" },
  ];
  const settings = {
    theme: new URLSearchParams(location.search).get("theme") || "dark",
    retention_months: 3, microphone: true, system_audio: true, shortcut: "Ctrl+Shift+Space", language: "it",
    whisper_model: "/engine/models/whisper-turbo", live_whisper_model: "/engine/models/whisper-tiny", live_transcription: false,
    stt_provider: "local", assistant_provider: "claude", claude_bin: "claude", claude_model: "sonnet", claude_connectors: true,
    disabled_connectors: [], trusted_tools: [], project_instructions: "", completion_notifications: true, tts_enabled: false,
    diarization: false, system_prompt: "",
  };
  const status = {
    capture: new URLSearchParams(location.search).has("overlay")
      ? { id: "test0", started_at: new Date().toISOString(), microphone: true, system_audio: true } : null,
    processing: null, queued: [], stage: "", chat_busy: false, chat_call: null, live_segments: [], live_stage: "",
  };
  const lower = s => s.toLowerCase();
  window.isTauri = true;
  window.__testCalls = [];
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    transformCallback: fn => { const id = ++counter; callbacks.set(id, fn); return id; },
    invoke: async (command, args = {}) => {
      window.__testCalls.push({ command, args });
      switch (command) {
        case "ui_language": return "it";
        case "list_calls": {
          const rows = calls.filter(c =>
            (!args.query || lower(c.title).includes(lower(args.query))) &&
            (!args.tags?.length || c.tags.some(t => args.tags.some(f => lower(f) === lower(t)))));
          return { items: rows.slice(args.offset, args.offset + args.limit), total: rows.length, has_more: args.offset + args.limit < rows.length, search_results: {} };
        }
        case "list_tags": {
          const counts = new Map();
          calls.forEach(c => c.tags.forEach(t => counts.set(t, (counts.get(t) || 0) + 1)));
          return [...counts].map(([tag, count]) => ({ tag, count })).sort((a, b) => b.count - a.count);
        }
        case "get_call": {
          const call = calls.find(c => c.id === args.id);
          if (!call) throw Error("Missing");
          return { call, segments, messages: [], notes: [], screenshots: [] };
        }
        case "get_settings": return structuredClone(settings);
        case "save_settings": Object.assign(settings, args.settings); emit("theme-changed", settings.theme); return;
        case "set_theme": settings.theme = args.theme; emit("theme-changed", args.theme); return structuredClone(settings);
        case "get_status": return status;
        case "recorder_dock_status": return null;
        case "get_health":
          return {
            windows_capture: false, shortcut_ready: true, whisper_ready: true, claude_ready: true, data_dir: "test", python_available: false,
            engine_root: "/engine",
            models: ["turbo", "small", "tiny"].map(id => ({ id, path: "/engine/models/whisper-" + id, ready: id === "turbo", preview_ready: id === "tiny", download_size: "circa 500 MB" })),
          };
        case "engine_setup_status": return { running: false, message: "", error: null, log: [] };
        case "claude_auth_status": return { installed: true, installing: false, logged_in: true, pending: false, url: null, error: null };
        case "claude_models": return { models: [{ value: "sonnet", displayName: "Sonnet", description: "", resolvedModel: "claude-sonnet" }], checked_at: new Date().toISOString(), cli_version: "test" };
        case "claude_connectors": return [{ name: "claude.ai Drive test", status: "connected", tools: 3 }];
        case "chat_sources": return { sources: [], generated: [], shared_folder: null };
        case "update_status": return { state: "idle" };
        case "plugin:event|listen": { const id = ++counter; listeners.set(id, args); return id; }
        case "plugin:event|unlisten": listeners.delete(args.eventId); return;
        default: return command.startsWith("plugin:event|") ? ++counter : null;
      }
    },
  };
}

(async () => {
  const browser = await chromium.launch({ headless: true, channel: process.env.UI_CHECK_CHANNEL || undefined });
  const errors = [];
  const open = async (query = "", viewport = { width: 1280, height: 820 }) => {
    const page = await browser.newPage({ viewport });
    page.setDefaultTimeout(8000);
    page.on("pageerror", e => errors.push(`${query || "/"}: ${e.message}`));
    await page.addInitScript(mockTauri);
    await page.goto(`${URL}/${query}`, { waitUntil: "domcontentloaded" });
    return page;
  };
  const shot = async (page, name) => {
    if (!screenshots) return;
    fs.mkdirSync(".tools/qa", { recursive: true });
    await page.screenshot({ path: `.tools/qa/${name}.png` });
  };
  const step = async (name, fn) => {
    try { await fn(); console.log(`✓ ${name}`); }
    catch (e) { console.error(`✗ ${name}\n  ${e.message.split("\n")[0]}`); process.exitCode = 1; }
  };

  for (const theme of ["dark", "light"]) {
    const page = await open(`?theme=${theme}`);
    await step(`[${theme}] archivio e documento`, async () => {
      await page.getByRole("navigation", { name: "Trascrizioni" }).getByText("Conversazione 0").waitFor();
      await page.getByRole("navigation", { name: "Trascrizioni" }).getByText("Conversazione 0").click();
      await page.getByText("La demo sarà pronta venerdì.").waitFor();
      await page.getByRole("complementary", { name: "Chat della call" }).waitFor();
      await shot(page, `document-${theme}`);
    });
    await step(`[${theme}] filtri per tag e periodo`, async () => {
      await page.getByRole("button", { name: "Filtra per tag e date" }).click();
      const filters = page.getByRole("region", { name: "Filtri dell’archivio" });
      await filters.getByRole("button", { name: /^Interno/ }).click();
      await page.waitForFunction(() => window.__testCalls.some(c => c.command === "list_calls" && c.args.tags?.includes("Interno")));
      await filters.getByRole("button", { name: "Ultimi 7 giorni" }).click();
      await shot(page, `filters-${theme}`);
      await page.getByRole("button", { name: "Chiudi filtri" }).click();
      await page.getByRole("button", { name: "Rimuovi filtro tag Interno" }).click();
      assert.equal(await page.getByLabel("Filtri attivi").getByText("Ultimi 7 giorni").count(), 1);
    });
    await step(`[${theme}] scorciatoia ricerca`, async () => {
      await page.keyboard.press("ControlOrMeta+K");
      assert.equal(await page.evaluate(() => document.activeElement?.getAttribute("aria-label")), "Cerca nelle trascrizioni");
    });
    await step(`[${theme}] impostazioni`, async () => {
      await page.evaluate(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" })));
      await page.locator("button", { hasText: "Impostazioni" }).first().click();
      const dialog = page.getByRole("dialog");
      await dialog.getByRole("heading", { name: "Impostazioni" }).waitFor();
      for (const tab of await dialog.getByRole("tab").all()) {
        await tab.click();
        await dialog.getByRole("tabpanel").waitFor();
      }
      await dialog.getByRole("tab", { name: /Voce/ }).click();
      assert.equal(await dialog.getByText("ElevenLabs").count(), 0, "provider voce visibili con sintesi spenta");
      await shot(page, `settings-${theme}`);
      await dialog.getByRole("button", { name: "Chiudi impostazioni" }).click();
    });
    await page.close();
  }

  const overlay = await open("?overlay", { width: 440, height: 188 });
  await step("indicatore di registrazione", async () => {
    await overlay.getByRole("button", { name: "Termina registrazione" }).waitFor();
    await shot(overlay, "recorder");
  });

  await browser.close();
  if (errors.length) { console.error("Errori nella pagina:\n" + errors.join("\n")); process.exitCode = 1; }
})();
