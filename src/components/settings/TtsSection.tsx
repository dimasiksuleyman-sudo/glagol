import { currentLocale } from "@/i18n";
import { AudioPlayer } from "@/components/AudioPlayer";
import { t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useState, useEffect } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { useTts } from "@/contexts/TtsContext";
import { cancelTts, installTts, previewTts, removeTts } from "@/lib/tts";
import { voicesFor } from "@/lib/voices";
import { TtsLanguageSwitch } from "@/components/TtsLanguageSwitch";
const mb = (bytes: number) => (bytes / 1e6).toLocaleString(currentLocale(), { maximumFractionDigits: 1 });
export function TtsSection({ compact = false }: { compact?: boolean }) {
  useI18n();
  const { status, error, refresh, provider } = useTts();
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const { preferences, setTtsVoice } = usePreferences();
  const speechLanguage = preferences?.tts_language ?? "ru";
  const voice = (speechLanguage === "en" ? preferences?.tts_voice_en : preferences?.tts_voice_ru) ?? (speechLanguage === "en" ? "en_0" : "xenia");
  const VOICES = voicesFor(speechLanguage);
  const [audio, setAudio] = useState<string | null>(null);
  useEffect(() => { setChecked(false); setAudio(null); }, [provider]);
  useEffect(() => { setAudio(null); }, [voice]);
  async function action(fn: () => Promise<unknown>) {
    setBusy(true);
    try { await fn(); } catch (e) { toast.error(String(e)); }
    finally { setBusy(false); await refresh(); }
  }
  const active = busy || !!status?.progress;
  const accepted = checked || !!status?.accepted;
  const stage = status?.progress?.stage;
  return <Card id="local-tts">
    <CardHeader>
      <CardTitle>{t("Local text to speech — Silero")}</CardTitle>
      <CardDescription>{t("Optional. Works offline after downloading.")}</CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      <TtsLanguageSwitch disabled={active} />
      <p className="text-xs text-muted-foreground">{t("sharedRuntimeNote")}</p>
      {status && <p className="text-sm">{t("downloadRequired", {size: mb(status.download_bytes)})}</p>}
      <p className="text-sm">Silero Team · CC BY-NC-SA 4.0 · <strong>{t("for noncommercial use")}</strong>{t(". Download the model and runtime separately if you want to use them. Silero is not required for dictation, including office servers.")}</p>
      {error && <p role="alert" className="text-destructive text-sm">{error}</p>}
      {!status ? <p className="text-sm text-muted-foreground">{t("Checking status…")}</p> : <>
        <details className="text-sm"><summary className="cursor-pointer">{t("Full Silero license")}</summary><pre className="mt-2 max-h-64 overflow-auto whitespace-pre-wrap rounded border p-3 text-xs">{status.license}</pre></details>
        <p className="text-sm font-medium">{speechLanguage === "en" ? "Silero v3 English" : "Silero v5.5 Russian"}</p>
        {!compact && <p className="text-sm text-muted-foreground">{t("Model:")}{" "}{mb(status.model_bytes)} {t("MB · runtime:")}{" "}{mb(status.runtime_bytes)} {t("MB. Installation requires at least")}{" "}{mb(status.disk_bytes)} {t("MB.")}</p>}
        {!status.supported && <p className="text-sm">{t("Local text to speech currently supports Windows x64.")}</p>}
        {!status.accepted && <label className="flex items-start gap-2 text-sm"><input type="checkbox" checked={checked} disabled={active} onChange={e => setChecked(e.target.checked)} className="mt-1" />{t("I have read the Silero terms and will use it noncommercially.")}</label>}
        <p className="text-sm" aria-live="polite">{stage === "downloading" ? t("Downloading components…") : stage === "verifying" ? t("Verifying and installing components…") : stage === "preparing" ? t("Preparing speech…") : status.installed && status.accepted ? t("Ready") : t("Not installed")}</p>
        {status.progress && status.progress.total > 0 && <div className="space-y-1"><Progress value={100 * status.progress.downloaded / status.progress.total} /><p className="text-xs">{mb(status.progress.downloaded)} / {mb(status.progress.total)} {t("MB")}</p></div>}
        {status.error && <p role="alert" className="text-sm text-destructive">{status.error}</p>}
        <div className="flex flex-wrap gap-2">
          <Button disabled={active || !accepted || !status.supported} onClick={() => void action(() => installTts(accepted, status.license_hash, null, provider))}>{status.installed ? t("Verify and repair") : status.partial_bytes ? t("Resume download") : t("Download and enable")}</Button>
          <Button variant="outline" disabled={active || !accepted || !status.supported} onClick={() => void action(async () => {
            const path = await open({ multiple: false, filters: [{ name: status.model_file, extensions: ["pt"] }] });
            if (typeof path === "string") await installTts(accepted, status.license_hash, path, provider);
          })}>{t("Choose a downloaded model")}</Button>
          {active ? <Button variant="outline" onClick={() => void cancelTts().catch(e => toast.error(String(e)))}>{t("Cancel")}</Button> : (status.installed || status.partial_bytes > 0 || status.accepted) && <Button variant="outline" onClick={() => void action(async () => { setAudio(null); await removeTts(provider); setChecked(false); })}>{t("removeModel")}</Button>}
        </div>
        {status.installed && status.accepted && <div className="space-y-2">
          <label className="flex flex-wrap items-center gap-2 text-sm">{t("Voice")}<select className="rounded border bg-background p-2" value={voice} disabled={active} onChange={e => { setAudio(null); void setTtsVoice(e.target.value).catch(e => toast.error(String(e))); }}>{VOICES.map(v => <option key={v.id} value={v.id}>{v.label}</option>)}</select></label>
          <Button variant="outline" disabled={active} onClick={() => void action(async () => { setAudio(null); const path = await previewTts(voice, provider); setAudio(convertFileSrc(path) + "?v=" + Date.now()); })}>{t("Preview voice")}</Button>
          {audio && <AudioPlayer key={audio} autoPlay src={audio} />}
        </div>}
      </>}
    </CardContent>
  </Card>;
}
