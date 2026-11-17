import { Fragment, useEffect, useLayoutEffect, useRef, type RefObject } from "react";
import { MENTION_PATTERN } from "../sourceMentions";

/**
 * Mirror of the composer text drawn behind a transparent textarea, so @mentions
 * look like links while the user keeps editing plain text. Fonts, padding and
 * wrapping must match the textarea (see .composer-input in assistant.css).
 */
export default function ComposerHighlight({ text, target }: { text: string; target: RefObject<HTMLTextAreaElement | null> }) {
  const layer = useRef<HTMLDivElement>(null);
  const sync = () => {
    if (layer.current && target.current) layer.current.scrollTop = target.current.scrollTop;
  };
  useEffect(() => {
    const box = target.current;
    if (!box) return;
    box.addEventListener("scroll", sync);
    return () => box.removeEventListener("scroll", sync);
  }, [target]);
  useLayoutEffect(sync, [text]);
  const parts: { text: string; mention?: true }[] = [];
  let cursor = 0;
  for (const match of text.matchAll(MENTION_PATTERN)) {
    if (match.index > cursor) parts.push({ text: text.slice(cursor, match.index) });
    parts.push({ text: match[0], mention: true });
    cursor = match.index + match[0].length;
  }
  if (cursor < text.length) parts.push({ text: text.slice(cursor) });
  return (
    <div ref={layer} className="composer-highlight" aria-hidden="true">
      {parts.map((part, index) => part.mention
        ? <mark key={index}>{part.text}</mark>
        : <Fragment key={index}>{part.text}</Fragment>)}
      {/* Keeps a trailing newline as tall as in the textarea. */}
      {"​"}
    </div>
  );
}
