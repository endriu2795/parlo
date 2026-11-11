// Calendar bounds follow the same local timezone used to display each call.
// The end is the following midnight, so the selected final day is included.
export function archiveDateBounds(from: string, to: string) {
  if (from && to && from > to) return null;
  const start = from ? new Date(`${from}T00:00:00`) : null;
  const end = to ? new Date(`${to}T00:00:00`) : null;
  if (
    (start && !Number.isFinite(start.getTime())) ||
    (end && !Number.isFinite(end.getTime()))
  )
    return null;
  if (end) end.setDate(end.getDate() + 1);
  return { from: start?.toISOString() ?? null, to: end?.toISOString() ?? null };
}
