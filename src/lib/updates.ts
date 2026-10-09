import { useSyncExternalStore } from "react";
import { checkForUpdate, getUpdateSettings, markUpdateReminderShown, setUpdateAutoCheck, type UpdateInfo } from "@/lib/tauri";

/**
 * The update found by the latest check, shared by the startup check in the
 * shell and the Updates section in Settings. The backend keeps the matching
 * `Update` for `install_update`.
 */
let available: UpdateInfo | null = null;
const listeners = new Set<() => void>();

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Run a check and publish its result; throws the localized backend error. */
export async function runUpdateCheck(): Promise<UpdateInfo | null> {
  available = await checkForUpdate();
  listeners.forEach((listener) => listener());
  return available;
}

export function useAvailableUpdate(): UpdateInfo | null {
  return useSyncExternalStore(subscribe, () => available);
}

/** Auto-check flag shared by the startup reminder and the Settings switch. */
let autoCheck: boolean | null = null;

export function useAutoCheck(): boolean | null {
  return useSyncExternalStore(subscribe, () => autoCheck);
}

export async function changeAutoCheck(enabled: boolean): Promise<void> {
  await setUpdateAutoCheck(enabled);
  autoCheck = enabled;
  listeners.forEach((listener) => listener());
}

export type StartupResult =
  | { kind: "update"; update: UpdateInfo }
  | { kind: "reminder" }
  | null;

let startupDone = false;

/**
 * Once per app run: with auto-check on, check silently (failures stay
 * silent — the user did not press anything); with it off, report a due
 * monthly reminder and start its 30-day pause right away.
 */
export async function runStartupUpdateCheck(): Promise<StartupResult> {
  if (startupDone) return null;
  startupDone = true;
  try {
    const settings = await getUpdateSettings();
    autoCheck = settings.autoCheck;
    listeners.forEach((listener) => listener());
    if (settings.autoCheck) {
      const update = await runUpdateCheck();
      return update ? { kind: "update", update } : null;
    }
    if (settings.reminderDue) {
      await markUpdateReminderShown();
      return { kind: "reminder" };
    }
  } catch {
    // Network or settings errors never interrupt startup.
  }
  return null;
}
