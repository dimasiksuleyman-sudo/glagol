//! Auto-insertion of the recognised text into the **active window** (Sprint 6
//! PR4) — the second half of the user-facing dictation feature.
//!
//! PR3 delivered the transcript to the OS clipboard and stopped there; the user
//! still pressed `Ctrl+V` by hand. PR4 synthesises that paste. The whole path is
//! built as a **pure planner + a seam-driven executor** so it is exercised with
//! no keyboard, no clipboard, and no target application:
//!
//! - [`plan_insertion`] is a pure function: `(snapshot, transcript, mode)` in, a
//!   `Vec<InsertStep>` trace out. Unit tests assert on the exact step order.
//! - [`execute_insertion`] walks a plan against the two seams and returns an
//!   [`InsertOutcome`]. A `FakeInserter` / `FakeClipboard` record the calls and
//!   can fail at any chosen step (D9), so both the **trace** and the **outcome**
//!   are asserted.
//!
//! # Why two seams (D8)
//!
//! Keystroke synthesis ([`TextInserter`], real impl [`EnigoInserter`]) and
//! clipboard get/set ([`ClipboardAccess`], real impl [`ArboardClipboard`]) are
//! independent capabilities with independent failure modes, so they are separate
//! traits. Both are `Clone + Send + 'static` and hold **no** live OS handle: the
//! real impls open a fresh `Enigo` / `arboard::Clipboard` per call. That is what
//! lets the pipeline clone them into `spawn_blocking` (both libraries are
//! blocking) without ever storing an `Enigo` in `AppState` (D8), and it means a
//! panic mid-paste still drops the `Enigo`, releasing any key it left held
//! (`release_keys_when_dropped` defaults `true` — verified in the enigo 0.6.1
//! source during Phase 0).
//!
//! # The honest semantics of "pasted" (D11)
//!
//! [`InsertOutcome::Pasted`] means **the paste events were handed to the OS**,
//! not that the text landed in the target application. The latter cannot be
//! confirmed: per Microsoft's `SendInput` documentation, a synthetic input
//! blocked by UIPI (an elevated target window) is dropped **silently** — neither
//! the return value nor `GetLastError` reports it. enigo *does* surface the
//! detectable short-write (`Err(InputError::Simulate("...blocked by UIPI"))`),
//! which buys loudness on the exotic cases (a low-level hook eating the input, a
//! full input queue), but the elevated-window case stays invisible by design of
//! the OS. The UX mitigation is that the transcript remains available in the
//! clipboard / history (PR5) for a manual retry.

use std::time::{Duration, Instant};

/// Windows virtual-key code for the `V` key. Sent as a **raw VK** via
/// [`enigo::Key::Other`] so the paste is layout-independent (D4).
const VK_V: u32 = 0x56;

/// How long to wait after issuing the paste before reading the clipboard back to
/// decide whether to restore (D7). The target app processes `WM_PASTE`
/// **asynchronously** off its own message queue; Windows never signals when it
/// has read the clipboard, so this is a best-effort settle, not a guarantee. The
/// actual per-insertion timing is logged (the D7 rider) so a week of real use
/// replaces this guess with data.
pub const SETTLE_DELAY: Duration = Duration::from_millis(300);

/// Retry budget for the initial clipboard write when the clipboard is locked by
/// another process (a clipboard manager, an RDP session) — `OpenClipboard` is a
/// global Windows mutex (D10). Three attempts, then a loud failure: if we cannot
/// even write the clipboard we have nothing at all, so the user must be told.
const CLIPBOARD_SET_MAX_ATTEMPTS: usize = 3;
/// Delay between clipboard-write retries (D10).
const CLIPBOARD_RETRY_DELAY: Duration = Duration::from_millis(50);

// ── Insertion mode (D12) ────────────────────────────────────────────────

/// Where the recognised text goes. Persisted (as a string) under the
/// `stt_insertion_mode` key in `app_settings`; read in PR4, chosen by a radio
/// button in PR5. Deliberately **two-valued**: a `Type` (per-character) mode is
/// forbidden by the marathon plan and would add a *matched-but-unexecuted* branch
/// on an **input** value, so it is not added until the writer (PR5 UI) exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertionMode {
    /// Write to the clipboard, then synthesise `Ctrl+V` into the active window.
    Paste,
    /// Write to the clipboard only (PR3 behaviour) — the user pastes by hand.
    ClipboardOnly,
}

/// Persisted string for [`InsertionMode::Paste`] — also the default when the key
/// is absent (D12).
pub const INSERTION_MODE_PASTE: &str = "paste";
/// Persisted string for [`InsertionMode::ClipboardOnly`] (D12).
pub const INSERTION_MODE_CLIPBOARD_ONLY: &str = "clipboard_only";

/// Parse the persisted `stt_insertion_mode` value (D12).
///
/// An unknown value is a **loud** fallback: `error!` to the log and drop to
/// [`InsertionMode::ClipboardOnly`], never `Paste`. Silently synthesising
/// keystrokes for a value we do not understand is the worse failure — pasting
/// into the wrong window guesses at the user's intent; clipboard-only never does.
pub fn parse_insertion_mode(raw: &str) -> InsertionMode {
    match raw {
        INSERTION_MODE_PASTE => InsertionMode::Paste,
        INSERTION_MODE_CLIPBOARD_ONLY => InsertionMode::ClipboardOnly,
        other => {
            tracing::error!(
                value = other,
                "unknown stt_insertion_mode; falling back to clipboard-only"
            );
            InsertionMode::ClipboardOnly
        }
    }
}

