import { t } from "@/i18n";
import { useI18n, usePreferences } from "@/contexts/PreferencesContext";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  AudioLines, Bell, ChevronRight, ClipboardPaste, FileUp, Gauge, Headset, History, Library, Mic, Stethoscope, Volume2, Archive,
  type LucideIcon,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { TOURS, tourLink, type Tour, type TourId } from "@/lib/tours";

/** Topics grouped like the app: dictation, text to speech, general. */
const GROUPS: { title: "dictation" | "Text to speech" | "General"; ids: TourId[] }[] = [
  { title: "dictation", ids: ["dictate", "speed", "microphone", "insert_mode", "history"] },
  { title: "Text to speech", ids: ["synthesize", "voice", "library", "tts_off"] },
  { title: "General", ids: ["backup", "updates", "diagnostics"] },
];
import { setTipsEnabled, tipsEnabled } from "@/lib/tips";

const ICONS: Record<TourId, LucideIcon> = {
  dictate: Mic, speed: Gauge, microphone: Headset, insert_mode: ClipboardPaste, history: History,
  synthesize: FileUp, voice: Volume2, library: Library, tts_off: AudioLines, backup: Archive,
  updates: Bell, diagnostics: Stethoscope,
};

/** «Help»: short steps per task; «Show» opens the screen and highlights the real element. */
export function Help() {
  useI18n();
  const navigate = useNavigate();
  const { preferences } = usePreferences();
  const [tips, setTips] = useState(tipsEnabled);
  const ttsEnabled = preferences?.tts_enabled ?? true;

  function toggleTips(enabled: boolean) {
    setTipsEnabled(enabled);
    setTips(enabled);
  }

  return <div className="space-y-6">
    <div>
      <h2 className="text-2xl font-semibold tracking-tight">{t("helpTitle")}</h2>
      <p className="mt-1 text-sm text-muted-foreground">{t("helpIntro")}</p>
    </div>
    <Card className="py-0">
      <CardContent className="flex items-center justify-between gap-3 py-4">
        <div className="space-y-0.5">
          <label htmlFor="help-tips" className="text-sm font-medium">{t("helpTipsSwitch")}</label>
          <p className="text-xs text-muted-foreground">{t("helpTipsNote")}</p>
        </div>
        <Switch id="help-tips" checked={tips} onCheckedChange={toggleTips} />
      </CardContent>
    </Card>
    {GROUPS.map(group => <section key={group.title} className="space-y-2">
      <h3 className="text-sm font-semibold text-muted-foreground">{t(group.title)}</h3>
      <Card className="py-0">
        <CardContent className="divide-y p-0">
          {group.ids.map(id => TOURS.find(tour => tour.id === id)).filter((tour): tour is Tour => !!tour).map(tour => {
            const Icon = ICONS[tour.id];
            const steps = t(tour.steps).split("\n").filter(Boolean);
            const link = tourLink(tour, ttsEnabled);
            return <details key={tour.id} className="group">
              <summary className="flex cursor-pointer list-none items-center gap-3 px-4 py-3 [&::-webkit-details-marker]:hidden">
                <ChevronRight className="h-4 w-4 shrink-0 text-muted-foreground transition-transform group-open:rotate-90" aria-hidden />
                <Icon className="h-4 w-4 shrink-0" aria-hidden />
                <span className="flex-1 text-sm font-medium">{t(tour.title)}</span>
                <Button variant="outline" size="sm" onClick={event => { event.preventDefault(); navigate(link.route, { state: link.state }); }}>{t("helpShow")}</Button>
              </summary>
              <ol className="list-decimal space-y-1 pr-4 pb-4 pl-16 text-sm leading-snug">
                {steps.map(step => <li key={step}>{step}</li>)}
              </ol>
            </details>;
          })}
        </CardContent>
      </Card>
    </section>)}
  </div>;
}
