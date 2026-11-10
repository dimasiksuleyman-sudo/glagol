import { currentLocale } from "@/i18n";
import { AudioPlayer } from "@/components/AudioPlayer";
import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { AudioLines, Download, Pencil, Play, Search, Trash2, TriangleAlert, Volume2 } from "lucide-react";

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
import { markFeatureUsed } from "@/lib/tips";

/**
 * Library page — compact list of every synthesized document with one shared
 * player for the selected document, disk export, rename and delete. A search
 * field appears once the list is long.
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

/** Show the search field from this many documents. */
const SEARCH_FROM = 6;

export function Library() {
  useI18n();
  const [state, setState] = useState<LibraryState>({ kind: "loading" });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [assetUrl, setAssetUrl] = useState<string | null>(null);

  // Only the selected document's audio is resolved and loaded.
  useEffect(() => {
    if (selectedId === null) { setAssetUrl(null); return; }
    let cancelled = false;
    getAudioPath(selectedId)
      .then(abs => { if (!cancelled) setAssetUrl(convertFileSrc(abs)); })
      .catch(err => { if (!cancelled) { setAssetUrl(null); toast.error(stringifyError(err)); } });
    return () => { cancelled = true; };
  }, [selectedId]);

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
    if (selectedId === doc.id) setSelectedId(null);
    if (state.kind === "ready") {
      const filtered = state.documents.filter((d) => d.id !== doc.id);
      setState(
        filtered.length === 0 ? { kind: "empty" } : { kind: "ready", documents: filtered },
      );
    }
    try {
      await deleteDocument(doc.id);
    } catch (err) {
      toast.error(t("Could not delete: {p0}", { p0: stringifyError(err) }));
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
      toast.error(t("Could not rename: {p0}", { p0: stringifyError(err) }));
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
      title: t("Save WAV"),
      defaultPath: `${safeStem}.wav`,
      filters: [{ name: "WAV audio", extensions: ["wav"] }],
    });
    if (dest === null) return;
    try {
      await exportAudio(doc.id, dest);
      const filename = dest.split(/[\\/]/).pop() ?? dest;
      toast.success(t("Saved: {p0}", { p0: filename }));
    } catch (err) {
      toast.error(t("Could not save: {p0}", { p0: stringifyError(err) }));
    }
  }

  const documents = state.kind === "ready" ? state.documents : [];
  const needle = query.trim().toLocaleLowerCase(currentLocale());
  const shown = needle ? documents.filter(d => d.title.toLocaleLowerCase(currentLocale()).includes(needle)) : documents;
  const selected = documents.find(d => d.id === selectedId) ?? null;

  return (
    <div className="space-y-4">
      <Header />
      {state.kind === "loading" && <LoadingSkeleton />}
      {state.kind === "empty" && <EmptyState />}
      {state.kind === "error" && (
        <ErrorCard message={state.message} onRetry={fetchDocuments} />
      )}
      {state.kind === "ready" && (
        <>
          {documents.length >= SEARCH_FROM && <div className="relative">
            <Search className="text-muted-foreground absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2" aria-hidden />
            <Input value={query} onChange={e => setQuery(e.target.value)} placeholder={t("Search by title")} aria-label={t("Search by title")} className="pl-9" />
          </div>}
          <Card data-tour="library" className="py-0">
            <CardContent className="divide-y p-0">
              {shown.map((doc) => (
                <DocumentRow
                  key={doc.id}
                  document={doc}
                  selected={doc.id === selectedId}
                  onPlay={() => { markFeatureUsed("library"); setSelectedId(doc.id); }}
                  onDelete={() => handleDelete(doc)}
                  onExport={() => handleExport(doc)}
                  onRename={(next) => handleRename(doc, next)}
                />
              ))}
              {shown.length === 0 && <p className="text-muted-foreground p-4 text-sm">{t("Nothing found.")}</p>}
            </CardContent>
          </Card>
          {selected && assetUrl !== null && (
            <div className="bg-background/95 sticky bottom-0 space-y-2 border-t pt-3 pb-1 backdrop-blur" data-tour="library-player">
              <p className="flex items-center gap-2 truncate text-sm font-medium"><Volume2 className="h-4 w-4 shrink-0" aria-hidden />{selected.title}</p>
              <AudioPlayer key={selected.id} src={assetUrl} autoPlay />
            </div>
          )}
        </>
      )}
    </div>
  );
}

