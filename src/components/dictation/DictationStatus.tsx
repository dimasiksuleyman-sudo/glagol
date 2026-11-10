import { currentLocale, t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Settings } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { DictationSetupCard } from "@/components/DictationSetupCard";
import { SttLanguageSwitch } from "@/components/SttLanguageSwitch";
import { pluralizeMinutes } from "@/lib/pluralize";
import {
  getSpeechSettings, localEngineState, localModelsStatus,
  type LocalEngineState, type LocalModelsStatus, type SpeechSettings,
} from "@/lib/tauri";

/**
 * Top of the Dictation page: what recognizes speech right now, in one line, with
 * a link to Settings → Dictation. Engine and model management live only in
 * Settings; an unconfigured local model shows the short first-setup card instead.
 */
export function DictationStatus({ minutes, hotkey }: { minutes: number; hotkey: string }) {
  useI18n();
  const { preferences } = usePreferences();
  const [speech, setSpeech] = useState<SpeechSettings | null>(null);
  const [models, setModels] = useState<LocalModelsStatus | null>(null);
  const [engine, setEngine] = useState<LocalEngineState | null>(null);

  useEffect(() => {
    let disposed = false;
    const refresh = () => void Promise.all([getSpeechSettings(), localModelsStatus(), localEngineState()])
      .then(([s, m, e]) => { if (!disposed) { setSpeech(s); setModels(m); setEngine(e); } })
      .catch(() => undefined);
    const listeners = ["local-models-changed", "speech-settings-changed"].map(name => listen(name, refresh));
    refresh();
    return () => { disposed = true; listeners.forEach(l => void l.then(stop => stop())); };
  }, [preferences?.stt_language]);

  if (!speech || !models) return <Skeleton className="h-24 w-full" />;

  const mode = speech.active_mode;
  const model = mode === "local" ? models.models.find(m => m.id === speech.profile.model) : undefined;
  const ready = mode !== "local" || (!!model && model.installed && model.download_bytes === 0);
  if (!ready) return <div data-tour="dictation-setup"><DictationSetupCard /></div>;

  const where = mode === "local" ? t("On this computer") : mode === "server" ? t("Organization server") : t("Cloud service");
  const device = mode === "local" && model?.provider === "gigaam" && engine
    ? (engine.options.device === "gpu" && !engine.gpu_fallback ? t("graphics card") : t("processor"))
    : null;
  const what = [where, model?.name ?? (mode === "local" ? "" : speech.profile.model), device].filter(Boolean).join(" · ");
  const recommendationPending = mode === "local" && model?.provider === "gigaam" && engine
    && (model.id !== engine.recommendation.model || engine.options.device !== engine.recommendation.device);

  return <Card data-tour="dictation-status" className="py-0">
    <CardContent className="space-y-3 py-5">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex items-start gap-3">
          <CheckCircle2 className="mt-0.5 h-5 w-5 shrink-0 text-emerald-600" aria-hidden />
          <div className="space-y-1">
            <p className="font-medium">{t("Ready: hold {p0} and speak.", { p0: hotkey })}</p>
            <p className="text-sm text-muted-foreground">{what}</p>
          </div>
        </div>
        <Button asChild size="sm" variant="outline">
          <Link to="/settings/dictation"><Settings className="h-4 w-4" aria-hidden />{t("Change")}</Link>
        </Button>
      </div>
      <SttLanguageSwitch />
      {recommendationPending && <p className="text-sm">
        {t("Glagol has a faster setup for this computer.")}{" "}
        <Link to="/settings/dictation" state={{ tour: "speed" }} className="underline underline-offset-2">{t("Show")}</Link>
      </p>}
      <p className="text-xs text-muted-foreground">
        {t("Total dictated:")}{" "}
        <span className="font-medium">{new Intl.NumberFormat(currentLocale()).format(minutes)} {pluralizeMinutes(minutes)}</span>
      </p>
    </CardContent>
  </Card>;
}
