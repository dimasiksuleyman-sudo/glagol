//! The Dictation microphone recorder (Sprint 6 PR2).
//!
//! This module is the **audio source** the dictation stack sits on. PR1 gave
//! the provider seam (what to do with a WAV clip); PR2 gives the source of the
//! clip (where the audio comes from). Everything that leaves the recorder is
//! already **16 kHz / mono / signed-16-bit little-endian** — no consumer
//! downstream ever deals with sample rate, channel count, or sample format
//! again. [`PcmAudio`] plugs straight into PR1's
//! [`crate::stt::wav::wrap_wav_s16le_mono`] without an adapter.
//!
//! # Shape of the recorder
//!
//! `cpal::Stream` is not `Send` on WASAPI, so the stream lives on a dedicated
//! OS thread that owns it ([`recorder::run_recorder`]). The rest of the app
//! holds only a [`RecorderHandle`] — a clonable `mpsc::Sender<RecorderMsg>`.
//! Commands, captured samples, and stream errors all travel through that
//! **single** queue (D2): `std::sync::mpsc` cannot `select` over two channels,
//! so one queue means one `recv_timeout(50 ms)` loop — no polling, no extra
//! `crossbeam` dependency — and the same tick drives the RMS level emit and
//! the 60 s cap check. Reply values ride back on a `tokio::sync::oneshot` so a
//! `.await`ing Tauri command never blocks a tokio worker (the recorder thread
//! is synchronous and needs the blocking `std` receiver).
//!
//! # This PR has no user surface
//!
//! The only command PR2 exposes is `list_audio_input_devices`; nothing calls
//! `Start`/`Stop` yet. The hotkey + overlay + pipeline that turn this into a
//! feature arrive in PR3. The pure functions ([`downmix_to_mono`], [`rms`],
//! [`f32_to_i16`], [`resample::resample_to_16k`]) and the [`SampleSource`] /
//! [`LevelSink`] seams keep the whole path testable without hardware; a live
//! `#[ignore]` microphone test (see `recorder`) is the manual gate (D14).

pub mod insert;
pub mod pipeline;
pub mod recorder;
pub mod resample;
pub mod session;
pub mod vad;

use thiserror::Error;
use tokio::sync::oneshot;

/// Target sample rate for every clip leaving the recorder — Whisper's native
/// rate. The downmix + resample + `i16` conversion on finalization all target
/// this value.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;

/// Hard cap on a single recording. On reaching it the recorder **auto-finalizes**
/// what it has (with `truncated: true`) rather than cancelling — losing 60 s of
/// dictated text is worse than any other outcome (D9). Counted by accumulated
/// samples at the native rate, never by wall-clock, so it is deterministic in
/// tests driven by a fake source.
pub const MAX_RECORDING_MS: u32 = 60_000;

/// RMS level-meter window, in milliseconds, measured on the **native** capture
/// rate. The recorder accumulates `native_rate * LEVEL_WINDOW_MS / 1000`
/// samples, emits one linear RMS value, and resets (D6).
pub const LEVEL_WINDOW_MS: u32 = 50;

// ── Output contract ─────────────────────────────────────────────────────

/// A finished recording, normalized to the invariant the whole marathon
/// relies on: **16 kHz, mono, S16LE** (D7).
///
/// `sample_rate` is always [`TARGET_SAMPLE_RATE`]; it is carried explicitly for
/// readability and as a regression guard, not because a consumer may choose
/// another rate. `Serialize` so PR3's pipeline can hand it across IPC if needed;
/// it is not a command input, so no `Deserialize` (convention: return types get
/// `Serialize` only).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PcmAudio {
    /// 16 kHz, mono, signed-16-bit little-endian samples.
    pub samples: Vec<i16>,
    /// Always [`TARGET_SAMPLE_RATE`].
    pub sample_rate: u32,
    /// Duration derived from `samples.len()` at 16 kHz.
    pub duration_ms: u32,
    /// `true` when the 60 s cap ([`MAX_RECORDING_MS`]) forced finalization.
    pub truncated: bool,
}

impl PcmAudio {
    /// Build a `PcmAudio` from final 16 kHz mono samples, deriving `duration_ms`.
    pub fn from_samples_16k(samples: Vec<i16>, truncated: bool) -> Self {
        let duration_ms = ((samples.len() as u64 * 1000) / TARGET_SAMPLE_RATE as u64) as u32;
        Self {
            samples,
            sample_rate: TARGET_SAMPLE_RATE,
            duration_ms,
            truncated,
        }
    }
}

// ── Recorder messaging ──────────────────────────────────────────────────

