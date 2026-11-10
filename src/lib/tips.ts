import type { TourId } from "./tours";

/** Features whose use is remembered so tips do not repeat what the user already does. */
export type Feature = "preview" | "backup" | "library";

interface TipsState {
  disabled: boolean;
  /** First dismissal: during the first month tips come weekly, then monthly. */
  firstDismissedAt?: string;
  lastDismissedAt?: string;
  seen: TourId[];
}

/** Facts about the user's setup that decide whether a tip is relevant. */
export interface TipContext {
  ttsEnabled: boolean;
  historyEnabled: boolean;
  documents: number;
  minutes: number;
  recommendationPending: boolean;
  autoCheck: boolean;
}

const STATE_KEY = "glagol.tips";
const FEATURES_KEY = "glagol.usedFeatures";
const DAY = 24 * 60 * 60 * 1000;

// In priority order; a tip is shown only while it is relevant.
const TIPS: { id: TourId; relevant: (c: TipContext) => boolean }[] = [
  { id: "dictate", relevant: c => c.minutes === 0 },
  { id: "speed", relevant: c => c.recommendationPending },
  { id: "history", relevant: c => !c.historyEnabled && c.minutes > 0 },
  { id: "synthesize", relevant: c => c.ttsEnabled && c.documents === 0 },
  { id: "voice", relevant: c => c.ttsEnabled && !isFeatureUsed("preview") },
  { id: "library", relevant: c => c.documents > 0 && !isFeatureUsed("library") },
  { id: "tts_off", relevant: c => c.ttsEnabled && c.documents === 0 && c.minutes >= 10 },
  { id: "backup", relevant: c => c.documents >= 3 && !isFeatureUsed("backup") },
  { id: "updates", relevant: c => !c.autoCheck },
  { id: "insert_mode", relevant: c => c.minutes >= 5 },
];

// Browser storage is a per-user convenience here; it may be unavailable.
function read<T>(key: string, fallback: T): T {
  try {
    return { ...fallback, ...JSON.parse(localStorage.getItem(key) ?? "null") };
  } catch {
    return fallback;
  }
}

function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Storage unavailable: tips simply start over next time.
  }
}

const getState = (): TipsState => read<TipsState>(STATE_KEY, { disabled: false, seen: [] });

export function markFeatureUsed(feature: Feature): void {
  write(FEATURES_KEY, { ...read<Record<string, boolean>>(FEATURES_KEY, {}), [feature]: true });
}

export function isFeatureUsed(feature: Feature): boolean {
  return Boolean(read<Record<string, boolean>>(FEATURES_KEY, {})[feature]);
}

export const tipsEnabled = () => !getState().disabled;

export function setTipsEnabled(enabled: boolean): void {
  write(STATE_KEY, { ...getState(), disabled: !enabled });
}

/** The tip to show now, or null (disabled, too early, or nothing new to tell). */
export function pickTip(context: TipContext, now = new Date()): TourId | null {
  const state = getState();
  if (state.disabled) return null;
  if (state.lastDismissedAt) {
    const firstMonth = !state.firstDismissedAt || now.getTime() - new Date(state.firstDismissedAt).getTime() < 30 * DAY;
    const interval = (firstMonth ? 7 : 30) * DAY;
    if (now.getTime() - new Date(state.lastDismissedAt).getTime() < interval) return null;
  }
  return TIPS.find(tip => !state.seen.includes(tip.id) && tip.relevant(context))?.id ?? null;
}

/** The user closed a tip; optionally turn tips off. */
export function dismissTip(id: TourId, disableAll = false, now = new Date()): void {
  const state = getState();
  write(STATE_KEY, {
    ...state,
    disabled: state.disabled || disableAll,
    seen: [...new Set([...state.seen, id])],
    firstDismissedAt: state.firstDismissedAt ?? now.toISOString(),
    lastDismissedAt: now.toISOString(),
  });
}
