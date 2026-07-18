/**
 * Russian three-form plural selection.
 *
 * Russian nouns take three forms depending on the count:
 *   - 1, 21, 31, … (but not 11) → singular nominative («один документ»)
 *   - 2-4, 22-24, 32-34, … (but not 12-14) → genitive singular («два документа»)
 *   - 0, 5-20, 25-30, … → genitive plural («пять документов»)
 *
 * The 11-14 exception is real Russian grammar, not a typo — those numerals
 * always take the genitive plural form even though their last digit
 * would otherwise pick a different one.
 *
 * @param n     count to pluralize (must be a non-negative integer; negatives
 *              are treated as their absolute value)
 * @param one   form for the singular-nominative case
 * @param few   form for the 2-4 paucal case
 * @param many  form for the 0/5+ default case
 */
export function pluralRu(n: number, one: string, few: string, many: string): string {
  const abs = Math.abs(Math.trunc(n));
  const mod100 = abs % 100;
  if (mod100 >= 11 && mod100 <= 14) return many;
  const mod10 = abs % 10;
  if (mod10 === 1) return one;
  if (mod10 >= 2 && mod10 <= 4) return few;
  return many;
}

/** «1 документ» / «2 документа» / «5 документов». */
export function pluralizeDocuments(n: number): string {
  return pluralRu(n, "документ", "документа", "документов");
}

/** «1 файл» / «2 файла» / «5 файлов» — used by progress modals. */
export function pluralizeFiles(n: number): string {
  return pluralRu(n, "файл", "файла", "файлов");
}

/** «1 минута» / «2 минуты» / «5 минут» — the «Надиктовано всего» counter. */
export function pluralizeMinutes(n: number): string {
  return pluralRu(n, "минута", "минуты", "минут");
}

/** «1 запись» / «2 записи» / «5 записей» — the dictation history counter. */
export function pluralizeEntries(n: number): string {
  return pluralRu(n, "запись", "записи", "записей");
}
