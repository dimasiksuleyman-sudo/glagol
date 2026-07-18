import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { formatRelativeTime } from "@/lib/format";
import { pluralizeEntries } from "@/lib/pluralize";
import { cn } from "@/lib/utils";
import type { Dictation } from "@/lib/tauri";

/** Preview length before an entry is truncated (D6). */
const PREVIEW_CHARS = 80;

/** Collapse a transcript to a single-line preview (D6). */
export function previewText(text: string): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  if (oneLine.length <= PREVIEW_CHARS) return oneLine;
  return oneLine.slice(0, PREVIEW_CHARS).trimEnd() + "…";
}

interface StatusBadgeProps {
  status: string;
}

/** Map a row `status` to a compact Russian badge (D6). */
function StatusBadge({ status }: StatusBadgeProps) {
  const map: Record<string, { label: string; className: string }> = {
    pasted: { label: "Вставлено", className: "text-emerald-600 dark:text-emerald-400" },
    clipboard: { label: "Скопировано", className: "text-muted-foreground" },
    error: { label: "Ошибка", className: "text-destructive" },
  };
  const entry = map[status] ?? { label: status, className: "text-muted-foreground" };
  return <span className={cn("text-xs font-medium", entry.className)}>{entry.label}</span>;
}

interface DictationHistoryProps {
  entries: Dictation[];
  enabled: boolean;
  busy?: boolean;
  onToggle: (enabled: boolean) => void;
  onClear: () => void;
  onCopy: (text: string) => void;
}

/**
 * Dictation history section (D5/D6). The toggle governs the *future* — turning
 * it off only stops new transcripts being written (D5); it does not wipe what is
 * already there. «Очистить историю» is the only path that deletes the past. Each
 * of the ≤10 rows shows a one-line preview + status; clicking expands the full
 * transcript with a «Копировать» button that puts it back on the clipboard so
 * the user can re-paste an earlier dictation. `error` rows surface their message.
 */
export function DictationHistory({
  entries,
  enabled,
  busy,
  onToggle,
  onClear,
  onCopy,
}: DictationHistoryProps) {
  const [expandedId, setExpandedId] = useState<number | null>(null);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div className="space-y-0.5">
          <label htmlFor="history-toggle" className="text-sm font-medium">
            Сохранять историю
          </label>
          <p className="text-muted-foreground text-xs">
            По умолчанию выключено — тексты не касаются диска без необходимости. При
            выключении накопленное остаётся видимым до нажатия «Очистить».
          </p>
        </div>
        <Switch
          id="history-toggle"
          checked={enabled}
          onCheckedChange={onToggle}
          disabled={busy}
        />
      </div>

      {entries.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          {enabled
            ? "История пуста — надиктуйте что-нибудь, и последние записи появятся здесь."
            : "История выключена. Включите переключатель, чтобы сохранять до 10 последних расшифровок."}
        </p>
      ) : (
        <>
          <ul className="divide-border divide-y rounded-md border">
            {entries.map((entry) => {
              const expanded = expandedId === entry.id;
              return (
                <li key={entry.id} className="p-0">
                  <button
                    type="button"
                    className="hover:bg-muted/50 flex w-full items-center justify-between gap-3 px-3 py-2 text-left transition-colors"
                    aria-expanded={expanded}
                    onClick={() => setExpandedId(expanded ? null : entry.id)}
                  >
                    <span className="min-w-0 flex-1 truncate text-sm">
                      {previewText(entry.text) || "—"}
                    </span>
                    <span className="flex shrink-0 items-center gap-3">
                      <StatusBadge status={entry.status} />
                      <span className="text-muted-foreground text-xs">
                        {formatRelativeTime(entry.created_at)}
                      </span>
                    </span>
                  </button>
                  {expanded && (
                    <div className="bg-muted/30 space-y-2 px-3 py-2">
                      <p className="text-sm break-words whitespace-pre-wrap">{entry.text}</p>
                      {entry.status === "error" && entry.error_message && (
                        <p className="text-destructive text-xs">{entry.error_message}</p>
                      )}
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => onCopy(entry.text)}
                        disabled={entry.text.trim().length === 0}
                      >
                        Копировать
                      </Button>
                    </div>
                  )}
                </li>
              );
            })}
          </ul>

          <div className="flex items-center justify-between gap-3">
            <span className="text-muted-foreground text-xs">
              {entries.length} {pluralizeEntries(entries.length)} (максимум 10)
            </span>
            <Button variant="destructive" size="sm" onClick={onClear} disabled={busy}>
              Очистить историю
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
