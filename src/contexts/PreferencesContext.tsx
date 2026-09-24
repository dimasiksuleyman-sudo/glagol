import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { translator, localeName, setInterfaceLanguage, type Language } from "@/i18n";

export interface Preferences {
  ui_language: Language;
  onboarding_stage: "language" | "speech" | "complete";
  upgraded: boolean;
  stt_language: Language;
  tts_language: Language;
  tts_voice_en: string;
  tts_voice_ru: string;
  legacy_voice_migrated: boolean;
}
interface Context {
  preferences: Preferences | null;
  error: string | null;
  saving: boolean;
  reload: () => Promise<void>;
  chooseLanguage: (language: Language) => Promise<void>;
  complete: () => Promise<void>;
  setTtsLanguage: (language: Language) => Promise<void>;
  setSttLanguage: (language: Language) => Promise<void>;
  setTtsVoice: (voice: string) => Promise<void>;
}
const PreferencesContext = createContext<Context | null>(null);
export function PreferencesProvider({ children, overlay = false }: { children: ReactNode; overlay?: boolean }) {
  const [preferences, setPreferences] = useState<Preferences | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const reload = useCallback(async () => {
    try {
      let value = await invoke<Preferences>("get_preferences");
      if (!overlay && !value.legacy_voice_migrated) {
        let voice: string | null = null;
        try { voice = localStorage.getItem("tts-voice"); } catch { /* Storage may be unavailable. */ }
        value = await invoke<Preferences>("migrate_legacy_voice", { voice });
        // SQLite is authoritative now. Do not remove the old value until IPC succeeds.
        try { localStorage.removeItem("tts-voice"); } catch { /* SQLite migration is already persisted. */ }
      }
      setInterfaceLanguage(value.ui_language); setPreferences(value); setError(null);
    } catch (err) { setError(String(err)); }
  }, [overlay]);
  useEffect(() => {
    let disposed = false;
    const unlisten = listen<Preferences>("preferences-changed", event => {
      if (!disposed) { setInterfaceLanguage(event.payload.ui_language); setPreferences(event.payload); }
    });
    void unlisten.then(() => { if (!disposed) void reload(); }).catch(err => setError(String(err)));
    return () => { disposed = true; void unlisten.then(stop => stop()).catch(() => undefined); };
  }, [reload]);
  useEffect(() => { document.documentElement.lang = preferences?.ui_language ?? "en"; }, [preferences?.ui_language]);
  async function change(command: string, args?: Record<string, unknown>) {
    setSaving(true);
    try { const value = await invoke<Preferences>(command, args); setInterfaceLanguage(value.ui_language); setPreferences(value); setError(null); }
    catch (err) { setError(String(err)); throw err; }
    finally { setSaving(false); }
  }
  return <PreferencesContext.Provider value={{ preferences, error, saving, reload,
    chooseLanguage: language => change("set_ui_language", { language }),
    complete: () => change("complete_onboarding"),
    setTtsLanguage: language => change("set_tts_preference", { language, voice: null }),
    setSttLanguage: language => change("set_stt_preference", { language }),
    setTtsVoice: voice => change("set_tts_preference", { language: preferences?.tts_language ?? "ru", voice }),
  }}>{children}</PreferencesContext.Provider>;
}
export function usePreferences() {
  const context = useContext(PreferencesContext);
  if (!context) throw new Error("PreferencesProvider is missing");
  return context;
}
export function useI18n() {
  const { preferences } = usePreferences();
  const language = preferences?.ui_language ?? "en";
  const t = translator(language);
  return { language, locale: localeName(language), t };
}
