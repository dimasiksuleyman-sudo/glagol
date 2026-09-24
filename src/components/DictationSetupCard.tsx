import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { SttLanguageSwitch } from "@/components/SttLanguageSwitch";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { cancelModelDownload, downloadLocalModel, getSpeechSettings, localModelsStatus, saveSpeechSettings, type LocalModelsStatus, type SpeechProfile, type ModelProgress } from "@/lib/tauri";

export function DictationSetupCard() {
  const { t, locale } = useI18n();
  const { preferences } = usePreferences();
  const [status, setStatus] = useState<LocalModelsStatus | null>(null);
  const [profile, setProfile] = useState<SpeechProfile | null>(null);
  const [progress, setProgress] = useState<ModelProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const language = preferences?.stt_language ?? "en";
  useEffect(() => {
    let disposed = false;
    const refresh = () => void Promise.all([localModelsStatus(), getSpeechSettings()]).then(([status, settings]) => {
      if (!disposed) { setStatus(status); setProfile(settings.profile); setProgress(status.progress); }
    }).catch(e => { if (!disposed) setError(String(e)); });
    const changed = listen("local-models-changed", refresh);
    const downloading = listen<ModelProgress>("local-model-progress", ({ payload }) => { if (!disposed) setProgress(payload); });
    refresh();
    return () => { disposed = true; void changed.then(stop => stop()); void downloading.then(stop => stop()); };
  }, [language]);
  const model = status?.models.find(m => m.id === profile?.model && m.language === language) ?? status?.models.find(m => m.language === language);
  const selected = profile?.mode === "local" && profile.model === model?.id && model.installed && model.download_bytes === 0;
  async function install() {
    if (!model) return;
    setBusy(true); setError("");
    try {
      if (!model.installed || model.download_bytes > 0) await downloadLocalModel(model.id);
      await saveSpeechSettings({ mode: "local", model: model.id, language, base_url: "", proxy: "" });
      setProfile((await getSpeechSettings()).profile);
      setStatus(await localModelsStatus());
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); setProgress(null); }
  }
  return <Card><CardHeader><CardTitle>{t("dictation")}</CardTitle></CardHeader><CardContent className="space-y-4">
    <SttLanguageSwitch disabled={busy} />
    {profile && profile.mode !== "local" && <p className="text-sm break-all">{t("savedSpeechProfile", { mode: profile.mode === "server" ? t("Organization server") : t("Cloud service"), model: profile.model, language: profile.language })}<br />{profile.base_url}</p>}
    {model && <><p className="font-medium">{model.name}</p><p className="text-sm">{t("downloadRequired", { size: new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(model.download_bytes / 1e6) })}</p>
      <Button disabled={busy || selected || !status?.supported} onClick={() => void install()}>{selected ? t("Selected") : model.installed ? t("Use") : t("Download and use")}</Button></>}
    {progress && <div role="status"><progress className="w-full" value={progress.downloaded} max={progress.total} aria-label={t("Model download")} /><Button variant="outline" onClick={() => void cancelModelDownload().catch(e => setError(String(e)))}>{t("Cancel download")}</Button></div>}
    {error && <p role="alert" className="text-sm text-destructive">{error}</p>}
  </CardContent></Card>;
}