// ── Plan (D6) ───────────────────────────────────────────────────────────

/// One step in an insertion plan (D6). A plan is a `Vec<InsertStep>` produced by
/// [`plan_insertion`] and consumed by [`execute_insertion`]; steps carry the data
/// they need so the trace is self-describing and directly assertable in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertStep {
    /// Write the transcript to the clipboard (retried on a busy clipboard, D10).
    SetText(String),
    /// Release Shift / Alt / Meta before pasting (D5). A modifier the user is
    /// still physically holding would turn our `Ctrl+V` into `Ctrl+Shift+V`
    /// ("paste without formatting") or worse; a synthetic key-up is more
    /// deterministic than polling for the physical release. Documented practice
    /// per MSDN `SendInput` Remarks.
    ReleaseModifiers,
    /// Synthesise `Ctrl+V` via a raw `VK_V` (D4).
    Paste,
    /// Wait for the target app to read the clipboard (D7). Best-effort — see
    /// [`SETTLE_DELAY`].
    Settle(Duration),
    /// Read the clipboard back and confirm it still holds our transcript before
    /// restoring (D7, race #2). The payload is the transcript we expect to find.
    VerifyOwn(String),
    /// Restore the prior clipboard text (the payload). Only reached when the
    /// snapshot was text (D7, condition 1); executed only when [`VerifyOwn`]
    /// confirmed ownership (condition 2) and the paste did not fail (condition 3).
    ///
    /// [`VerifyOwn`]: InsertStep::VerifyOwn
    Restore(String),
}

/// Build the insertion plan from the pre-read clipboard `snapshot`, the
/// `transcript`, and the `mode` (D6). Pure — no IO, no logging.
///
/// - [`InsertionMode::ClipboardOnly`] → just `[SetText]`; the user pastes by hand.
/// - [`InsertionMode::Paste`] → `SetText → ReleaseModifiers → Paste → Settle`,
///   plus `VerifyOwn → Restore` **iff** the snapshot was `Ok` (text). A non-text
///   snapshot (image, files → `Err`) leaves nothing to restore, so those two
///   steps are omitted entirely (D7, condition 1).
pub fn plan_insertion(
    snapshot: Result<String, String>,
    transcript: &str,
    mode: InsertionMode,
) -> Vec<InsertStep> {
    match mode {
        InsertionMode::ClipboardOnly => vec![InsertStep::SetText(transcript.to_string())],
        InsertionMode::Paste => {
            let mut steps = vec![
                InsertStep::SetText(transcript.to_string()),
                InsertStep::ReleaseModifiers,
                InsertStep::Paste,
                InsertStep::Settle(SETTLE_DELAY),
            ];
            // Restore is only possible when the prior clipboard was text.
            if let Ok(prior) = snapshot {
                steps.push(InsertStep::VerifyOwn(transcript.to_string()));
                steps.push(InsertStep::Restore(prior));
            }
            steps
        }
    }
}

// ── Outcome ─────────────────────────────────────────────────────────────

/// What [`execute_insertion`] achieved. Mapped to a `Disposition` at the pipeline
/// boundary: `Pasted → pasted`, `ClipboardOnly → clipboard`, `Failed → error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// The paste events were sent to the OS (D11 — *sent*, not *landed*).
    Pasted,
    /// The transcript is in the clipboard but no paste happened — either the mode
    /// was [`InsertionMode::ClipboardOnly`], or the paste keystroke failed and we
    /// deliberately left the text on the clipboard for a manual `Ctrl+V`.
    ClipboardOnly,
    /// The clipboard write itself failed after retries (D10) — nothing was
    /// delivered anywhere; the user must be told loudly.
    Failed,
}

// ── Seams (D8) ──────────────────────────────────────────────────────────

/// Keystroke synthesis into the active window (D8). Real impl:
/// [`EnigoInserter`]. `Clone + Send + 'static` so the pipeline clones it into
/// `spawn_blocking`; the real impl holds no handle, opening a fresh `Enigo` per
/// call.
pub trait TextInserter: Clone + Send + 'static {
    /// Release Shift / Alt / Meta so a still-held modifier cannot corrupt the
    /// paste (D5). Best-effort: the executor logs a failure and continues.
    fn release_modifiers(&self) -> Result<(), String>;
    /// Synthesise `Ctrl+V` via a raw `VK_V` (D4).
    fn paste(&self) -> Result<(), String>;
}

/// Clipboard get/set (D8). Real impl: [`ArboardClipboard`] (reuses the `arboard`
/// dependency from PR3). `Clone + Send + 'static`; the real impl opens a fresh
/// `arboard::Clipboard` per call.
pub trait ClipboardAccess: Clone + Send + 'static {
    /// Read the clipboard as text. `Err` when the clipboard holds a non-text
    /// payload (image, files) or is inaccessible — the snapshot then cannot be
    /// restored (D7).
    fn get_text(&self) -> Result<String, String>;
    /// Replace the clipboard contents with `text`.
    fn set_text(&self, text: &str) -> Result<(), String>;
}

