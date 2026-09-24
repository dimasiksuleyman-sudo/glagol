import { Button } from "@/components/ui/button";
import { LanguageSwitch } from "@/components/LanguageSwitch";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { DictationSection } from "@/components/settings/DictationSection";
import { TtsSection } from "@/components/settings/TtsSection";
import { DictationSetupCard } from "@/components/DictationSetupCard";

export function Onboarding() {
  const { t } = useI18n();
  const { preferences, error, saving, reload, chooseLanguage, complete } = usePreferences();
  if (!preferences) return <main className="mx-auto max-w-xl space-y-4 p-12"><h1 className="text-2xl">Glagol</h1>
    <p role={error ? "alert" : "status"}>{error ? t("preferenceError") : t("loading")}</p>
    {error && <Button onClick={() => void reload()}>{t("retry")}</Button>}</main>;
  return <main className="mx-auto max-w-5xl space-y-6 p-8">
    <h1 className="text-3xl font-semibold">Glagol</h1>
    {preferences.onboarding_stage === "language" ? <>
      <h2 className="text-xl">Choose your interface language / Выберите язык интерфейса</h2>
      <div className="grid grid-cols-2 gap-4">
        <Button className="h-28 text-2xl" disabled={saving} onClick={() => void chooseLanguage("en").catch(() => undefined)}>English</Button>
        <Button className="h-28 text-2xl" variant="outline" disabled={saving} onClick={() => void chooseLanguage("ru").catch(() => undefined)}>Русский</Button>
      </div>
      <p className="text-muted-foreground">{t("languageNote")}</p>
    </> : <>
      <LanguageSwitch />
      <h2 className="text-xl">{t("setupSpeech")}</h2>
      <p className="text-muted-foreground">{t("setupNote")}</p>
      {preferences.upgraded && <p>{t("upgradeNote")}</p>}
      <div className="grid items-start gap-4 md:grid-cols-2"><DictationSetupCard /><TtsSection compact /></div>
      <details><summary className="cursor-pointer text-sm">{t("advancedSpeechSettings")}</summary><div className="mt-4"><DictationSection /></div></details>
      <Button disabled={saving} onClick={() => void complete().catch(() => undefined)}>{t("continue")}</Button>
    </>}
    {error && <p role="alert" className="text-destructive">{t("saveError")}</p>}
  </main>;
}
