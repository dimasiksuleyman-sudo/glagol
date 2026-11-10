import { t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Lightbulb } from "lucide-react";

import { Button } from "@/components/ui/button";
import { dismissTip, pickTip } from "@/lib/tips";
import { getTour, tourLink, type TourId } from "@/lib/tours";
import {
  getDictationSettings, getRecognitionsMinutes, getSpeechSettings, getUpdateSettings,
  listDocuments, localEngineState, localModelsStatus,
} from "@/lib/tauri";

/**
 * «Did you know?» card. Stays until closed — no timers: one tip a week during
 * the first month, then once a month, only about features not used yet.
 */
export function TipCard() {
  useI18n();
  const navigate = useNavigate();
  const { preferences } = usePreferences();
  const [tipId, setTipId] = useState<TourId | null>(null);
  const [dontShowAgain, setDontShowAgain] = useState(false);
  const ttsEnabled = preferences?.tts_enabled ?? true;

  // Pick once per visit to the screen: the card must not jump between tips.
  useEffect(() => {
    let disposed = false;
    void (async () => {
      try {
        const [dictation, minutes, updates, speech, models, engine, documents] = await Promise.all([
          getDictationSettings(), getRecognitionsMinutes(), getUpdateSettings(), getSpeechSettings(),
          localModelsStatus(), localEngineState(), ttsEnabled ? listDocuments() : Promise.resolve([]),
        ]);
        const model = models.models.find(m => m.id === speech.profile.model);
        const recommendationPending = speech.active_mode === "local" && model?.provider === "gigaam"
          && (model.id !== engine.recommendation.model || engine.options.device !== engine.recommendation.device);
        const picked = pickTip({
          ttsEnabled, historyEnabled: dictation.history_enabled, documents: documents.length, minutes,
          recommendationPending, autoCheck: updates.autoCheck,
        });
        if (!disposed) setTipId(picked);
      } catch {
        // No tip when the facts cannot be read.
      }
    })();
    return () => { disposed = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const tour = getTour(tipId);
  if (!tipId || !tour) return null;

  const close = () => {
    dismissTip(tipId, dontShowAgain);
    setTipId(null);
  };
  const show = () => {
    close();
    const link = tourLink(tour, ttsEnabled);
    navigate(link.route, { state: link.state });
  };

  return <div className="space-y-3 rounded-lg border border-amber-200 bg-amber-50 p-4 dark:border-amber-900 dark:bg-amber-950/40" role="note">
    <div className="flex gap-3">
      <Lightbulb className="h-5 w-5 shrink-0 text-amber-500" aria-hidden />
      <div>
        <p className="text-sm font-semibold">{t("helpTipTitle")}</p>
        <p className="mt-1 text-sm leading-snug">{t(tour.tip)}</p>
      </div>
    </div>
    <div className="flex flex-wrap items-center gap-2">
      <Button size="sm" onClick={show}>{t("helpShow")}</Button>
      <Button size="sm" variant="outline" onClick={close}>{t("helpGotIt")}</Button>
      <label className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
        <input type="checkbox" checked={dontShowAgain} onChange={e => setDontShowAgain(e.target.checked)} className="h-4 w-4" />
        {t("helpDontShowTips")}
      </label>
    </div>
  </div>;
}
