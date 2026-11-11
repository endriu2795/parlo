type TimedPassage = { start_ms: number; end_ms: number };

/** Return a passage containing the position, or the nearest one when requested. */
export function passageAtTime(passages: readonly TimedPassage[], time: number, nearest = false): number | null {
  if (!Number.isFinite(time)) return null;
  let match: number | null = null;
  let nearestIndex: number | null = null;
  let distance = Infinity;
  for (let index = 0; index < passages.length; index++) {
    const { start_ms: start, end_ms: end } = passages[index];
    if (!Number.isFinite(start) || !Number.isFinite(end) || start < 0 || end < start) continue;
    if (start <= time && time <= end && (match === null || start >= passages[match].start_ms)) match = index;
    const delta = time < start ? start - time : time > end ? time - end : 0;
    if (delta < distance) { distance = delta; nearestIndex = index; }
  }
  return match ?? (nearest ? nearestIndex : null);
}
