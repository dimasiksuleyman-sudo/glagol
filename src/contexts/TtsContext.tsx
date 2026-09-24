import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getTtsStatus, type TtsStatus } from "@/lib/tts";
import { usePreferences } from "@/contexts/PreferencesContext";
const Context = createContext<{ status: TtsStatus | null; error: string | null; provider: string; refresh: () => Promise<void> } | null>(null);
export function TtsProvider({ children }: { children: ReactNode }) {
  const { preferences } = usePreferences();
  const provider = preferences?.tts_language === "en" ? "silero-en" : "silero";
  const currentProvider = useRef(provider);
  currentProvider.current = provider;
  const [status, setStatus] = useState<TtsStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(async () => {
    try { const result = await getTtsStatus(provider); if (currentProvider.current === provider) { setStatus(result); setError(null); } }
    catch (e) { if (currentProvider.current === provider) setError(String(e)); }
  }, [provider, preferences?.ui_language]);
  useEffect(() => {
    void refresh();
    const listener = listen("tts-status", () => { void refresh(); });
    return () => { void listener.then(unlisten => unlisten()); };
  }, [refresh]);
  return <Context.Provider value={{ status: status?.provider === provider ? status : null, error, provider, refresh }}>{children}</Context.Provider>;
}
export function useTts() {
  const context = useContext(Context);
  if (!context) throw new Error("TtsProvider missing");
  return context;
}
