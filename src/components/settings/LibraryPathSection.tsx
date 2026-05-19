import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { FolderOpen, RotateCcw } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";

import { getLibraryPath, setLibraryPath, type LibraryPathInfo } from "@/lib/tauri";

/**
 * Settings → "Папка библиотеки" section.
 *
 * Reads the current effective path on mount, lets the user pick a new
 * folder via the native picker, or reset to the default. Backend runs
 * the D4 validation chain (absolute → create_dir_all → writable probe
 * → canonicalise → compare-with-default → register asset-protocol
 * scope) and returns Russian-language error strings on failure;
 * we surface them as toasts.
 *
 * Refetches after every successful save so the UI reflects backend
 * canonicalisation (e.g. saving a path that equals the default
 * collapses to `configured: null`, "Сбросить" button hides).
 */
export function LibraryPathSection() {
  const [info, setInfo] = useState<LibraryPathInfo | null>(null);
  const [isBusy, setIsBusy] = useState<boolean>(false);

  async function refresh() {
    try {
      const next = await getLibraryPath();
      setInfo(next);
    } catch (err) {
      toast.error(stringifyError(err));
    }
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function handleChange() {
    if (info === null) return;
    setIsBusy(true);
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        defaultPath: info.effective,
      });
      // User cancelled the picker, or (defensively) plugin returned an
      // array shape we didn't ask for — both treated as no-op.
      if (typeof picked !== "string") return;
      await setLibraryPath(picked);
      toast.success("Папка библиотеки обновлена.");
      await refresh();
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsBusy(false);
    }
  }

  async function handleReset() {
    setIsBusy(true);
    try {
      await setLibraryPath("");
      toast.success("Возвращено к расположению по умолчанию.");
      await refresh();
    } catch (err) {
      toast.error(stringifyError(err));
    } finally {
      setIsBusy(false);
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>Папка библиотеки</CardTitle>
        <CardDescription>
          Где хранятся озвученные документы. По умолчанию — папка приложения; можно перенести
          на любой диск.
        </CardDescription>
      </CardHeader>
      <CardContent className="space-y-4">
        {info === null ? (
          <Skeleton className="h-5 w-3/4" />
        ) : (
          <div className="text-sm">
            <code className="bg-muted block overflow-x-auto rounded px-2 py-1 font-mono text-xs">
              {info.effective}
            </code>
            {info.configured === null && (
              <p className="text-muted-foreground mt-1 text-xs">Расположение по умолчанию.</p>
            )}
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          <Button onClick={handleChange} disabled={isBusy || info === null}>
            <FolderOpen className="mr-1 h-4 w-4" />
            Изменить…
          </Button>
          {info?.configured !== null && info?.configured !== undefined && (
            <Button onClick={handleReset} disabled={isBusy} variant="outline">
              <RotateCcw className="mr-1 h-4 w-4" />
              Сбросить
            </Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
