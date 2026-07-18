//! The dictation pipeline — the first user-facing feature of the marathon
//! (Sprint 6 PR3).
//!
//! [`run_dictation`] is the single async flow that stitches the two invisible
//! foundations together: it drives the recorder (PR2) from `Start` to a
//! finalized [`PcmAudio`], filters out silence and accidental taps (D8), sends
//! the clip to the STT provider (PR1), and delivers the recognised text into the
//! active window (PR4). Every side effect happens behind a **seam** so the whole
//! pipeline is exercised without a microphone, a network, a real clipboard, a
//! real keyboard, or a Tauri runtime:
//!
//! - [`RecorderControl`] — start/stop the capture (real impl: [`RecorderHandle`];
//!   a fake returns a canned [`PcmAudio`]).
//! - [`crate::stt::SttProvider`] — transcription (real: `OpenAiCompatStt`; a
//!   `FakeStt` returns canned text or errors).
//! - [`TextInserter`] + [`ClipboardAccess`] — the two delivery seams (PR4, D8):
//!   keystroke synthesis (real: `insert::EnigoInserter`) and clipboard get/set
//!   (real: `insert::ArboardClipboard`), both blocking, both run inside one
//!   `spawn_blocking`. `FakeInserter`/`FakeClipboard` keep the headless CI box
//!   hardware-free (D9).
//! - [`DictationEmitter`] — the `dictation-state` event stream that drives the
//!   overlay pill (real: `AppHandle` in `lib.rs`; a `FakeEmitter` collects the
//!   states).
//!
//! # Watchdog (D10)
//!
//! The 60 s cap lives here as a **wall-clock** timer, not only in the recorder.
//! The recorder's sample-count cap (PR2 D9) auto-finalizes the buffer, but it
//! only hands the clip over in response to a `Stop`; if the hotkey's `Released`
//! is ever lost, that `Stop` would never arrive and the phase would wedge in
//! `Recording` forever. So the pipeline's own `select!` between the release
//! signal and a `sleep(watchdog)` guarantees a `Stop` is always sent. Whichever
//! branch wins, `recorder.stop()` returns the (possibly `truncated`) clip and
//! the flow continues. `truncated` rides all the way to the `done` event so the
//! user sees "обрезано по 60 с" rather than a silent truncation.

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::oneshot;

use super::insert::{
    insert_transcript, ClipboardAccess, InsertOutcome, InsertionMode, TextInserter,
};
use super::{DictationPhase, PcmAudio, RecorderError, RecorderHandle, StartedInfo};
use crate::commands::dictation::{recorder_error_to_user_facing_ru, stt_error_to_user_facing_ru};
use crate::stt::{wav, SttProvider};

/// Broadcast event name carrying the dictation state-machine transitions (D7).
/// Kept in lock-step with `DICTATION_STATE_EVENT` in `src/lib/tauri.ts`.
/// kebab-case per project convention (`dictation-level`, `synthesis-completed`).
pub const DICTATION_STATE_EVENT: &str = "dictation-state";

/// A recording shorter than this is treated as an accidental tap and discarded
/// without touching the network (D8): a stray brush of the hotkey must not burn
/// an API request.
pub const MIN_DICTATION_MS: u32 = 300;

/// Linear-RMS silence floor for a whole clip (D8). A clip quieter than this is
/// discarded. `0.005` is the *start* value from D8 — a mic noise floor with
/// Windows AGC sits at 0.001–0.01, tided speech well above. Logged per clip at
/// `debug` (D8-a) so a week of real use calibrates it; the weakening criterion
/// (D8-b: halve it if live whispers get cut) is a QA follow-up, not shipped code.
pub const SILENCE_RMS_THRESHOLD: f32 = 0.005;

/// Default wall-clock watchdog: the same 60 s as the recorder's sample cap
/// ([`super::MAX_RECORDING_MS`]). Injectable via [`DictationOpts`] so tests can
/// drive the watchdog branch in milliseconds.
pub const DEFAULT_WATCHDOG: Duration = Duration::from_millis(super::MAX_RECORDING_MS as u64);

// ── Event contract (D7) ─────────────────────────────────────────────────

