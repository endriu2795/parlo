import { t, uiLocale } from "./i18n";
// Shared display helpers: every view formats calls, times and states the same way.
const statusLabels: Record<string, string> = {
  ready: t("Pronta"),
  recording: t("In registrazione"),
  pending: t("In attesa"),
  processing: t("In elaborazione"),
  finalizing: t("Salvataggio"),
  failed: t("Da riprovare"),
};
export const statusLabel = (status: string) => statusLabels[status] || status;

export const fmtTime = (ms: number) =>
  `${Math.floor(ms / 60000).toString().padStart(2, "0")}:${Math.floor((ms / 1000) % 60).toString().padStart(2, "0")}`;

export const fmtDate = (iso: string) =>
  new Date(iso).toLocaleDateString(uiLocale, { day: "numeric", month: "short", year: "numeric" });

export const fmtClock = (iso: string) =>
  new Date(iso).toLocaleTimeString(uiLocale, { hour: "2-digit", minute: "2-digit" });

export const archiveDay = (iso: string) => new Date(iso).toLocaleDateString(uiLocale);

export function archiveHeading(iso: string) {
  const today = new Date();
  if (archiveDay(iso) === today.toLocaleDateString(uiLocale)) return t("Oggi");
  today.setDate(today.getDate() - 1);
  return archiveDay(iso) === today.toLocaleDateString(uiLocale) ? t("Ieri") : fmtDate(iso);
}

export const fmtIsoDay = (day: string) => new Date(`${day}T00:00:00`).toLocaleDateString(uiLocale, {
  day: "2-digit", month: "2-digit", year: "numeric",
});
