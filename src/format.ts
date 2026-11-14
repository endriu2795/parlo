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

let dateFormatters: { locale: string; date: Intl.DateTimeFormat; clock: Intl.DateTimeFormat; day: Intl.DateTimeFormat } | undefined;

function formatters() {
  if (dateFormatters?.locale !== uiLocale) {
    dateFormatters = {
      locale: uiLocale,
      date: new Intl.DateTimeFormat(uiLocale, { day: "numeric", month: "short", year: "numeric" }),
      clock: new Intl.DateTimeFormat(uiLocale, { hour: "2-digit", minute: "2-digit" }),
      day: new Intl.DateTimeFormat(uiLocale),
    };
  }
  return dateFormatters;
}

function formatDate(formatter: Intl.DateTimeFormat, iso: string) {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? date.toString() : formatter.format(date);
}

export const fmtDate = (iso: string) => formatDate(formatters().date, iso);
export const fmtClock = (iso: string) => formatDate(formatters().clock, iso);
export const archiveDay = (iso: string) => formatDate(formatters().day, iso);

export function archiveHeading(iso: string) {
  const today = new Date();
  const day = archiveDay(iso);
  const formatter = formatters().day;
  if (day === formatter.format(today)) return t("Oggi");
  today.setDate(today.getDate() - 1);
  return day === formatter.format(today) ? t("Ieri") : fmtDate(iso);
}

export const fmtIsoDay = (day: string) => new Date(`${day}T00:00:00`).toLocaleDateString(uiLocale, {
  day: "2-digit", month: "2-digit", year: "numeric",
});