/// How the recognised text was delivered (D7/D13). `disposition` was reserved in
/// PR2's event design specifically so PR4 (auto-paste) could add `Pasted` without
/// breaking the wire contract. `Discarded` covers "nothing to deliver" — an
/// accidental tap, a silent clip, or an empty transcript — so the overlay can
/// hide the pill immediately with no "Скопировано" flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Disposition {
    /// Paste events were sent to the active window (PR4). Serialises to `"pasted"`
    /// — the overlay shows «Вставлено». Means *sent to the OS*, not *landed in the
    /// app* (D11); the two cannot be distinguished by design of Windows.
    Pasted,
    /// Text written to the OS clipboard only — clipboard-only mode, or the paste
    /// keystroke failed and the transcript was left for a manual Ctrl+V.
    Clipboard,
    /// Nothing delivered — the clip was filtered or the transcript was empty.
    Discarded,
}

/// The dictation state machine as broadcast to the overlay (D7).
///
/// `#[serde(tag = "kind", rename_all = "camelCase")]` matches the discriminated
/// union in `src/lib/tauri.ts`, so the overlay narrows with `switch (e.kind)`.
/// The `done` variant carries `truncated` (D10) so a 60 s-capped recording
/// surfaces "обрезано по 60 с" instead of truncating silently.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DictationState {
    /// Capturing — the pill shows live RMS bars (fed by `dictation-level`).
    Recording,
    /// Recording stopped, transcription in flight — the pill shows «Распознаю…».
    Processing,
    /// Terminal success/silent-discard. `disposition` says what happened;
    /// `truncated` is `true` only when the 60 s cap forced finalization.
    Done {
        disposition: Disposition,
        truncated: bool,
    },
    /// Terminal failure. `message` is a ready-to-display Russian sentence.
    Error { message: String },
}

/// A sink for [`DictationState`] transitions (D7).
///
/// The real implementation (for `tauri::AppHandle`, in `lib.rs`) emits the
/// `dictation-state` event; a `FakeEmitter` collects the states in tests.
/// `Clone + Send + 'static` because the pipeline runs on a spawned task.
pub trait DictationEmitter: Clone + Send + 'static {
    /// Broadcast one state transition to the overlay.
    fn emit_state(&self, state: DictationState);
}

/// Start/stop the microphone capture (the recorder seam for the pipeline).
///
/// [`RecorderHandle`] is the real implementation — it forwards to the dedicated
/// recorder thread (PR2). A `FakeRecorder` in tests returns a canned
/// [`PcmAudio`] so the pipeline's control flow is deterministic without a
/// capture thread or hardware.
#[allow(async_fn_in_trait)]
pub trait RecorderControl {
    /// Open the device (`None` = system default) and begin capture.
    async fn start(&self, device: Option<String>) -> Result<StartedInfo, RecorderError>;
    /// Stop capture and return the finalized 16 kHz mono clip.
    async fn stop(&self) -> Result<PcmAudio, RecorderError>;
}

impl RecorderControl for RecorderHandle {
    async fn start(&self, device: Option<String>) -> Result<StartedInfo, RecorderError> {
        RecorderHandle::start(self, device).await
    }
    async fn stop(&self) -> Result<PcmAudio, RecorderError> {
        RecorderHandle::stop(self).await
    }
}

// ── Pipeline inputs / outputs ───────────────────────────────────────────

/// Why a recording produced nothing to deliver (D8). Carried by
/// [`DictationOutcome::Discarded`] for logging/assertions; the user just sees
/// the pill disappear (a `done` event with [`Disposition::Discarded`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscardReason {
    /// Shorter than [`MIN_DICTATION_MS`] — an accidental tap.
    TooShort,
    /// Whole-clip RMS below [`SILENCE_RMS_THRESHOLD`] — silence.
    Silent,
    /// The provider returned empty/whitespace-only text.
    EmptyTranscript,
}

/// What [`run_dictation`] did, for the caller's follow-up (tray icon reset,
/// advisory usage accounting) and for tests. Not sent over IPC — the user-facing
/// signal is the [`DictationState`] event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationOutcome {
    /// Text delivered. `duration_ms` feeds the advisory recognition-seconds
    /// counter (D13); `text` + `disposition` feed the opt-in history write
    /// (Sprint 6 PR5a, D2/D4) — the caller maps `disposition` to the persisted
    /// `dictations.status` (`pasted` | `clipboard`).
    Delivered {
        truncated: bool,
        duration_ms: u32,
        text: String,
        disposition: Disposition,
    },
    /// Nothing delivered — filtered clip or empty transcript. Never written to
    /// history (D2: `discarded` has no transcript to record).
    Discarded(DiscardReason),
    /// A recorder / provider / clipboard error was surfaced as an `error` event.
    /// `message` is the Russian sentence shown to the user, persisted as the
    /// history row's `error_message` when history is on (D2).
    Failed { message: String },
}

