import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

import { testCredentials } from "@/lib/tauri";

/**
 * Tri-state credentials signal mirrored from the backend.
 *
 * - `"unknown"` — first paint, while the mount-time probe is in
 *   flight. The UI renders a neutral loading view rather than briefly
 *   showing the "configure first" gate.
 * - `"valid"` — Sberbank accepted the stored Authorization Key on the
 *   most recent probe; Synthesize can proceed.
 * - `"invalid"` — no key configured, or the key exists but failed
 *   OAuth (revoked, malformed, etc.). The Synthesize page shows a
 *   gating Card with a link back to Settings.
 *
 * Source of truth for the *backend* state is the keyring plus the
 * cached `SaluteAuth` on the Rust side; this context just mirrors the
 * latest known answer so the UI doesn't have to await on every render.
 */
export type CredentialsState = "unknown" | "valid" | "invalid";

interface CredentialsContextValue {
  state: CredentialsState;
  setState: (next: CredentialsState) => void;
}

const CredentialsContext = createContext<CredentialsContextValue | undefined>(undefined);

interface CredentialsProviderProps {
  children: ReactNode;
}

export function CredentialsProvider({ children }: CredentialsProviderProps) {
  const [state, setState] = useState<CredentialsState>("unknown");

  // Mount-time probe: ask the backend whether the stored AK still
  // authenticates with Sberbank. The Settings page can re-trigger this
  // later via its own Test handler.
  useEffect(() => {
    let cancelled = false;
    testCredentials(false)
      .then(() => {
        if (!cancelled) setState("valid");
      })
      .catch(() => {
        if (!cancelled) setState("invalid");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <CredentialsContext.Provider value={{ state, setState }}>
      {children}
    </CredentialsContext.Provider>
  );
}

/** Hook for any component beneath {@link CredentialsProvider}. */
export function useCredentials(): CredentialsContextValue {
  const ctx = useContext(CredentialsContext);
  if (!ctx) {
    throw new Error("useCredentials must be used within a CredentialsProvider");
  }
  return ctx;
}
