/** A SaluteSpeech voice that Glagol exposes in the UI. */
export interface Voice {
  /** Sber API identifier sent over the wire, e.g. `"Nec_24000"`. */
  id: string;
  /** Display name shown in the voice picker (Russian). */
  label: string;
}

export const VOICES: readonly Voice[] = [
  { id: "Nec_24000", label: "Наталья" },
  { id: "Bys_24000", label: "Борис" },
  { id: "May_24000", label: "Марфа" },
  { id: "Tur_24000", label: "Тарас" },
  { id: "Ost_24000", label: "Александра" },
  { id: "Pon_24000", label: "Сергей" },
] as const;

/** Default voice selected on first visit to the Synthesize page. */
export const DEFAULT_VOICE_ID = "Nec_24000";

/**
 * Look up the Russian display label for a SaluteSpeech voice API id.
 * Falls back to the raw id if the voice isn't in our catalogue —
 * forward-safe for new voices Sberbank may roll out.
 */
export function getVoiceLabel(voiceApiId: string): string {
  return VOICES.find((v) => v.id === voiceApiId)?.label ?? voiceApiId;
}
