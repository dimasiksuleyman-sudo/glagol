import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { AudioLines, Download, Pencil, Trash2, TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";

import {
  type DocumentRecord,
  deleteDocument,
  exportAudio,
  getAudioPath,
  listDocuments,
  updateDocumentTitle,
} from "@/lib/tauri";
import { formatRelativeTime } from "@/lib/format";
import { getVoiceLabel } from "@/lib/voices";

/**
 * Library page — stacked list of every synthesized document, with
 * native HTML5 audio playback, disk export, and instant delete.
 *
 * State machine (discriminated union):
 *   loading → ready | empty | error
 *   ready   → empty (after last delete) | error (on retry failure)
 *   empty   → ready (after navigating back from a new synthesis)
 *   error   → loading (on retry)
 *
 * Refetch strategy: mount-time only. React Router unmounts/remounts on
 * navigation so coming back to /library after synthesising is enough
 * to see the new row. Sprint 4 will add event-based updates when
 * background synthesis lands.
 */
type LibraryState =
  | { kind: "loading" }
  | { kind: "empty" }
  | { kind: "ready"; documents: DocumentRecord[] }
  | { kind: "error"; message: string };

export function Library() {
  const [state, setState] = useState<LibraryState>({ kind: "loading" });

  async function fetchDocuments() {
    setState({ kind: "loading" });
    try {
      const docs = await listDocuments();
      setState(docs.length === 0 ? { kind: "empty" } : { kind: "ready", documents: docs });
    } catch (err) {
      setState({ kind: "error", message: stringifyError(err) });
    }
  }

  useEffect(() => {
    fetchDocuments();
  }, []);

  async function handleDelete(doc: DocumentRecord) {
    // Optimistic UI: remove the row first; if the backend complains,
    // resync from server. The reverse order (await then update) would
    // give a sluggish click feel for a single-user app on local IPC.
    if (state.kind === "ready") {
      const filtered = state.documents.filter((d) => d.id !== doc.id);
      setState(
        filtered.length === 0 ? { kind: "empty" } : { kind: "ready", documents: filtered },
      );
    }
    try {
      await deleteDocument(doc.id);
    } catch (err) {
      toast.error(`Не удалось удалить: ${stringifyError(err)}`);
      await fetchDocuments();
    }
  }

  /**
   * Optimistic rename: swap the title in local state immediately,
   * fire the backend command, and on failure revert + toast. Never
   * throws — DocumentRow can leave edit mode unconditionally on
   * await completion.
   */
  async function handleRename(doc: DocumentRecord, newTitle: string) {
    if (state.kind !== "ready") return;
    const original = doc.title;
    const updated = state.documents.map((d) =>
      d.id === doc.id ? { ...d, title: newTitle } : d,
    );
    setState({ kind: "ready", documents: updated });
    try {
      await updateDocumentTitle(doc.id, newTitle);
    } catch (err) {
      toast.error(`Не удалось переименовать: ${stringifyError(err)}`);
      // Revert the optimistic change. Re-read state (it may have
      // shifted out from under us) before mutating, but the common
      // path is: still in `ready`, just put the original title back.
      setState((prev) =>
        prev.kind === "ready"
          ? {
              kind: "ready",
              documents: prev.documents.map((d) =>
                d.id === doc.id ? { ...d, title: original } : d,
              ),
            }
          : prev,
      );
    }
  }

  async function handleExport(doc: DocumentRecord) {
    const safeStem =
      doc.title.replace(/[\\/:*?"<>|]/g, "_").trim().slice(0, 80) || "glagol";
    const dest = await save({
      title: "Сохранить WAV",
      defaultPath: `${safeStem}.wav`,
      filters: [{ name: "WAV audio", extensions: ["wav"] }],
    });
    if (dest === null) return;
    try {
      await exportAudio(doc.id, dest);
      const filename = dest.split(/[\\/]/).pop() ?? dest;
      toast.success(`Сохранено: ${filename}`);
    } catch (err) {
      toast.error(`Не удалось сохранить: ${stringifyError(err)}`);
    }
  }

  return (
    <div className="space-y-6">
      <Header />
      {state.kind === "loading" && <LoadingSkeleton />}
      {state.kind === "empty" && <EmptyState />}
      {state.kind === "error" && (
        <ErrorCard message={state.message} onRetry={fetchDocuments} />
      )}
      {state.kind === "ready" && (
        <div className="space-y-3">
          {state.documents.map((doc) => (
            <DocumentRow
              key={doc.id}
              document={doc}
              onDelete={() => handleDelete(doc)}
              onExport={() => handleExport(doc)}
              onRename={(next) => handleRename(doc, next)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function Header() {
  return (
    <div>
      <h2 className="text-2xl font-semibold tracking-tight">Библиотека</h2>
      <p className="text-muted-foreground mt-1 text-sm">
        История озвученных документов.
      </p>
    </div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="space-y-3">
      {[0, 1, 2].map((i) => (
        <Card key={i}>
          <CardContent className="space-y-2 pt-4">
            <Skeleton className="h-5 w-2/3" />
            <Skeleton className="h-4 w-1/3" />
            <Skeleton className="h-10 w-full" />
          </CardContent>
        </Card>
      ))}
    </div>
  );
}

function EmptyState() {
  return (
    <Card>
      <CardContent className="space-y-4 pt-12 pb-12 text-center">
        <AudioLines className="text-muted-foreground mx-auto h-12 w-12" />
        <p className="text-lg">Здесь будут ваши озвученные документы</p>
        <Button asChild>
          <Link to="/synthesize">Озвучить первый документ</Link>
        </Button>
      </CardContent>
    </Card>
  );
}

interface ErrorCardProps {
  message: string;
  onRetry: () => void;
}

function ErrorCard({ message, onRetry }: ErrorCardProps) {
  return (
    <Card>
      <CardContent className="space-y-4 pt-6">
        <div className="flex items-start gap-3">
          <TriangleAlert className="text-destructive mt-0.5 h-5 w-5 shrink-0" />
          <div className="space-y-1">
            <p className="font-medium">Не удалось загрузить библиотеку</p>
            <p className="text-muted-foreground text-sm">{message}</p>
          </div>
        </div>
        <Button onClick={onRetry} variant="secondary">
          Попробовать снова
        </Button>
      </CardContent>
    </Card>
  );
}

interface DocumentRowProps {
  document: DocumentRecord;
  onDelete: () => void;
  onExport: () => void;
  /** Awaited by the row's save handler before exiting edit mode.
   * Parent does the optimistic update and revert-on-error; this
   * callback never throws. */
  onRename: (newTitle: string) => Promise<void>;
}

function DocumentRow({ document, onDelete, onExport, onRename }: DocumentRowProps) {
  // Asset URL is resolved lazily per row: getAudioPath is a cheap IPC
  // call, and doing it here keeps `list_documents` a thin wrapper.
  // For Sprint 2 row counts (<= a few dozen) parallel resolution is
  // fine; Sprint 5 may push paths into list_documents if profiling
  // shows it.
  const [assetUrl, setAssetUrl] = useState<string | null>(null);

  useEffect(() => {
    if (document.audio_path === null) {
      setAssetUrl(null);
      return;
    }
    let cancelled = false;
    getAudioPath(document.id)
      .then((abs) => {
        if (!cancelled) setAssetUrl(convertFileSrc(abs));
      })
      .catch(() => {
        if (!cancelled) setAssetUrl(null);
      });
    return () => {
      cancelled = true;
    };
  }, [document.id, document.audio_path]);

  // ── Inline title edit state ──────────────────────────────────────
  // `null` = display mode; `string` = edit mode with the current draft.
  // The `inFlight` ref guards against the Enter-then-Blur double-save
  // sequence (keyDown triggers commit + clears state; the imminent
  // blur sees the cleared state and skips, but if the blur fires
  // *before* React applies the state update the guard still catches
  // it).
  const [draft, setDraft] = useState<string | null>(null);
  const inFlight = useRef<boolean>(false);

  function enterEdit() {
    setDraft(document.title);
  }

  async function commit() {
    if (draft === null || inFlight.current) return;
    const trimmed = draft.trim();
    inFlight.current = true;
    try {
      // Empty / whitespace-only → silent revert (no backend call,
      // no toast). Same-value → no-op revert (skip the IPC roundtrip).
      // Otherwise → fire optimistic rename; parent handles errors.
      if (trimmed.length > 0 && trimmed !== document.title) {
        await onRename(trimmed);
      }
    } finally {
      setDraft(null);
      inFlight.current = false;
    }
  }

  function cancel() {
    setDraft(null);
  }

  const charCountLabel = `${document.char_count.toLocaleString("ru-RU")} симв.`;
  const isEditing = draft !== null;

  return (
    <Card>
      <CardContent className="space-y-3 pt-4">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            {isEditing ? (
              <Input
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void commit();
                  } else if (e.key === "Escape") {
                    e.preventDefault();
                    cancel();
                  }
                }}
                onBlur={() => {
                  void commit();
                }}
                autoFocus
                onFocus={(e) => e.target.select()}
                aria-label="Название документа"
                className="h-8"
              />
            ) : (
              <p className="truncate font-medium">{document.title}</p>
            )}
            <p className="text-muted-foreground mt-1 text-sm">
              {getVoiceLabel(document.voice)} · {charCountLabel} ·{" "}
              {formatRelativeTime(document.created_at)}
            </p>
          </div>
          <div className="flex shrink-0 gap-1">
            <Button
              size="icon"
              variant="ghost"
              onClick={enterEdit}
              disabled={isEditing}
              title="Переименовать"
              aria-label="Переименовать"
            >
              <Pencil className="h-4 w-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={onExport}
              title="Сохранить на диск"
              aria-label="Сохранить на диск"
            >
              <Download className="h-4 w-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={onDelete}
              title="Удалить"
              aria-label="Удалить"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
        </div>
        {assetUrl !== null && (
          <audio
            key={document.id}
            src={assetUrl}
            controls
            controlsList="nodownload"
            preload="none"
            className="w-full"
          />
        )}
      </CardContent>
    </Card>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
