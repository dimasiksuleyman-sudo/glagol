import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getTtsStatus, type TtsStatus } from "@/lib/tts";
const Context = createContext<{ status: TtsStatus | null; error: string | null; refresh: () => Promise<void> } | null>(null);
export function TtsProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<TtsStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(async () => {
    try { setStatus(await getTtsStatus()); setError(null); }
    catch (e) { setError(String(e)); }
  }, []);
  useEffect(() => {
    void refresh();
    const listener = listen("tts-status", () => { void refresh(); });
    return () => { void listener.then(unlisten => unlisten()); };
  }, [refresh]);
  return <Context.Provider value={{ status, error, refresh }}>{children}</Context.Provider>;
}
export function useTts() {
  const context = useContext(Context);
  if (!context) throw new Error("TtsProvider missing");
  return context;
}
