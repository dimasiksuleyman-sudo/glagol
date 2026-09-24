import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import type { Language } from "@/i18n";
export function LanguageSwitch() {
  const { t, language } = useI18n();
  const { chooseLanguage, saving } = usePreferences();
  return <label className="flex items-center gap-2 text-sm">
    <span>{t("interfaceLanguage")}</span>
    <select aria-label={t("interfaceLanguage")} className="rounded-md border bg-background px-2 py-1" value={language} disabled={saving}
      onChange={event => void chooseLanguage(event.target.value as Language).catch(() => undefined)}>
      <option value="en">English</option><option value="ru">Русский</option>
    </select>
  </label>;
}
