import { useEffect, useState } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  deleteSttKey,
  getSttSettings,
  hasSttKey,
  saveSttSettings,
  setSttKey,
  testSttKey,
  type SttSettings,
} from "@/lib/tauri";

/**
 * Provider presets. Each fills `base_url` + a suggested `model`; path quirks
 * (`/openai/v1` vs `/v1`) live in the string, never in code (kickoff D4). The
 * `custom` preset leaves the fields for manual entry.
 */
const PRESETS = [
  {
    id: "aitunnel",
    label: "AITunnel",
    baseUrl: "https://api.aitunnel.ru/v1",
    model: "whisper-large-v3-turbo",
  },
  {
    id: "proxyapi",
    label: "ProxyAPI",
    baseUrl: "https://api.proxyapi.ru/openai/v1",
    model: "whisper-1",
  },
  {
    id: "vsegpt",
    label: "VseGPT",
    baseUrl: "https://api.vsegpt.ru/v1",
    model: "whisper-1",
  },
  {
    id: "groq",
    label: "Groq",
    baseUrl: "https://api.groq.com/openai/v1",
    model: "whisper-large-v3-turbo",
  },
  {
    id: "local",
    label: "Локальный сервер",
    baseUrl: "http://localhost:8000/v1",
    model: "whisper-1",
  },
  { id: "custom", label: "Свой", baseUrl: "", model: "" },
] as const;

const CUSTOM_PRESET_ID = "custom";

/** Resolve which preset (if any) a saved base_url matches, else "custom". */
function presetIdForBaseUrl(baseUrl: string): string {
  const match = PRESETS.find((p) => p.id !== CUSTOM_PRESET_ID && p.baseUrl === baseUrl);
  return match ? match.id : CUSTOM_PRESET_ID;
}

type LoadState =
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "error"; message: string };

type CheckStatus = "idle" | "valid" | "invalid";