/// Per-run knobs. `device`/`lang`/`mode` come from persisted STT settings;
/// `watchdog` defaults to [`DEFAULT_WATCHDOG`] and is overridden only by tests.
pub struct DictationOpts {
    pub device: Option<String>,
    pub lang: Option<String>,
    /// How to deliver the transcript (D12). Read from `stt_insertion_mode`.
    pub mode: InsertionMode,
    pub watchdog: Duration,
}

impl DictationOpts {
    /// Production options: no explicit device (system default), the given
    /// language + insertion mode, the full 60 s watchdog.
    pub fn new(device: Option<String>, lang: Option<String>, mode: InsertionMode) -> Self {
        Self {
            device,
            lang,
            mode,
            watchdog: DEFAULT_WATCHDOG,
        }
    }
}

/// The borrowed dependencies [`run_dictation`] drives. Bundled into a struct so
/// the function signature stays readable as the seam count grows.
pub struct DictationDeps<'a, R, P, I, C, E>
where
    R: RecorderControl,
    P: SttProvider,
    I: TextInserter,
    C: ClipboardAccess,
    E: DictationEmitter,
{
    pub recorder: &'a R,
    pub provider: &'a P,
    /// Keystroke synthesis seam (D8). Real impl: `insert::EnigoInserter`.
    pub inserter: &'a I,
    /// Clipboard get/set seam (D8). Real impl: `insert::ArboardClipboard`.
    pub clipboard: &'a C,
    pub emitter: &'a E,
    /// App-level phase (D13). Set to `Recording` for the capture, `Processing`
    /// during transcription, `Idle` on every exit. `std::sync::Mutex`, only ever
    /// locked in a block scope — never held across an `.await`.
    pub phase: &'a Mutex<DictationPhase>,
}

// ── Pure helpers ────────────────────────────────────────────────────────

/// Linear RMS of a finalized clip, computed on the `i16` samples reinterpreted
/// as `f32` in `[-1, 1)`. Uses [`super::rms_iter`] over a lazy map so the level
/// meter and the silence filter measure loudness identically **without** first
/// allocating a parallel `Vec<f32>` (the PR4 `rms_iter` carry-over — ~3.84 MB
/// saved on a 60 s clip).
fn clip_rms(pcm: &PcmAudio) -> f32 {
    super::rms_iter(pcm.samples.iter().map(|&s| s as f32 / 32768.0))
}

/// Decide whether a finalized clip should be discarded before spending an API
/// call (D8). `rms` is passed in (already computed + logged by the caller) so
/// the debug log fires for *every* clip, discarded or not.
fn discard_reason(duration_ms: u32, rms: f32) -> Option<DiscardReason> {
    if duration_ms < MIN_DICTATION_MS {
        return Some(DiscardReason::TooShort);
    }
    if rms < SILENCE_RMS_THRESHOLD {
        return Some(DiscardReason::Silent);
    }
    None
}

/// Set the shared phase in a tight block scope (never held across an `.await`).
fn set_phase(phase: &Mutex<DictationPhase>, next: DictationPhase) {
    if let Ok(mut guard) = phase.lock() {
        *guard = next;
    }
}

// ── The pipeline ────────────────────────────────────────────────────────

