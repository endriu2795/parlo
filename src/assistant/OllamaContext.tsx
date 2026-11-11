import { t, uiLocale } from "../i18n";
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, desktop } from "../api";
import type { Detail, OllamaContextBudget, Segment, Settings } from "../types";

export default function OllamaContext({ id, settings, detail, liveSegments, revision, prompt, active, onSettings }: {
  id: string;
  settings: Settings | null;
  detail: Detail;
  liveSegments: Segment[];
  revision: unknown;
  prompt: string;
  active: boolean;
  onSettings: () => void;
}) {
  const [budget, setBudget] = useState<OllamaContextBudget | null>(null);
  const [error, setError] = useState("");
  const [live, setLive] = useState<{ tokens: number | null; compacted: boolean }>({ tokens: null, compacted: false });
  useEffect(() => {
    let disposed = false;
    if (active || !desktop) return;
    setBudget(null); setError("");
    const timer = setTimeout(() => {
      void api.ollamaContextEstimate(id).then(value => {
        if (!disposed) setBudget(value);
      }).catch(reason => { if (!disposed) setError(String(reason)); });
    }, 450);
    return () => { disposed = true; clearTimeout(timer); };
  }, [id, settings, detail, liveSegments, revision, active]);
  useEffect(() => {
    if (active) setLive({ tokens: null, compacted: false });
  }, [active]);
  useEffect(() => {
    if (!desktop) return;
    let disposed = false;
    const stop = listen<{ id: string; input_tokens?: number; compacted?: boolean }>("ollama-context-status", ({ payload }) => {
      if (disposed || payload.id !== id) return;
      setLive(previous => ({ tokens: payload.input_tokens ?? previous.tokens, compacted: previous.compacted || !!payload.compacted }));
    });
    return () => { disposed = true; void stop.then(unlisten => unlisten()); };
  }, [id]);

  const limit = budget?.limit ?? settings?.ollama_context_length ?? 32768;
  const actual = live.tokens !== null;
  const toolsEnabled = settings?.ollama_tools_enabled !== false;
  const total = actual ? live.tokens! : budget
    ? budget.input_tokens + Math.ceil(Array.from(prompt.trim()).length / 2) : null;
  const occupied = total === null ? null : total + (budget?.reserved_tokens ?? 4096);
  const over = occupied !== null && occupied > limit;
  const near = occupied !== null && occupied >= limit * 0.85;
  const format = (value: number) => value.toLocaleString(uiLocale);
  return <div className={`ollama-context ${over ? "over" : near ? "near" : ""}`}>
    <div className="ollama-context-label" title={actual
      ? t("Token di input comunicati dal backend nell’ultimo passaggio. Per l’avviso si riservano anche 4.096 token per la risposta.")
      : toolsEnabled ? t("Stima del solo testo: trascrizione, cronologia, istruzioni e domanda. Gli strumenti non sono conteggiati qui; il totale reale compare quando il modello comunica i token. La barra riserva anche 4.096 token per la risposta. La stima non blocca l’invio.") : t("Stima del testo inviato. Gli strumenti sono disattivati. La barra riserva anche 4.096 token per la risposta.")}>
      <span>{actual ? t("Input ultimo passaggio") : toolsEnabled ? t("Testo stimato · strumenti esclusi") : t("Testo stimato · strumenti disattivati")}</span>
      <span>{total === null ? t("Limite {0} token", format(limit)) : t("{0}{1} / {2} token", actual ? "" : "≈ ", format(total), format(limit))}</span>
    </div>
    {occupied !== null && <progress aria-label={actual ? t("Contesto ultimo passaggio con riserva risposta") : t("Testo stimato con riserva risposta")} max={limit} value={Math.min(occupied, limit)} />}
    {!actual && toolsEnabled && <p>{t("Gli strumenti saranno inclusi nel conteggio reale. La stima non blocca l’invio.")}</p>}
    {error && <p role="status">{t("Stima non disponibile. Puoi comunque inviare la domanda.")}</p>}
    {over ? <p role="alert">{actual ? t("Poco spazio per la risposta nell’ultimo passaggio. La CLI può compattare la cronologia; se il contesto non basta, la richiesta può fallire.") : t("Il testo stimato supera lo spazio disponibile per la risposta. Puoi inviare comunque: la CLI gestisce il contesto e può compattare la cronologia o restituire un errore. PARLO non taglia la trascrizione.")}</p>
      : near && <p role="status">{t("Spazio ridotto per la risposta. Puoi comunque inviare; aumentare il contesto lascia più margine.")}</p>}
    <button type="button" className="widget-link" onClick={onSettings}>{t("Impostazioni contesto")}</button>
    {live.compacted && <p role="status">{t("La CLI ha compattato la cronologia in un riepilogo: alcuni dettagli precedenti possono essere persi. La trascrizione originale resta integra.")}</p>}
  </div>;
}
