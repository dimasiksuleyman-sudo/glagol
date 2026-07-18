import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

/**
 * How long capture waits before it reveals the manual-entry fallback. Phase 0
 * finding: the Tauri webview delivers `Ctrl`/`Shift`/`Alt` + a normal key to
 * `keydown` reliably, but the OS swallows `Meta`/`Win` combos and reserved
 * chords (`Alt+Tab`, `Win+V`) before they reach the webview. So a user whose
 * combo never arrives is offered the text field after this timeout (D7).
 */
const CAPTURE_FALLBACK_MS = 4000;

/** Modifier `event.code` values — a keydown of one of these alone is ignored. */
const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
]);

/**
 * Map a `KeyboardEvent.code` to a global-shortcut accelerator key token, or
 * `null` when it is a bare modifier / an unusable key. `KeyD → "D"`,
 * `Digit1 → "1"`, `Space → "Space"`, `F5 → "F5"`; everything else passes the
 * code through (the backend `Shortcut::from_str` is the final arbiter and
 * rejects garbage with a Russian error).
 */
export function codeToAccelKey(code: string): string | null {
  if (MODIFIER_CODES.has(code)) return null;
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^Numpad[0-9]$/.test(code)) return code; // keep Numpad0..9 explicit
  if (code === "") return null;
  return code;
}

/**
 * Build an accelerator string from a keydown event, or `null` when the press is
 * not a usable shortcut (bare modifier, or a main key with no modifier — a
 * global hotkey without a modifier would fire on every keystroke). Modifiers are
 * emitted in a stable order so the same physical combo always yields the same
 * string.
 */
export function accelFromEvent(e: {
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
}): string | null {
  const key = codeToAccelKey(e.code);
  if (key === null) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  if (e.metaKey) parts.push("Super");
  // A global hotkey needs at least one modifier, else it would swallow the key
  // everywhere. Reject a bare key so capture keeps waiting for a real combo.
  if (parts.length === 0) return null;
  parts.push(key);
  return parts.join("+");
}

interface HotkeyEditorProps {
  /** The currently-saved accelerator (e.g. `"CmdOrCtrl+Shift+Space"`). */
  value: string;
  /**
   * Persist a new accelerator via `set_dictation_hotkey`. Rejects (throws) with
   * a Russian string on a malformed accelerator or a registration conflict; the
   * editor surfaces it via {@link HotkeyEditorProps.onError} and stays open so
   * the user can try another combo.
   */
  onSave: (hotkey: string) => Promise<void>;
  /** Report a save failure (shown by the parent as a toast). */
  onError: (message: string) => void;
  disabled?: boolean;
}

type Mode =
  | { kind: "idle" }
  | { kind: "capturing" }
  | { kind: "manual"; draft: string };

/**
 * Push-to-talk hotkey editor (D7). Two ways in:
 *
 * - **Capture** — «Изменить» arms a `keydown` listener; the user physically
 *   presses the combo and it is saved. This is the primary path.
 * - **Manual** — a text field for typing an accelerator by hand. Revealed on
 *   demand, and automatically after {@link CAPTURE_FALLBACK_MS} of capture with
 *   nothing caught, so a user whose combo the webview never receives is never
 *   stuck (Phase 0).
 */
export function HotkeyEditor({ value, onSave, onError, disabled }: HotkeyEditorProps) {
  const [mode, setMode] = useState<Mode>({ kind: "idle" });
  const [saving, setSaving] = useState(false);
  const [showFallbackHint, setShowFallbackHint] = useState(false);
  const fallbackTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Arm / disarm the capture keydown listener with the editor's mode.
  useEffect(() => {
    if (mode.kind !== "capturing") return;

    setShowFallbackHint(false);
    fallbackTimer.current = setTimeout(() => setShowFallbackHint(true), CAPTURE_FALLBACK_MS);

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setMode({ kind: "idle" });
        return;
      }
      const accel = accelFromEvent(e);
      if (accel === null) return; // bare modifier / no-modifier key: keep waiting
      void commit(accel);
    };

    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      if (fallbackTimer.current !== null) {
        clearTimeout(fallbackTimer.current);
        fallbackTimer.current = null;
      }
    };
    // `commit` is stable enough for this effect; re-arming only on mode change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode.kind]);

  async function commit(accel: string) {
    setSaving(true);
    try {
      await onSave(accel);
      setMode({ kind: "idle" });
    } catch (err) {
      onError(stringifyError(err));
      // Stay in the current mode so the user can immediately try another combo.
    } finally {
      setSaving(false);
    }
  }

  if (mode.kind === "capturing") {
    return (
      <div className="space-y-2">
        <div className="border-input bg-muted/40 flex items-center justify-between gap-3 rounded-md border border-dashed px-3 py-2">
          <span className="text-sm">
            {saving ? "Сохраняю…" : "Нажмите комбинацию клавиш…"}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setMode({ kind: "idle" })}
            disabled={saving}
          >
            Отмена
          </Button>
        </div>
        <button
          type="button"
          className="text-muted-foreground hover:text-foreground text-xs underline underline-offset-2"
          onClick={() => setMode({ kind: "manual", draft: value })}
        >
          {showFallbackHint
            ? "Комбинация не распозналась? Введите её вручную"
            : "…или ввести вручную"}
        </button>
      </div>
    );
  }

  if (mode.kind === "manual") {
    const draft = mode.draft;
    return (
      <div className="space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <Input
            autoFocus
            autoComplete="off"
            spellCheck={false}
            placeholder="CmdOrCtrl+Shift+Space"
            value={draft}
            onChange={(e) => setMode({ kind: "manual", draft: e.target.value })}
            onKeyDown={(e) => {
              if (e.key === "Enter" && draft.trim().length > 0) void commit(draft.trim());
            }}
            disabled={saving}
            className="max-w-xs font-mono text-sm"
          />
          <Button
            size="sm"
            onClick={() => void commit(draft.trim())}
            disabled={saving || draft.trim().length === 0}
          >
            Сохранить
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setMode({ kind: "idle" })}
            disabled={saving}
          >
            Отмена
          </Button>
        </div>
        <p className="text-muted-foreground text-xs">
          Формат: модификатор(ы) + клавиша, например{" "}
          <code>CmdOrCtrl+Shift+Space</code> или <code>Alt+Shift+D</code>.
        </p>
      </div>
    );
  }

  return (
    <div className="flex flex-wrap items-center gap-3">
      <kbd
        className={cn(
          "border-input bg-muted text-foreground inline-flex items-center rounded-md border px-2.5 py-1 font-mono text-sm",
        )}
      >
        {value}
      </kbd>
      <Button
        variant="secondary"
        size="sm"
        onClick={() => setMode({ kind: "capturing" })}
        disabled={disabled}
      >
        Изменить
      </Button>
    </div>
  );
}

function stringifyError(err: unknown): string {
  if (typeof err === "string") return err;
  if (err instanceof Error) return err.message;
  return String(err);
}
