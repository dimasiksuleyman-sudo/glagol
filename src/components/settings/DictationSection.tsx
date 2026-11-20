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
  checkSavedSpeechModel, getSpeechSettings, listServerSpeechModels, saveSpeechSettings, testSpeechSettings, deleteSpeechKey, inspectLanConnectionCode,
  localModelsStatus, downloadLocalModel, cancelModelDownload, removeLocalModel,
  localEngineState, setLocalEngineOptions, DICTATION_STATE_EVENT, SPEECH_MODEL_STATUS_EVENT,
  type DictationState,
  type SpeechMode, type SpeechModelStatus, type SpeechProfile, type SpeechSettings, type LocalModel, type LocalModelsStatus, type ModelProgress,
  type LocalEngineOptions, type LocalEngineState,
} from "@/lib/tauri";

const PRESETS = [
  { id: "aitunnel", label: "AITunnel", base_url: "https://api.aitunnel.ru/v1", model: "whisper-large-v3-turbo" },
  { id: "proxyapi", label: "ProxyAPI", base_url: "https://api.proxyapi.ru/openai/v1", model: "whisper-1" },
  { id: "vsegpt", label: "VseGPT", base_url: "https://api.vsegpt.ru/v1", model: "whisper-1" },
  { id: "groq", label: "Groq", base_url: "https://api.groq.com/openai/v1", model: "whisper-large-v3-turbo" },
];
const MAX_VISIBLE_SERVER_MODELS = 100;
const size = (bytes: number) => t("{p0} MB", { p0: new Intl.NumberFormat(currentLocale(), { maximumFractionDigits: 1 }).format(bytes / 1_000_000) });
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

type RemoteDraft = { profile: SpeechProfile; apiKey: string; connectionCode: string };

function hasModelFiles(model: LocalModel) {
  return model.installed && model.download_bytes === 0;
}