export function DictationSection() {
  const [load, setLoad] = useState<LoadState>({ kind: "loading" });
  const [presetId, setPresetId] = useState<string>("aitunnel");
  const [baseUrl, setBaseUrl] = useState<string>("");
  const [model, setModel] = useState<string>("");
  const [proxy, setProxy] = useState<string>("");
  const [language, setLanguage] = useState<string>("ru");
  const [apiKey, setApiKey] = useState<string>("");
  const [keyStored, setKeyStored] = useState<boolean>(false);
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [checkStatus, setCheckStatus] = useState<CheckStatus>("idle");

  useEffect(() => {
    let cancelled = false;

    async function loadSettings() {
      try {
        const [settings, stored] = await Promise.all([getSttSettings(), hasSttKey()]);
        if (cancelled) return;
        setBaseUrl(settings.base_url);
        setModel(settings.model);
        setProxy(settings.proxy);
        setLanguage(settings.language);
        setPresetId(presetIdForBaseUrl(settings.base_url));
        setKeyStored(stored);
        setLoad({ kind: "ready" });
      } catch (err) {
        if (!cancelled) setLoad({ kind: "error", message: stringifyError(err) });
      }
    }

    loadSettings();
    return () => {
      cancelled = true;
    };
  }, []);

  function handlePresetChange(id: string) {
    setPresetId(id);
    setCheckStatus("idle");
    const preset = PRESETS.find((p) => p.id === id);
    // "Свой" keeps whatever the user has typed; other presets fill the fields.
    if (preset && preset.id !== CUSTOM_PRESET_ID) {
      setBaseUrl(preset.baseUrl);
      setModel(preset.model);
    }
  }

  /** Persist settings + (if entered) the key. Shared by Save and Test. */
  async function persist(): Promise<void> {
    const settings: SttSettings = {
      base_url: baseUrl,
      model,
      proxy,
      language,
    };
    await saveSttSettings(settings);
    if (apiKey.trim().length > 0) {
      await setSttKey(apiKey.trim());
      setApiKey("");
      setKeyStored(true);
    }
  }

  async function handleSave() {
    setIsBusy(true);
    try {
      await persist();
      setCheckStatus("idle");
      toast.success("Настройки диктовки сохранены. Нажмите «Проверить».");
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleTest() {
    setIsBusy(true);
    try {
      await persist();
      await testSttKey(true);
      setCheckStatus("valid");
      toast.success("Ключ действителен.");
    } catch (err) {
      setCheckStatus("invalid");
      toast.error(stringifyError(err));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleDelete() {
    setIsBusy(true);
    try {
      await deleteSttKey();
      setKeyStored(false);
      setApiKey("");
      setCheckStatus("idle");
      toast.success("Ключ удалён.");
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Диктовка (STT)</CardTitle>
        <CardDescription>
          Распознавание речи через OpenAI-совместимый эндпоинт. Работает без системного
          VPN: можно указать свой прокси только для распознавания.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {load.kind === "loading" && (
          <>
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-9 w-2/3" />
          </>
        )}

        {load.kind === "error" && (
          <p className="text-muted-foreground text-sm">
            Не удалось загрузить настройки диктовки: {load.message}
          </p>
        )}

        {load.kind === "ready" && (
          <>
            <div className="space-y-2">
              <Label htmlFor="stt-preset">Провайдер</Label>
              <Select value={presetId} onValueChange={handlePresetChange} disabled={isBusy}>
                <SelectTrigger id="stt-preset" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {PRESETS.map((p) => (
                    <SelectItem key={p.id} value={p.id}>
                      {p.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="stt-base-url">Адрес эндпоинта (base URL)</Label>
              <Input
                id="stt-base-url"
                autoComplete="off"
                spellCheck={false}
                placeholder="https://api.aitunnel.ru/v1"
                value={baseUrl}
                onChange={(e) => {
                  setBaseUrl(e.target.value);
                  setPresetId(presetIdForBaseUrl(e.target.value));
                  setCheckStatus("idle");
                }}
                disabled={isBusy}
              />
              <p className="text-muted-foreground text-xs">
                Включая путь <code>/v1</code>. Для внешних адресов — только https.
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="stt-model">Модель</Label>
              <Input
                id="stt-model"
                autoComplete="off"
                spellCheck={false}
                placeholder="whisper-large-v3-turbo"
                value={model}
                onChange={(e) => {
                  setModel(e.target.value);
                  setCheckStatus("idle");
                }}
                disabled={isBusy}
              />
            </div>

            <div className="space-y-2">
              <Label htmlFor="stt-language">Язык распознавания</Label>
              <Select
                value={language}
                onValueChange={(v) => {
                  setLanguage(v);
                  setCheckStatus("idle");
                }}
                disabled={isBusy}
              >
                <SelectTrigger id="stt-language" className="w-full">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="ru">Русский</SelectItem>
                  <SelectItem value="en">Английский</SelectItem>
                  <SelectItem value="auto">Автоопределение</SelectItem>
                </SelectContent>
              </Select>
            </div>

            <div className="space-y-2">
              <Label htmlFor="stt-proxy">Прокси (необязательно)</Label>
              <Input
                id="stt-proxy"
                autoComplete="off"
                spellCheck={false}
                placeholder="host:port или login:pass@host:port или socks5://host:port"
                value={proxy}
                onChange={(e) => {
                  setProxy(e.target.value);
                  setCheckStatus("idle");
                }}
                disabled={isBusy}
              />
              <p className="text-muted-foreground text-xs">
                Применяется только к распознаванию речи. Озвучка и система не затрагиваются.
              </p>
            </div>

            <div className="space-y-2">
              <Label htmlFor="stt-key">Ключ провайдера</Label>
              <Input
                id="stt-key"
                type="password"
                autoComplete="off"
                spellCheck={false}
                placeholder={
                  keyStored ? "Ключ сохранён — введите новый, чтобы заменить" : "Вставьте ключ"
                }
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                disabled={isBusy}
              />
              <p className="text-muted-foreground text-xs">
                Ключ: <StatusLabel keyStored={keyStored} checkStatus={checkStatus} />
              </p>
            </div>

            <div className="flex flex-wrap gap-2">
              <Button onClick={handleSave} disabled={isBusy}>
                Сохранить
              </Button>
              <Button variant="secondary" onClick={handleTest} disabled={isBusy}>
                Проверить
              </Button>
              <Button
                variant="destructive"
                onClick={handleDelete}
                disabled={isBusy || !keyStored}
              >
                Удалить
              </Button>
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}

function StatusLabel({
  keyStored,
  checkStatus,
}: {
  keyStored: boolean;
  checkStatus: CheckStatus;
}) {
  if (checkStatus === "valid") {
    return <span className="text-foreground font-medium">действителен</span>;
  }
  if (checkStatus === "invalid") {
    return <span className="text-muted-foreground">не прошёл проверку</span>;
  }
  if (keyStored) {
    return <span className="text-foreground font-medium">сохранён (не проверен)</span>;
  }
  return <span className="text-muted-foreground">не задан</span>;
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