// ── No-restore reasons (D7) ─────────────────────────────────────────────

/// Why a restore did not happen. Each maps to a **distinct** trace line (D7), so
/// when a week of logs is read the three cases are three different signals, not
/// one. Single source of truth for the strings — the executor emits
/// [`NoRestoreReason::message`], never a bare literal, so a message cannot drift
/// away from the `no_restore_reason_messages_are_exact` test that pins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoRestoreReason {
    /// The clipboard snapshot was non-text (image/files) — nothing to restore.
    NonTextSnapshot,
    /// The clipboard was overwritten between our `SetText` and the verify read.
    BufferChanged,
    /// The paste keystroke failed — the transcript is left for a manual `Ctrl+V`.
    PasteFailed,
}

impl NoRestoreReason {
    /// The exact, distinct Russian trace line for this reason (D7).
    const fn message(self) -> &'static str {
        match self {
            NoRestoreReason::NonTextSnapshot => "рестор пропущен: снапшот не-текст",
            NoRestoreReason::BufferChanged => "рестор отменён: буфер изменился",
            NoRestoreReason::PasteFailed => "рестор не выполнялся: вставка провалилась",
        }
    }
}

// ── Executor (D6) ───────────────────────────────────────────────────────

/// Take the clipboard snapshot, plan, and run one insertion end-to-end (D6/D7).
///
/// This is the single function the pipeline calls inside `spawn_blocking`. It
/// owns the three "no restore" log reasons (D7, [`NoRestoreReason`]) so each
/// shows a *distinct* message when the logs are read after a week of use.
pub fn insert_transcript<I: TextInserter, C: ClipboardAccess>(
    inserter: &I,
    clipboard: &C,
    transcript: &str,
    mode: InsertionMode,
) -> InsertOutcome {
    // Snapshot only matters for Paste mode; ClipboardOnly never restores.
    let snapshot = match mode {
        InsertionMode::ClipboardOnly => Err("clipboard-only mode: no snapshot".to_string()),
        InsertionMode::Paste => clipboard.get_text(),
    };
    if mode == InsertionMode::Paste && snapshot.is_err() {
        // D7 reason 1 + D9 scenario 4: continue the paste, restore nothing.
        tracing::warn!("{}", NoRestoreReason::NonTextSnapshot.message());
    }

    let plan = plan_insertion(snapshot, transcript, mode);
    execute_insertion(&plan, inserter, clipboard)
}

/// Walk a plan against the seams and return the outcome (D6). Separated from
/// [`insert_transcript`] so the pure planner and the executor are tested
/// independently: a test can hand-build any plan (including zero-delay `Settle`s)
/// and assert on both the recorded seam calls and the returned [`InsertOutcome`].
pub fn execute_insertion<I: TextInserter, C: ClipboardAccess>(
    plan: &[InsertStep],
    inserter: &I,
    clipboard: &C,
) -> InsertOutcome {
    let start = Instant::now();
    let mut pasted = false;
    // Ownership of the clipboard is unknown until VerifyOwn runs; a Restore is
    // only ever emitted after a VerifyOwn, so this default is never observed by a
    // Restore step. It stays `false` so a missing verify can never restore blind.
    let mut owns_clipboard = false;

    for step in plan {
        match step {
            InsertStep::SetText(text) => {
                if let Err(e) = write_clipboard_with_retry(clipboard, text) {
                    tracing::error!(
                        error = %e,
                        attempts = CLIPBOARD_SET_MAX_ATTEMPTS,
                        "clipboard write failed after retries (clipboard busy)"
                    );
                    return InsertOutcome::Failed;
                }
            }
            InsertStep::ReleaseModifiers => {
                if let Err(e) = inserter.release_modifiers() {
                    // Best-effort (D5): a failed release is not worth aborting a
                    // paste over — the worst case is the modifier stays held,
                    // which the next real keypress resolves.
                    tracing::warn!(error = %e, "failed to release modifiers before paste");
                }
            }
            InsertStep::Paste => {
                if let Err(e) = inserter.paste() {
                    // D7 condition 3 + D9 scenario 3: the transcript is already on
                    // the clipboard, so leave it there for a manual Ctrl+V and do
                    // not restore. Every step after Paste is paste-dependent, so
                    // returning here is equivalent to skipping them.
                    tracing::warn!(error = %e, "{}", NoRestoreReason::PasteFailed.message());
                    log_timing(start, InsertOutcome::ClipboardOnly);
                    return InsertOutcome::ClipboardOnly;
                }
                pasted = true;
            }
            InsertStep::Settle(delay) => std::thread::sleep(*delay),
            InsertStep::VerifyOwn(expected) => {
                owns_clipboard =
                    matches!(clipboard.get_text(), Ok(current) if &current == expected);
            }
            InsertStep::Restore(prior) => {
                if owns_clipboard {
                    if let Err(e) = clipboard.set_text(prior) {
                        // Restore is best-effort: the paste already succeeded, so a
                        // failed restore must NOT downgrade the outcome (D9
                        // scenario 5). Worst case: the transcript lingers on the
                        // clipboard instead of the prior text.
                        tracing::warn!(error = %e, "clipboard restore failed; paste already succeeded");
                    }
                } else {
                    // D7 reason 2 / race #2: someone wrote the clipboard between
                    // our SetText and now — do not clobber their content. `warn`
                    // like the other two no-restore reasons, so all three land in
                    // the release log under the base `warn` directive (D14), not
                    // only via the dictation=debug clause.
                    tracing::warn!("{}", NoRestoreReason::BufferChanged.message());
                }
            }
        }
    }

    let outcome = if pasted {
        InsertOutcome::Pasted
    } else {
        InsertOutcome::ClipboardOnly
    };
    log_timing(start, outcome);
    outcome
}

