import { currentLocale } from "@/i18n";
import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
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
import { DictationSection } from "@/components/settings/DictationSection";
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
  useI18n();
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
  // Reads are ungated (D5): always re-sync the list, never conditioned on the
  // toggle. With history off nothing new was written, so this just re-shows the
  // same accumulated rows — but the load path stays uniformly toggle-agnostic
  // (no half-gate) rather than relying on that as an optimisation.
  useEffect(() => {
    const unlisten = listen<DictationState>(DICTATION_STATE_EVENT, (event) => {
      if (event.payload.kind !== "done") return;
      void getRecognitionsMinutes().then(setMinutes).catch(() => {});
      void listDictations(HISTORY_LIMIT).then(setHistory).catch(() => {});
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

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
    toast.success(t("Shortcut updated."));
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
      toast.success(t("History cleared."));
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(t("Copied to clipboard."));
    } catch {
      toast.error(t("Could not copy to clipboard."));
    }
  }

  return (
    <div className="space-y-6">
      <DictationSection />
      <div>
        <h2 className="text-2xl font-semibold tracking-tight">{t("Dictation")}</h2>
        <p className="text-muted-foreground mt-1 text-sm">
          {t("Dictate into any application: hold the shortcut, speak, then release it to insert the recognized text into the active window.")}{" "}</p>
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
              {t("Could not load dictation settings:")}{" "}{load.message}
            </p>
          </CardContent>
        </Card>
      )}

      {load.kind === "ready" && settings && (
        <>
          <Card>
            <CardHeader>
              <CardTitle>{t("Input and shortcut")}</CardTitle>
              <CardDescription>
                {t("Choose how recognized text reaches your application and which shortcut starts recording.")}{" "}</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="space-y-3">
                <Label>{t("Insertion mode")}</Label>
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
                      <span className="block text-sm font-medium">{t("Automatic insertion")}</span>
                      <span className="text-muted-foreground block text-xs">
                        {t("Text is inserted into the active window automatically (Ctrl+V).")}{" "}</span>
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
                      <span className="block text-sm font-medium">{t("Clipboard only")}</span>
                      <span className="text-muted-foreground block text-xs">
                        {t("Text is copied to the clipboard. Paste it manually (Ctrl+V) when convenient.")}{" "}</span>
                    </span>
                  </label>
                </RadioGroup>
              </div>

              <Separator />

              <div className="space-y-3">
                <div className="space-y-0.5">
                  <Label>{t("Shortcut")}</Label>
                  <p className="text-muted-foreground text-xs">
                    {t("Hold to record. If another application uses the combination, your previous shortcut stays active.")}{" "}</p>
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
                  <Label htmlFor="device">{t("Microphone")}</Label>
                  <p className="text-muted-foreground text-xs">
                    {t("System default follows your Windows setting.")}{" "}</p>
                </div>
                <DevicePicker
                  value={settings.device}
                  devices={devices}
                  onChange={handleDevice}
                  disabled={busy}
                />
                {deviceError && (
                  <p className="text-muted-foreground text-xs">
                    {t("Could not list devices:")}{" "}{deviceError}
                  </p>
                )}
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>{t("Dictation history")}</CardTitle>
              <CardDescription>
                {t("Last")}{" "}{HISTORY_LIMIT} {t("transcripts, so you can reuse earlier dictation. Stored only on this computer.")}{" "}</CardDescription>
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
              <CardTitle>{t("Statistics")}</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <p className="text-sm">
                {t("Total dictated:")}{" "}
                <span className="font-medium">
                  {new Intl.NumberFormat(currentLocale()).format(minutes)} {pluralizeMinutes(minutes)}
                </span>
              </p>
              <p className="text-muted-foreground text-xs">
                {t("Configure the recognition provider and key in")}{" "}
                <Link to="/settings" className="underline underline-offset-2">
                  {t("Settings")}{" "}</Link>{" "}
                {t("→ Dictation (STT).")}{" "}</p>
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
