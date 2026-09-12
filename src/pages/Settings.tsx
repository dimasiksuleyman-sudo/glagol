import { BackupSection } from "@/components/settings/BackupSection";
import { DictationSection } from "@/components/settings/DictationSection";
import { TtsSection } from "@/components/settings/TtsSection";
export function Settings() {
  return <div className="space-y-6"><div><h2 className="text-2xl font-semibold tracking-tight">Настройки</h2><p className="mt-1 text-sm text-muted-foreground">Диктовка и озвучка настраиваются независимо.</p></div><DictationSection /><TtsSection /><BackupSection /></div>;
}