/// Write `text` to the clipboard, retrying a busy clipboard up to
/// [`CLIPBOARD_SET_MAX_ATTEMPTS`] times (D10).
fn write_clipboard_with_retry<C: ClipboardAccess>(clipboard: &C, text: &str) -> Result<(), String> {
    let mut last_err = String::new();
    for attempt in 1..=CLIPBOARD_SET_MAX_ATTEMPTS {
        match clipboard.set_text(text) {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                if attempt < CLIPBOARD_SET_MAX_ATTEMPTS {
                    std::thread::sleep(CLIPBOARD_RETRY_DELAY);
                }
            }
        }
    }
    Err(last_err)
}

/// The D7 timing rider: record how long an insertion actually took, so the 300 ms
/// `Settle` guess can be replaced with data from real use.
fn log_timing(start: Instant, outcome: InsertOutcome) {
    tracing::debug!(
        elapsed_ms = start.elapsed().as_millis() as u64,
        ?outcome,
        "dictation insertion executed"
    );
}

// ── Real implementations ────────────────────────────────────────────────

/// The production [`TextInserter`] — synthesises keystrokes via `enigo` (D8).
///
/// A zero-field unit struct: it holds no `Enigo` (that must never live in
/// `AppState`, D8), constructing a fresh one per call. The constructor is cheap
/// and stateless, and dropping the `Enigo` at the end of each method releases any
/// key still held (`release_keys_when_dropped` defaults `true`).
#[derive(Clone, Copy, Default)]
pub struct EnigoInserter;

impl TextInserter for EnigoInserter {
    fn release_modifiers(&self) -> Result<(), String> {
        use enigo::{Direction::Release, Key, Keyboard};
        let mut enigo =
            enigo::Enigo::new(&enigo::Settings::default()).map_err(|e| e.to_string())?;
        // A failed individual release is not fatal to the sequence; report the
        // first error but attempt all three so no modifier is left held.
        let mut first_err: Option<String> = None;
        for key in [Key::Shift, Key::Alt, Key::Meta] {
            if let Err(e) = enigo.key(key, Release) {
                first_err.get_or_insert_with(|| e.to_string());
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn paste(&self) -> Result<(), String> {
        use enigo::{
            Direction::{Click, Press, Release},
            Key, Keyboard,
        };
        let mut enigo =
            enigo::Enigo::new(&enigo::Settings::default()).map_err(|e| e.to_string())?;
        enigo.key(Key::Control, Press).map_err(|e| e.to_string())?;
        // D4: raw VK for V, NEVER `Key::Unicode('v')`. Under a Russian keyboard
        // layout `'v'` has no VK mapping, so enigo would fall back to
        // `KEYEVENTF_UNICODE` (verified in the 0.6.1 source) and the Ctrl+V would
        // not register as a paste. `Key::Other(0x56)` sends a raw `wVk`,
        // layout-independent. Do not "simplify" this to `Key::Unicode`.
        let paste_result = enigo
            .key(Key::Other(VK_V), Click)
            .map_err(|e| e.to_string());
        // Always attempt to release Control, even if the V click failed, so we do
        // not leave Ctrl stuck down (the Drop safety net covers a panic; this
        // covers the ordinary error path).
        let release_result = enigo.key(Key::Control, Release).map_err(|e| e.to_string());
        paste_result.and(release_result)
    }
}

/// The production [`ClipboardAccess`] — get/set via `arboard` (D8), reusing the
/// PR3 dependency. A zero-field unit struct: `arboard::Clipboard` is not `Clone`
/// and the documented pattern is one handle per operation, so each call opens a
/// fresh handle. `arboard` is built with `default-features = false`, so only the
/// text path is compiled in (never the `image` crate).
#[derive(Clone, Copy, Default)]
pub struct ArboardClipboard;

impl ClipboardAccess for ArboardClipboard {
    fn get_text(&self) -> Result<String, String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard.get_text().map_err(|e| e.to_string())
    }

    fn set_text(&self, text: &str) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
        clipboard.set_text(text).map_err(|e| e.to_string())
    }
}

// ── Test doubles (shared with the pipeline tests) ───────────────────────

#[cfg(test)]
pub(crate) mod fakes {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A [`TextInserter`] that records every call and can fail at a chosen step
    /// (D9). `Clone` shares one `Arc<Mutex<..>>` so a clone handed to
    /// `spawn_blocking` and the test's handle observe the same trace.
    #[derive(Clone, Default)]
    pub(crate) struct FakeInserter {
        inner: Arc<Mutex<FakeInserterState>>,
    }

    #[derive(Default)]
    struct FakeInserterState {
        calls: Vec<&'static str>,
        fail_paste: bool,
        fail_release: bool,
    }

