/**
 * Russian relative-time formatter for the Library list.
 *
 * Buckets:
 *   < 60 s    → "только что"
 *   < 60 min  → "N минут назад"
 *   < 24 h    → "N часов назад"
 *   < 48 h    → "вчера"
 *   < 7 d     → "N дней назад"
 *   older     → "DD.MM.YYYY" absolute
 *
 * No formatting library — pluralisation is ~10 lines and avoids
 * pulling in date-fns or dayjs just for one helper. Time staleness on
 * a long-open page is acceptable (Sprint 2 refetches on
 * unmount/remount via React Router navigation).
 */
export function formatRelativeTime(unixMs: number): string {
  const diffMs = Date.now() - unixMs;
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 60) return "только что";
  if (diffMin < 60) return `${diffMin} ${pluralizeRu(diffMin, "минута", "минуты", "минут")} назад`;
  if (diffHour < 24) return `${diffHour} ${pluralizeRu(diffHour, "час", "часа", "часов")} назад`;
  if (diffDay < 2) return "вчера";
  if (diffDay < 7) return `${diffDay} ${pluralizeRu(diffDay, "день", "дня", "дней")} назад`;

  const d = new Date(unixMs);
  const dd = String(d.getDate()).padStart(2, "0");
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  return `${dd}.${mm}.${d.getFullYear()}`;
}

/**
 * Russian noun pluralisation rule (works for any cardinal):
 *   1, 21, 31…   → `one`
 *   2-4, 22-24…  → `few`
 *   else         → `many` (including 0, 5-20, 25-30, …)
 */
function pluralizeRu(n: number, one: string, few: string, many: string): string {
  const lastTwo = Math.abs(n) % 100;
  if (lastTwo >= 11 && lastTwo <= 14) return many;
  const last = Math.abs(n) % 10;
  if (last === 1) return one;
  if (last >= 2 && last <= 4) return few;
  return many;
}
