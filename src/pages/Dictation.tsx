import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";

import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { DevicePicker } from "@/components/dictation/DevicePicker";
import { DictationHistory } from "@/components/dictation/DictationHistory";
import { HotkeyEditor } from "@/components/dictation/HotkeyEditor";
import { pluralizeMinutes } from "@/lib/pluralize";
import {
  clearDictationHistory,
  DICTATION_STATE_EVENT,
  getDictationSettings,
  getRecognitionsMinutes,
  listAudioInputDevices,
  listDictations,
  setDictationHotkey,
  setDictationSetting,
  type Dictation,
  type DictationSettings,
  type DictationState,
} from "@/lib/tauri";

/** History rows are capped at 10 on disk (D4); ask for exactly that many. */
const HISTORY_LIMIT = 10;

type LoadState =
  | { kind: "loading" }
  | { kind: "ready" }
  | { kind: "error"; message: string };

/**
 * Dictation page (Sprint 6 PR5b) — the settings home for the push-to-talk
 * dictation feature the backend built across PR1–PR5a: insertion mode, hotkey,
 * microphone, opt-in history, and the lifetime «Надиктовано всего» counter.
 *
 * Data is fetched once on mount (the settings are cheap local DB reads — no
 * network, so the `force`-based cache-first probe pattern used for credential
 * validation does not apply; mount-fetch-once is the faithful analogue). While
 * the page is open it also `listen()`s for `dictation-state` `done` events so a
 * dictation triggered by the global hotkey refreshes the counter and history
 * without a manual reload (the event-driven refresh convention).
 */