/// The single message type flowing into the recorder thread (D2).
///
/// Control messages (`Start`/`Stop`/`Cancel`/`Shutdown`), captured audio
/// (`Samples`, from the cpal data callback), and stream faults (`StreamError`,
/// from the cpal error callback) share one queue so the recorder loop is a
/// single `recv_timeout`.
pub enum RecorderMsg {
    /// Begin capture from `device` (`None` = system default). Replies with the
    /// resolved device info or an error.
    Start {
        device: Option<String>,
        reply: oneshot::Sender<Result<StartedInfo, RecorderError>>,
    },
    /// Stop capture and finalize; replies with the normalized [`PcmAudio`].
    Stop {
        reply: oneshot::Sender<Result<PcmAudio, RecorderError>>,
    },
    /// Stop capture and discard the buffer (no reply).
    Cancel,
    /// Drain and terminate the recorder thread (sent at app exit).
    Shutdown,
    /// A batch of interleaved native-rate `f32` samples from the data callback.
    Samples(Vec<f32>),
    /// The cpal stream reported a fault (device unplugged, driver error).
    StreamError(String),
}

/// Device info reported back from a successful `Start` (D2/D10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedInfo {
    /// The device actually opened.
    pub device_name: String,
    /// `true` when the requested device was missing and capture fell back to
    /// the system default — a warning for the UI (PR3/PR5), **not** an error
    /// (D10).
    pub fell_back_to_default: bool,
}

/// What a [`SampleSource`] hands the recorder thread when a stream opens: the
/// public [`StartedInfo`] plus the **native** capture format the recorder needs
/// to downmix, resample, size the RMS window, and count the 60 s cap. The
/// format is deliberately not part of `StartedInfo` — the UI never sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartedConfig {
    /// Reported back to the caller via the `Start` reply.
    pub info: StartedInfo,
    /// Native device sample rate (e.g. 48 000 on WASAPI shared mode).
    pub sample_rate: u32,
    /// Native device channel count (1 = mono, 2 = stereo, …).
    pub channels: u16,
}

/// A clonable handle to the recorder thread — just the send side of the queue.
///
/// Cloning is cheap (`mpsc::Sender` is `Clone`). Stored in `AppState`; the
/// async helpers are used by future PRs' commands and by the state-machine
/// tests. A closed channel (recorder thread gone) maps to
/// [`RecorderError::DeviceLost`] rather than panicking.
#[derive(Clone)]
pub struct RecorderHandle {
    tx: std::sync::mpsc::Sender<RecorderMsg>,
}

impl RecorderHandle {
    /// Wrap a sender. The matching receiver is owned by
    /// [`recorder::run_recorder`].
    pub fn new(tx: std::sync::mpsc::Sender<RecorderMsg>) -> Self {
        Self { tx }
    }

    /// Clone the underlying sender — used to hand the cpal callbacks a way to
    /// push `Samples`/`StreamError` back into the same queue.
    pub fn sender(&self) -> std::sync::mpsc::Sender<RecorderMsg> {
        self.tx.clone()
    }

    /// Start capture. `device = None` uses the system default.
    pub async fn start(&self, device: Option<String>) -> Result<StartedInfo, RecorderError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(RecorderMsg::Start { device, reply })
            .map_err(|_| RecorderError::DeviceLost("recorder thread stopped".into()))?;
        rx.await
            .map_err(|_| RecorderError::DeviceLost("recorder thread stopped".into()))?
    }

    /// Stop capture and get the finalized clip.
    pub async fn stop(&self) -> Result<PcmAudio, RecorderError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(RecorderMsg::Stop { reply })
            .map_err(|_| RecorderError::DeviceLost("recorder thread stopped".into()))?;
        rx.await
            .map_err(|_| RecorderError::DeviceLost("recorder thread stopped".into()))?
    }

    /// Cancel capture, discarding the buffer. Best-effort — a closed channel is
    /// ignored (the thread is already gone).
    pub fn cancel(&self) {
        let _ = self.tx.send(RecorderMsg::Cancel);
    }

    /// Ask the recorder thread to drain and exit. Best-effort.
    pub fn shutdown(&self) {
        let _ = self.tx.send(RecorderMsg::Shutdown);
    }
}

#[cfg(test)]
impl RecorderHandle {
    /// A handle with no recorder thread behind it — every send fails silently.
    /// For unit tests that construct an [`crate::state::AppState`] but never
    /// exercise the recorder.
    pub fn disconnected() -> Self {
        let (tx, _rx) = std::sync::mpsc::channel();
        Self { tx }
    }
}

/// App-level dictation phase tracked in [`crate::state::AppState`] (D13).
///
/// Distinct from the recorder thread's internal phase: this is what the
/// command layer coordinates against. `Start` arriving in a non-`Idle` phase is
/// the recorder's `Busy` (mechanism); whether to swallow a double hotkey press
/// is the pipeline's policy in PR3. Transitions: `Idle → Recording → Processing
/// → Idle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DictationPhase {
    #[default]
    Idle,
    Recording,
    Processing,
}

