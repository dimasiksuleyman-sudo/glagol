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
      title: "Выберите папку для резервной копии",
    });
    if (typeof folder !== "string") {
      return;
    }

    setActiveOp("create");
    setProgress({ current: 0, total: 0 });
    try {
      const fullPath = await createBackup(folder);
      const filename = fullPath.split(/[\\/]/).pop() ?? fullPath;
      toast.success(`Резервная копия создана: ${filename}`);
    } catch (err) {
      toast.error(`Не удалось создать резервную копию: ${stringifyError(err)}`);
    } finally {
      setActiveOp(null);
      setProgress(null);
    }
  }

  async function handlePickRestoreSource() {
    const file = await open({
      multiple: false,
      directory: false,
      title: "Выберите резервную копию",
      filters: [{ name: "Резервная копия Glagol", extensions: ["zip"] }],
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
        `Этот файл не является корректной резервной копией Glagol: ${stringifyError(err)}`,
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
      toast.success("Восстановление завершено. Приложение перезапустится.");
      // Brief pause so the success toast is readable before the
      // process is replaced. relaunchApp never resolves on success.
      await new Promise((resolve) => setTimeout(resolve, RELAUNCH_DELAY_MS));
      await relaunchApp();
    } catch (err) {
      toast.error(`Восстановление не удалось: ${stringifyError(err)}`);
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
    activeOp === "restore" ? "Восстановление из резервной копии" : "Создание резервной копии";
  const progressVerb = activeOp === "restore" ? "Восстанавливаю" : "Создаю резервную копию";

  return (
    <Card>
      <CardHeader>
        <CardTitle>Резервное копирование</CardTitle>
        <CardDescription>
          Сохраните или восстановите всю библиотеку — документы и аудиофайлы — одним
          архивом.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex flex-wrap gap-2">
          <Button onClick={handleCreateBackup} disabled={operationInProgress}>
            <Archive className="mr-2 size-4" />
            Создать резервную копию
          </Button>
          <Button
            variant="secondary"
            onClick={handlePickRestoreSource}
            disabled={operationInProgress}
          >
            <Upload className="mr-2 size-4" />
            Восстановить из резервной копии
          </Button>
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
                      {progress.current} / {progress.total} файлов
                    </span>
                  ) : (
                    <span className="text-muted-foreground">подготовка…</span>
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
            <AlertDialogTitle>Восстановление из резервной копии</AlertDialogTitle>
            <AlertDialogDescription asChild>
              <div className="space-y-3 text-sm">
                <p>Эта операция полностью заменит текущую библиотеку:</p>
                {pendingRestore && (
                  <ul className="bg-muted/40 space-y-1 rounded-md border p-3 font-mono text-xs">
                    <li>
                      Сейчас в библиотеке:{" "}
                      <span className="text-foreground font-semibold">
                        {pendingRestore.currentCount}
                      </span>{" "}
                      документов
                    </li>
                    <li>
                      В резервной копии:{" "}
                      <span className="text-foreground font-semibold">
                        {pendingRestore.manifest.document_count}
                      </span>{" "}
                      документов
                    </li>
                  </ul>
                )}
                <p>Текущие данные будут безвозвратно удалены.</p>
                <p className="text-muted-foreground">
                  Резервная копия текущего состояния создаётся автоматически перед
                  восстановлением (в той же папке, что и исходный файл).
                </p>
              </div>
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Отмена</AlertDialogCancel>
            <AlertDialogAction
              className={cn(buttonVariants({ variant: "destructive" }))}
              onClick={handleConfirmRestore}
            >
              Восстановить
            </AlertDialogAction>
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
