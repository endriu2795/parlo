import { Fragment, memo } from "react";

function folded(text: string) {
  let value = "", offset = 0;
  const starts: number[] = [], ends: number[] = [];
  for (const character of text) {
    const normalized = character.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
    value += normalized;
    for (let i = 0; i < normalized.length; i++) { starts.push(offset); ends.push(offset + character.length); }
    if (!normalized && ends.length) ends[ends.length - 1] = offset + character.length;
    offset += character.length;
  }
  return {value, starts, ends};
}

export function searchRanges(text: string, query: string, substring = false): [number, number][] {
  if (!query.trim()) return [];
  const normalized = folded(text);
  const needle = folded(query.trim()).value;
  const ranges: [number, number][] = [];
  if (substring) {
    let at = 0;
    while (needle && (at = normalized.value.indexOf(needle, at)) >= 0) {
      ranges.push([normalized.starts[at], normalized.ends[at + needle.length - 1]]);
      at += needle.length;
    }
    return ranges;
  }
  const words = needle.match(/[\p{L}\p{N}]+/gu);
  if (!words?.length) return [];
  // Match FTS phrase-prefix queries: previous words are complete, the final word may be partial.
  const pattern = new RegExp(`(?<![\\p{L}\\p{N}])${words.join("[^\\p{L}\\p{N}]+")}`, "gu");
  for (const match of normalized.value.matchAll(pattern)) {
    const at = match.index!;
    ranges.push([normalized.starts[at], normalized.ends[at + match[0].length - 1]]);
  }
  return ranges;
}

export const HighlightedText = memo(function HighlightedText({text, query, passage = false, substring = false}: {text: string; query: string; passage?: boolean; substring?: boolean}) {
  const ranges = searchRanges(text, query, substring);
  if (!ranges.length) return <>{text}</>;
  let end = 0;
  return <>{ranges.map(([start, stop], index) => {
    const previous = end; end = stop;
    return <Fragment key={index}>{text.slice(previous, start)}<mark data-search-hit={passage ? "true" : undefined}>{text.slice(start, stop)}</mark></Fragment>;
  })}{text.slice(end)}</>;
});

// Render FTS markers as React nodes; excerpts are never interpreted as HTML.
export function SearchExcerpt({text}: {text: string}) {
  let marked = false;
  return <>{text.split(/([\ue000\ue001])/).map((part, i) => {
    if (part === "\ue000") { marked = true; return null; }
    if (part === "\ue001") { marked = false; return null; }
    return marked ? <mark key={i}>{part}</mark> : <Fragment key={i}>{part}</Fragment>;
  })}</>;
}