/// Run one dictation from `Start` to delivery (D13).
///
/// Sequence: emit `recording` → start the recorder → wait for `Released` *or*
/// the watchdog → stop the recorder → silence filter (D8) → emit `processing`
/// → transcribe (PR1) → insert the text into the active window behind the
/// [`TextInserter`] + [`ClipboardAccess`] seams (D6) → emit the terminal
/// `done`/`error`. Every failure surfaces as an
/// `error` event with a Russian sentence and returns [`DictationOutcome::Failed`];
/// the phase is reset to `Idle` on every exit.
///
/// `released` resolves when the hotkey is released (the sender is stored by the
/// `Pressed` handler and fired by `Released`); a dropped sender (app shutdown)
/// resolves it too, which simply stops the recording — the correct behaviour.
pub async fn run_dictation<R, P, I, C, E>(
    deps: DictationDeps<'_, R, P, I, C, E>,
    opts: DictationOpts,
    released: oneshot::Receiver<()>,
) -> DictationOutcome
where
    R: RecorderControl,
    P: SttProvider,
    I: TextInserter,
    C: ClipboardAccess,
    E: DictationEmitter,
{
    set_phase(deps.phase, DictationPhase::Recording);
    deps.emitter.emit_state(DictationState::Recording);

    // Start capture. A start failure (no mic, permission denied) is the first
    // thing the user can hit — surface it in the pill (test #9). A successful
    // start that fell back to the system default (the pinned device is gone,
    // D6) is a warning, never an error — dictation proceeds on the default mic.
    match deps.recorder.start(opts.device).await {
        Ok(info) => {
            if info.fell_back_to_default {
                tracing::warn!(
                    device = %info.device_name,
                    "pinned dictation device unavailable — falling back to system default"
                );
            }
        }
        Err(e) => return fail(&deps, recorder_error_to_user_facing_ru(&e)),
    }

    // Wait for the hotkey release or the watchdog, whichever comes first (D10).
    tokio::select! {
        _ = released => {}
        _ = tokio::time::sleep(opts.watchdog) => {
            tracing::debug!("dictation watchdog fired after {:?}; sending Stop", opts.watchdog);
        }
    }

    // Stop + finalize. Either branch above converges here.
    let pcm = match deps.recorder.stop().await {
        Ok(pcm) => pcm,
        Err(e) => return fail(&deps, recorder_error_to_user_facing_ru(&e)),
    };

    // Silence filter (D8). Compute + log RMS for every clip (D8-a), then decide.
    let rms = clip_rms(&pcm);
    tracing::debug!(
        duration_ms = pcm.duration_ms,
        rms,
        truncated = pcm.truncated,
        "dictation clip finalized"
    );
    if let Some(reason) = discard_reason(pcm.duration_ms, rms) {
        tracing::debug!(?reason, "dictation clip discarded before transcription");
        return discard(&deps, reason);
    }

    // Transcribe (PR1). The clip is already 16 kHz mono S16LE; wrap it as WAV.
    set_phase(deps.phase, DictationPhase::Processing);
    deps.emitter.emit_state(DictationState::Processing);
    let wav_bytes = wav::wrap_wav_s16le_mono(&pcm.samples, pcm.sample_rate);
    let text = match deps
        .provider
        .transcribe(wav_bytes, opts.lang.as_deref())
        .await
    {
        Ok(t) => t.text,
        Err(e) => return fail(&deps, stt_error_to_user_facing_ru(&e)),
    };

    // A provider can return empty text for a clip that passed the RMS gate
    // (breath, a click). Nothing to deliver — discard silently rather than
    // stamping an empty clipboard.
    let text = text.trim();
    if text.is_empty() {
        tracing::debug!("provider returned empty transcript; discarding");
        return discard(&deps, DiscardReason::EmptyTranscript);
    }

    // Deliver: snapshot → clipboard → paste → restore, all behind the seams and
    // off the async worker (D6/D9). `enigo` + `arboard` are both blocking, so the
    // whole insertion runs in one `spawn_blocking`; the seams are cheap `Clone`s
    // that construct their real OS handle per call (D8).
    let inserter = deps.inserter.clone();
    let clipboard = deps.clipboard.clone();
    let owned = text.to_string();
    let mode = opts.mode;
    let delivery =
        tokio::task::spawn_blocking(move || insert_transcript(&inserter, &clipboard, &owned, mode))
            .await;

    match delivery {
        Ok(insert_outcome) => {
            let disposition = match insert_outcome {
                InsertOutcome::Pasted => Disposition::Pasted,
                InsertOutcome::ClipboardOnly => Disposition::Clipboard,
                // We couldn't even write the clipboard (busy) — nothing was
                // delivered anywhere, so tell the user loudly (D10).
                InsertOutcome::Failed => {
                    return fail(
                        &deps,
                        "Буфер обмена занят другим приложением. Попробуйте ещё раз.".to_string(),
                    );
                }
            };
            deps.emitter.emit_state(DictationState::Done {
                disposition,
                truncated: pcm.truncated,
            });
            set_phase(deps.phase, DictationPhase::Idle);
            DictationOutcome::Delivered {
                truncated: pcm.truncated,
                duration_ms: pcm.duration_ms,
                text: text.to_string(),
                disposition,
            }
        }
        // spawn_blocking join failure (panic inside the blocking task).
        Err(e) => fail(&deps, format!("Не удалось выполнить вставку: {e}.")),
    }
}

