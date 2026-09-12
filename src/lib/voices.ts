/** Active Silero voices; legacy labels only identify existing library items. */
export interface Voice { id: string; label: string }
export const VOICES: readonly Voice[] = [
  { id: "aidar", label: "Айдар" }, { id: "baya", label: "Бая" },
  { id: "kseniya", label: "Ксения" }, { id: "xenia", label: "Ксения (Xenia)" },
  { id: "eugene", label: "Евгений" },
];
export const DEFAULT_VOICE_ID = "xenia";
const legacy: Record<string,string> = { Nec_24000: "Наталья", Bys_24000: "Борис", May_24000: "Марфа", Tur_24000: "Тарас", Ost_24000: "Александра", Pon_24000: "Сергей", Kin_24000: "Кира" };
export function getVoiceLabel(id: string): string { return VOICES.find(v => v.id === id)?.label ?? legacy[id] ?? id; }
