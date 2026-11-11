import { t } from "./i18n";
import ChatMarkdown from "./ChatMarkdown";
import ChatActivity from "./ChatActivity";
import AssistantIdentity from "./AssistantIdentity";
import type { ChatActivityEntry } from "./types";

export const isChatText = (entry: ChatActivityEntry) => entry.kind === "text";
export function ChatText({
  text,
  streaming = false,
}: {
  text: string;
  streaming?: boolean;
}) {
  return (
    <article
      className={`assistant-message assistant ${streaming ? "streaming" : ""}`}
    >
      <AssistantIdentity streaming={streaming}/>
      <div className="message-markdown">
        <ChatMarkdown>{text}</ChatMarkdown>
      </div>
    </article>
  );
}
export default function ChatTimeline({
  entries,
  live = false,
  finalAnswer,
}: {
  entries: ChatActivityEntry[];
  live?: boolean;
  finalAnswer?: string;
}) {
  const groups: ChatActivityEntry[][] = [];
  for (const entry of entries) {
    if (isChatText(entry) || !groups.length || isChatText(groups.at(-1)![0]))
      groups.push([entry]);
    else groups.at(-1)!.push(entry);
  }
  return (
    <div className="chat-timeline" aria-label={t("Sequenza della risposta")}>
      {groups.map((group) => {
        const first = group[0];
        if (isChatText(first)) {
          const text =
            first.id === "final-answer" && first.truncated && finalAnswer
              ? finalAnswer
              : first.text;
          return (
            <div key={first.id} data-chat-step="text">
              {text && (
                <ChatText
                  text={text}
                  streaming={live && first.status === "running"}
                />
              )}
              {first.truncated && first.id !== "final-answer" && (
                <p className="timeline-note">{t("Testo intermedio abbreviato.")}</p>
              )}
            </div>
          );
        }
        return (
          <div key={first.id} data-chat-step="activity">
            <ChatActivity entries={group} live={live} />
          </div>
        );
      })}
    </div>
  );
}