    impl FakeInserter {
        pub(crate) fn failing_paste() -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeInserterState {
                    fail_paste: true,
                    ..Default::default()
                })),
            }
        }
        pub(crate) fn calls(&self) -> Vec<&'static str> {
            self.inner.lock().unwrap().calls.clone()
        }
    }

    impl TextInserter for FakeInserter {
        fn release_modifiers(&self) -> Result<(), String> {
            let mut s = self.inner.lock().unwrap();
            s.calls.push("release_modifiers");
            if s.fail_release {
                Err("mock release failure".into())
            } else {
                Ok(())
            }
        }
        fn paste(&self) -> Result<(), String> {
            let mut s = self.inner.lock().unwrap();
            s.calls.push("paste");
            if s.fail_paste {
                Err("mock paste failure".into())
            } else {
                Ok(())
            }
        }
    }

    /// A [`ClipboardAccess`] simulating the OS clipboard with configurable
    /// failure injection (D9/D10). All state is behind one shared `Arc<Mutex>`.
    #[derive(Clone)]
    pub(crate) struct FakeClipboard {
        inner: Arc<Mutex<FakeClipboardState>>,
    }

    #[derive(Default)]
    struct FakeClipboardState {
        /// Simulated clipboard text; `None` models a non-text payload.
        content: Option<String>,
        /// Force `get_text` to error (non-text snapshot — image/files).
        get_err: bool,
        /// Fail the first N `set_text` calls (busy clipboard, D10).
        set_fail_first_n: usize,
        /// Total `set_text` attempts seen.
        set_attempts: usize,
        /// External write injected right before the VerifyOwn read: on the get
        /// call at this 1-based index, overwrite `content` first (race #2, D7).
        external_write_on_get: Option<(usize, String)>,
        /// 1-based count of `get_text` calls.
        get_calls: usize,
        /// Ordered trace of successfully written values.
        writes: Vec<String>,
    }

    impl FakeClipboard {
        /// Empty clipboard, no failures.
        pub(crate) fn empty() -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeClipboardState::default())),
            }
        }
        /// Clipboard pre-loaded with text `prior` (the thing a restore must put
        /// back).
        pub(crate) fn with_text(prior: &str) -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeClipboardState {
                    content: Some(prior.to_string()),
                    ..Default::default()
                })),
            }
        }
        /// A clipboard whose snapshot read fails (image/files — non-text).
        pub(crate) fn non_text_snapshot() -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeClipboardState {
                    get_err: true,
                    ..Default::default()
                })),
            }
        }
        /// Fail the first `n` `set_text` calls, then succeed (busy clipboard).
        pub(crate) fn set_fails_first(n: usize) -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeClipboardState {
                    set_fail_first_n: n,
                    ..Default::default()
                })),
            }
        }
        /// Snapshot returns text `prior`, but the first `n` `set_text` calls fail
        /// — models "the write failed even though a snapshot was taken".
        pub(crate) fn with_text_set_fails(prior: &str, n: usize) -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeClipboardState {
                    content: Some(prior.to_string()),
                    set_fail_first_n: n,
                    ..Default::default()
                })),
            }
        }
        /// Text `prior` present, but a third party overwrites the clipboard with
        /// `intruder` right before the VerifyOwn read (get call #2: #1 is the
        /// snapshot, #2 is the verify).
        pub(crate) fn overwritten_before_verify(prior: &str, intruder: &str) -> Self {
            Self {
                inner: Arc::new(Mutex::new(FakeClipboardState {
                    content: Some(prior.to_string()),
                    external_write_on_get: Some((2, intruder.to_string())),
                    ..Default::default()
                })),
            }
        }

        /// Current simulated clipboard content.
        pub(crate) fn content(&self) -> Option<String> {
            self.inner.lock().unwrap().content.clone()
        }
        /// Ordered trace of successful writes.
        pub(crate) fn writes(&self) -> Vec<String> {
            self.inner.lock().unwrap().writes.clone()
        }
        /// Total `set_text` attempts (including failed retries).
        pub(crate) fn set_attempts(&self) -> usize {
            self.inner.lock().unwrap().set_attempts
        }
    }

    impl ClipboardAccess for FakeClipboard {
        fn get_text(&self) -> Result<String, String> {
            let mut s = self.inner.lock().unwrap();
            s.get_calls += 1;
            let this_get = s.get_calls;
            if let Some((at, intruder)) = s.external_write_on_get.clone() {
                if this_get == at {
                    s.content = Some(intruder);
                }
            }
            if s.get_err {
                return Err("clipboard holds non-text data".into());
            }
            s.content
                .clone()
                .ok_or_else(|| "clipboard is empty / non-text".into())
        }
        fn set_text(&self, text: &str) -> Result<(), String> {
            let mut s = self.inner.lock().unwrap();
            s.set_attempts += 1;
            if s.set_attempts <= s.set_fail_first_n {
                return Err("clipboard busy".into());
            }
            s.content = Some(text.to_string());
            s.writes.push(text.to_string());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::{FakeClipboard, FakeInserter};
    use super::*;

    const TRANSCRIPT: &str = "привет мир";
    const PRIOR: &str = "старый буфер";

    // ── parse_insertion_mode (D12) ──

    #[test]
    fn parse_mode_known_values() {
        assert_eq!(parse_insertion_mode("paste"), InsertionMode::Paste);
        assert_eq!(
            parse_insertion_mode("clipboard_only"),
            InsertionMode::ClipboardOnly
        );
    }

    #[test]
    fn parse_mode_unknown_falls_back_to_clipboard_only() {
        // D12: an unknown value must NOT silently become Paste.
        assert_eq!(parse_insertion_mode("type"), InsertionMode::ClipboardOnly);
        assert_eq!(parse_insertion_mode(""), InsertionMode::ClipboardOnly);
    }

    // ── plan_insertion trace (D6) ──

    #[test]
    fn plan_paste_with_text_snapshot_has_full_ordered_trace() {
        let plan = plan_insertion(Ok(PRIOR.to_string()), TRANSCRIPT, InsertionMode::Paste);
        assert_eq!(
            plan,
            vec![
                InsertStep::SetText(TRANSCRIPT.to_string()),
                InsertStep::ReleaseModifiers,
                InsertStep::Paste,
                InsertStep::Settle(SETTLE_DELAY),
                InsertStep::VerifyOwn(TRANSCRIPT.to_string()),
                InsertStep::Restore(PRIOR.to_string()),
            ]
        );
    }

    #[test]
    fn plan_paste_orders_settle_before_verify_before_restore() {
        let plan = plan_insertion(Ok(PRIOR.to_string()), TRANSCRIPT, InsertionMode::Paste);
        let pos = |pred: fn(&InsertStep) -> bool| plan.iter().position(pred).unwrap();
        let settle = pos(|s| matches!(s, InsertStep::Settle(_)));
        let verify = pos(|s| matches!(s, InsertStep::VerifyOwn(_)));
        let restore = pos(|s| matches!(s, InsertStep::Restore(_)));
        assert!(settle < verify, "Settle must precede VerifyOwn");
        assert!(verify < restore, "VerifyOwn must precede Restore");
    }

    #[test]
    fn plan_paste_with_nontext_snapshot_omits_verify_and_restore() {
        // D7 condition 1: a non-text snapshot (Err) leaves nothing to restore.
        let plan = plan_insertion(Err("non-text".into()), TRANSCRIPT, InsertionMode::Paste);
        assert_eq!(
            plan,
            vec![
                InsertStep::SetText(TRANSCRIPT.to_string()),
                InsertStep::ReleaseModifiers,
                InsertStep::Paste,
                InsertStep::Settle(SETTLE_DELAY),
            ]
        );
        assert!(!plan.iter().any(|s| matches!(s, InsertStep::Restore(_))));
    }

    #[test]
    fn plan_clipboard_only_is_just_set_text() {
        let plan = plan_insertion(
            Ok(PRIOR.to_string()),
            TRANSCRIPT,
            InsertionMode::ClipboardOnly,
        );
        assert_eq!(plan, vec![InsertStep::SetText(TRANSCRIPT.to_string())]);
    }

    // ── happy path: restore fires when all three conditions hold (D7) ──

    #[test]
    fn paste_with_text_snapshot_pastes_and_restores() {
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::with_text(PRIOR);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Pasted);
        assert_eq!(inserter.calls(), vec!["release_modifiers", "paste"]);
        // Wrote the transcript, then restored the prior text.
        assert_eq!(clipboard.writes(), vec![TRANSCRIPT, PRIOR]);
        assert_eq!(clipboard.content(), Some(PRIOR.to_string()));
    }

    // ── D9 hostile fakes: assert on trace AND outcome ──

    #[test]
    fn d9_set_text_fails_immediately_is_failed() {
        // set_text fails on every attempt → Failed, nothing pasted.
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::set_fails_first(CLIPBOARD_SET_MAX_ATTEMPTS);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Failed);
        assert!(
            inserter.calls().is_empty(),
            "no keystrokes if we never wrote"
        );
        assert!(clipboard.writes().is_empty());
    }

    #[test]
    fn d9_set_text_fails_after_snapshot_is_failed_no_restore() {
        // D9 scenario 2: the snapshot succeeds (prior text present), but the write
        // fails → Failed. Because we never wrote the transcript, there is nothing
        // to restore and the prior text is left untouched on the clipboard.
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::with_text_set_fails(PRIOR, CLIPBOARD_SET_MAX_ATTEMPTS);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Failed);
        assert!(
            inserter.calls().is_empty(),
            "no keystrokes if we never wrote"
        );
        assert!(
            clipboard.writes().is_empty(),
            "restore impossible: never wrote"
        );
        assert_eq!(
            clipboard.content(),
            Some(PRIOR.to_string()),
            "prior clipboard text is left intact"
        );
    }

    #[test]
    fn d9_paste_fails_leaves_transcript_and_skips_restore() {
        // D9 scenario 3: paste keystroke fails → ClipboardOnly, transcript stays,
        // restore NOT performed.
        let inserter = FakeInserter::failing_paste();
        let clipboard = FakeClipboard::with_text(PRIOR);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::ClipboardOnly);
        assert_eq!(inserter.calls(), vec!["release_modifiers", "paste"]);
        // Only the transcript was written; the prior text was never restored.
        assert_eq!(clipboard.writes(), vec![TRANSCRIPT]);
        assert_eq!(clipboard.content(), Some(TRANSCRIPT.to_string()));
    }

    #[test]
    fn d9_nontext_snapshot_continues_paste_skips_restore() {
        // D9 scenario 4: snapshot fails (image) → paste proceeds, restore skipped.
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::non_text_snapshot();

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Pasted);
        assert_eq!(inserter.calls(), vec!["release_modifiers", "paste"]);
        // Wrote only the transcript; nothing to restore.
        assert_eq!(clipboard.writes(), vec![TRANSCRIPT]);
    }

    #[test]
    fn d9_restore_failure_does_not_mask_paste_success() {
        // D9 scenario 5: the restore write fails → outcome stays Pasted.
        // set_fails_first(2): attempt #1 = SetText transcript FAILS, retry #2
        // FAILS... that would be Failed, not what we want. Instead we need the
        // FIRST write (transcript) to succeed and the SECOND (restore) to fail.
        // Model it by hand-building the plan and a clipboard that fails its 2nd
        // successful-slot write.
        let inserter = FakeInserter::default();
        let clipboard = RestoreFailingClipboard::new(PRIOR);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(
            outcome,
            InsertOutcome::Pasted,
            "restore failure must not downgrade"
        );
    }

    #[test]
    fn d9_buffer_changed_before_restore_cancels_restore() {
        // D7 race #2 / D9 scenario 6: someone overwrites the clipboard between our
        // SetText and the verify read → restore is cancelled, the intruder's
        // content is kept. Driven through the full `insert_transcript` path so the
        // snapshot is get #1 and the verify is get #2 (where the intruder writes).
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::overwritten_before_verify(PRIOR, "чужой текст");

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Pasted);
        // The prior text was NEVER written back — only the transcript write shows.
        assert_eq!(clipboard.writes(), vec![TRANSCRIPT.to_string()]);
        assert_eq!(clipboard.content(), Some("чужой текст".to_string()));
    }

    // ── D10 clipboard-busy retry ──

    #[test]
    fn d10_clipboard_busy_three_retries_then_failed() {
        // Exactly 3 failed attempts → Failed (retries exhausted).
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::set_fails_first(CLIPBOARD_SET_MAX_ATTEMPTS);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Failed);
        assert_eq!(
            clipboard.set_attempts(),
            CLIPBOARD_SET_MAX_ATTEMPTS,
            "must try exactly the retry budget"
        );
    }

    #[test]
    fn d10_clipboard_busy_recovers_within_budget() {
        // Fails twice, succeeds on the third → proceeds to a normal paste.
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::set_fails_first(CLIPBOARD_SET_MAX_ATTEMPTS - 1);

        let outcome = insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);

        assert_eq!(outcome, InsertOutcome::Pasted);
    }

    // ── ClipboardOnly mode ──

    #[test]
    fn clipboard_only_writes_text_without_pasting() {
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();

        let outcome = insert_transcript(
            &inserter,
            &clipboard,
            TRANSCRIPT,
            InsertionMode::ClipboardOnly,
        );

        assert_eq!(outcome, InsertOutcome::ClipboardOnly);
        assert!(
            inserter.calls().is_empty(),
            "no keystrokes in clipboard-only mode"
        );
        assert_eq!(clipboard.writes(), vec![TRANSCRIPT.to_string()]);
    }

    // ── NEGATIVE REGRESSION CYCLE 1: VerifyOwn is load-bearing ──
    //
    // `d9_buffer_changed_before_restore_cancels_restore` is the guard test. To
    // prove VerifyOwn is causal, comment out the `owns_clipboard = matches!(...)`
    // assignment in `execute_insertion` (leaving `owns_clipboard = false`) — then
    // Restore never runs and the intruder's content is kept, but so is the
    // no-restore for a legitimate own... Actually the load-bearing direction is:
    // replace the VerifyOwn arm with `owns_clipboard = true;` (restore blindly).
    // Then this test fails: the prior text WOULD be written back over the
    // intruder, so `writes()` becomes `[TRANSCRIPT, PRIOR]` and `content()`
    // becomes PRIOR. Restore the real check → green again. Verified manually.

    // ── NEGATIVE REGRESSION CYCLE 2: ReleaseModifiers is present in the trace ──
    //
    // `paste_with_text_snapshot_pastes_and_restores` asserts
    // `inserter.calls() == ["release_modifiers", "paste"]`. Remove the
    // `InsertStep::ReleaseModifiers` push from `plan_insertion` (or its arm from
    // `execute_insertion`) → the recorded calls become `["paste"]` and this test
    // fails. Restore the step → green. Verified manually. `plan_release_modifiers_
    // present` below pins the same invariant on the pure planner.

    #[test]
    fn plan_release_modifiers_present() {
        // Guards negative cycle 2 at the planner level: the defensive modifier
        // release must be in every Paste plan (D5).
        let plan = plan_insertion(Ok(PRIOR.to_string()), TRANSCRIPT, InsertionMode::Paste);
        assert!(
            plan.iter()
                .any(|s| matches!(s, InsertStep::ReleaseModifiers)),
            "ReleaseModifiers must be planned before Paste (D5)"
        );
        let rm = plan
            .iter()
            .position(|s| matches!(s, InsertStep::ReleaseModifiers))
            .unwrap();
        let paste = plan
            .iter()
            .position(|s| matches!(s, InsertStep::Paste))
            .unwrap();
        assert!(rm < paste, "ReleaseModifiers must come before Paste");
    }

    // ── outcome → disposition-relevant smoke (executor determinism) ──

    #[test]
    fn empty_plan_is_clipboard_only_outcome() {
        // Defensive: an empty plan pastes nothing.
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        assert_eq!(
            execute_insertion(&[], &inserter, &clipboard),
            InsertOutcome::ClipboardOnly
        );
    }

    // ── the three "no restore" reasons are distinct AND correctly wired (D7) ──

    #[test]
    fn no_restore_reason_messages_are_exact_and_distinct() {
        // Pins the exact D7 strings so a refactor cannot silently reword one, and
        // asserts all three differ — the whole point is three separate signals.
        assert_eq!(
            NoRestoreReason::NonTextSnapshot.message(),
            "рестор пропущен: снапшот не-текст"
        );
        assert_eq!(
            NoRestoreReason::BufferChanged.message(),
            "рестор отменён: буфер изменился"
        );
        assert_eq!(
            NoRestoreReason::PasteFailed.message(),
            "рестор не выполнялся: вставка провалилась"
        );
        let set: std::collections::HashSet<&str> = [
            NoRestoreReason::NonTextSnapshot.message(),
            NoRestoreReason::BufferChanged.message(),
            NoRestoreReason::PasteFailed.message(),
        ]
        .into_iter()
        .collect();
        assert_eq!(set.len(), 3, "the three reasons must be distinct messages");
    }

    /// Run `f` under a thread-local capturing subscriber and return the `message`
    /// of every event it emitted. Thread-local (`with_default`), so it is
    /// parallel-safe with the rest of the suite.
    fn capture_messages(f: impl FnOnce()) -> Vec<String> {
        use std::sync::{Arc, Mutex};
        use tracing::field::{Field, Visit};
        use tracing::subscriber::with_default;
        use tracing_subscriber::layer::{Context, Layer};
        use tracing_subscriber::prelude::*;

        #[derive(Default)]
        struct MsgVisitor(Arc<Mutex<Vec<String>>>);
        impl Visit for MsgVisitor {
            fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0.lock().unwrap().push(format!("{value:?}"));
                }
            }
        }
        struct CaptureLayer(Arc<Mutex<Vec<String>>>);
        impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
            fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
                event.record(&mut MsgVisitor(self.0.clone()));
            }
        }

        let captured = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(CaptureLayer(captured.clone()));
        with_default(subscriber, f);
        let out = captured.lock().unwrap().clone();
        out
    }

    #[test]
    fn nontext_snapshot_emits_its_distinct_reason() {
        // A non-text snapshot must log reason 1 — and only reason 1.
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::non_text_snapshot();
        let msgs = capture_messages(|| {
            insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);
        });
        assert!(
            msgs.iter()
                .any(|m| m.contains(NoRestoreReason::NonTextSnapshot.message())),
            "expected the non-text-snapshot reason, got: {msgs:?}"
        );
        assert!(
            !msgs
                .iter()
                .any(|m| m.contains(NoRestoreReason::BufferChanged.message())),
            "must not also log the buffer-changed reason"
        );
    }

    #[test]
    fn paste_failure_emits_its_distinct_reason() {
        let inserter = FakeInserter::failing_paste();
        let clipboard = FakeClipboard::with_text(PRIOR);
        let msgs = capture_messages(|| {
            insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);
        });
        assert!(
            msgs.iter()
                .any(|m| m.contains(NoRestoreReason::PasteFailed.message())),
            "expected the paste-failed reason, got: {msgs:?}"
        );
    }

    #[test]
    fn buffer_changed_emits_its_distinct_reason() {
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::overwritten_before_verify(PRIOR, "чужой текст");
        let msgs = capture_messages(|| {
            insert_transcript(&inserter, &clipboard, TRANSCRIPT, InsertionMode::Paste);
        });
        assert!(
            msgs.iter()
                .any(|m| m.contains(NoRestoreReason::BufferChanged.message())),
            "expected the buffer-changed reason, got: {msgs:?}"
        );
    }

    // A clipboard whose first successful write succeeds and whose second fails —
    // used only to prove a failed *restore* does not downgrade the outcome.
    #[derive(Clone)]
    struct RestoreFailingClipboard {
        inner: std::sync::Arc<std::sync::Mutex<RfcState>>,
    }
    struct RfcState {
        content: Option<String>,
        successful_writes: usize,
    }
    impl RestoreFailingClipboard {
        fn new(prior: &str) -> Self {
            Self {
                inner: std::sync::Arc::new(std::sync::Mutex::new(RfcState {
                    content: Some(prior.to_string()),
                    successful_writes: 0,
                })),
            }
        }
    }
    impl ClipboardAccess for RestoreFailingClipboard {
        fn get_text(&self) -> Result<String, String> {
            self.inner
                .lock()
                .unwrap()
                .content
                .clone()
                .ok_or_else(|| "empty".into())
        }
        fn set_text(&self, text: &str) -> Result<(), String> {
            let mut s = self.inner.lock().unwrap();
            s.successful_writes += 1;
            // First write (transcript) succeeds; second write (restore) fails.
            if s.successful_writes >= 2 {
                return Err("restore write failed".into());
            }
            s.content = Some(text.to_string());
            Ok(())
        }
    }
}
