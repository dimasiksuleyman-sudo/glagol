import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { Archive, Upload } from "lucide-react";

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
import { buttonVariants } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { pluralizeDocuments, pluralizeFiles } from "@/lib/pluralize";
import {
  BACKUP_PROGRESS_EVENT,
  BACKUP_RESTORE_PROGRESS_EVENT,
  createBackup,
  listDocuments,
  relaunchApp,
  restoreBackup,
  validateBackup,
  type BackupManifest,
  type BackupProgressEvent,
} from "@/lib/tauri";

/**
 * Wait this long after the success toast before relaunching the app
 * so the user can read the «Приложение перезапустится…» message
 * before the window blinks away. Matches Sprint 5c D4.
 */
const RELAUNCH_DELAY_MS = 2000;

type ProgressState = { current: number; total: number };

type ActiveOperation = "create" | "restore" | null;

type PendingRestore = {
  sourcePath: string;
  manifest: BackupManifest;
  currentCount: number;
};

export function BackupSection() {
  useI18n();
  const [activeOp, setActiveOp] = useState<ActiveOperation>(null);
  const [progress, setProgress] = useState<ProgressState | null>(null);
  const [pendingRestore, setPendingRestore] = useState<PendingRestore | null>(null);

  // Wire up the two progress channels for the lifetime of the
  // component. Both update the same `progress` state — only one
  // operation runs at a time, so reusing the slot avoids modal
  // gymnastics and keeps the render logic linear.
  useEffect(() => {
    const unlistenCreate = listen<BackupProgressEvent>(BACKUP_PROGRESS_EVENT, (event) => {
      setProgress(event.payload);
    });
    const unlistenRestore = listen<BackupProgressEvent>(
      BACKUP_RESTORE_PROGRESS_EVENT,
      (event) => {
        setProgress(event.payload);
      },
    );
    return () => {
      unlistenCreate.then((fn) => fn()).catch(() => {});
      unlistenRestore.then((fn) => fn()).catch(() => {});
    };
  }, []);

  async function handleCreateBackup() {
    const folder = await open({
      directory: true,
      multiple: false,
      title: t("Choose a backup folder"),
    });
    if (typeof folder !== "string") {
      return;
    }

    setActiveOp("create");
    setProgress({ current: 0, total: 0 });
    try {
      const fullPath = await createBackup(folder);
      const filename = fullPath.split(/[\\/]/).pop() ?? fullPath;
      toast.success(t("Backup created: {p0}", { p0: filename }));
    } catch (err) {
      toast.error(t("Could not create backup: {p0}", { p0: stringifyError(err) }));
    } finally {
      setActiveOp(null);
      setProgress(null);
    }
  }

  async function handlePickRestoreSource() {
    const file = await open({
      multiple: false,
      directory: false,
      title: t("Choose a backup"),
      filters: [{ name: t("Glagol backup"), extensions: ["zip"] }],
    });
    if (typeof file !== "string") {
      return;
    }

    // Validation is fast (~50 ms) and non-destructive. Run it inline
    // so we can pre-fill the confirm dialog with backup counts.
    let manifest: BackupManifest;
    try {
      manifest = await validateBackup(file);
    } catch (err) {
      toast.error(
        t("This file is not a valid Glagol backup: {p0}", { p0: stringifyError(err) }),
      );
      return;
    }

    // Look up the current library size so the confirm dialog can
    // contrast "now" vs "after restore" in concrete numbers.
    let currentCount = 0;
    try {
      const docs = await listDocuments();
      currentCount = docs.length;
    } catch {
      // Library page already surfaces its own errors; for the
      // confirm dialog "unknown" effectively renders as zero, which
      // is the safer default — the user can still see the backup
      // size and decide whether to proceed.
    }

    setPendingRestore({ sourcePath: file, manifest, currentCount });
  }

  async function handleConfirmRestore() {
    if (!pendingRestore) return;
    const sourcePath = pendingRestore.sourcePath;
    setPendingRestore(null);
    setActiveOp("restore");
    setProgress({ current: 0, total: 0 });
    try {
      await restoreBackup(sourcePath);
      toast.success(t("Restore complete. The application will restart."));
      // Brief pause so the success toast is readable before the
      // process is replaced. relaunchApp never resolves on success.
      await new Promise((resolve) => setTimeout(resolve, RELAUNCH_DELAY_MS));
      await relaunchApp();
    } catch (err) {
      toast.error(t("Restore failed: {p0}", { p0: stringifyError(err) }));
      setActiveOp(null);
      setProgress(null);
    }
  }

  const operationInProgress = activeOp !== null;
  const percent =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.current / progress.total) * 100))
      : 0;
  const progressTitle =
    activeOp === "restore" ? t("Restore from backup") : t("Creating backup");
  const progressVerb = activeOp === "restore" ? t("Restoring") : t("Creating backup");

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("Backup")}</CardTitle>
        <CardDescription>
          {t("Save or restore your entire library, including documents and audio, in one archive.")}{" "}</CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap gap-2">
          <Button onClick={handleCreateBackup} disabled={operationInProgress}>
            <Archive className="mr-2 size-4" />
            {t("Create backup")}{" "}</Button>
          <Button
            variant="secondary"
            onClick={handlePickRestoreSource}
            disabled={operationInProgress}
          >
            <Upload className="mr-2 size-4" />
            {t("Restore from backup")}{" "}</Button>
        </div>
      </CardContent>

      {/* Progress modal — non-dismissible while activeOp is set. */}
      <AlertDialog open={operationInProgress}>
        <AlertDialogContent
          // AlertDialog already ignores outside-click; only Esc would
          // dismiss it, which we also block during the operation so a
          // bumped key doesn't tear down the modal mid-write.
          onEscapeKeyDown={(e) => e.preventDefault()}
        >
          <AlertDialogHeader>
            <AlertDialogTitle>{progressTitle}</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 text-sm">
                <p>
                  {progressVerb}…{" "}
                  {progress && progress.total > 0 ? (
                    <span className="text-foreground font-medium">
                      {progress.current} / {progress.total} {pluralizeFiles(progress.total)}
                    </span>
                  ) : (
                    <span className="text-muted-foreground">{t("preparing…")}</span>
                  )}
                </p>
                <Progress value={percent} />
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
        </AlertDialogContent>
      </AlertDialog>

      {/* Confirm restore — destructive, requires explicit click. */}
      <AlertDialog
        open={pendingRestore !== null}
        onOpenChange={(open) => {
          if (!open) setPendingRestore(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{t("Restore from backup")}</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 text-sm">
                <p>{t("This will replace your entire current library:")}</p>
                {pendingRestore && (
                  <ul className="bg-muted/40 space-y-1 rounded-md border p-3 font-mono text-xs">
                    <li>
                      {t("Currently in library:")}{" "}
                      <span className="text-foreground font-semibold">
                        {pendingRestore.currentCount}
                      </span>{" "}
                      {pluralizeDocuments(pendingRestore.currentCount)}
                    </li>
                    <li>
                      {t("In backup:")}{" "}
                      <span className="text-foreground font-semibold">
                        {pendingRestore.manifest.document_count}
                      </span>{" "}
                      {pluralizeDocuments(pendingRestore.manifest.document_count)}
                    </li>
                  </ul>
                )}
                <p>{t("Current data will be permanently deleted.")}</p>
                <p className="text-muted-foreground">
                  {t("A backup of the current state is created automatically before restoring, in the same folder as the selected backup.")}{" "}</p>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("Cancel")}</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={handleConfirmRestore}
            >
              {t("Restore")}{" "}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
