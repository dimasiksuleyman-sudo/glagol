//! Voice-activity detection gate for hands-free dictation (Hands-free PR1).
//!
//! This module is the **decision core** of the hands-free feature: the piece
//! that replaces the finger on the push-to-talk key. It answers one question,
//! frame by frame — *is the user speaking right now, and did an utterance just
//! begin or end?* — and emits [`VadEvent`] transitions the continuous-listen
//! driver (PR2) turns into the same `Start`/`Released` signals the
//! [`pipeline`](crate::dictation::pipeline) already consumes.
//!
//! # Why it lives here and why it is pure
//!
//! Everything in [`VadGate`] is a pure state machine over a stream of
//! per-frame **linear RMS** values. No microphone, no `cpal`, no Tauri runtime,
//! no network — a synthetic sequence of RMS numbers drives it end to end, so
//! the whole hands-free control flow is unit-testable exactly like
//! [`super::rms`] and [`super::downmix_to_mono`]. The recorder already measures
//! one RMS value per [`super::LEVEL_WINDOW_MS`] window (PR2 D6); the driver
//! feeds those straight in.
//!
//! # Energy VAD first (kickoff D3)
//!
//! The MVP decides "voiced" by a single RMS threshold, reusing the whole-clip
//! silence calibration from the pipeline
//! ([`super::pipeline::SILENCE_RMS_THRESHOLD`], validated on day-10 signal
//! classes) as the per-frame cut. This adds **zero dependencies**. A neural
//! Silero VAD (robust to noise) is a later PR that swaps the `is_voiced`
//! decision for a model probability while keeping this exact state machine and
//! its event contract.
//!
//! # The two knobs that matter
//!
//! - **Onset debounce** ([`VadConfig::onset_frames`]): how many consecutive
//!   voiced frames must arrive before an utterance is declared. A single loud
//!   click must not open a session — the pipeline's [`super::pipeline::MIN_DICTATION_MS`]
//!   is the second line, but debouncing here keeps a stray transient from ever
//!   starting a recording.
//! - **Hangover** ([`VadConfig::hangover_ms`]): how much trailing silence ends
//!   an utterance. It must be long enough to ride over the natural pauses
//!   *inside* a sentence (breath, comma) without cutting the speaker off, and
//!   short enough that the text lands promptly after they stop. For a user who
//!   cannot press a key to correct a mistake, cutting mid-sentence is the worse
//!   failure — so the default leans generous.

/// A transition in the speech/silence state, emitted at most once per frame.
///
/// The driver (PR2) maps `SpeechStarted` to "begin an utterance segment" (open
/// the recorder / mark the pre-roll boundary) and `SpeechEnded` to "finalize
/// this segment and hand it to [`super::pipeline`]" — the hands-free
/// counterparts of the hotkey's `Pressed`/`Released`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    /// Enough consecutive voiced frames arrived: an utterance has begun.
    SpeechStarted,
    /// Enough trailing silence accumulated after speech: the utterance ended.
    SpeechEnded,
}

/// Consecutive voiced frames required to declare speech (kickoff D2 onset
/// debounce). At the recorder's 50 ms window that is 150 ms of sustained
/// energy — long enough to reject a click, short enough to feel instant.
pub const DEFAULT_ONSET_FRAMES: u32 = 3;

/// Trailing silence, in milliseconds, that ends an utterance (kickoff D2
/// hangover). 800 ms rides over in-sentence pauses (comma, breath) without
/// cutting the speaker, while still delivering promptly once they truly stop.
pub const DEFAULT_HANGOVER_MS: u32 = 800;

/// Per-frame linear-RMS cut for "voiced" (kickoff D3/D5). Reuses the pipeline's
/// day-10-calibrated whole-clip silence threshold so the VAD and the downstream
/// silence filter agree on what counts as sound. A frame at or above this is
/// voiced; below it is silence — mirroring the pipeline's `rms <
/// SILENCE_RMS_THRESHOLD` discard test so the two gates never disagree at the
/// boundary.
pub const DEFAULT_RMS_THRESHOLD: f32 = super::pipeline::SILENCE_RMS_THRESHOLD;

