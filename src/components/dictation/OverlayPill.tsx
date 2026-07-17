import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  DICTATION_LEVEL_EVENT,
  DICTATION_STATE_EVENT,
  type DictationDisposition,
  type DictationLevelPayload,
  type DictationState,
} from "@/lib/tauri";

/**
 * Map a terminal `done` disposition to the pill's success label (D13), or `null`
 * when the pill should show nothing and hide (a silent discard).
 *
 * The `switch` is **exhaustive**: the `never`-typed fall-through makes `tsc` fail
 * if the Rust `Disposition` enum grows a variant and {@link DictationDisposition}
 * is updated without a matching case here — the compile-time half of the Rust ↔
 * TS lock-step (there is no JS test runner in this project).
 */
export function dispositionPillText(
  disposition: DictationDisposition,
): string | null {
  switch (disposition) {
    case "pasted":
      return "Вставлено";
    case "clipboard":
      return "Скопировано";
    case "discarded":
      return null;
    default: {
      const _exhaustive: never = disposition;
      return _exhaustive;
    }
  }
}

/**
 * The dictation overlay pill (Sprint 6 PR3).
 *
 * Rendered only in the `overlay` window (see {@link ../../main.tsx}); the window
 * itself is created hidden at startup and shown/positioned by Rust the instant
 * the hotkey is pressed (D5). This component owns the pill's *content* — it
 * `listen()`s for `dictation-state` transitions and `dictation-level` RMS
 * values — and hides the window itself on a terminal state, with the timing the
 * UX spec calls for (D7): a 1.5 s dwell on «Скопировано», an immediate hide for
 * a silent discard, a longer dwell on an error so it can be read.
 *
 * The pill is an opaque dark rounded rectangle *inside* a transparent window
 * (D6): if Windows transparency does not take, it degrades to a dark rectangle
 * with no code branch.
 */

const HIDE_AFTER_DONE_MS = 1500;
const HIDE_AFTER_ERROR_MS = 3000;
const BAR_MULTIPLIERS = [0.45, 0.75, 1.0, 0.85, 1.0, 0.7, 0.4];

export function OverlayPill() {
  // Default to `recording` so a freshly shown pill never flashes stale terminal
  // content: Rust shows the window on Pressed, the `recording` event lands a few
  // ms later, and on hide we reset back to this.
  const [state, setState] = useState<DictationState>({ kind: "recording" });
  const [level, setLevel] = useState(0);
  const hideTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    const clearHideTimer = () => {
      if (hideTimer.current !== null) {
        clearTimeout(hideTimer.current);
        hideTimer.current = null;
      }
    };

    const scheduleHide = (next: DictationState) => {
      clearHideTimer();
      let delay = HIDE_AFTER_DONE_MS;
      if (next.kind === "error") {
        delay = HIDE_AFTER_ERROR_MS;
      } else if (next.kind === "done" && next.disposition === "discarded") {
        // A tap / silent clip: no success text, just disappear.
        delay = 0;
      }
      hideTimer.current = setTimeout(() => {
        void getCurrentWindow().hide();
        // Reset so the next show starts on the recording view, flash-free.
        setState({ kind: "recording" });
        setLevel(0);
      }, delay);
    };

    const stateUnlisten = listen<DictationState>(DICTATION_STATE_EVENT, (event) => {
      const next = event.payload;
      setState(next);
      if (next.kind === "recording") {
        clearHideTimer();
      } else if (next.kind === "done" || next.kind === "error") {
        scheduleHide(next);
      }
    });

    const levelUnlisten = listen<DictationLevelPayload>(DICTATION_LEVEL_EVENT, (event) => {
      setLevel(event.payload.level);
    });

    return () => {
      clearHideTimer();
      void stateUnlisten.then((fn) => fn());
      void levelUnlisten.then((fn) => fn());
    };
  }, []);

  return (
    <div style={rootStyle}>
      <style>{keyframes}</style>
      <div style={pillStyle} data-state={state.kind}>
        <PillContent state={state} level={level} />
      </div>
    </div>
  );
}