/// Emit an `error` event, reset the phase, and report failure.
fn fail<R, P, I, C, E>(deps: &DictationDeps<'_, R, P, I, C, E>, message: String) -> DictationOutcome
where
    R: RecorderControl,
    P: SttProvider,
    I: TextInserter,
    C: ClipboardAccess,
    E: DictationEmitter,
{
    deps.emitter.emit_state(DictationState::Error {
        message: message.clone(),
    });
    set_phase(deps.phase, DictationPhase::Idle);
    DictationOutcome::Failed { message }
}

/// Emit a silent-discard `done` event, reset the phase, and report the reason.
fn discard<R, P, I, C, E>(
    deps: &DictationDeps<'_, R, P, I, C, E>,
    reason: DiscardReason,
) -> DictationOutcome
where
    R: RecorderControl,
    P: SttProvider,
    I: TextInserter,
    C: ClipboardAccess,
    E: DictationEmitter,
{
    deps.emitter.emit_state(DictationState::Done {
        disposition: Disposition::Discarded,
        truncated: false,
    });
    set_phase(deps.phase, DictationPhase::Idle);
    DictationOutcome::Discarded(reason)
}

#[cfg(test)]
mod tests {
    use super::super::insert::fakes::{FakeClipboard, FakeInserter};
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::stt::{SttError, Transcript};

    // ── test doubles ──

    /// A recorder that returns canned start/stop results — no thread, no cpal.
    struct FakeRecorder {
        start: Result<StartedInfo, RecorderError>,
        stop: std::sync::Mutex<Option<Result<PcmAudio, RecorderError>>>,
    }

    impl FakeRecorder {
        fn ok(stop: Result<PcmAudio, RecorderError>) -> Self {
            Self {
                start: Ok(StartedInfo {
                    device_name: "fake".into(),
                    fell_back_to_default: false,
                }),
                stop: std::sync::Mutex::new(Some(stop)),
            }
        }
        fn start_fails(err: RecorderError) -> Self {
            Self {
                start: Err(err),
                stop: std::sync::Mutex::new(Some(Ok(PcmAudio::from_samples_16k(vec![], false)))),
            }
        }
        /// A recorder whose `start` succeeds but reports the pinned device was
        /// missing and it fell back to the system default (D6).
        fn ok_fell_back(stop: Result<PcmAudio, RecorderError>) -> Self {
            Self {
                start: Ok(StartedInfo {
                    device_name: "default".into(),
                    fell_back_to_default: true,
                }),
                stop: std::sync::Mutex::new(Some(stop)),
            }
        }
    }

    impl RecorderControl for FakeRecorder {
        async fn start(&self, _device: Option<String>) -> Result<StartedInfo, RecorderError> {
            self.start.clone()
        }
        async fn stop(&self) -> Result<PcmAudio, RecorderError> {
            self.stop.lock().unwrap().take().expect("stop called once")
        }
    }

    /// A provider returning a canned transcript or error.
    struct FakeStt {
        result: std::sync::Mutex<Option<Result<Transcript, SttError>>>,
        calls: Arc<std::sync::Mutex<u32>>,
    }

    impl FakeStt {
        fn text(s: &str) -> Self {
            Self {
                result: std::sync::Mutex::new(Some(Ok(Transcript { text: s.into() }))),
                calls: Arc::new(std::sync::Mutex::new(0)),
            }
        }
        fn err(e: SttError) -> Self {
            Self {
                result: std::sync::Mutex::new(Some(Err(e))),
                calls: Arc::new(std::sync::Mutex::new(0)),
            }
        }
    }

    impl SttProvider for FakeStt {
        async fn transcribe(
            &self,
            _wav: Vec<u8>,
            _lang: Option<&str>,
        ) -> Result<Transcript, SttError> {
            *self.calls.lock().unwrap() += 1;
            self.result.lock().unwrap().take().expect("transcribe once")
        }
        async fn list_models(&self) -> Result<Vec<String>, SttError> {
            Ok(vec![])
        }
    }

    #[derive(Clone, Default)]
    struct FakeEmitter {
        states: Arc<std::sync::Mutex<Vec<DictationState>>>,
    }

    impl FakeEmitter {
        fn states(&self) -> Vec<DictationState> {
            self.states.lock().unwrap().clone()
        }
    }

    impl DictationEmitter for FakeEmitter {
        fn emit_state(&self, state: DictationState) {
            self.states.lock().unwrap().push(state);
        }
    }

    /// A loud clip (`amp` per sample) of `duration_ms` at 16 kHz.
    fn loud_clip(duration_ms: u32, amp: i16, truncated: bool) -> PcmAudio {
        let n = (16_000u64 * duration_ms as u64 / 1000) as usize;
        PcmAudio::from_samples_16k(vec![amp; n], truncated)
    }

    /// Assemble deps + fired-release and run the pipeline to completion.
    ///
    /// Uses [`InsertionMode::ClipboardOnly`] so the plan is a single `SetText`
    /// (no synthetic keystrokes, no 300 ms settle) — the pipeline's control flow
    /// and the surviving-PR3 `Disposition::Clipboard` contract are what these
    /// tests exercise; the paste path itself is covered exhaustively in
    /// `insert.rs` and by the dedicated Paste-mode test below.
    async fn run(
        recorder: &FakeRecorder,
        provider: &FakeStt,
        inserter: &FakeInserter,
        clipboard: &FakeClipboard,
        emitter: &FakeEmitter,
        phase: &Mutex<DictationPhase>,
    ) -> DictationOutcome {
        let (tx, rx) = oneshot::channel();
        tx.send(()).unwrap(); // release immediately — watchdog untouched
        run_dictation(
            DictationDeps {
                recorder,
                provider,
                inserter,
                clipboard,
                emitter,
                phase,
            },
            DictationOpts {
                device: None,
                lang: Some("ru".into()),
                mode: InsertionMode::ClipboardOnly,
                watchdog: DEFAULT_WATCHDOG,
            },
            rx,
        )
        .await
    }

    // ── happy path ──

    #[tokio::test]
    async fn delivers_text_to_clipboard_on_success() {
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("Привет, мир");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;

        assert!(
            matches!(
                &outcome,
                DictationOutcome::Delivered {
                    truncated: false,
                    duration_ms: 1000,
                    text,
                    disposition: Disposition::Clipboard,
                } if text == "Привет, мир"
            ),
            "got: {outcome:?}"
        );
        assert_eq!(clipboard.content(), Some("Привет, мир".to_string()));
        assert_eq!(
            emitter.states(),
            vec![
                DictationState::Recording,
                DictationState::Processing,
                DictationState::Done {
                    disposition: Disposition::Clipboard,
                    truncated: false
                },
            ]
        );
        assert_eq!(*phase.lock().unwrap(), DictationPhase::Idle);
    }

    #[tokio::test]
    async fn trims_transcript_before_delivery() {
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("  привет  \n");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert_eq!(clipboard.content(), Some("привет".to_string()));
    }

    // ── silence filter (D8) ──

    #[tokio::test]
    async fn discards_tap_shorter_than_min_ms() {
        // 200 ms < 300 ms — discarded before any transcription.
        let recorder = FakeRecorder::ok(Ok(loud_clip(200, 8000, false)));
        let provider = FakeStt::text("should not be called");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;

        assert_eq!(
            outcome,
            DictationOutcome::Discarded(DiscardReason::TooShort)
        );
        assert_eq!(*provider.calls.lock().unwrap(), 0, "API must not be hit");
        assert_eq!(clipboard.writes().len(), 0, "clipboard untouched");
        assert_eq!(
            emitter.states(),
            vec![
                DictationState::Recording,
                DictationState::Done {
                    disposition: Disposition::Discarded,
                    truncated: false
                },
            ],
            "no processing event for a discarded tap"
        );
        assert_eq!(*phase.lock().unwrap(), DictationPhase::Idle);
    }

    #[tokio::test]
    async fn discards_silent_clip_below_rms_floor() {
        // 1 s but amplitude 100/32768 ≈ 0.003 RMS < 0.005 floor.
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 100, false)));
        let provider = FakeStt::text("should not be called");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert_eq!(outcome, DictationOutcome::Discarded(DiscardReason::Silent));
        assert_eq!(*provider.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn keeps_clip_just_above_rms_floor() {
        // amplitude 200/32768 ≈ 0.0061 RMS > 0.005 floor — must NOT be discarded.
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 200, false)));
        let provider = FakeStt::text("на грани");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(matches!(outcome, DictationOutcome::Delivered { .. }));
        assert_eq!(clipboard.content(), Some("на грани".to_string()));
    }

    #[tokio::test]
    async fn discards_empty_transcript_without_writing_clipboard() {
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("   \n  ");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert_eq!(
            outcome,
            DictationOutcome::Discarded(DiscardReason::EmptyTranscript)
        );
        assert_eq!(
            clipboard.writes().len(),
            0,
            "empty transcript must not touch clipboard"
        );
    }

    // ── device fallback (D6) ──

    #[tokio::test]
    async fn recorder_fallback_to_default_still_delivers() {
        // D6: a pinned device that has vanished falls back to the system default
        // (fell_back_to_default = true). That is a warning, never an error — the
        // pipeline must proceed to a normal delivery.
        let recorder = FakeRecorder::ok_fell_back(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("на дефолтном микрофоне");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(
            matches!(outcome, DictationOutcome::Delivered { .. }),
            "fallback must not fail the dictation: {outcome:?}"
        );
        assert_eq!(
            clipboard.content(),
            Some("на дефолтном микрофоне".to_string())
        );
    }

    // ── error paths ──

    #[tokio::test]
    async fn recorder_start_error_surfaces_russian_error() {
        let recorder = FakeRecorder::start_fails(RecorderError::PermissionDenied);
        let provider = FakeStt::text("unused");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(matches!(outcome, DictationOutcome::Failed { .. }));
        let states = emitter.states();
        assert!(matches!(states.last(), Some(DictationState::Error { .. })));
        if let Some(DictationState::Error { message }) = states.last() {
            assert!(message.contains("Конфиденциальность"), "got: {message}");
        }
        assert_eq!(*phase.lock().unwrap(), DictationPhase::Idle);
    }

    #[tokio::test]
    async fn recorder_stop_device_lost_surfaces_error() {
        let recorder = FakeRecorder::ok(Err(RecorderError::DeviceLost("unplugged".into())));
        let provider = FakeStt::text("unused");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(matches!(outcome, DictationOutcome::Failed { .. }));
        assert_eq!(*provider.calls.lock().unwrap(), 0);
        assert!(matches!(
            emitter.states().last(),
            Some(DictationState::Error { .. })
        ));
    }

    #[tokio::test]
    async fn transcribe_auth_error_surfaces_russian_error() {
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::err(SttError::Auth);
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(matches!(outcome, DictationOutcome::Failed { .. }));
        assert_eq!(clipboard.writes().len(), 0);
        if let Some(DictationState::Error { message }) = emitter.states().last() {
            assert!(message.contains("401"), "got: {message}");
        } else {
            panic!("expected error state");
        }
    }

    #[tokio::test]
    async fn clipboard_failure_surfaces_error() {
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("текст");
        let inserter = FakeInserter::default();
        // A clipboard that fails every write → busy (D10) → Failed.
        let clipboard = FakeClipboard::set_fails_first(3);
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(matches!(outcome, DictationOutcome::Failed { .. }));
        if let Some(DictationState::Error { message }) = emitter.states().last() {
            assert!(message.contains("Буфер обмена занят"), "got: {message}");
        } else {
            panic!("expected error state");
        }
        assert_eq!(*phase.lock().unwrap(), DictationPhase::Idle);
    }

    // ── truncation (D10) ──

    #[tokio::test]
    async fn truncated_flag_reaches_done_event() {
        let recorder = FakeRecorder::ok(Ok(loud_clip(60_000, 8000, true)));
        let provider = FakeStt::text("длинная речь");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let outcome = run(
            &recorder, &provider, &inserter, &clipboard, &emitter, &phase,
        )
        .await;
        assert!(
            matches!(
                &outcome,
                DictationOutcome::Delivered {
                    truncated: true,
                    duration_ms: 60_000,
                    ..
                }
            ),
            "got: {outcome:?}"
        );
        assert_eq!(
            emitter.states().last(),
            Some(&DictationState::Done {
                disposition: Disposition::Clipboard,
                truncated: true
            }),
            "truncated must ride the done event to the overlay"
        );
    }

    // ── watchdog (D10) ──

    #[tokio::test]
    async fn watchdog_sends_stop_when_release_never_fires() {
        // The release receiver is dropped without ever firing; a 10 ms watchdog
        // must still drive the flow to a delivered result.
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("сработал сторож");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let (_tx, rx) = oneshot::channel(); // never fired; sender kept alive
        let outcome = run_dictation(
            DictationDeps {
                recorder: &recorder,
                provider: &provider,
                inserter: &inserter,
                clipboard: &clipboard,
                emitter: &emitter,
                phase: &phase,
            },
            DictationOpts {
                device: None,
                lang: None,
                mode: InsertionMode::ClipboardOnly,
                watchdog: Duration::from_millis(10),
            },
            rx,
        )
        .await;

        assert!(matches!(outcome, DictationOutcome::Delivered { .. }));
        assert_eq!(clipboard.content(), Some("сработал сторож".to_string()));
    }

    // ── pure helpers ──

    #[test]
    fn discard_reason_boundaries() {
        // exactly at the floor is discarded; above it is kept.
        assert_eq!(discard_reason(299, 0.9), Some(DiscardReason::TooShort));
        assert_eq!(discard_reason(300, 0.004), Some(DiscardReason::Silent));
        assert_eq!(
            discard_reason(300, SILENCE_RMS_THRESHOLD),
            None,
            "exactly at threshold is not below it"
        );
        assert_eq!(discard_reason(300, 0.9), None);
    }

    // ── insertion mode → disposition (D12/D13) ──

    /// Run the pipeline to completion under an explicit insertion `mode` and
    /// return the terminal `done` disposition.
    async fn run_with_mode(mode: InsertionMode) -> (DictationOutcome, Vec<DictationState>) {
        let recorder = FakeRecorder::ok(Ok(loud_clip(1000, 8000, false)));
        let provider = FakeStt::text("Привет");
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);

        let (tx, rx) = oneshot::channel();
        tx.send(()).unwrap();
        let outcome = run_dictation(
            DictationDeps {
                recorder: &recorder,
                provider: &provider,
                inserter: &inserter,
                clipboard: &clipboard,
                emitter: &emitter,
                phase: &phase,
            },
            DictationOpts {
                device: None,
                lang: None,
                mode,
                watchdog: DEFAULT_WATCHDOG,
            },
            rx,
        )
        .await;
        (outcome, emitter.states())
    }

    #[tokio::test]
    async fn paste_mode_emits_pasted_disposition() {
        // Paste mode with an empty clipboard: snapshot is non-text → paste
        // proceeds, no restore → InsertOutcome::Pasted → Disposition::Pasted.
        let (outcome, states) = run_with_mode(InsertionMode::Paste).await;
        assert!(matches!(outcome, DictationOutcome::Delivered { .. }));
        assert_eq!(
            states.last(),
            Some(&DictationState::Done {
                disposition: Disposition::Pasted,
                truncated: false
            }),
            "Paste mode must report `pasted` so the overlay shows «Вставлено»"
        );
    }

    #[tokio::test]
    async fn clipboard_only_mode_emits_clipboard_disposition() {
        // The surviving PR3 contract (D13): ClipboardOnly mode still reports
        // `clipboard` so the overlay shows «Скопировано». This snapshot must
        // outlive PR4.
        let (outcome, states) = run_with_mode(InsertionMode::ClipboardOnly).await;
        assert!(matches!(outcome, DictationOutcome::Delivered { .. }));
        assert_eq!(
            states.last(),
            Some(&DictationState::Done {
                disposition: Disposition::Clipboard,
                truncated: false
            })
        );
    }

    // ── event wire contract (D7) ──

    #[test]
    fn done_clipboard_serializes_to_expected_wire_shape() {
        // The overlay + PR4 depend on this exact shape. `done.disposition ==
        // "clipboard"` is the contract PR4 mutates when it adds auto-paste.
        let json = serde_json::to_value(DictationState::Done {
            disposition: Disposition::Clipboard,
            truncated: false,
        })
        .unwrap();
        assert_eq!(json["kind"], "done");
        assert_eq!(json["disposition"], "clipboard");
        assert_eq!(json["truncated"], false);
    }

    #[test]
    fn done_pasted_serializes_to_expected_wire_shape() {
        // PR4's new variant: the overlay maps `"pasted"` → «Вставлено» (D13). The
        // Rust ↔ TS lock-step depends on this exact string.
        let json = serde_json::to_value(DictationState::Done {
            disposition: Disposition::Pasted,
            truncated: false,
        })
        .unwrap();
        assert_eq!(json["kind"], "done");
        assert_eq!(json["disposition"], "pasted");
    }

    #[test]
    fn done_discarded_and_truncated_serialize() {
        let discarded = serde_json::to_value(DictationState::Done {
            disposition: Disposition::Discarded,
            truncated: false,
        })
        .unwrap();
        assert_eq!(discarded["disposition"], "discarded");

        let truncated = serde_json::to_value(DictationState::Done {
            disposition: Disposition::Clipboard,
            truncated: true,
        })
        .unwrap();
        assert_eq!(truncated["truncated"], true);
    }

    #[test]
    fn recording_processing_error_serialize_with_kind_tag() {
        assert_eq!(
            serde_json::to_value(DictationState::Recording).unwrap()["kind"],
            "recording"
        );
        assert_eq!(
            serde_json::to_value(DictationState::Processing).unwrap()["kind"],
            "processing"
        );
        let err = serde_json::to_value(DictationState::Error {
            message: "ошибка".into(),
        })
        .unwrap();
        assert_eq!(err["kind"], "error");
        assert_eq!(err["message"], "ошибка");
    }
}