export function Dictation() {
  const [load, setLoad] = useState<LoadState>({ kind: "loading" });
  const [settings, setSettings] = useState<DictationSettings | null>(null);
  const [devices, setDevices] = useState<string[]>([]);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  const [minutes, setMinutes] = useState<number>(0);
  const [history, setHistory] = useState<Dictation[]>([]);
  const [busy, setBusy] = useState<boolean>(false);

  useEffect(() => {
    let cancelled = false;

    async function loadPage() {
      try {
        const [dictSettings, mins] = await Promise.all([
          getDictationSettings(),
          getRecognitionsMinutes(),
        ]);
        if (cancelled) return;
        setSettings(dictSettings);
        setMinutes(mins);
        // Reads are ungated (D5): accumulated history is shown regardless of the
        // toggle, which only governs whether NEW rows are written. So always load
        // it on mount — the rows stay visible across remount until «Очистить».
        setHistory(await listDictations(HISTORY_LIMIT));
        // Device enumeration failing must not sink the whole page — the picker
        // just falls back to «Системный по умолчанию» with an inline note.
        try {
          setDevices(await listAudioInputDevices());
        } catch (err) {
          if (!cancelled) setDeviceError(stringifyError(err));
        }
        if (!cancelled) setLoad({ kind: "ready" });
      } catch (err) {
        if (!cancelled) setLoad({ kind: "error", message: stringifyError(err) });
      }
    }

    loadPage();
    return () => {
      cancelled = true;
    };
  }, []);

  // Refresh the counter + history when a dictation finishes while the page is open.
  useEffect(() => {
    const unlisten = listen<DictationState>(DICTATION_STATE_EVENT, (event) => {
      if (event.payload.kind !== "done") return;
      void getRecognitionsMinutes().then(setMinutes).catch(() => {});
      if (settings?.history_enabled) {
        void listDictations(HISTORY_LIMIT).then(setHistory).catch(() => {});
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [settings?.history_enabled]);

  async function handleInsertionMode(mode: string) {
    if (!settings || mode === settings.insertion_mode) return;
    const prev = settings.insertion_mode;
    setSettings({ ...settings, insertion_mode: mode }); // optimistic
    setBusy(true);
    try {
      await setDictationSetting("stt_insertion_mode", mode);
    } catch (err) {
      setSettings((s) => (s ? { ...s, insertion_mode: prev } : s)); // revert
      toast.error(stringifyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleDevice(device: string) {
    if (!settings) return;
    const prev = settings.device;
    setSettings({ ...settings, device }); // optimistic
    setBusy(true);
    try {
      await setDictationSetting("dictation_device", device);
    } catch (err) {
      setSettings((s) => (s ? { ...s, device: prev } : s)); // revert
      toast.error(stringifyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleHotkeySave(hotkey: string) {
    // Throws on conflict/invalid — HotkeyEditor surfaces it and stays open.
    await setDictationHotkey(hotkey);
    setSettings((s) => (s ? { ...s, hotkey } : s));
    toast.success("Хоткей обновлён.");
  }

  async function handleHistoryToggle(enabled: boolean) {
    if (!settings) return;
    const prev = settings.history_enabled;
    setSettings({ ...settings, history_enabled: enabled }); // optimistic
    setBusy(true);
    try {
      await setDictationSetting("dictation_history_enabled", enabled ? "true" : "false");
      // Reads are ungated (D5), so re-sync with the authoritative DB either way:
      // toggling only changes whether FUTURE dictations are written, never what is
      // already stored. Off keeps the accumulated rows on screen until «Очистить».
      setHistory(await listDictations(HISTORY_LIMIT));
    } catch (err) {
      setSettings((s) => (s ? { ...s, history_enabled: prev } : s)); // revert
      toast.error(stringifyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleHistoryClear() {
    setBusy(true);
    try {
      await clearDictationHistory();
      setHistory([]);
      toast.success("История очищена.");
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success("Скопировано в буфер обмена.");
    } catch {
      toast.error("Не удалось скопировать в буфер обмена.");
    }
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-2xl font-semibold tracking-tight">Диктовка</h2>
        <p className="text-muted-foreground mt-1 text-sm">
          Голосовой ввод в любое приложение: удерживайте хоткей, говорите, отпустите —
          распознанный текст вставится в активное окно.
        </p>
      </div>

      {load.kind === "loading" && (
        <Card>
          <CardContent className="space-y-4 py-6">
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-9 w-2/3" />
            <Skeleton className="h-9 w-1/2" />
          </CardContent>
        </Card>
      )}

      {load.kind === "error" && (
        <Card>
          <CardContent className="py-6">
            <p className="text-muted-foreground text-sm">
              Не удалось загрузить настройки диктовки: {load.message}
            </p>
          </CardContent>
        </Card>
      )}

      {load.kind === "ready" && settings && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>Ввод и горячая клавиша</CardTitle>
              <CardDescription>
                Как распознанный текст попадает в приложение и какой комбинацией
                запускается запись.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-3">
                <Label>Режим вставки</Label>
                <RadioGroup
                  value={settings.insertion_mode}
                  onValueChange={handleInsertionMode}
                  className="gap-3"
                >
                  <label
                    htmlFor="mode-paste"
                    className="flex cursor-pointer items-start gap-3"
                  >
                    <RadioGroupItem id="mode-paste" value="paste" className="mt-0.5" />
                    <span className="space-y-0.5">
                      <span className="block text-sm font-medium">Автовставка</span>
                      <span className="text-muted-foreground block text-xs">
                        Текст вставляется в активное окно автоматически (Ctrl+V).
                      </span>
                    </span>
                  </label>
                  <label
                    htmlFor="mode-clipboard"
                    className="flex cursor-pointer items-start gap-3"
                  >
                    <RadioGroupItem
                      id="mode-clipboard"
                      value="clipboard_only"
                      className="mt-0.5"
                    />
                    <span className="space-y-0.5">
                      <span className="block text-sm font-medium">Только буфер обмена</span>
                      <span className="text-muted-foreground block text-xs">
                        Текст кладётся в буфер — вставьте вручную (Ctrl+V), когда удобно.
                      </span>
                    </span>
                  </label>
                </RadioGroup>
              </div>

              <Separator />

              <div className="space-y-3">
                <div className="space-y-0.5">
                  <Label>Горячая клавиша</Label>
                  <p className="text-muted-foreground text-xs">
                    Удерживайте, чтобы записывать. Если комбинация занята другим
                    приложением, прежняя останется активной.
                  </p>
                </div>
                <HotkeyEditor
                  value={settings.hotkey}
                  onSave={handleHotkeySave}
                  onError={(m) => toast.error(m)}
                  disabled={busy}
                />
              </div>

              <Separator />

              <div className="space-y-3">
                <div className="space-y-0.5">
                  <Label htmlFor="device">Микрофон</Label>
                  <p className="text-muted-foreground text-xs">
                    «Системный по умолчанию» следует за настройкой Windows.
                  </p>
                </div>
                <DevicePicker
                  value={settings.device}
                  devices={devices}
                  onChange={handleDevice}
                  disabled={busy}
                />
                {deviceError && (
                  <p className="text-muted-foreground text-xs">
                    Не удалось получить список устройств: {deviceError}
                  </p>
                )}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>История диктовки</CardTitle>
              <CardDescription>
                Последние {HISTORY_LIMIT} расшифровок — чтобы перевставить сказанное
                ранее. Хранится только на этом компьютере.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <DictationHistory
                entries={history}
                enabled={settings.history_enabled}
                busy={busy}
                onToggle={handleHistoryToggle}
                onClear={handleHistoryClear}
                onCopy={handleCopy}
              />
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Статистика</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <p className="text-sm">
                Надиктовано всего:{" "}
                <span className="font-medium">
                  {new Intl.NumberFormat("ru-RU").format(minutes)} {pluralizeMinutes(minutes)}
                </span>
              </p>
              <p className="text-muted-foreground text-xs">
                Провайдер распознавания и ключ настраиваются в разделе{" "}
                <Link to="/settings" className="underline underline-offset-2">
                  «Настройки»
                </Link>{" "}
                → «Диктовка (STT)».
              </p>
            </CardContent>
          </Card>
        </>
      )}
    </div>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
