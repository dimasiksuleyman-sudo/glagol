import { currentLocale, t } from "@/i18n";
/** Relative times follow the interface language; document content stays unchanged. */
export function formatRelativeTime(unixMs: number): string {
  const seconds = Math.max(0, Math.floor((Date.now() - unixMs) / 1000));
  if (seconds < 60) return t("justNow");
  const relative = new Intl.RelativeTimeFormat(currentLocale(), { numeric: "auto" });
  if (seconds < 3600) return relative.format(-Math.floor(seconds / 60), "minute");
  if (seconds < 86400) return relative.format(-Math.floor(seconds / 3600), "hour");
  if (seconds < 604800) return relative.format(-Math.floor(seconds / 86400), "day");
  return new Intl.DateTimeFormat(currentLocale()).format(new Date(unixMs));
}