// ── Error taxonomy ──────────────────────────────────────────────────────

/// Everything the recorder can fail with (D12).
///
/// `Display` forms stay English so tests assert on them; the command layer
/// translates to a user-facing Russian sentence
/// (`commands::dictation::recorder_error_to_user_facing_ru`).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecorderError {
    /// No input devices exist at all.
    #[error("no input device available")]
    NoDevice,

    /// Best-effort heuristic: devices enumerate but the stream fails to build,
    /// which on Windows usually means the microphone privacy toggle is off.
    // NOTE: best-effort heuristic, see kickoff D12. cpal/WASAPI give no clean
    // "denied in Privacy settings" signal; a reliable winreg ConsentStore
    // detector is deferred to PR5/post-MVP.
    #[error("permission denied (best-effort heuristic)")]
    PermissionDenied,

    /// No usable input configuration (unexpected sample format, etc.).
    #[error("unsupported audio configuration: {0}")]
    UnsupportedConfig(String),

    /// `build_input_stream` failed for a reason other than permission.
    #[error("failed to build input stream: {0}")]
    BuildStream(String),

    /// The stream's error callback fired mid-capture (device unplugged, …).
    #[error("audio device lost: {0}")]
    DeviceLost(String),

    /// `Start` arrived while the recorder was not `Idle` (D13). The *policy*
    /// decision (swallow the double hotkey press) belongs to PR3; the recorder
    /// only reports the fact.
    #[error("recorder is busy")]
    Busy,
}

// ── Seams ───────────────────────────────────────────────────────────────

/// A sink for RMS level values (D11).
///
/// The real implementation (for `tauri::AppHandle`) emits a `dictation-level`
/// event; `FakeSink` collects values in tests. `Send + 'static` because it is
/// owned by the recorder thread.
pub trait LevelSink: Send + 'static {
    /// Receive one linear RMS value in `0.0..=1.0`. Scaling (dB curve,
    /// smoothing, pill threshold) is the overlay's job in PR3 — the recorder
    /// measures, the UI interprets.
    fn level(&self, rms: f32);
}

/// The audio-capture seam (D2).
///
/// `CpalSource` is the real implementation; `FakeSource` in tests pushes the
/// same `Samples(..)` the cpal callback would, so the state machine is
/// exercised without hardware. Generic (not `dyn`) at the call site to match
/// the zero-cost seam convention established in PR1.
pub trait SampleSource {
    /// Open `device` (`None` = system default) and start delivering
    /// `RecorderMsg::Samples` batches (and `RecorderMsg::StreamError` on fault)
    /// to `tx`. Returns the resolved device info plus the native capture format.
    fn start(
        &mut self,
        device: Option<&str>,
        tx: std::sync::mpsc::Sender<RecorderMsg>,
    ) -> Result<StartedConfig, RecorderError>;

    /// Stop delivering samples and release the device.
    fn stop(&mut self);
}

// ── Pure DSP helpers ────────────────────────────────────────────────────

/// Downmix interleaved multi-channel `f32` frames to mono by averaging each
/// frame's channels (D5).
///
/// `chunks_exact` (not `chunks`) drops a trailing partial frame: half a frame
/// on a callback boundary is not physically valid. `channels <= 1` short-circuits
/// to a copy.
pub fn downmix_to_mono(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    let ch = channels as usize;
    let inv = 1.0 / ch as f32;
    interleaved
        .chunks_exact(ch)
        .map(|frame| frame.iter().sum::<f32>() * inv)
        .collect()
}

/// Root-mean-square over a stream of mono samples — `sqrt(mean(x²))` (D6).
/// Empty → `0.0`.
///
/// Iterator-based (the PR4 `rms_iter` carry-over) so a caller that only has a
/// lazy sequence never has to materialise a `Vec` just to measure it — the
/// pipeline's whole-clip RMS reinterprets `i16` samples as `f32` on the fly
/// instead of allocating a parallel `Vec<f32>` (~3.84 MB on a 60 s clip). One
/// pass computes both the sum of squares and the count. [`rms`] is the slice
/// wrapper over this, so the recorder and the pipeline share one implementation.
pub fn rms_iter<I: IntoIterator<Item = f32>>(samples: I) -> f32 {
    let mut sum_sq = 0.0f32;
    let mut n = 0usize;
    for x in samples {
        sum_sq += x * x;
        n += 1;
    }
    if n == 0 {
        return 0.0;
    }
    (sum_sq / n as f32).sqrt()
}