export function DictationSection() {
  const { language } = useI18n();
  const { preferences } = usePreferences();
  const dictating = useDictationBusy();
  const MODES: Record<SpeechMode, string> = {
    local: t("On this computer"), server: t("Organization server"), cloud: t("Cloud service"),
  };
  const [profile, setProfile] = useState<SpeechProfile | null>(null);
  const [savedProfile, setSavedProfile] = useState<SpeechProfile | null>(null);
  const [active, setActive] = useState<SpeechMode>("cloud");
  const [activeModel, setActiveModel] = useState("");
  const [models, setModels] = useState<LocalModelsStatus | null>(null);
  const [progress, setProgress] = useState<ModelProgress | null>(null);
  const [keyStored, setKeyStored] = useState(false);
  const [modelStatuses, setModelStatuses] = useState<Partial<Record<SpeechMode, SpeechModelStatus>>>({});
  const [connectionCode, setConnectionCode] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [serverModels, setServerModels] = useState<string[] | null>(null);
  const [modelFilter, setModelFilter] = useState("");
  const [loadingServerModels, setLoadingServerModels] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [loadError, setLoadError] = useState("");
  const [engine, setEngine] = useState<LocalEngineState | null>(null);
  const alive = useRef(true);
  const remoteDrafts = useRef<Partial<Record<"cloud" | "server", RemoteDraft>>>({});

  useEffect(() => { void refreshModels().catch(() => undefined); }, [language]);
  useEffect(() => {
    if (profile?.mode !== "local") return;
    let disposed = false;
    void getSpeechSettings("local").then(settings => {
      if (!disposed) {
        setProfile(current => current?.mode === "local" ? settings.profile : current);
        if (settings.active_mode === "local") setActiveModel(settings.profile.model);
      }
    }).catch(() => undefined);
    return () => { disposed = true; };
  }, [preferences?.stt_language]);

  function rememberModelStatus(settings: SpeechSettings) {
    setModelStatuses(previous => ({ ...previous, [settings.profile.mode]: settings.model_status }));
  }

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
    // Refresh the recent-speed line after each finished dictation.
    const doneListener = listen<DictationState>(DICTATION_STATE_EVENT, ({ payload }) => {
      if (!cancelled && payload.kind === "done") void localEngineState().then(setEngine).catch(() => undefined);
    });
    const modelStatusListener = listen<SpeechMode>(SPEECH_MODEL_STATUS_EVENT, ({ payload: mode }) => {
      void getSpeechSettings(mode).then(settings => {
        if (!cancelled) rememberModelStatus(settings);
      }).catch(() => undefined);
    });
    void Promise.all([getSpeechSettings(), localModelsStatus()]).then(([settings, status]) => {
      if (cancelled) return;
      setProfile(settings.profile); setSavedProfile(settings.profile); setActive(settings.active_mode);
      setActiveModel(settings.profile.model); setKeyStored(settings.key_stored);
      rememberModelStatus(settings);
      setModels(status); setProgress(status.progress);
    }).catch(e => { if (!cancelled) setLoadError(errorText(e)); });
    void localEngineState().then(state => { if (!cancelled) setEngine(state); }).catch(() => undefined);
    return () => {
      cancelled = true; alive.current = false;
      void progressListener.then(unlisten => unlisten());
      void changeListener.then(unlisten => unlisten());
      void doneListener.then(unlisten => unlisten());
      void modelStatusListener.then(unlisten => unlisten());
    };
  }, []);

  function clearServerModelList() {
    setServerModels(null);
    setModelFilter("");
  }
  async function chooseMode(mode: SpeechMode) {
    setBusy(true); setMessage("");
    try {
      const [settings, freshModels] = await Promise.all([
        getSpeechSettings(mode),
        mode === "local" ? localModelsStatus() : null,
      ]);
      if (!alive.current) return;
      if (profile && profile.mode !== "local") {
        if (remoteEdited || connectionCode.trim()) {
          remoteDrafts.current[profile.mode] = { profile, apiKey, connectionCode };
        } else {
          delete remoteDrafts.current[profile.mode];
        }
      }
      const draft = mode === "local" ? undefined : remoteDrafts.current[mode];
      clearServerModelList();
      setProfile(draft?.profile ?? settings.profile);
      setSavedProfile(settings.profile);
      setKeyStored(settings.key_stored);
      setApiKey(draft?.apiKey ?? "");
      setConnectionCode(draft?.connectionCode ?? "");
      rememberModelStatus(settings);

      if (freshModels) {
        setModels(freshModels);
        setProgress(freshModels.progress);
        const selected = freshModels.models.find(m => m.id === settings.profile.model);
        if (mode !== settings.active_mode && freshModels.supported && selected && hasModelFiles(selected)) {
          setMessage(t("Preparing model…"));
          await save(settings.profile);
        }
      }
    } catch (e) {
      if (alive.current) { setMessage(errorText(e)); toast.error(errorText(e)); }
    } finally { if (alive.current) setBusy(false); }
  }
  function update(change: Partial<SpeechProfile>) {
    if (change.base_url !== undefined || change.proxy !== undefined) clearServerModelList();
    setProfile(p => p ? { ...p, ...change } : p); setMessage("");
  }
  async function loadServerModels() {
    if (profile?.mode !== "server") return;
    setBusy(true);
    setLoadingServerModels(true);
    setMessage("");
    clearServerModelList();
    try {
      const ids = await listServerSpeechModels(profile, apiKey.trim() || undefined);
      if (alive.current) setServerModels(ids);
    } catch (e) {
      if (alive.current) toast.error(errorText(e));
    } finally {
      if (alive.current) {
        setLoadingServerModels(false);
        setBusy(false);
      }
    }
  }
  async function save(p: SpeechProfile, test = false) {
    await saveSpeechSettings(p, p.mode === "local" ? undefined : apiKey.trim() || undefined);
    if (alive.current) {
      setActive(p.mode); setActiveModel(p.model); setApiKey(""); setSavedProfile(p);
      if (p.mode !== "local") delete remoteDrafts.current[p.mode];
    }
    const settings = await getSpeechSettings(p.mode);
    if (alive.current) {
      setProfile(settings.profile); setSavedProfile(settings.profile);
      setKeyStored(settings.key_stored); rememberModelStatus(settings);
    }
    if (test) await testSpeechSettings();
    if (alive.current) {
      setMessage(test || p.mode === "server" ? t("Connection works.") : p.mode === "local" ? t("Model ready. You can start dictating.") : t("Settings saved."));
    }
  }
  /** «Paste connection code»: a Glagol server on another computer (GL-010). */
  async function connectByCode() {
    if (!profile || !connectionCode.trim()) return;
    setBusy(true); setMessage(t("Checking the server model…"));
    try {
      const found = await inspectLanConnectionCode(connectionCode);
      const next: SpeechProfile = { ...profile, mode: "server", base_url: found.base_url, model: found.model, language: "ru", proxy: "" };
      await saveSpeechSettings(next, found.key);
      const settings = await getSpeechSettings("server");
      if (alive.current) {
        clearServerModelList();
        setProfile(settings.profile); setSavedProfile(settings.profile); setKeyStored(settings.key_stored);
        delete remoteDrafts.current.server;
        rememberModelStatus(settings);
        setActive("server"); setActiveModel(next.model); setApiKey(""); setConnectionCode("");
        const connected = t("Connected to Glagol at {p0}. You can start dictating.", { p0: found.base_url.replace(/^http:\/\//, "").replace(/\/v1$/, "") });
        setMessage(connected);
        // The status line can be below the visible area; a toast is always seen.
        toast.success(connected);
      }
    } catch (e) { setMessage(errorText(e)); toast.error(errorText(e)); }
    finally { setBusy(false); }
  }
  async function remoteSave(test: boolean) {
    if (!profile) return;
    setBusy(true); setMessage(profile.mode === "server" ? t("Checking the server model…") : "");
    try { await save(profile, test); }
    catch (e) { setMessage(errorText(e)); toast.error(errorText(e)); }
    finally { setBusy(false); }
  }
  async function useModel(model: LocalModel, repair = false) {
    if (!profile) return;
    setBusy(true); setMessage(repair || hasModelFiles(model) ? t("Preparing model…") : t("Downloading model…"));
    try {
      if (repair || !hasModelFiles(model)) await downloadLocalModel(model.id);
      if (!alive.current) return;
      setMessage(t("Preparing model…"));
      await save({ ...profile, mode: "local", model: model.id, language: model.language });
    } catch (e) { if (alive.current) { setMessage(errorText(e)); toast.error(errorText(e)); } }
    finally { if (alive.current) { setBusy(false); await refreshModels(); } }
  }
  async function remove(id: string) {
    setBusy(true);
    try { await removeLocalModel(id); await refreshModels(); }
    catch (e) { toast.error(errorText(e)); }
    finally { setBusy(false); }
  }
  async function applyRecommendation() {
    if (!engine) return;
    const recommendation = engine.recommendation;
    const model = models?.models.find(m => m.id === recommendation.model);
    if (!model) return;
    if (active !== "local" || activeModel !== recommendation.model) await useModel(model);
    if (engine.options.device !== recommendation.device) await changeEngine({ device: recommendation.device });
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
  async function checkModel() {
    if (!profile || profile.mode === "local") return;
    const mode = profile.mode;
    setBusy(true);
    setMessage(t("Checking the saved model…"));
    try {
      const result = await checkSavedSpeechModel(mode);
      if (!alive.current) return;
      if (result === "available") toast.success(t("The saved model is in the provider’s list."));
      if (result === "unknown") toast.info(t("The provider did not return a usable model list."));
    } catch (e) {
      if (alive.current) toast.error(errorText(e));
    } finally {
      if (alive.current) {
        setMessage("");
        setBusy(false);
      }
    }
  }

  async function clearKey() {
    if (!profile) return;
    const mode = profile.mode;
    setBusy(true);
    try {
      await deleteSpeechKey(mode);
      if (!alive.current) return;
      clearServerModelList();
      setKeyStored(false);
      setApiKey("");
      setMessage(t("Key deleted."));
      const settings = await getSpeechSettings(mode);
      if (alive.current) rememberModelStatus(settings);
    } catch (e) { toast.error(errorText(e)); }
    finally { setBusy(false); }
  }

  const locked = busy || progress !== null || dictating;
  const remoteEdited = !!profile && profile.mode !== "local" && (
    !!apiKey.trim() || profile.base_url !== savedProfile?.base_url ||
    profile.model !== savedProfile?.model || profile.language !== savedProfile?.language ||
    profile.proxy !== savedProfile?.proxy
  );
  const codePending = profile?.mode === "server" && !!connectionCode.trim();
  const unsaved = !!profile && (profile.mode !== active || remoteEdited || codePending);
  const localChoice = profile?.mode === "local" ? models?.models.find(m => m.id === profile.model) : undefined;
  const modelStatus = profile ? modelStatuses[profile.mode] : undefined;
  const modelQuery = modelFilter.trim().toLowerCase();
  const matchingServerModels = serverModels?.filter(id => id.toLowerCase().includes(modelQuery)) ?? [];
  const visibleServerModels = matchingServerModels.slice(0, MAX_VISIBLE_SERVER_MODELS);
  const ctc = models?.models.find(m => m.id === "gigaam-v3-e2e-ctc");
  const visibleModels = models?.models.filter(m => m.language === (preferences?.stt_language ?? "en") || m.installed) ?? [];
  const activeLocalModel = active === "local" ? models?.models.find(m => m.id === activeModel && m.installed && m.download_bytes === 0) : undefined;
  const recommendationApplied = !!engine && active === "local" && activeModel === engine.recommendation.model && engine.options.device === engine.recommendation.device;
  const recommendationText = !engine ? "" : engine.recommendation.kind === "graphics_card"
    ? t("Recommended for this computer: GigaAM CTC on the graphics card ({p0}).", { p0: engine.gpu_name ?? "" })
    : engine.recommendation.kind === "processor"
      ? t("Recommended for this computer: GigaAM CTC on the processor (it supports AVX2).")
      : t("This processor has no AVX2 and no graphics card was found, so local recognition is slow. Recommended: GigaAM CTC on the processor, or an organization server or cloud service.");
  return <Card>
    <CardHeader>
      <CardTitle>{t("Speech recognition")}</CardTitle>
      <CardDescription>{t("Recognize speech on this computer, on your organization's server, or in the cloud.")}</CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      {loadError && <p role="alert" className="text-sm text-destructive">{loadError}</p>}
      {!profile && !loadError && <Skeleton className="h-32 w-full" />}
      {profile && <>
        <div className="space-y-2">
          <Label htmlFor="speech-mode" data-tour="speech-mode">{t("Where to recognize speech")}</Label>
          <Select value={profile.mode} onValueChange={v => void chooseMode(v as SpeechMode)} disabled={locked}>
            <SelectTrigger id="speech-mode" className="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>{Object.entries(MODES).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectContent>
          </Select>
          {profile.mode !== active && <p className="text-xs text-muted-foreground">{t("Currently using:")}{" "}{MODES[active]}.</p>}
        </div>
        {unsaved && <div className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-amber-500/50 bg-amber-500/10 p-3">
          <p role="status" className="text-sm font-medium">{t("There are unsaved changes")}</p>
          {profile.mode === "local" ? localChoice && <Button size="sm" disabled={locked || !models?.supported}
            onClick={() => void useModel(localChoice)}>
            {hasModelFiles(localChoice) ? t("Use") : localChoice.partial_bytes > 0 ? t("Resume and use") : t("Download and use")}
          </Button> : codePending
            ? <Button size="sm" disabled={locked} onClick={() => void connectByCode()}>{t("Connect")}</Button>
            : <Button size="sm" disabled={locked} onClick={() => void remoteSave(false)}>{t("Save and use")}</Button>}
        </div>}
        {profile.mode === "local" ? <>
          <SttLanguageSwitch disabled={locked} />
          {models && !models.supported && <p role="alert">{t("Local models currently require Windows x64. Server connections also work on other systems.")}</p>}
          {activeLocalModel && <p className="text-sm" data-tour="active-model">{t("Model in use: {p0}", { p0: activeLocalModel.name })}</p>}
          {preferences?.stt_language === "ru" && engine && models?.supported && <div className="rounded-lg border p-4 space-y-3" data-tour="speed">
            <p className="font-medium">{t("Recognition speed")}</p>
            {active === "local" && ctc && activeModel.startsWith("gigaam-") && (recommendationApplied
              ? <p className="text-xs text-muted-foreground" data-tour="recommendation">{t("Recommended settings for this computer are in use.")}</p>
              : <SpeedHint
                tour="recommendation"
                text={recommendationText}
                action={ctc.installed && ctc.download_bytes === 0 ? t("Apply recommendation") : t("Download CTC and apply ({p0})", { p0: size(ctc.download_bytes) })}
                disabled={locked} onClick={() => void applyRecommendation()} />)}
            {recommendationApplied && engine.recommendation.kind === "slow_processor" && <p className="text-xs text-muted-foreground">{recommendationText}</p>}
            <div className="space-y-2">
              <Label htmlFor="stt-device">{t("Compute on")}</Label>
              <Select value={engine.options.device} onValueChange={device => void changeEngine({ device: device as LocalEngineOptions["device"] })} disabled={locked}>
                <SelectTrigger id="stt-device" className="w-full"><SelectValue /></SelectTrigger>
                <SelectContent><SelectItem value="cpu">{t("Processor")}</SelectItem><SelectItem value="gpu">{t("Graphics card (Vulkan)")}</SelectItem></SelectContent>
              </Select>
            </div>
            {engine.backend && <p className="text-xs text-muted-foreground">{t("Now computing on: {p0}", { p0: engine.backend })}</p>}
            {!engine.hardware_known && <p className="text-xs text-muted-foreground">{t("The graphics card is detected once the local engine has been loaded.")}</p>}
            {engine.recent_rtf !== null && <p className="text-xs text-muted-foreground">{t("Recent dictations: about {p0} s of waiting per 10 s of speech.", { p0: new Intl.NumberFormat(currentLocale(), { maximumFractionDigits: 1 }).format(engine.recent_rtf * 10) })}</p>}
            {engine.recent_rtf !== null && engine.recent_rtf > 0.5 && recommendationApplied && engine.recommendation.kind !== "slow_processor" && <p className="text-sm">{t("Recognition is still slow on this computer. An organization server or cloud service can be faster.")}</p>}
            {engine.gpu_fallback && <p role="alert" className="text-sm text-destructive">{t("The graphics card could not be used, so the processor is used. Details are in the log.")}</p>}
            <details className="text-sm">
              <summary className="cursor-pointer text-muted-foreground">{t("Advanced")}</summary>
              <div className="mt-3 space-y-2">
                <p className="text-xs text-muted-foreground">{t("For the Russian model. Try the graphics card on a computer with an older processor; if it fails, Glagol uses the processor.")}</p>
                <Label htmlFor="stt-threads">{t("Processor threads")}</Label>
                <Select value={String(engine.options.threads)} onValueChange={threads => void changeEngine({ threads: Number(threads) })} disabled={locked}>
                  <SelectTrigger id="stt-threads" className="w-full"><SelectValue /></SelectTrigger>
                  <SelectContent>
                    <SelectItem value="0">{t("Automatic ({p0})", { p0: engine.default_threads })}</SelectItem>
                    {Array.from({ length: Math.min(engine.logical_cpus, 64) }, (_, i) => i + 1).map(n => <SelectItem key={n} value={String(n)}>{String(n)}</SelectItem>)}
                  </SelectContent>
                </Select>
              </div>
            </details>
          </div>}
          <details className="rounded-lg border" open={!activeLocalModel} data-tour="models">
            <summary className="cursor-pointer p-4 font-medium">{t("Manage models")}</summary>
            <div className="space-y-3 px-4 pb-4">
              <p className="text-sm text-muted-foreground">{t("Download a model once to recognize speech offline on this computer. Glagol updates preserve downloaded models.")}</p>
              {visibleModels.map(m => {
                const selected = active === "local" && activeModel === m.id && m.installed && m.download_bytes === 0;
                return <div key={m.id} className="rounded-md border p-3 space-y-2">
                  <div className="flex flex-wrap justify-between gap-2"><span className="font-medium">{m.name}</span><span className="text-sm text-muted-foreground">{size(m.bytes)}{selected ? t(" · In use") : m.installed ? t(" · Downloaded") : ""}</span></div>
                  <p className="text-xs text-muted-foreground">{m.description}</p>
                  {m.download_bytes > 0 && <p className="text-xs text-muted-foreground">{t("downloadRequired", { size: new Intl.NumberFormat(currentLocale(), { maximumFractionDigits: 1 }).format(m.download_bytes / 1_000_000) })}</p>}
                  <div className="flex flex-wrap gap-2">
                    {selected
                      ? <Button size="sm" variant="outline" disabled={locked} onClick={() => void useModel(m, true)}>{t("Verify files and repair")}</Button>
                      : <Button size="sm" disabled={locked || !models?.supported} onClick={() => void useModel(m)}>
                        {hasModelFiles(m) ? t("Use") : m.partial_bytes > 0 ? t("Resume and use") : t("Download and use")}
                      </Button>}
                    {!selected && (m.installed || m.partial_bytes > 0) && <Button size="sm" variant="outline" disabled={locked} onClick={() => void remove(m.id)}>{t("Remove ·")}{" "}{size(m.installed ? m.bytes : m.partial_bytes)}</Button>}
                  </div>
                </div>;
              })}
            </div>
          </details>
          {progress && <div className="space-y-2" role="status">
            <p className="text-sm">{progress.stage === "verifying" ? t("Verifying files…") : t("Downloaded {p0} of {p1}", { p0: size(progress.downloaded), p1: size(progress.total) })}</p>
            <progress className="h-2 w-full accent-primary" value={progress.downloaded} max={progress.total} aria-label={t("Model download")} />
            <Button variant="outline" disabled={progress.stage === "verifying"} onClick={() => void cancelModelDownload().catch(e => toast.error(errorText(e)))}>{t("Cancel download")}</Button>
          </div>}
        </> : <>
          {profile.mode === "server" && <div className="space-y-2 rounded-lg border p-3" data-tour="lan-connect">
            <Label htmlFor="lan-code">{t("Glagol on another computer")}</Label>
            <p className="text-xs text-muted-foreground">{t("On the server computer: Settings → Dictation → Server for other computers → Copy connection code. Paste it here.")}</p>
            <div className="flex gap-2">
              <Input id="lan-code" value={connectionCode} onChange={e => setConnectionCode(e.target.value)} disabled={locked} spellCheck={false} autoComplete="off" placeholder="glagol://192.168.1.10:8765/?key=…" className="font-mono text-xs" />
              <Button disabled={locked || !connectionCode.trim()} onClick={() => void connectByCode()}>{t("Connect")}</Button>
            </div>
          </div>}
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
          {profile.mode === "cloud" && <div className="space-y-2"><Label htmlFor="speech-model">{t("Model name on server")}</Label><Input id="speech-model" value={profile.model} onChange={e => update({ model: e.target.value })} disabled={locked} spellCheck={false} /></div>}
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
            <Input id="speech-key" type="password" value={apiKey} onChange={e => { setApiKey(e.target.value); clearServerModelList(); }} disabled={locked} autoComplete="off" placeholder={keyStored ? t("A key is saved for this profile") : t("Enter a key")} />
            <p className="text-xs text-muted-foreground">{t("Cloud and server keys are stored separately. Enter a new key when changing the service address.")}</p>
          </div>
          {profile.mode === "server" && <div className="space-y-3" data-tour="server-model-picker">
            <div className="space-y-2">
              <Label htmlFor="speech-model">{t("Model name on server")}</Label>
              <div className="flex flex-wrap gap-2">
                <Input id="speech-model" className="min-w-40 flex-1" value={profile.model}
                  onChange={e => update({ model: e.target.value })} disabled={locked} spellCheck={false} />
                <Button type="button" variant="outline" disabled={locked || !profile.base_url.trim()}
                  onClick={() => void loadServerModels()}>
                  {loadingServerModels ? t("Loading server models…") : t("Load models")}
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">{t("Glagol checks the selected model with a short built-in audio clip before saving.")}</p>
            </div>
            {serverModels?.length === 0 && <p role="status" className="text-sm text-muted-foreground">
              {t("The server did not return a model list. Enter the model name manually.")}
            </p>}
            {!!serverModels?.length && <div className="space-y-2">
              <Label htmlFor="server-model-filter">{t("Filter models")}</Label>
              <Input id="server-model-filter" value={modelFilter} disabled={locked} spellCheck={false}
                onChange={e => setModelFilter(e.target.value)} />
              {visibleServerModels.length ? <Select
                value={visibleServerModels.includes(profile.model) ? profile.model : ""}
                onValueChange={model => update({ model })} disabled={locked}>
                <SelectTrigger className="w-full" aria-label={t("Choose a server model")}>
                  <SelectValue placeholder={t("Choose a server model")} />
                </SelectTrigger>
                <SelectContent position="popper">
                  {visibleServerModels.map(id => <SelectItem key={id} value={id}>{id}</SelectItem>)}
                </SelectContent>
              </Select> : <p role="status" className="text-sm text-muted-foreground">{t("No matching server models.")}</p>}
              {matchingServerModels.length > MAX_VISIBLE_SERVER_MODELS && <p className="text-xs text-muted-foreground">
                {t("Showing the first {p0} models. Narrow the filter to see more.", { p0: MAX_VISIBLE_SERVER_MODELS })}
              </p>}
            </div>}
          </div>}
          <div className="space-y-2 rounded-lg border p-3" data-tour="saved-speech-model">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <p className="text-sm font-medium">{t("Saved model availability")}</p>
              <Button variant="outline" size="sm" disabled={locked || !modelStatus?.configured}
                onClick={() => void checkModel()}>{t("Check saved model")}</Button>
            </div>
            <p className="text-sm text-muted-foreground">
              {modelCheckResult(modelStatus)}
              {modelStatus?.checked_at != null && <>{" "}{t("Last checked {p0}.", {
                p0: new Intl.DateTimeFormat(currentLocale(), { dateStyle: "medium", timeStyle: "short" }).format(modelStatus.checked_at),
              })}</>}
            </p>
            {modelStatus?.missing && (
              <p role="alert" className="rounded-md border border-amber-500/50 bg-amber-500/10 p-2 text-sm">
                {t("The saved model is missing from the provider’s list. Choose another model from the list.")}
              </p>
            )}
          </div>
          <div className="flex flex-wrap gap-2">
            <Button disabled={locked} onClick={() => void remoteSave(false)}>{t("Save and use")}</Button>
            {profile.mode === "cloud" && <Button variant="secondary" disabled={locked} onClick={() => void remoteSave(true)}>{t("Save and test")}</Button>}
            <Button variant="outline" disabled={locked || !keyStored} onClick={() => void clearKey()}>{t("Delete key")}</Button>
          </div>
        </>}
        {message && <p role="status" className="text-sm">{message}</p>}
      </>}
    </CardContent>
  </Card>;
}

function modelCheckResult(status: SpeechModelStatus | undefined) {
  if (!status?.configured) return t("Save the profile to check its model.");
  switch (status.last_result) {
    case "available": return t("The saved model is in the provider’s list.");
    case "missing": return t("The saved model was not found.");
    case "unknown": return t("The provider did not return a usable model list.");
    case "failed": return t("The model check failed.");
    case null: return t("The saved model has not been checked yet.");
  }
}

/** A recommendation with a one-click action inside «Recognition speed». */
function SpeedHint({ text, action, disabled, onClick, tour }: { text: string; action: string; disabled: boolean; onClick: () => void; tour?: string }) {
  return <div className="flex flex-wrap items-center justify-between gap-2 rounded-md bg-muted p-3" data-tour={tour}>
    <p className="text-sm">{text}</p>
    <Button size="sm" variant="outline" disabled={disabled} onClick={onClick}>{action}</Button>
  </div>;
}
