/** Active Silero voices; legacy labels only identify existing library items. */
import { currentLocale, type Language } from "@/i18n";
export interface Voice { id: string; label: string }
export const VOICES: readonly Voice[] = [
  { id: "aidar", label: "Айдар" }, { id: "baya", label: "Бая" },
  { id: "kseniya", label: "Ксения" }, { id: "xenia", label: "Ксения (Xenia)" },
  { id: "eugene", label: "Евгений" },
];
export const DEFAULT_VOICE_ID = "xenia";
const latin: Record<string,string> = {aidar:"Aidar",baya:"Baya",kseniya:"Kseniya",xenia:"Xenia",eugene:"Eugene",Nec_24000:"Natalya",Bys_24000:"Boris",May_24000:"Marfa",Tur_24000:"Taras",Ost_24000:"Alexandra",Pon_24000:"Sergey",Kin_24000:"Kira"};
export function voicesFor(language: Language): readonly Voice[] {
  if (language === "en") return [0,1,2,3].map(n => ({id:`en_${n}`,label:`EN ${n}`}));
  return VOICES.map(v => ({...v,label:currentLocale()==="en-US" ? latin[v.id] : v.label}));
}
const legacy: Record<string,string> = { Nec_24000: "Наталья", Bys_24000: "Борис", May_24000: "Марфа", Tur_24000: "Тарас", Ost_24000: "Александра", Pon_24000: "Сергей", Kin_24000: "Кира" };
export function getVoiceLabel(id: string): string { return /^en_[0-3]$/.test(id) ? id.replace("en_", "EN ") : currentLocale()==="en-US" && latin[id] ? latin[id] : VOICES.find(v => v.id === id)?.label ?? legacy[id] ?? id; }