/// Root-mean-square of a mono buffer — `sqrt(mean(x²))` (D6). Empty → `0.0`.
/// Thin slice wrapper over [`rms_iter`].
pub fn rms(mono: &[f32]) -> f32 {
    rms_iter(mono.iter().copied())
}

/// Convert mono `f32` in `[-1.0, 1.0]` to `i16` (D7).
///
/// The `clamp` **before** multiplying is mandatory: a WASAPI F32 mix can
/// formally exceed ±1.0 under aggressive mic gain, and without the clamp the
/// `as i16` cast would wrap around into a loud crackle.
pub fn f32_to_i16(mono: &[f32]) -> Vec<i16> {
    mono.iter()
        .map(|&x| (x.clamp(-1.0, 1.0) * 32767.0).round() as i16)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── downmix_to_mono ──

    #[test]
    fn downmix_mono_fast_path_copies() {
        let input = [0.1, -0.2, 0.3];
        assert_eq!(downmix_to_mono(&input, 1), vec![0.1, -0.2, 0.3]);
    }

    #[test]
    fn downmix_stereo_averages_pairs() {
        // (L, R) frames: (1.0, 0.0) -> 0.5, (-0.4, -0.6) -> -0.5.
        let input = [1.0, 0.0, -0.4, -0.6];
        let out = downmix_to_mono(&input, 2);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[1] - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn downmix_six_channels_averages() {
        // One 6-channel frame of all 0.6 -> 0.6.
        let input = [0.6; 6];
        let out = downmix_to_mono(&input, 6);
        assert_eq!(out.len(), 1);
        assert!((out[0] - 0.6).abs() < 1e-6);
    }

    #[test]
    fn downmix_drops_trailing_partial_frame() {
        // Stereo, but 3 samples = 1 full frame + 1 dangling sample. The
        // dangling half-frame must be dropped, not misread as a frame.
        let input = [1.0, 1.0, 0.5];
        let out = downmix_to_mono(&input, 2);
        assert_eq!(out.len(), 1, "partial frame must be dropped");
        assert!((out[0] - 1.0).abs() < 1e-6);
    }

    // ── rms ──

    #[test]
    fn rms_of_silence_is_zero() {
        assert_eq!(rms(&[0.0; 128]), 0.0);
    }

    #[test]
    fn rms_of_empty_is_zero() {
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn rms_iter_matches_slice_rms() {
        // The PR4 carry-over: `rms_iter` must be bit-for-bit the slice `rms`, so
        // the pipeline's allocation-free path measures loudness identically to the
        // recorder's slice path.
        let data = vec![0.1f32, -0.5, 0.3, 0.9, -0.2, 0.0, 0.7];
        assert_eq!(rms_iter(data.iter().copied()), rms(&data));
        // Empty streams agree at 0.0.
        assert_eq!(rms_iter(std::iter::empty::<f32>()), rms(&[]));
        assert_eq!(rms_iter(std::iter::empty::<f32>()), 0.0);
    }

    #[test]
    fn rms_of_full_scale_dc_is_one() {
        // Constant ±1.0 -> RMS 1.0.
        assert!((rms(&[1.0; 64]) - 1.0).abs() < 1e-6);
        assert!((rms(&[-1.0; 64]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn rms_of_unit_sine_is_about_root_half() {
        // A full-amplitude sine has RMS ≈ 1/√2 ≈ 0.7071.
        let n = 16_000;
        let sine: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 16_000.0).sin())
            .collect();
        assert!((rms(&sine) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }

    // ── f32_to_i16 ──

    #[test]
    fn f32_to_i16_clamps_above_and_below_full_scale() {
        // +2.0 and -2.0 must clamp to the i16 rails, never wrap.
        let out = f32_to_i16(&[2.0, -2.0]);
        assert_eq!(out, vec![32767, -32767]);
    }

    #[test]
    fn f32_to_i16_maps_zero_and_unit() {
        let out = f32_to_i16(&[0.0, 1.0, -1.0]);
        assert_eq!(out, vec![0, 32767, -32767]);
    }

    #[test]
    fn f32_to_i16_rounds_to_nearest() {
        // 0.5 * 32767 = 16383.5 -> rounds to 16384.
        let out = f32_to_i16(&[0.5]);
        assert_eq!(out, vec![16384]);
    }

    // ── PcmAudio ──

    #[test]
    fn pcm_audio_derives_duration_at_16k() {
        // 16 000 samples at 16 kHz = 1000 ms.
        let pcm = PcmAudio::from_samples_16k(vec![0i16; 16_000], false);
        assert_eq!(pcm.sample_rate, TARGET_SAMPLE_RATE);
        assert_eq!(pcm.duration_ms, 1000);
        assert!(!pcm.truncated);
    }
}
