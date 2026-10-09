import { t, currentLocale } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Download, RefreshCw } from "lucide-react";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Progress } from "@/components/ui/progress";
import { Switch } from "@/components/ui/switch";
import {
  getUpdateSettings,
  installUpdate,
  type UpdateProgress,
  type UpdateSettings,
} from "@/lib/tauri";
import { changeAutoCheck, runUpdateCheck, useAutoCheck, useAvailableUpdate } from "@/lib/updates";

const mb = (bytes: number) => (bytes / 1e6).toLocaleString(currentLocale(), { maximumFractionDigits: 1 });

type Busy = "idle" | "checking" | "installing";

/** Settings → Updates: current version, manual check, auto-check toggle, install. */
export function UpdatesSection() {
  useI18n();
  const [settings, setSettings] = useState<UpdateSettings | null>(null);
  const [busy, setBusy] = useState<Busy>("idle");
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const update = useAvailableUpdate();
  const sharedAutoCheck = useAutoCheck();

  useEffect(() => {
    getUpdateSettings().then(setSettings).catch((e) => toast.error(String(e)));
  }, []);

  async function handleCheck() {
    setBusy("checking");
    try {
      const found = await runUpdateCheck();
      if (!found) toast.success(t("You have the latest version."));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusy("idle");
    }
  }

  async function handleToggle(enabled: boolean) {
    try {
      await changeAutoCheck(enabled);
    } catch (e) {
      toast.error(String(e));
    }
  }

  async function handleInstall() {
    setConfirmOpen(false);
    setBusy("installing");
    setProgress(null);
    try {
      // On success the installer replaces the app and this process exits.
      await installUpdate(setProgress);
    } catch (e) {
      toast.error(String(e));
      setBusy("idle");
      setProgress(null);
    }
  }

  const downloading = progress?.kind === "downloading" ? progress : null;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("Updates")}</CardTitle>
        <CardDescription>
          {settings ? t("Version {version}", { version: settings.currentVersion }) : " "}
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center justify-between gap-3">
          <div className="space-y-0.5">
            <label htmlFor="update-auto-check" className="text-sm font-medium">
              {t("Check automatically at startup")}
            </label>
            <p className="text-muted-foreground text-xs">
              {t("Off by default. When on, Glagol asks GitHub for a newer version once after it starts.")}
            </p>
          </div>
          <Switch
            id="update-auto-check"
            checked={sharedAutoCheck ?? settings?.autoCheck ?? false}
            onCheckedChange={handleToggle}
            disabled={!settings || busy === "installing"}
          />
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <Button variant="outline" onClick={handleCheck} disabled={busy !== "idle"}>
            <RefreshCw className={busy === "checking" ? "h-4 w-4 animate-spin" : "h-4 w-4"} aria-hidden />
            {busy === "checking" ? t("Checking…") : t("Check for updates")}
          </Button>
          {update && (
            <Button onClick={() => setConfirmOpen(true)} disabled={busy !== "idle"}>
              <Download className="h-4 w-4" aria-hidden />
              {t("Install update")}
            </Button>
          )}
        </div>

        {update && busy !== "installing" && (
          <p className="text-sm">{t("Version {version} is available.", { version: update.version })}</p>
        )}

        {busy === "installing" && (
          <div className="space-y-1">
            <Progress
              value={downloading?.total ? (100 * downloading.downloaded) / downloading.total : progress ? 100 : 0}
            />
            <p className="text-muted-foreground text-xs">
              {progress?.kind === "installing"
                ? t("Installing update…")
                : `${t("Downloading update…")}${
                    downloading
                      ? ` ${mb(downloading.downloaded)}${downloading.total ? ` / ${mb(downloading.total)}` : ""} ${t("MB")}`
                      : ""
                  }`}
            </p>
          </div>
        )}
      </CardContent>

      <AlertDialog open={confirmOpen} onOpenChange={setConfirmOpen}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {update ? t("Install version {version}?", { version: update.version }) : t("Install update")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("Glagol will download the update, verify its signature, close and install it, then start again. Your documents, settings, keys and speech components are kept.")}
            </AlertDialogDescription>
          </AlertDialogHeader>
          {update?.notes?.trim() && (
            <div className="space-y-1">
              <p className="text-sm font-medium">{t("What's new")}</p>
              {/* Plain text from the signed release manifest; rendered as text, never as HTML. */}
              <p className="text-muted-foreground max-h-48 overflow-y-auto text-sm whitespace-pre-line">
                {update.notes.trim()}
              </p>
            </div>
          )}
          <AlertDialogFooter>
            <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
            <AlertDialogAction onClick={handleInstall}>{t("Install")}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  );
}
