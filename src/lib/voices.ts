/** Voices of the local engines (Silero, Qwen3-TTS); legacy labels only identify existing library items. */
import { currentLocale, type Language } from "@/i18n";
export interface Voice { id: string; label: string }
export const VOICES: readonly Voice[] = [
  { id: "aidar", label: "Айдар" }, { id: "baya", label: "Бая" },
  { id: "kseniya", label: "Ксения" }, { id: "xenia", label: "Ксения (Xenia)" },
  { id: "eugene", label: "Евгений" },
];
export const DEFAULT_VOICE_ID = "xenia";
const latin: Record<string,string> = {aidar:"Aidar",baya:"Baya",kseniya:"Kseniya",xenia:"Xenia",eugene:"Eugene",Nec_24000:"Natalya",Bys_24000:"Boris",May_24000:"Marfa",Tur_24000:"Taras",Ost_24000:"Alexandra",Pon_24000:"Sergey",Kin_24000:"Kira"};
/** Qwen3-TTS named speakers (CustomVoice); the same for both languages. */
export const QWEN_VOICES: readonly Voice[] = [
  { id: "ryan", label: "Ryan" }, { id: "aiden", label: "Aiden" }, { id: "serena", label: "Serena" },
  { id: "vivian", label: "Vivian" }, { id: "uncle_fu", label: "Uncle Fu" }, { id: "ono_anna", label: "Ono Anna" },
  { id: "sohee", label: "Sohee" }, { id: "eric", label: "Eric" }, { id: "dylan", label: "Dylan" },
];
export type Engine = "silero" | "qwen";
/** The voice in use for the selected engine and speech language. */
export function activeVoice(p: { tts_engine?: string; tts_voice_qwen?: string; tts_voice_en?: string; tts_voice_ru?: string } | null | undefined, language: Language): string {
  if (p?.tts_engine === "qwen") return p.tts_voice_qwen ?? "ryan";
  return (language === "en" ? p?.tts_voice_en : p?.tts_voice_ru) ?? (language === "en" ? "en_0" : "xenia");
}
/** Provider ID for TTS commands: engine and speech language. */
export function providerFor(engine: string | undefined, language: Language): string {
  if (engine === "qwen") return language === "en" ? "qwen-en" : "qwen";
  return language === "en" ? "silero-en" : "silero";
}
export function voicesFor(language: Language, engine?: string): readonly Voice[] {
  if (engine === "qwen") return QWEN_VOICES;
  if (language === "en") return [0,1,2,3].map(n => ({id:`en_${n}`,label:`EN ${n}`}));
  return VOICES.map(v => ({...v,label:currentLocale()==="en-US" ? latin[v.id] : v.label}));
}
const legacy: Record<string,string> = { Nec_24000: "Наталья", Bys_24000: "Борис", May_24000: "Марфа", Tur_24000: "Тарас", Ost_24000: "Александра", Pon_24000: "Сергей", Kin_24000: "Кира" };
export function getVoiceLabel(id: string): string { const qwen = QWEN_VOICES.find(v => v.id === id); if (qwen) return `Qwen · ${qwen.label}`; return /^en_[0-3]$/.test(id) ? id.replace("en_", "EN ") : currentLocale()==="en-US" && latin[id] ? latin[id] : VOICES.find(v => v.id === id)?.label ?? legacy[id] ?? id; }

/** Qwen3-TTS reading styles: an instruction sent with each fragment (none: the model decides). */
export const QWEN_STYLES = [
  { id: "auto", instruction: null },
  { id: "neutral", instruction: "Read in a neutral, even tone without emotion." },
  { id: "calm", instruction: "Read calmly and slowly, in a soft voice." },
  { id: "news", instruction: "Read like a professional news anchor: clear, confident, measured." },
  { id: "warm", instruction: "Read in a warm, friendly tone." },
  { id: "lively", instruction: "Read cheerfully and energetically." },
  { id: "custom", instruction: null },
] as const;
export type QwenStyleId = (typeof QWEN_STYLES)[number]["id"];
/** The instruction to send for a style choice. */
export function styleInstruction(id: QwenStyleId, custom: string): string | null {
  if (id === "custom") return custom.trim() || null;
  return QWEN_STYLES.find(s => s.id === id)?.instruction ?? null;
}