/// Tunable thresholds for [`VadGate`]. Defaults come from the `DEFAULT_*`
/// constants; PR3 will let the user relax `rms_threshold` for a quiet mic or
/// lengthen `hangover_ms` for slower speech.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VadConfig {
    /// Consecutive voiced frames required before [`VadEvent::SpeechStarted`].
    /// Clamped to a minimum of 1 by [`VadGate::new`] — 0 would fire on the very
    /// first frame regardless of energy, defeating the debounce.
    pub onset_frames: u32,
    /// Trailing silence (ms) after the last voiced frame before
    /// [`VadEvent::SpeechEnded`].
    pub hangover_ms: u32,
    /// Linear-RMS value at/above which a frame is voiced.
    pub rms_threshold: f32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            onset_frames: DEFAULT_ONSET_FRAMES,
            hangover_ms: DEFAULT_HANGOVER_MS,
            rms_threshold: DEFAULT_RMS_THRESHOLD,
        }
    }
}

/// Internal phase of the gate. Kept private — consumers observe transitions via
/// [`VadEvent`] and the current phase via [`VadGate::is_speaking`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Between utterances. `voiced_run` counts consecutive voiced frames toward
    /// the onset threshold; any silent frame resets it to 0.
    Silence { voiced_run: u32 },
    /// Inside an utterance. `trailing_silence_ms` accumulates since the last
    /// voiced frame; any voiced frame resets it to 0.
    Speech { trailing_silence_ms: u32 },
}

/// A frame-driven voice-activity gate (kickoff D2).
///
/// Feed it one linear-RMS value per audio frame with [`push_frame`]; it returns
/// `Some(event)` on the frames where speech begins or ends, and `None`
/// otherwise. The gate is deterministic and allocation-free: identical frame
/// sequences always produce identical event streams, which is what makes the
/// hands-free path testable without hardware.
///
/// [`push_frame`]: VadGate::push_frame
#[derive(Debug, Clone)]
pub struct VadGate {
    config: VadConfig,
    state: State,
}

impl VadGate {
    /// Build a gate with the given config. `onset_frames` is clamped to at least
    /// 1 so the debounce can never degenerate into "fire on the first frame".
    pub fn new(config: VadConfig) -> Self {
        let config = VadConfig {
            onset_frames: config.onset_frames.max(1),
            ..config
        };
        Self {
            config,
            state: State::Silence { voiced_run: 0 },
        }
    }

    /// The config in force (post-clamp), for the UI / tests.
    pub fn config(&self) -> VadConfig {
        self.config
    }

    /// Whether the gate currently considers the user to be mid-utterance.
    pub fn is_speaking(&self) -> bool {
        matches!(self.state, State::Speech { .. })
    }

    /// Return to the between-utterances phase, discarding any partial onset run
    /// or trailing-silence accumulation. The driver calls this when it tears the
    /// listener down (mode turned off, device lost) so a fresh session starts
    /// clean. Emits nothing.
    pub fn reset(&mut self) {
        self.state = State::Silence { voiced_run: 0 };
    }

    /// Feed one frame's linear RMS and its duration in milliseconds; return the
    /// transition it caused, if any.
    ///
    /// At most one event per call: a frame can either complete the onset run
    /// (→ [`VadEvent::SpeechStarted`]) or complete the hangover
    /// (→ [`VadEvent::SpeechEnded`]), never both. `frame_ms` is taken per call
    /// rather than baked into the config so a caller with a variable window (or
    /// a test) stays exact; in production it is [`super::LEVEL_WINDOW_MS`].
    pub fn push_frame(&mut self, rms: f32, frame_ms: u32) -> Option<VadEvent> {
        let voiced = rms >= self.config.rms_threshold;
        match &mut self.state {
            State::Silence { voiced_run } => {
                if voiced {
                    *voiced_run += 1;
                    if *voiced_run >= self.config.onset_frames {
                        self.state = State::Speech {
                            trailing_silence_ms: 0,
                        };
                        return Some(VadEvent::SpeechStarted);
                    }
                } else {
                    *voiced_run = 0;
                }
                None
            }
            State::Speech {
                trailing_silence_ms,
            } => {
                if voiced {
                    *trailing_silence_ms = 0;
                    None
                } else {
                    *trailing_silence_ms += frame_ms;
                    if *trailing_silence_ms >= self.config.hangover_ms {
                        self.state = State::Silence { voiced_run: 0 };
                        return Some(VadEvent::SpeechEnded);
                    }
                    None
                }
            }
        }
    }
}

