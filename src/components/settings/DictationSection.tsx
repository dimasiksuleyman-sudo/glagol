import { currentLocale } from "@/i18n";
import { t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { SttLanguageSwitch, useDictationBusy } from "@/components/SttLanguageSwitch";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  getSpeechSettings, saveSpeechSettings, testSpeechSettings, deleteSpeechKey,
  localModelsStatus, downloadLocalModel, cancelModelDownload, removeLocalModel,
  localEngineState, setLocalEngineOptions,
  type SpeechMode, type SpeechProfile, type LocalModelsStatus, type ModelProgress,
  type LocalEngineOptions, type LocalEngineState,
} from "@/lib/tauri";

const PRESETS = [
  { id: "aitunnel", label: "AITunnel", base_url: "https://api.aitunnel.ru/v1", model: "whisper-large-v3-turbo" },
  { id: "proxyapi", label: "ProxyAPI", base_url: "https://api.proxyapi.ru/openai/v1", model: "whisper-1" },
  { id: "vsegpt", label: "VseGPT", base_url: "https://api.vsegpt.ru/v1", model: "whisper-1" },
  { id: "groq", label: "Groq", base_url: "https://api.groq.com/openai/v1", model: "whisper-large-v3-turbo" },
];
const size = (bytes: number) => t("{p0} MB", { p0: new Intl.NumberFormat(currentLocale(), { maximumFractionDigits: 1 }).format(bytes / 1_000_000) });
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

