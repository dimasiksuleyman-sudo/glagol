import { useEffect, useState } from "react";
import { t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { Navigate, NavLink, useParams } from "react-router-dom";
import { cn } from "@/lib/utils";
import { BackupSection } from "@/components/settings/BackupSection";
import { DictationSection } from "@/components/settings/DictationSection";
import { LanServerSection } from "@/components/settings/LanServerSection";
import { TtsSection } from "@/components/settings/TtsSection";
import { AiNormalizationSection } from "@/components/settings/AiNormalizationSection";
import { PronunciationSection } from "@/components/settings/PronunciationSection";
import { UpdatesSection } from "@/components/settings/UpdatesSection";
import { DiagnosticsSection } from "@/components/settings/DiagnosticsSection";
import { LanguageSwitch } from "@/components/LanguageSwitch";

/** Settings tabs; each has its own address so help can open the right one. */
export type SettingsTab = "dictation" | "speech" | "general";
const TABS: SettingsTab[] = ["dictation", "speech", "general"];

export function Settings() {
  useI18n();
  const { tab } = useParams();
  const { preferences } = usePreferences();
  const [visitedDictation, setVisitedDictation] = useState(tab === "dictation");
  useEffect(() => {
    if (tab === "dictation") setVisitedDictation(true);
  }, [tab]);
  if (!TABS.includes(tab as SettingsTab)) return <Navigate to="/settings/dictation" replace />;
  const label: Record<SettingsTab, string> = {
    dictation: t("Dictation"),
    speech: t("Text to speech"),
    general: t("General"),
  };
  return <div className="space-y-6">
    <div>
      <h2 className="text-2xl font-semibold tracking-tight">{t("Settings")}</h2>
      <p className="mt-1 text-sm text-muted-foreground">{t("Dictation and text to speech are configured independently.")}</p>
    </div>
    <nav role="tablist" aria-label={t("Settings")} className="flex gap-1 rounded-lg bg-muted p-1">
      {TABS.map(id => <NavLink key={id} to={`/settings/${id}`} role="tab" aria-selected={tab === id}
        className={({ isActive }) => cn("flex-1 rounded-md px-3 py-1.5 text-center text-sm font-medium transition-colors",
          isActive ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground")}>
        {label[id]}
      </NavLink>)}
    </nav>
    {(visitedDictation || tab === "dictation") && <div hidden={tab !== "dictation"}>
      <DictationSection />
    </div>}
    {tab === "dictation" && <LanServerSection />}
    {tab === "speech" && <><TtsSection />{preferences?.tts_enabled !== false && <><PronunciationSection /><AiNormalizationSection /></>}</>}
    {tab === "general" && <>
      <div data-tour="interface-language"><LanguageSwitch /></div>
      <UpdatesSection />
      <BackupSection />
      <DiagnosticsSection />
    </>}
  </div>;
}
