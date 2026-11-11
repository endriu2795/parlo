import { t } from "./i18n";
import { useState } from "react";
import {
  Check,
  ChevronDown,
  CircleAlert,
  LoaderCircle,
  Plug,
  X,
} from "lucide-react";
import type { ChatActivityEntry } from "./types";

export function toolLabel(tool: string) {
  if (tool === "antigravity_tool" || tool === "gemini_tool") return t("Strumento Antigravity");
  if (tool === "ToolSearch") return t("Ricerca degli strumenti");
  return tool
    .replace(/^mcp__/, "")
    .split("__")
    .map((s) => s.replace(/^claude_ai_/, "").replace(/_/g, " "))
    .join(" · ");
}
const states: Record<string, string> = {
  preparing: t("Preparazione"),
  waiting: t("Attende il tuo consenso"),
  running: t("In corso"),
  complete: t("Completato"),
  error: t("Errore"),
  denied: t("Non consentito"),
  cancelled: t("Interrotto"),
  unknown: t("Esito non comunicato"),
};
export default function ChatActivity({
  entries,
  live = false,
}: {
  entries: ChatActivityEntry[];
  live?: boolean;
}) {
  const [expanded, setExpanded] = useState<boolean | null>(null);
  if (!entries.length) return null;
  const current = [...entries]
    .reverse()
    .find((e) => ["preparing", "waiting", "running"].includes(e.status));
  const open = expanded ?? live;
  const lastTool = [...entries].reverse().find((entry) => entry.tool);
  return (
    <section
      className={`chat-activity ${live ? "live" : ""}`}
      aria-label={t("Attività della risposta")}
    >
      <button
        className="activity-toggle"
        aria-expanded={open}
        onClick={() => setExpanded(!open)}
      >
        <span>
          {live
            ? current?.tool
              ? toolLabel(current.tool)
              : current?.label || t("Elaborazione della richiesta")
            : lastTool?.tool
              ? toolLabel(lastTool.tool)
              : t("Attività della risposta")}
        </span>
        <small>
          {entries.length} {entries.length === 1 ? "passaggio" : "passaggi"}
        </small>
        <ChevronDown size={13} className={open ? "rotated" : ""} />
      </button>
      {open && (
        <ol className="activity-steps">
          {entries.map((entry) => {
            const busy = ["preparing", "running"].includes(entry.status);
            return (
              <li key={entry.id} className={`activity-step ${entry.status}`}>
                <span className="activity-icon">
                  {busy ? (
                    <LoaderCircle size={13} className="spin" />
                  ) : entry.status === "complete" ? (
                    <Check size={13} />
                  ) : ["error", "unknown"].includes(entry.status) ? (
                    <CircleAlert size={13} />
                  ) : ["denied", "cancelled"].includes(entry.status) ? (
                    <X size={13} />
                  ) : (
                    <Plug size={13} />
                  )}
                </span>
                <div className="activity-content">
                  {entry.kind === "reasoning" ? (
                    <details className="activity-reasoning">
                      <summary>
                        {entry.label}
                        <ChevronDown size={11} />
                      </summary>
                      <p className="reasoning-note">
                        {t("Riepilogo condiviso da Claude")}
                      </p>
                      <div className="reasoning-text">
                        {entry.text || t("Riepilogo in arrivo…")}
                      </div>
                    </details>
                  ) : (
                    <span
                      className="activity-label"
                      title={entry.tool || undefined}
                    >
                      {entry.tool ? toolLabel(entry.tool) : entry.label}
                    </span>
                  )}
                  <small>
                    {states[entry.status] || entry.status}
                    {entry.elapsed_seconds != null
                      ? ` · ${Math.round(entry.elapsed_seconds)} s`
                      : ""}
                  </small>
                  {entry.kind === "tool" && entry.status === "error" && entry.text && <p className="activity-error">{entry.text}</p>}
                </div>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}
