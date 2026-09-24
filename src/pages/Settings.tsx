import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { BackupSection } from "@/components/settings/BackupSection";
import { DictationSection } from "@/components/settings/DictationSection";
import { TtsSection } from "@/components/settings/TtsSection";
import { LanguageSwitch } from "@/components/LanguageSwitch";
export function Settings() {
  useI18n();
  return <div className="space-y-6"><div><h2 className="text-2xl font-semibold tracking-tight">{t("Settings")}</h2><p className="mt-1 text-sm text-muted-foreground">{t("Dictation and text to speech are configured independently.")}</p></div><LanguageSwitch /><DictationSection /><TtsSection /><BackupSection /></div>;
}