export function DictationSection() {
  const { language } = useI18n();
  const { preferences } = usePreferences();
  const dictating = useDictationBusy();
  const MODES: Record<SpeechMode, string> = {
    local: t("On this computer"), server: t("Organization server"), cloud: t("Cloud service"),
  };
  const [profile, setProfile] = useState<SpeechProfile | null>(null);
  const [active, setActive] = useState<SpeechMode>("cloud");
  const [activeModel, setActiveModel] = useState("");
  const [models, setModels] = useState<LocalModelsStatus | null>(null);
  const [progress, setProgress] = useState<ModelProgress | null>(null);
  const [keyStored, setKeyStored] = useState(false);
  const [apiKey, setApiKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [loadError, setLoadError] = useState("");
  const [engine, setEngine] = useState<LocalEngineState | null>(null);
  const alive = useRef(true);

  useEffect(() => { void refreshModels().catch(() => undefined); }, [language]);
  useEffect(() => {
    if (profile?.mode !== "local") return;
    let disposed = false;
    void getSpeechSettings("local").then(settings => {
      if (!disposed) { setProfile(settings.profile); setActiveModel(settings.profile.model); }
    }).catch(() => undefined);
    return () => { disposed = true; };
  }, [preferences?.stt_language]);

  async function refreshModels() {
    const [status, engineState] = await Promise.all([localModelsStatus(), localEngineState()]);
    if (alive.current) { setModels(status); setProgress(status.progress); setEngine(engineState); }
  }
  useEffect(() => {
    alive.current = true;
    let cancelled = false;
    const progressListener = listen<ModelProgress>("local-model-progress", ({ payload }) => {
      if (!cancelled) setProgress(payload);
    });
    const changeListener = listen("local-models-changed", () => { if (!cancelled) void refreshModels().catch(() => {}); });
    void Promise.all([getSpeechSettings(), localModelsStatus()]).then(([settings, status]) => {
      if (cancelled) return;
      setProfile(settings.profile); setActive(settings.active_mode);
      setActiveModel(settings.profile.model); setKeyStored(settings.key_stored);
      setModels(status); setProgress(status.progress);
    }).catch(e => { if (!cancelled) setLoadError(errorText(e)); });
    void localEngineState().then(state => { if (!cancelled) setEngine(state); }).catch(() => undefined);
    return () => {
      cancelled = true; alive.current = false;
      void progressListener.then(unlisten => unlisten());
      void changeListener.then(unlisten => unlisten());
    };
  }, []);

  async function chooseMode(mode: SpeechMode) {
    setBusy(true); setMessage(""); setApiKey("");
    try {
      const settings = await getSpeechSettings(mode);
      if (alive.current) { setProfile(settings.profile); setKeyStored(settings.key_stored); }
    } catch (e) { toast.error(errorText(e)); }
    finally { setBusy(false); }
  }
  function update(change: Partial<SpeechProfile>) {
    setProfile(p => p ? { ...p, ...change } : p); setMessage("");
  }
  async function save(p: SpeechProfile, test = false) {
    await saveSpeechSettings(p, apiKey.trim() || undefined);
    if (alive.current) { setActive(p.mode); setActiveModel(p.model); setApiKey(""); }
    if (test) await testSpeechSettings();
    const settings = await getSpeechSettings(p.mode);
    if (alive.current) {
      setKeyStored(settings.key_stored);
      setMessage(test ? t("Connection works.") : p.mode === "local" ? t("Model ready. You can start dictating.") : t("Settings saved."));
    }
  }
  async function remoteSave(test: boolean) {
    if (!profile) return;
    setBusy(true); setMessage("");
    try { await save(profile, test); }
    catch (e) { setMessage(errorText(e)); toast.error(errorText(e)); }
    finally { setBusy(false); }
  }
  async function useModel(id: string, installed: boolean) {
    if (!profile) return;
    setBusy(true); setMessage(installed ? t("Preparing model…") : t("Downloading model…"));
    try {
      if (!installed || (models?.models.find(m => m.id === id)?.download_bytes ?? 0) > 0) await downloadLocalModel(id);
      if (!alive.current) return;
      setMessage(t("Preparing model…"));
      const p = { ...profile, mode: "local" as const, model: id, language: models?.models.find(m => m.id === id)?.language ?? profile.language };
      await save(p); setProfile(p);
    } catch (e) { if (alive.current) { setMessage(errorText(e)); toast.error(errorText(e)); } }
    finally { if (alive.current) { setBusy(false); await refreshModels(); } }
  }
  async function remove(id: string) {
    setBusy(true);
    try { await removeLocalModel(id); await refreshModels(); }
    catch (e) { toast.error(errorText(e)); }
    finally { setBusy(false); }
  }
  async function changeEngine(change: Partial<LocalEngineOptions>) {
    if (!engine) return;
    setBusy(true); setMessage(t("Applying…"));
    try {
      const state = await setLocalEngineOptions({ ...engine.options, ...change });
      if (alive.current) { setEngine(state); setMessage(state.gpu_fallback ? "" : t("Settings saved.")); }
    } catch (e) { if (alive.current) { setMessage(errorText(e)); toast.error(errorText(e)); } }
    finally { if (alive.current) setBusy(false); }
  }
  async function clearKey() {
    if (!profile) return;
    setBusy(true);
    try { await deleteSpeechKey(profile.mode); setKeyStored(false); setApiKey(""); setMessage(t("Key deleted.")); }
    catch (e) { toast.error(errorText(e)); }
    finally { setBusy(false); }
  }

  const locked = busy || progress !== null || dictating;
  const ctc = models?.models.find(m => m.id === "gigaam-v3-e2e-ctc");
  return <Card>
    <CardHeader>
      <CardTitle>{t("Dictation")}</CardTitle>
      <CardDescription>{t("Recognize speech on this computer, on your organization's server, or in the cloud.")}</CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      {loadError && <p role="alert" className="text-sm text-destructive">{loadError}</p>}
      {!profile && !loadError && <Skeleton className="h-32 w-full" />}
      {profile && <>
        <div className="space-y-2">
          <Label htmlFor="speech-mode">{t("Where to recognize speech")}</Label>
          <Select value={profile.mode} onValueChange={v => void chooseMode(v as SpeechMode)} disabled={locked}>
            <SelectTrigger id="speech-mode" className="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>{Object.entries(MODES).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectContent>
          </Select>
          <p className="text-xs text-muted-foreground">{t("Currently using:")}{" "}{MODES[active]}.</p>
        </div>
        {profile.mode === "local" ? <>
          <SttLanguageSwitch disabled={locked} />
          <p className="text-sm text-muted-foreground">{t("Download a model once to recognize speech offline on this computer. Glagol updates preserve downloaded models.")}</p>
          {models && !models.supported && <p role="alert">{t("Local models currently require Windows x64. Server connections also work on other systems.")}</p>}
          {models?.models.filter(m => m.language === (preferences?.stt_language ?? "en") || m.installed).map(m => {
            const selected = active === "local" && activeModel === m.id && m.installed && m.download_bytes === 0;
            return <div key={m.id} className="rounded-lg border p-4 space-y-2">
              <div className="flex flex-wrap justify-between gap-2"><span className="font-medium">{m.name}</span><span className="text-sm text-muted-foreground">{size(m.bytes)}{selected ? t(" · In use") : m.installed ? t(" · Downloaded") : ""}</span></div>
              <p className="text-sm text-muted-foreground">{m.description}</p>
              <p className="text-xs text-muted-foreground">{t("downloadRequired", { size: new Intl.NumberFormat(currentLocale(), { maximumFractionDigits: 1 }).format(m.download_bytes / 1_000_000) })}</p>
              <div className="flex flex-wrap gap-2">
                <Button disabled={locked || !models.supported || selected} onClick={() => void useModel(m.id, m.installed)}>
                  {selected ? t("Selected") : m.installed ? t("Use") : m.partial_bytes > 0 ? t("Resume and use") : t("Download and use")}
                </Button>
                {selected && <Button variant="outline" disabled={locked} onClick={() => void useModel(m.id, false)}>{t("Verify files and repair")}</Button>}
                {(m.installed || m.partial_bytes > 0) && <Button variant="outline" disabled={locked || selected} onClick={() => void remove(m.id)}>{t("Remove ·")}{" "}{size(m.installed ? m.bytes : m.partial_bytes)}</Button>}
              </div>
            </div>;
          })}
          {preferences?.stt_language === "ru" && engine && models?.supported && <div className="rounded-lg border p-4 space-y-3">
            <p className="font-medium">{t("Recognition speed")}</p>
            <p className="text-xs text-muted-foreground">{t("For the Russian model. Try the graphics card on a computer with an older processor; if it fails, Glagol uses the processor.")}</p>
            <div className="space-y-2">
              <Label htmlFor="stt-device">{t("Compute on")}</Label>
              <Select value={engine.options.device} onValueChange={device => void changeEngine({ device: device as LocalEngineOptions["device"] })} disabled={locked}>
                <SelectTrigger id="stt-device" className="w-full"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="cpu">{t("Processor")}</SelectItem><SelectItem value="gpu">{t("Graphics card (Vulkan)")}</SelectItem></SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label htmlFor="stt-threads">{t("Processor threads")}</Label>
              <Select value={String(engine.options.threads)} onValueChange={threads => void changeEngine({ threads: Number(threads) })} disabled={locked}>
                <SelectTrigger id="stt-threads" className="w-full"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem value="0">{t("Automatic ({p0})", { p0: engine.default_threads })}</SelectItem>
                  {Array.from({ length: Math.min(engine.logical_cpus, 64) }, (_, i) => i + 1).map(n => <SelectItem key={n} value={String(n)}>{String(n)}</SelectItem>)}
                </SelectContent>
              </Select>
            </div>
            {engine.backend && <p className="text-xs text-muted-foreground">{t("Now computing on: {p0}", { p0: engine.backend })}</p>}
            {engine.options.device === "cpu" && engine.gpu_name && <SpeedHint
              text={t("This computer has a graphics card ({p0}). Recognition on it is usually several times faster.", { p0: engine.gpu_name })}
              action={t("Use the graphics card")} disabled={locked} onClick={() => void changeEngine({ device: "gpu" })} />}
            {active === "local" && ctc && activeModel !== ctc.id && activeModel.startsWith("gigaam-") && <SpeedHint
              text={t("GigaAM CTC is faster than RNNT, especially on a graphics card. Compare the quality on your own phrases.")}
              action={ctc.installed && ctc.download_bytes === 0 ? t("Switch to CTC") : t("Download CTC ({p0})", { p0: size(ctc.download_bytes) })}
              disabled={locked} onClick={() => void useModel(ctc.id, ctc.installed)} />}
            {engine.gpu_fallback && <p role="alert" className="text-sm text-destructive">{t("The graphics card could not be used, so the processor is used. Details are in the log.")}</p>}
          </div>}
          {progress && <div className="space-y-2" role="status">
            <p className="text-sm">{progress.stage === "verifying" ? t("Verifying files…") : t("Downloaded {p0} of {p1}", { p0: size(progress.downloaded), p1: size(progress.total) })}</p>
            <progress className="h-2 w-full accent-primary" value={progress.downloaded} max={progress.total} aria-label={t("Model download")} />
            <Button variant="outline" disabled={progress.stage === "verifying"} onClick={() => void cancelModelDownload().catch(e => toast.error(errorText(e)))}>{t("Cancel download")}</Button>
          </div>}
        </> : <>
          {profile.mode === "server" ? <p className="text-sm text-muted-foreground">{t("One speech server can serve every computer in your office. No local model download is needed. Servers with the /v1/audio/transcriptions API are supported.")}</p> : <div className="space-y-2">
            <Label htmlFor="speech-provider">{t("Provider")}</Label>
            <Select value={PRESETS.find(p => p.base_url === profile.base_url)?.id ?? "custom"} disabled={locked} onValueChange={id => {
              const p = PRESETS.find(p => p.id === id); if (p) update({ base_url: p.base_url, model: p.model });
              else update({ base_url: "", model: "" });
            }}>
              <SelectTrigger id="speech-provider" className="w-full"><SelectValue /></SelectTrigger>
              <SelectContent>{PRESETS.map(p => <SelectItem key={p.id} value={p.id}>{p.label}</SelectItem>)}<SelectItem value="custom">{t("Custom service")}</SelectItem></SelectContent>
            </Select>
          </div>}
          <div className="space-y-2">
            <Label htmlFor="speech-url">{profile.mode === "server" ? t("serverAddress") : t("serviceAddress")}</Label>
            <Input id="speech-url" value={profile.base_url} onChange={e => update({ base_url: e.target.value })} disabled={locked} spellCheck={false} autoComplete="off" placeholder={profile.mode === "server" ? "http://192.168.1.10:8000/v1" : "https://api.example.com/v1"} />
            <p className="text-xs text-muted-foreground">{t("Include the /v1 path.")}{profile.mode === "server" ? t(" HTTP is allowed for private IP addresses and localhost; use HTTPS for domain names.") : t(" Use HTTPS for external addresses.")}</p>
            {profile.mode === "server" && profile.base_url.startsWith("http://") && <p className="text-xs text-muted-foreground">{t("HTTP sends speech and keys without encryption. Use it only on a trusted office network.")}</p>}
          </div>
          <div className="space-y-2"><Label htmlFor="speech-model">{t("Model name on server")}</Label><Input id="speech-model" value={profile.model} onChange={e => update({ model: e.target.value })} disabled={locked} spellCheck={false} /></div>
          <div className="space-y-2">
            <Label htmlFor="speech-language">{t("Language")}</Label>
            <Select value={profile.language} onValueChange={language => update({ language })} disabled={locked}>
              <SelectTrigger id="speech-language" className="w-full"><SelectValue /></SelectTrigger>
              <SelectContent><SelectItem value="ru">{t("Russian")}</SelectItem><SelectItem value="en">{t("English")}</SelectItem><SelectItem value="auto">{t("Auto-detect")}</SelectItem></SelectContent>
            </Select>
          </div>
          <div className="space-y-2"><Label htmlFor="speech-proxy">{t("Dictation proxy (optional)")}</Label><Input id="speech-proxy" value={profile.proxy} onChange={e => update({ proxy: e.target.value })} disabled={locked} autoComplete="off" placeholder={t("host:port or socks5://host:port")} /></div>
          <div className="space-y-2">
            <Label htmlFor="speech-key">{t("API key")}{profile.mode === "server" ? t(" (if required by the server)") : ""}</Label>
            <Input id="speech-key" type="password" value={apiKey} onChange={e => setApiKey(e.target.value)} disabled={locked} autoComplete="off" placeholder={keyStored ? t("A key is saved for this profile") : t("Enter a key")} />
            <p className="text-xs text-muted-foreground">{t("Cloud and server keys are stored separately. Enter a new key when changing the service address.")}</p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button disabled={locked} onClick={() => void remoteSave(false)}>{t("Save and use")}</Button>
            <Button variant="secondary" disabled={locked} onClick={() => void remoteSave(true)}>{t("Save and test")}</Button>
            <Button variant="outline" disabled={locked || !keyStored} onClick={() => void clearKey()}>{t("Delete key")}</Button>
          </div>
        </>}
        {message && <p role="status" className="text-sm">{message}</p>}
      </>}
    </CardContent>
  </Card>;
}

/** A recommendation with a one-click action inside «Recognition speed». */
function SpeedHint({ text, action, disabled, onClick }: { text: string; action: string; disabled: boolean; onClick: () => void }) {
  return <div className="flex flex-wrap items-center justify-between gap-2 rounded-md bg-muted p-3">
    <p className="text-sm">{text}</p>
    <Button size="sm" variant="outline" disabled={disabled} onClick={onClick}>{action}</Button>
  </div>;
}
