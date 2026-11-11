import { annotationBounds, shapeKinds, transformAnnotation } from "./annotations";
import type { Annotation, Point } from "./annotations";

export const annotationMime = "application/x-parlo-annotation+json";
export const legacyAnnotationMime = "application/x-presentio-annotation+json";
const prefix = "parlo-annotation:v1";
// System clipboards can replace LF with CRLF when transferring plain text.
const header = /^(?:parlo|presentio)-annotation:v1(?=[\r\n]|$)/;
export const hasAnnotationPrefix = (text: string) => header.test(text);
export type CopiedAnnotation = { version: 1; width: number; height: number; item: Annotation };
export const encodeAnnotation = (value: CopiedAnnotation) => prefix + "\n" + JSON.stringify(value);
export const annotationPlainText = (value: CopiedAnnotation) => value.item.kind === "text" ? value.item.text : encodeAnnotation(value);
const finite = (n: unknown): n is number => typeof n === "number" && Number.isFinite(n);
const fraction = (n: unknown): n is number => finite(n) && n >= 0 && n <= 1;
const positive = (n: unknown): n is number => fraction(n) && n > 0;
const point = (p: unknown): p is Point => Array.isArray(p) && p.length === 2 && p.every(fraction);

// Clipboard contents may come from other applications. Accept only our bounded
// data format; never execute HTML or scripts from pasted content.
export function decodeAnnotation(text: string): CopiedAnnotation | null {
  if (text.length > 1_000_000) return null;
  const match = header.exec(text);
  if (!match) return null;
  try {
    const value = JSON.parse(text.slice(match[0].length));
    if (value?.version !== 1 || !finite(value.width) || !finite(value.height)
      || value.width <= 0 || value.height <= 0 || value.width > 100_000 || value.height > 100_000) return null;
    const item = value.item;
    if (!item || typeof item.color !== "string" || !/^#[0-9a-f]{6}$/i.test(item.color)) return null;
    if (item.kind === "text") {
      if (!fraction(item.x) || !fraction(item.y) || !positive(item.width) || !positive(item.fontSize)
        || typeof item.text !== "string" || !item.text.trim() || item.text.length > 4000) return null;
    } else {
      if (!positive(item.width) || item.width > .1) return null;
      if (item.kind === "brush") {
        if (!Array.isArray(item.points) || !item.points.length || item.points.length > 10_000 || !item.points.every(point)) return null;
      } else if (item.kind === "shape") {
        if (!shapeKinds.includes(item.shape) || !point(item.start) || !point(item.end) || typeof item.filled !== "boolean") return null;
      } else return null;
    }
    return value as CopiedAnnotation;
  } catch { return null; }
}

export function pastedAnnotation(copy: CopiedAnnotation, width: number, height: number, context: CanvasRenderingContext2D, step: number): Annotation {
  // Preserve pixel size when the target display has different dimensions.
  const item: Annotation = structuredClone(copy.item);
  const move = (p: Point): Point => [p[0] * copy.width / width, p[1] * copy.height / height];
  if (item.kind === "text") {
    [item.x, item.y] = move([item.x, item.y]);
    item.width *= copy.width / width;
    item.fontSize *= copy.width / width;
  } else {
    item.width *= copy.width / width;
    if (item.kind === "shape") { item.start = move(item.start); item.end = move(item.end); }
    else item.points = item.points.map(move);
  }
  const bounds = annotationBounds(item, width, height, context);
  const scale = Math.min(1, Math.max(1, width - 24) / bounds.width, Math.max(1, height - 24) / bounds.height);
  const w = bounds.width * scale, h = bounds.height * scale;
  const offset = 24 * ((step - 1) % 10 + 1);
  const x = Math.max(0, Math.min(width - w, bounds.x + (bounds.x + w + offset > width ? -offset : offset)));
  const y = Math.max(0, Math.min(height - h, bounds.y + (bounds.y + h + offset > height ? -offset : offset)));
  const result = transformAnnotation(item, bounds, { x, y, width: w, height: h }, width, height);
  if (result.kind !== "text") result.width = Math.min(.1, result.width * scale);
  return result;
}
