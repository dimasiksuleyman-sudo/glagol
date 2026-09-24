import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { DICTATION_STATE_EVENT, type DictationState } from "@/lib/tauri";
import type { Language } from "@/i18n";

export function useDictationBusy() {
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let disposed = false, eventSeen = false;
    const listener = listen<DictationState>(DICTATION_STATE_EVENT, ({ payload }) => {
      eventSeen = true;
      if (!disposed) setBusy(["starting", "recording", "processing"].includes(payload.kind));
    });
    void listener.then(() => invoke<boolean>("is_dictating")).then(value => { if (!disposed && !eventSeen) setBusy(value); }).catch(() => undefined);
    return () => { disposed = true; void listener.then(stop => stop()); };
  }, []);
  return busy;
}

export function SttLanguageSwitch({ disabled = false }: { disabled?: boolean }) {
  const { t } = useI18n();
  const { preferences, saving, setSttLanguage } = usePreferences();
  const busy = useDictationBusy();
  return <label className="flex items-center gap-2 text-sm">
    <span>{t("speechLanguage")}</span>
    <select className="rounded border bg-background p-2" value={preferences?.stt_language ?? "en"} disabled={disabled || saving || busy}
      onChange={e => void setSttLanguage(e.target.value as Language).catch(() => undefined)}>
      <option value="en">English</option><option value="ru">Русский</option>
    </select>
  </label>;
}
