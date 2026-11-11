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
  source: ChatSource,
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

export function splitSourceMentions(text: string, sources: ChatSource[]) {
  const mentions = new Map<string, ChatSource | null>();
  for (const source of sources) {
    if (mentions.has(source.mention)) mentions.set(source.mention, null);
    else mentions.set(source.mention, source);
  }
  const parts: { text: string; source?: ChatSource }[] = [];
  let cursor = 0;
  for (const match of text.matchAll(/@«[^«»\r\n]+»/g)) {
    const source = mentions.get(match[0]);
    if (!source?.path || !source.available) continue;
    if (match.index > cursor) parts.push({ text: text.slice(cursor, match.index) });
    parts.push({ text: match[0], source });
    cursor = match.index + match[0].length;
  }
  if (cursor < text.length) parts.push({ text: text.slice(cursor) });
  return parts;
}
