import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { FolderOpen } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Switch } from "@/components/ui/switch";
import { getDiagnosticsSettings, openLogFolder, setDetailedLogging, type DiagnosticsSettings } from "@/lib/tauri";

/** Settings → Diagnostics: «Detailed log» switch and the log folder. */
export function DiagnosticsSection() {
  useI18n();
  const [settings, setSettings] = useState<DiagnosticsSettings | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    getDiagnosticsSettings().then(setSettings).catch((e) => toast.error(String(e)));
  }, []);

  async function handleToggle(enabled: boolean) {
    setSaving(true);
    try {
      setSettings(await setDetailedLogging(enabled));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("Diagnostics")}</CardTitle>
        <CardDescription>
          {t("The log records timings, hardware and errors. It never contains your speech, recognized text, documents or keys.")}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <div className="space-y-0.5">
            <label htmlFor="diagnostics-detailed" className="text-sm font-medium">
              {t("Detailed log")}
            </label>
            <p className="text-muted-foreground text-xs">
              {t("Off by default. Turn it on to investigate slow dictation, reproduce the problem, then send the log file from the folder below.")}
            </p>
          </div>
          <Switch
            id="diagnostics-detailed"
            checked={settings?.detailed ?? false}
            onCheckedChange={handleToggle}
            disabled={!settings || saving}
          />
        </div>
        <div className="space-y-1">
          <Button variant="outline" onClick={() => void openLogFolder().catch((e) => toast.error(String(e)))}>
            <FolderOpen className="h-4 w-4" aria-hidden />
            {t("Open log folder")}
          </Button>
          {settings && <p className="text-muted-foreground text-xs break-all">{settings.logDir}</p>}
        </div>
      </CardContent>
    </Card>
  );
}