function PillContent({ state, level }: { state: DictationState; level: number }) {
  switch (state.kind) {
    case "recording":
      return (
        <>
          <span style={dotStyle} aria-hidden />
          <LevelBars level={level} />
        </>
      );
    case "processing":
      return (
        <>
          <span style={spinnerStyle} aria-hidden />
          <span style={labelStyle}>Распознаю…</span>
        </>
      );
    case "done": {
      const label = dispositionPillText(state.disposition);
      if (label === null) {
        // Silent discard: hidden almost immediately; render nothing meaningful.
        return <span style={labelStyle} />;
      }
      return (
        <>
          <span style={{ ...glyphStyle, color: "#34d399" }} aria-hidden>
            ✓
          </span>
          <span style={labelStyle}>
            {label}
            {state.truncated ? " · обрезано по 60 с" : ""}
          </span>
        </>
      );
    }
    case "error":
      return (
        <>
          <span style={{ ...glyphStyle, color: "#f87171" }} aria-hidden>
            ⚠
          </span>
          <span style={errorLabelStyle}>{state.message}</span>
        </>
      );
  }
}

function LevelBars({ level }: { level: number }) {
  return (
    <div style={barsStyle}>
      {BAR_MULTIPLIERS.map((multiplier, index) => {
        // Map linear RMS (roughly 0..0.3 for speech) to a 20–100% bar height.
        const scaled = Math.min(1, level * multiplier * 6);
        const heightPct = 20 + scaled * 80;
        return (
          <span
            key={index}
            style={{
              ...barStyle,
              height: `${heightPct}%`,
            }}
          />
        );
      })}
    </div>
  );
}

// ── styles (self-contained; the pill must not depend on Tailwind config) ──

const rootStyle: React.CSSProperties = {
  width: "100vw",
  height: "100vh",
  margin: 0,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "transparent",
  overflow: "hidden",
  userSelect: "none",
  cursor: "default",
};

const pillStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
  boxSizing: "border-box",
  maxWidth: "calc(100vw - 8px)",
  height: 36,
  padding: "0 14px",
  borderRadius: 18,
  background: "rgba(20, 22, 28, 0.94)",
  border: "1px solid rgba(255, 255, 255, 0.08)",
  boxShadow: "0 6px 20px rgba(0, 0, 0, 0.35)",
  color: "#f4f4f5",
  fontFamily:
    "'Geist Variable', system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
  fontSize: 13,
  lineHeight: 1.2,
};

const dotStyle: React.CSSProperties = {
  flex: "0 0 auto",
  width: 9,
  height: 9,
  borderRadius: "50%",
  background: "#ef4444",
  animation: "glagol-pulse 1.1s ease-in-out infinite",
};

const barsStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 3,
  height: 20,
  flex: "1 1 auto",
  justifyContent: "center",
};

const barStyle: React.CSSProperties = {
  display: "inline-block",
  width: 3,
  minHeight: 3,
  borderRadius: 2,
  background: "linear-gradient(180deg, #60a5fa, #3b82f6)",
  transition: "height 90ms ease-out",
};

const spinnerStyle: React.CSSProperties = {
  flex: "0 0 auto",
  width: 13,
  height: 13,
  borderRadius: "50%",
  border: "2px solid rgba(255, 255, 255, 0.25)",
  borderTopColor: "#60a5fa",
  animation: "glagol-spin 0.7s linear infinite",
};

const glyphStyle: React.CSSProperties = {
  flex: "0 0 auto",
  fontSize: 14,
  fontWeight: 700,
};

const labelStyle: React.CSSProperties = {
  flex: "1 1 auto",
  whiteSpace: "nowrap",
  overflow: "hidden",
  textOverflow: "ellipsis",
};

const errorLabelStyle: React.CSSProperties = {
  flex: "1 1 auto",
  fontSize: 11.5,
  lineHeight: 1.15,
  display: "-webkit-box",
  WebkitLineClamp: 2,
  WebkitBoxOrient: "vertical",
  overflow: "hidden",
};

const keyframes = `
@keyframes glagol-pulse {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.45; transform: scale(0.8); }
}
@keyframes glagol-spin {
  to { transform: rotate(360deg); }
}
`;
