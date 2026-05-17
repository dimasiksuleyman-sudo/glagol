import { createContext, useContext, useState, type ReactNode } from "react";

/**
 * Shared boolean signalling whether the backend currently has valid
 * SaluteSpeech credentials configured (i.e. a working Authorization Key
 * is stored in the OS keyring and was last seen to authenticate).
 *
 * Used by the Synthesize page to gate the workflow ("Configure
 * credentials in Settings first") and by Settings to react to the
 * outcome of Save / Test / Delete actions.
 *
 * Source of truth for the *backend* state is the keyring + the cached
 * `SaluteAuth` on the Rust side; this context just mirrors the latest
 * known answer so the UI can render without an async round-trip on
 * every keystroke.
 */
interface CredentialsContextValue {
  hasCredentials: boolean;
  setHasCredentials: (value: boolean) => void;
}

const CredentialsContext = createContext<CredentialsContextValue | undefined>(undefined);

interface CredentialsProviderProps {
  children: ReactNode;
}

export function CredentialsProvider({ children }: CredentialsProviderProps) {
  // Initial state is `false` — first paint assumes "not configured".
  // Phase 3 will add a `useEffect` that calls `testCredentials()` once
  // on mount and updates this to `true` if Sberbank accepts the cached
  // key (i.e. the keyring is non-empty AND the AK is still valid).
  const [hasCredentials, setHasCredentials] = useState<boolean>(false);

  return (
    <CredentialsContext.Provider value={{ hasCredentials, setHasCredentials }}>
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