impl Default for VadGate {
    fn default() -> Self {
        Self::new(VadConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A voiced RMS comfortably above the default threshold (normal speech read
    /// ~0.0164 on day-10 calibration).
    const LOUD: f32 = 0.02;
    /// A silent RMS below the default threshold (measured room floor ~0.00396).
    const QUIET: f32 = 0.001;
    /// Convenience: the recorder's real per-frame window.
    const FRAME: u32 = super::super::LEVEL_WINDOW_MS; // 50 ms

    /// Feed a whole sequence of `(rms, frame_ms)` frames and collect every event.
    fn drive(gate: &mut VadGate, frames: &[(f32, u32)]) -> Vec<VadEvent> {
        frames
            .iter()
            .filter_map(|&(rms, ms)| gate.push_frame(rms, ms))
            .collect()
    }

    // ── config / construction ──

    #[test]
    fn default_config_matches_documented_constants() {
        let c = VadConfig::default();
        assert_eq!(c.onset_frames, DEFAULT_ONSET_FRAMES);
        assert_eq!(c.hangover_ms, DEFAULT_HANGOVER_MS);
        assert_eq!(c.rms_threshold, DEFAULT_RMS_THRESHOLD);
    }

    #[test]
    fn default_rms_threshold_tracks_pipeline_silence_threshold() {
        // D3/D5: the VAD's voiced cut is the pipeline's silence threshold, so the
        // two gates never disagree about what is sound.
        assert_eq!(
            DEFAULT_RMS_THRESHOLD,
            super::super::pipeline::SILENCE_RMS_THRESHOLD
        );
    }

    #[test]
    fn onset_frames_zero_is_clamped_to_one() {
        // A 0 onset would fire on the first frame regardless of energy. The clamp
        // guarantees at least one voiced frame is required.
        let gate = VadGate::new(VadConfig {
            onset_frames: 0,
            ..VadConfig::default()
        });
        assert_eq!(gate.config().onset_frames, 1);
    }

    #[test]
    fn fresh_gate_is_not_speaking() {
        assert!(!VadGate::default().is_speaking());
    }

    // ── onset ──

    #[test]
    fn onset_needs_full_run_of_voiced_frames() {
        let mut gate = VadGate::default(); // onset_frames = 3
                                           // Two voiced frames: not yet enough.
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert!(!gate.is_speaking());
        // Third voiced frame trips onset.
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
        assert!(gate.is_speaking());
    }

    #[test]
    fn single_loud_frame_does_not_start_speech() {
        // A stray click (one loud frame) must never open a session (D2 onset
        // debounce — the whole point of the feature not mis-firing).
        let mut gate = VadGate::default();
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert_eq!(gate.push_frame(QUIET, FRAME), None);
        assert!(!gate.is_speaking());
    }

    #[test]
    fn broken_onset_run_resets_and_requires_a_fresh_run() {
        // voiced, voiced, SILENT (reset), then a fresh full run is required.
        let mut gate = VadGate::default(); // onset_frames = 3
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert_eq!(gate.push_frame(QUIET, FRAME), None); // resets voiced_run to 0
        assert_eq!(gate.push_frame(LOUD, FRAME), None); // run = 1
        assert_eq!(gate.push_frame(LOUD, FRAME), None); // run = 2
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
    }

    #[test]
    fn frame_exactly_at_threshold_is_voiced() {
        // Mirrors the pipeline: silent iff rms < threshold, so == threshold is
        // voiced. onset_frames = 1 to check the single-frame decision directly.
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            ..VadConfig::default()
        });
        assert_eq!(
            gate.push_frame(DEFAULT_RMS_THRESHOLD, FRAME),
            Some(VadEvent::SpeechStarted)
        );
    }

    #[test]
    fn frame_just_below_threshold_is_silence() {
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            ..VadConfig::default()
        });
        let just_below = DEFAULT_RMS_THRESHOLD - 0.0001;
        assert_eq!(gate.push_frame(just_below, FRAME), None);
        assert!(!gate.is_speaking());
    }

    // ── hangover / end of utterance ──

    #[test]
    fn short_pause_inside_speech_does_not_end_utterance() {
        // Start speaking, then a pause shorter than the 800 ms hangover must be
        // ridden over, not treated as end-of-utterance.
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            ..VadConfig::default()
        });
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
        // 700 ms of silence (14 × 50 ms) — under the 800 ms hangover.
        for _ in 0..14 {
            assert_eq!(gate.push_frame(QUIET, FRAME), None);
        }
        assert!(gate.is_speaking(), "must still be mid-utterance");
        // Speaking resumes → trailing silence resets.
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert!(gate.is_speaking());
    }

    #[test]
    fn sustained_silence_ends_utterance_at_hangover() {
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            ..VadConfig::default()
        });
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
        // 800 ms hangover / 50 ms frames = 16 silent frames; the 16th ends it.
        for _ in 0..15 {
            assert_eq!(gate.push_frame(QUIET, FRAME), None);
        }
        assert_eq!(gate.push_frame(QUIET, FRAME), Some(VadEvent::SpeechEnded));
        assert!(!gate.is_speaking());
    }

    #[test]
    fn hangover_boundary_is_inclusive() {
        // trailing_silence_ms >= hangover_ms ends the utterance: exactly at the
        // boundary must fire, one frame earlier must not.
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            hangover_ms: 100,
            ..VadConfig::default()
        });
        gate.push_frame(LOUD, FRAME);
        assert_eq!(gate.push_frame(QUIET, 50), None); // 50 ms < 100 ms
        assert_eq!(gate.push_frame(QUIET, 50), Some(VadEvent::SpeechEnded)); // 100 ms
    }

    #[test]
    fn pause_then_resume_accumulates_from_zero_again() {
        // A pause under hangover, speech, then a full silence must measure the
        // hangover from the *second* pause, not carry the first pause forward.
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            hangover_ms: 100,
            ..VadConfig::default()
        });
        gate.push_frame(LOUD, FRAME);
        assert_eq!(gate.push_frame(QUIET, 50), None); // 50 ms pause
        assert_eq!(gate.push_frame(LOUD, FRAME), None); // resets to 0
        assert_eq!(gate.push_frame(QUIET, 50), None); // 50 ms, fresh count
        assert_eq!(gate.push_frame(QUIET, 50), Some(VadEvent::SpeechEnded)); // 100 ms
    }

    // ── multi-utterance / reset ──

    #[test]
    fn a_second_utterance_can_start_after_the_first_ends() {
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            hangover_ms: 100,
            ..VadConfig::default()
        });
        // Utterance 1.
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
        assert_eq!(gate.push_frame(QUIET, 50), None);
        assert_eq!(gate.push_frame(QUIET, 50), Some(VadEvent::SpeechEnded));
        // Utterance 2 — the gate is reusable.
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
        assert!(gate.is_speaking());
    }

    #[test]
    fn reset_clears_a_partial_onset_run() {
        let mut gate = VadGate::default(); // onset_frames = 3
        gate.push_frame(LOUD, FRAME); // run = 1
        gate.push_frame(LOUD, FRAME); // run = 2
        gate.reset();
        // The two prior voiced frames are forgotten: a fresh full run is needed.
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert_eq!(gate.push_frame(LOUD, FRAME), None);
        assert_eq!(gate.push_frame(LOUD, FRAME), Some(VadEvent::SpeechStarted));
    }

    #[test]
    fn reset_mid_utterance_returns_to_silence_without_event() {
        let mut gate = VadGate::new(VadConfig {
            onset_frames: 1,
            ..VadConfig::default()
        });
        gate.push_frame(LOUD, FRAME);
        assert!(gate.is_speaking());
        gate.reset();
        assert!(!gate.is_speaking());
    }

    // ── integration: a realistic utterance ──

    #[test]
    fn full_utterance_emits_started_then_ended_exactly_once() {
        // silence → onset → speech with an inner pause → trailing silence → end.
        let mut gate = VadGate::default(); // onset 3, hangover 800
        let mut seq: Vec<(f32, u32)> = Vec::new();
        seq.extend([(QUIET, FRAME); 4]); // ambient silence
        seq.extend([(LOUD, FRAME); 20]); // "привет, "
        seq.extend([(QUIET, FRAME); 10]); // 500 ms comma pause (< hangover)
        seq.extend([(LOUD, FRAME); 20]); // "мир"
        seq.extend([(QUIET, FRAME); 20]); // 1000 ms trailing silence (> hangover)

        let events = drive(&mut gate, &seq);
        assert_eq!(
            events,
            vec![VadEvent::SpeechStarted, VadEvent::SpeechEnded],
            "one clean utterance yields exactly one Started and one Ended"
        );
        assert!(!gate.is_speaking(), "utterance closed");
    }

    #[test]
    fn pure_silence_never_emits() {
        let mut gate = VadGate::default();
        let events = drive(&mut gate, &[(QUIET, FRAME); 200]);
        assert!(events.is_empty(), "silence must never start a session");
        assert!(!gate.is_speaking());
    }
}
