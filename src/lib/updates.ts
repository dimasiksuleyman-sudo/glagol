import { useSyncExternalStore } from "react";
import { checkForUpdate, getUpdateSettings, type UpdateInfo } from "@/lib/tauri";

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

let startupCheckDone = false;

/**
 * The one automatic check per app run, only when the user enabled it.
 * Failures stay silent: the user did not ask for this check explicitly.
 */
export async function runStartupUpdateCheck(): Promise<UpdateInfo | null> {
  if (startupCheckDone) return null;
  startupCheckDone = true;
  try {
    const settings = await getUpdateSettings();
    return settings.autoCheck ? await runUpdateCheck() : null;
  } catch {
    return null;
  }
}
