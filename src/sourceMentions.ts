import type { ChatSource } from "./types";

export function mentionQuery(text: string, caret: number) {
  const before = text.slice(0, caret);
  const match = /(?:^|\s)@([^@\n«»]{0,100})$/.exec(before);
  return match
    ? {
        start: before.lastIndexOf("@"),
        end: caret,
        query: match[1].toLocaleLowerCase(),
      }
    : null;
}
export function matchingSources(sources: ChatSource[], query: string) {
  return sources
    .filter(
      (source) =>
        source.enabled &&
        source.available &&
        !source.unsupported_reason &&
        `${source.name} ${source.path || ""} ${source.scope === "shared" ? "condivise" : "chat"}`
          .toLocaleLowerCase()
          .includes(query.trim().toLocaleLowerCase()),
    )
    .sort((a, b) => Number(a.scope === "shared") - Number(b.scope === "shared"))
    .slice(0, 40);
}
export function insertMention(
  text: string,
  query: { start: number; end: number },
  source: Pick<ChatSource, "mention">,
) {
  const insertion = `${source.mention} `;
  return {
    text:
      text.slice(0, query.start) +
      insertion +
      text.slice(query.end).replace(/^ /, ""),
    caret: query.start + insertion.length,
  };
}

// Archive calls are cited as @«Title · dd/mm/yyyy». The backend rebuilds the same
// label in local time to find the call (sources::call_mention), so keep both in sync.
const pad = (value: number) => String(value).padStart(2, "0");
export function callMention(call: { title: string; created_at: string }) {
  const date = new Date(call.created_at);
  const title = call.title.replace(/[«»\s]+/g, " ").trim() || "Call";
  const day = Number.isNaN(date.getTime())
    ? ""
    : ` · ${pad(date.getDate())}/${pad(date.getMonth() + 1)}/${date.getFullYear()}`;
  return `@«${title}${day}»`;
}
export const isCallMention = (mention: string) => / · \d{2}\/\d{2}\/\d{4}»$/.test(mention);
export const MENTION_PATTERN = /@«[^«»\r\n]+»/g;

/** Mention ending exactly at the caret, so Backspace can delete it as a whole. */
export function mentionBefore(text: string, caret: number) {
  const match = /@«[^«»\r\n]+»$/.exec(text.slice(0, caret));
  return match ? { start: match.index, end: caret } : null;
}

export function splitSourceMentions(text: string, sources: ChatSource[]) {
  const mentions = new Map<string, ChatSource | null>();
  for (const source of sources) {
    if (mentions.has(source.mention)) mentions.set(source.mention, null);
    else mentions.set(source.mention, source);
  }
  const parts: { text: string; source?: ChatSource; call?: true }[] = [];
  let cursor = 0;
  for (const match of text.matchAll(MENTION_PATTERN)) {
    const source = mentions.get(match[0]);
    const call = !source && isCallMention(match[0]);
    if (!call && (!source?.path || !source.available)) continue;
    if (match.index > cursor) parts.push({ text: text.slice(cursor, match.index) });
    parts.push(source ? { text: match[0], source } : { text: match[0], call: true });
    cursor = match.index + match[0].length;
  }
  if (cursor < text.length) parts.push({ text: text.slice(cursor) });
  return parts;
}
