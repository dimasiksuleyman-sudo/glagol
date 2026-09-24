import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useTts } from "@/contexts/TtsContext";
import type { Language } from "@/i18n";
export function TtsLanguageSwitch({ disabled = false }: { disabled?: boolean }) {
  const { t } = useI18n();
  const { preferences, saving, setTtsLanguage } = usePreferences();
  const { status } = useTts();
  return <label className="flex items-center gap-2 text-sm">
    <span>{t("speechLanguage")}</span>
    <select className="rounded border bg-background p-2" value={preferences?.tts_language ?? "ru"}
      disabled={disabled || saving || !!status?.progress}
      onChange={e => void setTtsLanguage(e.target.value as Language).catch(() => undefined)}>
      <option value="en">English</option><option value="ru">Русский</option>
    </select>
  </label>;
}
