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
  type SpeechMode, type SpeechProfile, type LocalModelsStatus, type ModelProgress,
} from "@/lib/tauri";

const MODES: Record<SpeechMode, string> = {
  local: "На этом компьютере", server: "Сервер организации", cloud: "Облачный сервис",
};
const PRESETS = [
  { id: "aitunnel", label: "AITunnel", base_url: "https://api.aitunnel.ru/v1", model: "whisper-large-v3-turbo" },
  { id: "proxyapi", label: "ProxyAPI", base_url: "https://api.proxyapi.ru/openai/v1", model: "whisper-1" },
  { id: "vsegpt", label: "VseGPT", base_url: "https://api.vsegpt.ru/v1", model: "whisper-1" },
  { id: "groq", label: "Groq", base_url: "https://api.groq.com/openai/v1", model: "whisper-large-v3-turbo" },
];
const size = (bytes: number) => `${new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 1 }).format(bytes / 1_000_000)} МБ`;
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

export function DictationSection() {
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
  const alive = useRef(true);

  async function refreshModels() {
    const status = await localModelsStatus();
    if (alive.current) { setModels(status); setProgress(status.progress); }
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
      setMessage(test ? "Подключение работает." : p.mode === "local" ? "Модель готова. Можно диктовать." : "Настройки сохранены.");
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
    setBusy(true); setMessage(installed ? "Подготовка модели…" : "Скачивание модели…");
    try {
      if (!installed || !models?.runtime_installed) await downloadLocalModel(id);
      if (!alive.current) return;
      setMessage("Подготовка модели…");
      const p = { ...profile, mode: "local" as const, model: id };
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
  async function clearKey() {
    if (!profile) return;
    setBusy(true);
    try { await deleteSpeechKey(profile.mode); setKeyStored(false); setApiKey(""); setMessage("Ключ удалён."); }
    catch (e) { toast.error(errorText(e)); }
    finally { setBusy(false); }
  }

  const locked = busy || progress !== null;
  return <Card>
    <CardHeader>
      <CardTitle>Диктовка</CardTitle>
      <CardDescription>Распознавание на компьютере, на сервере вашей организации или в облаке.</CardDescription>
    </CardHeader>
    <CardContent className="space-y-4">
      {loadError && <p role="alert" className="text-sm text-destructive">{loadError}</p>}
      {!profile && !loadError && <Skeleton className="h-32 w-full" />}
      {profile && <>
        <div className="space-y-2">
          <Label htmlFor="speech-mode">Где распознавать речь</Label>
          <Select value={profile.mode} onValueChange={v => void chooseMode(v as SpeechMode)} disabled={locked}>
            <SelectTrigger id="speech-mode" className="w-full"><SelectValue /></SelectTrigger>
            <SelectContent>{Object.entries(MODES).map(([value, label]) => <SelectItem key={value} value={value}>{label}</SelectItem>)}</SelectContent>
          </Select>
          <p className="text-xs text-muted-foreground">Сейчас используется: {MODES[active]}.</p>
        </div>
        {profile.mode === "local" ? <>
          <p className="text-sm text-muted-foreground">Скачайте модель один раз. Затем голос распознаётся без интернета и остаётся на этом компьютере. Обновления Глагола сохраняют скачанные модели.</p>
          {models && !models.supported && <p role="alert">Локальные модели пока доступны на Windows x64. Подключение к серверу работает и на других системах.</p>}
          {models && !models.runtime_installed && <p className="text-xs text-muted-foreground">При первой загрузке также скачается движок распознавания: {size(models.runtime_bytes)}.</p>}
          {models?.models.map(m => {
            const selected = active === "local" && activeModel === m.id;
            return <div key={m.id} className="rounded-lg border p-4 space-y-2">
              <div className="flex flex-wrap justify-between gap-2"><span className="font-medium">{m.name}</span><span className="text-sm text-muted-foreground">{size(m.bytes)}{selected ? " · Используется" : m.installed ? " · Скачана" : ""}</span></div>
              <p className="text-sm text-muted-foreground">{m.description}</p>
              <div className="flex flex-wrap gap-2">
                <Button disabled={locked || !models.supported || selected} onClick={() => void useModel(m.id, m.installed)}>
                  {selected ? "Выбрана" : m.installed ? "Использовать" : m.partial_bytes > 0 ? "Продолжить и использовать" : "Скачать и использовать"}
                </Button>
                {selected && <Button variant="outline" disabled={locked} onClick={() => void useModel(m.id, false)}>Проверить файлы и восстановить</Button>}
                {(m.installed || m.partial_bytes > 0) && <Button variant="outline" disabled={locked || selected} onClick={() => void remove(m.id)}>Удалить · {size(m.installed ? m.bytes : m.partial_bytes)}</Button>}
              </div>
            </div>;
          })}
          {progress && <div className="space-y-2" role="status">
            <p className="text-sm">{progress.stage === "verifying" ? "Проверка файлов…" : `Скачано ${size(progress.downloaded)} из ${size(progress.total)}`}</p>
            <progress className="h-2 w-full accent-primary" value={progress.downloaded} max={progress.total} aria-label="Загрузка модели" />
            <Button variant="outline" disabled={progress.stage === "verifying"} onClick={() => void cancelModelDownload().catch(e => toast.error(errorText(e)))}>Отменить загрузку</Button>
          </div>}
        </> : <>
          {profile.mode === "server" ? <p className="text-sm text-muted-foreground">Один сервер распознавания может обслуживать компьютеры всего офиса. Модели на этот компьютер скачивать не нужно. Поддерживаются серверы с API /v1/audio/transcriptions.</p> : <div className="space-y-2">
            <Label htmlFor="speech-provider">Провайдер</Label>
            <Select value={PRESETS.find(p => p.base_url === profile.base_url)?.id ?? "custom"} disabled={locked} onValueChange={id => {
              const p = PRESETS.find(p => p.id === id); if (p) update({ base_url: p.base_url, model: p.model });
              else update({ base_url: "", model: "" });
            }}>
              <SelectTrigger id="speech-provider" className="w-full"><SelectValue /></SelectTrigger>
              <SelectContent>{PRESETS.map(p => <SelectItem key={p.id} value={p.id}>{p.label}</SelectItem>)}<SelectItem value="custom">Свой сервис</SelectItem></SelectContent>
            </Select>
          </div>}
          <div className="space-y-2">
            <Label htmlFor="speech-url">Адрес {profile.mode === "server" ? "сервера" : "сервиса"}</Label>
            <Input id="speech-url" value={profile.base_url} onChange={e => update({ base_url: e.target.value })} disabled={locked} spellCheck={false} autoComplete="off" placeholder={profile.mode === "server" ? "http://192.168.1.10:8000/v1" : "https://api.example.com/v1"} />
            <p className="text-xs text-muted-foreground">Включая путь /v1.{profile.mode === "server" ? " HTTP допустим для частного IP-адреса и localhost; для доменного имени используйте HTTPS." : " Для внешних адресов используйте HTTPS."}</p>
            {profile.mode === "server" && profile.base_url.startsWith("http://") && <p className="text-xs text-muted-foreground">HTTP передаёт голос и ключ без шифрования. Используйте его только в доверенной офисной сети.</p>}
          </div>
          <div className="space-y-2"><Label htmlFor="speech-model">Название модели на сервере</Label><Input id="speech-model" value={profile.model} onChange={e => update({ model: e.target.value })} disabled={locked} spellCheck={false} /></div>
          <div className="space-y-2">
            <Label htmlFor="speech-language">Язык</Label>
            <Select value={profile.language} onValueChange={language => update({ language })} disabled={locked}>
              <SelectTrigger id="speech-language" className="w-full"><SelectValue /></SelectTrigger>
              <SelectContent><SelectItem value="ru">Русский</SelectItem><SelectItem value="en">Английский</SelectItem><SelectItem value="auto">Автоопределение</SelectItem></SelectContent>
            </Select>
          </div>
          <div className="space-y-2"><Label htmlFor="speech-proxy">Прокси для диктовки (необязательно)</Label><Input id="speech-proxy" value={profile.proxy} onChange={e => update({ proxy: e.target.value })} disabled={locked} autoComplete="off" placeholder="host:port или socks5://host:port" /></div>
          <div className="space-y-2">
            <Label htmlFor="speech-key">API-ключ{profile.mode === "server" ? " (если сервер требует)" : ""}</Label>
            <Input id="speech-key" type="password" value={apiKey} onChange={e => setApiKey(e.target.value)} disabled={locked} autoComplete="off" placeholder={keyStored ? "Ключ сохранён для этого профиля" : "Введите ключ"} />
            <p className="text-xs text-muted-foreground">Ключи облака и сервера хранятся отдельно. При смене адреса введите ключ для нового сервиса.</p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button disabled={locked} onClick={() => void remoteSave(false)}>Сохранить и использовать</Button>
            <Button variant="secondary" disabled={locked} onClick={() => void remoteSave(true)}>Сохранить и проверить</Button>
            <Button variant="outline" disabled={locked || !keyStored} onClick={() => void clearKey()}>Удалить ключ</Button>
          </div>
        </>}
        {message && <p role="status" className="text-sm">{message}</p>}
      </>}
    </CardContent>
  </Card>;
}