function Header() {
  useI18n();
  return (
    <div>
      <h2 className="text-2xl font-semibold tracking-tight">{t("Library")}</h2>
      <p className="text-muted-foreground mt-1 text-sm">
        {t("Your synthesized documents.")}{" "}</p>
    </div>
  );
}

function LoadingSkeleton() {
  useI18n();
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
  useI18n();
  return (
    <Card data-tour="library">
      <CardContent className="space-y-4 pt-12 pb-12 text-center">
        <AudioLines className="text-muted-foreground mx-auto h-12 w-12" />
        <p className="text-lg">{t("Your synthesized documents will appear here")}</p>
        <Button asChild>
          <Link to="/synthesize">{t("Synthesize your first document")}</Link>
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
  useI18n();
  return (
    <Card>
      <CardContent className="space-y-4 pt-6">
        <div className="flex items-start gap-3">
          <TriangleAlert className="text-destructive mt-0.5 h-5 w-5 shrink-0" />
          <div className="space-y-1">
            <p className="font-medium">{t("Could not load library")}</p>
            <p className="text-muted-foreground text-sm">{message}</p>
          </div>
        </div>
        <Button onClick={onRetry} variant="secondary">
          {t("Try again")}{" "}</Button>
      </CardContent>
    </Card>
  );
}

interface DocumentRowProps {
  document: DocumentRecord;
  /** This document is loaded in the shared player. */
  selected: boolean;
  onPlay: () => void;
  onDelete: () => void;
  onExport: () => void;
  /** Awaited by the row's save handler before exiting edit mode.
   * Parent does the optimistic update and revert-on-error; this
   * callback never throws. */
  onRename: (newTitle: string) => Promise<void>;
}

function DocumentRow({ document, selected, onPlay, onDelete, onExport, onRename }: DocumentRowProps) {
  useI18n();
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

  const charCountLabel = t("{p0} chars", { p0: document.char_count.toLocaleString(currentLocale()) });
  const isEditing = draft !== null;

  return (
    <div className={selected ? "bg-muted/60" : undefined}>
      <div className="flex items-center gap-3 px-4 py-3">
          {document.audio_path !== null
            ? <Button size="icon" variant={selected ? "default" : "outline"} onClick={onPlay} title={t("playAudio")} aria-label={t("playAudio")} aria-pressed={selected}>
              {selected ? <Volume2 className="h-4 w-4" /> : <Play className="h-4 w-4" />}
            </Button>
            : <TriangleAlert className="text-destructive mx-2.5 h-4 w-4 shrink-0" aria-label={t("Error")} />}
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
                aria-label={t("Document title")}
                className="h-8"
              />
            ) : (
              <p className="truncate font-medium">{document.title}</p>
            )}
            <p className="text-muted-foreground mt-0.5 truncate text-xs">
              {document.audio_duration_ms !== null && <>{formatDuration(document.audio_duration_ms)} · </>}
              {getVoiceLabel(document.voice)} · {charCountLabel} ·{" "}
              {formatRelativeTime(document.created_at)}
              {document.speech_language && <> · {document.speech_language === "en" ? t("English") : t("Russian")}</>}
            </p>
          </div>
          <div className="flex shrink-0 gap-1">
            <Button
              size="icon"
              variant="ghost"
              onClick={enterEdit}
              disabled={isEditing}
              title={t("Rename")}
              aria-label={t("Rename")}
            >
              <Pencil className="h-4 w-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={onExport}
              title={t("Save to disk")}
              aria-label={t("Save to disk")}
            >
              <Download className="h-4 w-4" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              onClick={onDelete}
              title={t("Delete")}
              aria-label={t("Delete")}
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          </div>
      </div>
    </div>
  );
}

/** 0:42, 12:05 or 1:02:05 for a length in milliseconds. */
function formatDuration(ms: number): string {
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600), m = Math.floor((total % 3600) / 60), s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
