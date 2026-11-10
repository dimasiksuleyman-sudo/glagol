/**
 * Help topics. "Show" opens a screen and highlights a real element marked with
 * data-tour="<target>" (pattern from blood-p-tracker). Texts are typed i18n keys.
 */
export type TourId =
  | "dictate" | "microphone" | "insert_mode" | "history" | "speed"
  | "synthesize" | "voice" | "library" | "tts_off" | "backup" | "updates" | "diagnostics";

export interface Tour {
  id: TourId;
  route: string;
  /** data-tour attribute of the element to highlight */
  target: string;
  /** Tried when the target is not on screen (for example, not configured yet). */
  fallback?: string;
  /** Needs text to speech turned on; otherwise the help shows the switch instead. */
  needsTts?: boolean;
  title: "helpDictateTitle" | "helpMicrophoneTitle" | "helpInsertModeTitle" | "helpHistoryTitle" | "helpSpeedTitle" | "helpSynthesizeTitle" | "helpVoiceTitle" | "helpLibraryTitle" | "helpTtsOffTitle" | "helpBackupTitle" | "helpUpdatesTitle" | "helpDiagnosticsTitle";
  steps: "helpDictateSteps" | "helpMicrophoneSteps" | "helpInsertModeSteps" | "helpHistorySteps" | "helpSpeedSteps" | "helpSynthesizeSteps" | "helpVoiceSteps" | "helpLibrarySteps" | "helpTtsOffSteps" | "helpBackupSteps" | "helpUpdatesSteps" | "helpDiagnosticsSteps";
  hint: "helpDictateHint" | "helpMicrophoneHint" | "helpInsertModeHint" | "helpHistoryHint" | "helpSpeedHint" | "helpSynthesizeHint" | "helpVoiceHint" | "helpLibraryHint" | "helpTtsOffHint" | "helpBackupHint" | "helpUpdatesHint" | "helpDiagnosticsHint";
  tip: "helpDictateTip" | "helpMicrophoneTip" | "helpInsertModeTip" | "helpHistoryTip" | "helpSpeedTip" | "helpSynthesizeTip" | "helpVoiceTip" | "helpLibraryTip" | "helpTtsOffTip" | "helpBackupTip" | "helpUpdatesTip" | "helpDiagnosticsTip";
}

export const TOURS: Tour[] = [
  { id: "dictate", route: "/dictation", target: "hotkey", title: "helpDictateTitle", steps: "helpDictateSteps", hint: "helpDictateHint", tip: "helpDictateTip" },
  { id: "speed", route: "/settings/dictation", target: "speed", fallback: "speech-mode", title: "helpSpeedTitle", steps: "helpSpeedSteps", hint: "helpSpeedHint", tip: "helpSpeedTip" },
  { id: "microphone", route: "/dictation", target: "microphone", title: "helpMicrophoneTitle", steps: "helpMicrophoneSteps", hint: "helpMicrophoneHint", tip: "helpMicrophoneTip" },
  { id: "insert_mode", route: "/dictation", target: "insert-mode", title: "helpInsertModeTitle", steps: "helpInsertModeSteps", hint: "helpInsertModeHint", tip: "helpInsertModeTip" },
  { id: "history", route: "/dictation", target: "history", title: "helpHistoryTitle", steps: "helpHistorySteps", hint: "helpHistoryHint", tip: "helpHistoryTip" },
  { id: "synthesize", route: "/synthesize", target: "choose-file", needsTts: true, title: "helpSynthesizeTitle", steps: "helpSynthesizeSteps", hint: "helpSynthesizeHint", tip: "helpSynthesizeTip" },
  { id: "voice", route: "/settings/speech", target: "voice", fallback: "tts-components", needsTts: true, title: "helpVoiceTitle", steps: "helpVoiceSteps", hint: "helpVoiceHint", tip: "helpVoiceTip" },
  { id: "library", route: "/library", target: "library", title: "helpLibraryTitle", steps: "helpLibrarySteps", hint: "helpLibraryHint", tip: "helpLibraryTip" },
  { id: "tts_off", route: "/settings/speech", target: "tts-switch", title: "helpTtsOffTitle", steps: "helpTtsOffSteps", hint: "helpTtsOffHint", tip: "helpTtsOffTip" },
  { id: "backup", route: "/settings/general", target: "backup", title: "helpBackupTitle", steps: "helpBackupSteps", hint: "helpBackupHint", tip: "helpBackupTip" },
  { id: "updates", route: "/settings/general", target: "updates", title: "helpUpdatesTitle", steps: "helpUpdatesSteps", hint: "helpUpdatesHint", tip: "helpUpdatesTip" },
  { id: "diagnostics", route: "/settings/general", target: "diagnostics", title: "helpDiagnosticsTitle", steps: "helpDiagnosticsSteps", hint: "helpDiagnosticsHint", tip: "helpDiagnosticsTip" },
];

export const getTour = (id: unknown): Tour | undefined => TOURS.find(tour => tour.id === id);

/** Router state that starts a hint on the opened screen. */
export interface TourState { tour: TourId; target?: string }

/** Where and what to highlight, taking the text-to-speech switch into account. */
export function tourLink(tour: Tour, ttsEnabled: boolean): { route: string; state: TourState } {
  if (tour.needsTts && !ttsEnabled) return { route: "/settings/speech", state: { tour: tour.id, target: "tts-switch" } };
  return { route: tour.route, state: { tour: tour.id } };
}
