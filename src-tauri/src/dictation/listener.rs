//! Continuous-listen driver for hands-free dictation (Hands-free PR2).
//!
//! Push-to-talk gives the pipeline a `Start`/`Released` pair from a finger on a
//! key. The hands-free user has no finger to give — so this module manufactures
//! the same two signals from **the voice itself**. It consumes a continuous
//! stream of captured [`Frame`]s, runs each frame's RMS through a
//! [`VadGate`](super::vad::VadGate), and segments the stream into utterances:
//! on speech onset it opens a segment, on trailing silence it finalizes one and
//! hands the clip to [`deliver_clip`](super::pipeline::deliver_clip) — the exact
//! delivery path push-to-talk uses.
//!
//! # Pre-roll: never clip the first word (kickoff D4)
//!
//! Speech onset is only *detected* after a few voiced frames (the VAD debounce),
//! by which point the first ~150 ms of the word has already played. A user who
//! cannot press a key also cannot easily repeat themselves, so losing the start
//! of a sentence is the worst failure this feature has. [`ListenerCore`] keeps a
//! small ring buffer of the most recent frames while idle ([`PRE_ROLL_MS`]); when
//! onset fires, that buffer **seeds** the segment, so the utterance begins before
//! the VAD noticed it.
//!
//! # Trailing-silence trim
//!
//! The hangover that ends an utterance is, by definition, silence. Keeping it in
//! the clip only dilutes the whole-clip RMS (risking the pipeline's silence
//! filter) and feeds the model dead air. The segment is truncated back to the
//! end of the last voiced frame before finalizing — leading pre-roll is kept
//! (anti-clip), trailing hangover is dropped (clean clip).
//!
//! # Headless by construction
//!
//! Nothing here touches `cpal`. Frames arrive through the [`FrameSource`] seam;
//! in production that seam is fed by the recorder's capture thread (PR3), in
//! tests by a `Vec` of synthetic frames. The whole hands-free control flow —
//! onset, pre-roll, segmentation, delivery — is therefore exercised without a
//! microphone, exactly like the pipeline and the VAD gate.

use std::collections::VecDeque;

use super::insert::{ClipboardAccess, InsertionMode, TextInserter};
use super::pipeline::{deliver_clip, set_phase, DeliverDeps, DictationEmitter, DictationState};
use super::vad::{VadConfig, VadEvent, VadGate};
use super::{DictationPhase, PcmAudio, MAX_RECORDING_MS};
use crate::stt::SttProvider;

/// How much audio before speech onset to retain and prepend to the utterance
/// (kickoff D4). 400 ms comfortably covers the VAD's onset debounce plus the
/// soft attack of the first syllable, so no word is ever clipped.
pub const PRE_ROLL_MS: u32 = 400;

/// One window of captured audio handed to the listener, already normalized to
/// the recorder's output invariant (16 kHz mono S16LE). The capture layer (PR3)
/// produces one of these per [`super::LEVEL_WINDOW_MS`] window; tests build them
/// directly. RMS is derived here, not carried, so the seam stays minimal.
#[derive(Debug, Clone)]
pub struct Frame {
    /// 16 kHz mono signed-16-bit samples for this window.
    pub samples: Vec<i16>,
    /// Window duration in milliseconds (production: [`super::LEVEL_WINDOW_MS`]).
    pub duration_ms: u32,
}

/// The signal the listener produces per frame — the hands-free counterparts of
/// the hotkey's `Pressed`/`Released`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListenerEvent {
    /// An utterance began: [`run_listener`] shows the `Recording` state.
    UtteranceStarted,
    /// An utterance finished; here is the finalized clip (pre-roll + speech, with
    /// trailing hangover trimmed), ready for delivery.
    UtteranceEnded(PcmAudio),
}

/// Linear RMS of a frame's `i16` samples, reinterpreted as `f32` in `[-1, 1)` —
/// identical to the pipeline's `clip_rms` so the VAD and the downstream silence
/// filter measure loudness the same way.
fn frame_rms(samples: &[i16]) -> f32 {
    super::rms_iter(samples.iter().map(|&s| s as f32 / 32768.0))
}

/// The segmentation state machine (kickoff D4). Pure and deterministic: a given
/// frame sequence always yields the same events and the same clips, so the whole
/// hands-free path is unit-testable without hardware.
pub struct ListenerCore {
    vad: VadGate,
    pre_roll_ms: u32,
    /// Recent frames retained while idle, capped by [`Self::pre_roll_ms`].
    pre_roll: VecDeque<Frame>,
    pre_roll_ms_acc: u32,
    /// `true` between onset and end-of-utterance.
    in_segment: bool,
    /// Accumulated 16 kHz mono samples for the current utterance.
    segment: Vec<i16>,
    segment_ms: u32,
    /// Sample index just past the last voiced frame — the trim point that drops
    /// trailing hangover silence.
    voiced_end: usize,
    /// Per-utterance hard cap (kickoff D11), mirroring the recorder's 60 s cap.
    max_utterance_ms: u32,
}

impl ListenerCore {
    /// Build a listener from a VAD config and a pre-roll window.
    pub fn new(vad_config: VadConfig, pre_roll_ms: u32) -> Self {
        Self {
            vad: VadGate::new(vad_config),
            pre_roll_ms,
            pre_roll: VecDeque::new(),
            pre_roll_ms_acc: 0,
            in_segment: false,
            segment: Vec::new(),
            segment_ms: 0,
            voiced_end: 0,
            max_utterance_ms: MAX_RECORDING_MS,
        }
    }

    /// Whether an utterance is currently being captured.
    pub fn is_capturing(&self) -> bool {
        self.in_segment
    }

    /// Feed one captured frame; return the segmentation event it caused, if any.
    ///
    /// While idle the frame joins the pre-roll ring buffer and its RMS drives the
    /// VAD; on onset the pre-roll seeds a new segment. While capturing the frame
    /// is appended and the VAD is checked for end-of-utterance, with the 60 s cap
    /// as a hard backstop.
    pub fn push_frame(&mut self, frame: Frame) -> Option<ListenerEvent> {
        let rms = frame_rms(&frame.samples);
        let voiced = rms >= self.vad.config().rms_threshold;

        if !self.in_segment {
            self.push_pre_roll(frame);
            match self.vad.push_frame(
                rms,
                self.pre_roll.back().map(|f| f.duration_ms).unwrap_or(0),
            ) {
                Some(VadEvent::SpeechStarted) => {
                    self.begin_segment();
                    Some(ListenerEvent::UtteranceStarted)
                }
                _ => None,
            }
        } else {
            let frame_ms = frame.duration_ms;
            self.segment.extend_from_slice(&frame.samples);
            self.segment_ms += frame_ms;
            if voiced {
                self.voiced_end = self.segment.len();
            }

            // Hard per-utterance cap (D11): finalize what we have rather than run
            // forever, marking it truncated so the UI can say «обрезано по 60 с».
            if self.segment_ms >= self.max_utterance_ms {
                return Some(ListenerEvent::UtteranceEnded(self.finalize(true)));
            }

            match self.vad.push_frame(rms, frame_ms) {
                Some(VadEvent::SpeechEnded) => {
                    Some(ListenerEvent::UtteranceEnded(self.finalize(false)))
                }
                _ => None,
            }
        }
    }

    /// Append a frame to the pre-roll ring buffer, evicting the oldest frames
    /// until the retained window fits inside [`Self::pre_roll_ms`]. At least one
    /// frame is always kept (the one just pushed).
    fn push_pre_roll(&mut self, frame: Frame) {
        self.pre_roll_ms_acc += frame.duration_ms;
        self.pre_roll.push_back(frame);
        while self.pre_roll_ms_acc > self.pre_roll_ms && self.pre_roll.len() > 1 {
            if let Some(old) = self.pre_roll.pop_front() {
                self.pre_roll_ms_acc = self.pre_roll_ms_acc.saturating_sub(old.duration_ms);
            }
        }
    }

    /// Seed a fresh segment from the pre-roll (so the first word survives) and
    /// enter the capturing state. The seed ends on the onset frames, which are
    /// voiced, so `voiced_end` starts at the full seeded length.
    fn begin_segment(&mut self) {
        self.in_segment = true;
        self.segment.clear();
        self.segment_ms = 0;
        for f in self.pre_roll.drain(..) {
            self.segment.extend_from_slice(&f.samples);
            self.segment_ms += f.duration_ms;
        }
        self.pre_roll_ms_acc = 0;
        self.voiced_end = self.segment.len();
    }

    /// Trim trailing hangover silence, build the clip, and reset for the next
    /// utterance. Leading pre-roll is preserved; only the silence after the last
    /// voiced frame is dropped.
    fn finalize(&mut self, truncated: bool) -> PcmAudio {
        let end = self.voiced_end.min(self.segment.len());
        let samples = self.segment[..end].to_vec();

        self.in_segment = false;
        self.segment.clear();
        self.segment_ms = 0;
        self.voiced_end = 0;
        self.pre_roll.clear();
        self.pre_roll_ms_acc = 0;
        // The SpeechEnded path already reset the VAD; the cap path did not. Reset
        // unconditionally so the next utterance starts from clean silence.
        self.vad.reset();

        PcmAudio::from_samples_16k(samples, truncated)
    }
}

impl Default for ListenerCore {
    fn default() -> Self {
        Self::new(VadConfig::default(), PRE_ROLL_MS)
    }
}

/// The source of captured frames the listener consumes (kickoff D2 seam).
///
/// Production feeds this from the recorder's continuous capture thread (PR3);
/// tests feed it from a `Vec`. `next_frame` resolves `None` when the stream ends
/// (device closed, hands-free mode turned off), which stops [`run_listener`].
#[allow(async_fn_in_trait)]
pub trait FrameSource {
    /// Await the next captured frame, or `None` when capture has stopped.
    async fn next_frame(&mut self) -> Option<Frame>;
}

/// Drive the hands-free loop: pull frames, segment them, deliver each utterance.
///
/// Runs until the [`FrameSource`] is exhausted. On `UtteranceStarted` it claims
/// the shared `Recording` phase and shows the pill (the same signal the hotkey's
/// `Pressed` emits); on `UtteranceEnded` it delivers the clip through the shared
/// [`deliver_clip`] path (silence filter → transcribe → insert → terminal event).
pub async fn run_listener<S, P, I, C, E>(
    mut source: S,
    core: &mut ListenerCore,
    deps: &DeliverDeps<'_, P, I, C, E>,
    lang: Option<&str>,
    mode: InsertionMode,
) where
    S: FrameSource,
    P: SttProvider,
    I: TextInserter,
    C: ClipboardAccess,
    E: DictationEmitter,
{
    while let Some(frame) = source.next_frame().await {
        match core.push_frame(frame) {
            Some(ListenerEvent::UtteranceStarted) => {
                set_phase(deps.phase, DictationPhase::Recording);
                deps.emitter.emit_state(DictationState::Recording);
            }
            Some(ListenerEvent::UtteranceEnded(pcm)) => {
                deliver_clip(deps, pcm, lang, mode).await;
            }
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::insert::fakes::{FakeClipboard, FakeInserter};
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::stt::{SttError, Transcript};

    const FRAME_MS: u32 = super::super::LEVEL_WINDOW_MS; // 50 ms
    /// Amplitude giving RMS ≈ 0.02 (normal speech), well above the 0.005 cut.
    const LOUD: i16 = 655;
    /// Amplitude giving RMS ≈ 0.001 (room floor), below the cut.
    const QUIET: i16 = 32;

    /// A frame of constant `amp` lasting `ms` at 16 kHz.
    fn frame(amp: i16, ms: u32) -> Frame {
        let n = (super::super::TARGET_SAMPLE_RATE as u64 * ms as u64 / 1000) as usize;
        Frame {
            samples: vec![amp; n],
            duration_ms: ms,
        }
    }

    /// Push a run of identical frames, collecting every event.
    fn push_run(core: &mut ListenerCore, amp: i16, count: usize) -> Vec<ListenerEvent> {
        (0..count)
            .filter_map(|_| core.push_frame(frame(amp, FRAME_MS)))
            .collect()
    }

    // ── ListenerCore: segmentation ──

    #[test]
    fn pure_silence_never_segments() {
        let mut core = ListenerCore::default();
        assert!(push_run(&mut core, QUIET, 200).is_empty());
        assert!(!core.is_capturing());
    }

    #[test]
    fn a_single_loud_frame_does_not_open_a_segment() {
        let mut core = ListenerCore::default(); // onset 3 frames
        assert!(core.push_frame(frame(LOUD, FRAME_MS)).is_none());
        assert!(!core.is_capturing());
    }

    #[test]
    fn onset_after_debounce_emits_started_and_captures() {
        let mut core = ListenerCore::default(); // onset 3
        assert_eq!(core.push_frame(frame(LOUD, FRAME_MS)), None);
        assert_eq!(core.push_frame(frame(LOUD, FRAME_MS)), None);
        assert_eq!(
            core.push_frame(frame(LOUD, FRAME_MS)),
            Some(ListenerEvent::UtteranceStarted)
        );
        assert!(core.is_capturing());
    }

    #[test]
    fn full_utterance_trims_hangover_and_keeps_pre_roll() {
        // 8 quiet (pre-roll fill) → 15 loud (speech) → 16 quiet (800 ms hangover).
        // Onset fires on the 3rd loud frame; the segment is seeded from the 400 ms
        // (8-frame) pre-roll = [5 quiet, 3 loud], then 12 more loud append, then
        // the trailing 16 quiet frames are trimmed. Result: 5 quiet + 15 loud =
        // 20 frames × 50 ms = 1000 ms.
        let mut core = ListenerCore::default();
        push_run(&mut core, QUIET, 8);
        let started = push_run(&mut core, LOUD, 15);
        assert_eq!(started, vec![ListenerEvent::UtteranceStarted]);
        let ended = push_run(&mut core, QUIET, 16);
        assert_eq!(ended.len(), 1, "exactly one end event");
        let ListenerEvent::UtteranceEnded(pcm) = &ended[0] else {
            panic!("expected UtteranceEnded, got {:?}", ended[0]);
        };
        assert!(!pcm.truncated);
        assert_eq!(
            pcm.duration_ms, 1000,
            "pre-roll (250 ms) + speech (750 ms), hangover trimmed"
        );
        assert!(!core.is_capturing(), "listener idle after end");
    }

    #[test]
    fn pre_roll_is_bounded_by_the_window() {
        // A long lead-in of silence must not bloat the clip: only the last 400 ms
        // (8 frames) of pre-roll may survive into the segment.
        let mut core = ListenerCore::default();
        push_run(&mut core, QUIET, 100); // 5 s of silence — most must be evicted
        push_run(&mut core, LOUD, 10); // speech
        let ended = push_run(&mut core, QUIET, 16); // hangover → end
        let ListenerEvent::UtteranceEnded(pcm) = &ended[0] else {
            panic!("expected end");
        };
        // 400 ms pre-roll cap = 8 frames; of those the last 3 are loud (onset), so
        // 5 quiet + 10 loud = 15 frames × 50 ms = 750 ms. Not 5 s + speech.
        assert_eq!(pcm.duration_ms, 750, "pre-roll capped at 400 ms");
    }

    #[test]
    fn two_utterances_segment_independently() {
        let mut core = ListenerCore::default();
        // Utterance 1.
        push_run(&mut core, QUIET, 8);
        assert_eq!(
            push_run(&mut core, LOUD, 10),
            vec![ListenerEvent::UtteranceStarted]
        );
        let end1 = push_run(&mut core, QUIET, 16);
        assert!(matches!(
            end1.as_slice(),
            [ListenerEvent::UtteranceEnded(_)]
        ));
        // Utterance 2 — fresh, not contaminated by the first.
        assert_eq!(
            push_run(&mut core, LOUD, 10),
            vec![ListenerEvent::UtteranceStarted]
        );
        let end2 = push_run(&mut core, QUIET, 16);
        let ListenerEvent::UtteranceEnded(pcm) = &end2[0] else {
            panic!("expected end");
        };
        // Second clip: pre-roll here is the trailing hangover of nothing (cleared
        // on finalize) refilled by the fresh loud run — 3 onset loud already in
        // pre-roll + 7 more = 10 loud frames = 500 ms, no stale utterance-1 audio.
        assert_eq!(pcm.duration_ms, 500);
    }

    #[test]
    fn inner_pause_under_hangover_does_not_split_the_utterance() {
        let mut core = ListenerCore::default();
        push_run(&mut core, QUIET, 8);
        push_run(&mut core, LOUD, 10); // starts
                                       // 700 ms pause (14 frames) — under the 800 ms hangover, no end.
        assert!(push_run(&mut core, QUIET, 14).is_empty());
        assert!(core.is_capturing(), "still one utterance");
        push_run(&mut core, LOUD, 10); // resume
        let ended = push_run(&mut core, QUIET, 16); // now end
        assert!(matches!(
            ended.as_slice(),
            [ListenerEvent::UtteranceEnded(_)]
        ));
    }

    #[test]
    fn utterance_hitting_the_cap_finalizes_truncated() {
        // Continuous speech past the 60 s cap must finalize with truncated = true
        // rather than run forever (D11).
        let mut core = ListenerCore::new(VadConfig::default(), PRE_ROLL_MS);
        push_run(&mut core, QUIET, 8);
        push_run(&mut core, LOUD, 3); // onset
                                      // 60_000 ms / 50 ms = 1200 loud frames; the cap trips inside this run.
        let events = push_run(&mut core, LOUD, 1300);
        let truncated = events
            .iter()
            .any(|e| matches!(e, ListenerEvent::UtteranceEnded(pcm) if pcm.truncated));
        assert!(truncated, "the 60 s cap must force a truncated finalize");
        // Continuous speech past the cap immediately re-onsets into a fresh
        // segment — long dictation is chunked, not lost — so the listener is
        // capturing again. Flushing trailing silence closes that segment and
        // returns to idle.
        assert!(
            core.is_capturing(),
            "speech continued → a new segment opened"
        );
        push_run(&mut core, QUIET, 16);
        assert!(
            !core.is_capturing(),
            "trailing silence closes the second segment"
        );
    }

    // ── run_listener: end-to-end over seams ──

    /// A provider returning a canned transcript.
    struct FakeStt {
        text: String,
        calls: Arc<Mutex<u32>>,
    }
    impl SttProvider for FakeStt {
        async fn transcribe(
            &self,
            _wav: Vec<u8>,
            _lang: Option<&str>,
        ) -> Result<Transcript, SttError> {
            *self.calls.lock().unwrap() += 1;
            Ok(Transcript {
                text: self.text.clone(),
            })
        }
        async fn list_models(&self) -> Result<Vec<String>, SttError> {
            Ok(vec![])
        }
    }

    #[derive(Clone, Default)]
    struct FakeEmitter {
        states: Arc<Mutex<Vec<DictationState>>>,
    }
    impl DictationEmitter for FakeEmitter {
        fn emit_state(&self, state: DictationState) {
            self.states.lock().unwrap().push(state);
        }
    }

    struct VecSource {
        frames: VecDeque<Frame>,
    }
    impl VecSource {
        fn new(frames: Vec<Frame>) -> Self {
            Self {
                frames: frames.into(),
            }
        }
    }
    impl FrameSource for VecSource {
        async fn next_frame(&mut self) -> Option<Frame> {
            self.frames.pop_front()
        }
    }

    /// Build the frame list for one clean spoken utterance.
    fn one_utterance_frames() -> Vec<Frame> {
        let mut v = Vec::new();
        v.extend((0..8).map(|_| frame(QUIET, FRAME_MS)));
        v.extend((0..15).map(|_| frame(LOUD, FRAME_MS)));
        v.extend((0..16).map(|_| frame(QUIET, FRAME_MS)));
        v
    }

    #[tokio::test]
    async fn run_listener_delivers_one_spoken_utterance() {
        let stt = FakeStt {
            text: "привет из голоса".into(),
            calls: Arc::new(Mutex::new(0)),
        };
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);
        let deps = DeliverDeps {
            provider: &stt,
            inserter: &inserter,
            clipboard: &clipboard,
            emitter: &emitter,
            phase: &phase,
        };
        let mut core = ListenerCore::default();

        run_listener(
            VecSource::new(one_utterance_frames()),
            &mut core,
            &deps,
            Some("ru"),
            InsertionMode::ClipboardOnly,
        )
        .await;

        assert_eq!(
            *stt.calls.lock().unwrap(),
            1,
            "one utterance → one transcribe"
        );
        assert_eq!(clipboard.content(), Some("привет из голоса".to_string()));
        let states = emitter.states.lock().unwrap().clone();
        assert_eq!(
            states,
            vec![
                DictationState::Recording,
                DictationState::Processing,
                DictationState::Done {
                    disposition: super::super::pipeline::Disposition::Clipboard,
                    truncated: false,
                },
            ],
            "hands-free must drive the same overlay states as push-to-talk"
        );
        assert_eq!(*phase.lock().unwrap(), DictationPhase::Idle, "phase reset");
    }

    #[tokio::test]
    async fn run_listener_on_pure_silence_delivers_nothing() {
        let stt = FakeStt {
            text: "не должно вызваться".into(),
            calls: Arc::new(Mutex::new(0)),
        };
        let inserter = FakeInserter::default();
        let clipboard = FakeClipboard::empty();
        let emitter = FakeEmitter::default();
        let phase = Mutex::new(DictationPhase::Idle);
        let deps = DeliverDeps {
            provider: &stt,
            inserter: &inserter,
            clipboard: &clipboard,
            emitter: &emitter,
            phase: &phase,
        };
        let mut core = ListenerCore::default();

        let silence: Vec<Frame> = (0..200).map(|_| frame(QUIET, FRAME_MS)).collect();
        run_listener(
            VecSource::new(silence),
            &mut core,
            &deps,
            None,
            InsertionMode::ClipboardOnly,
        )
        .await;

        assert_eq!(*stt.calls.lock().unwrap(), 0, "silence must not transcribe");
        assert_eq!(clipboard.content(), None, "nothing delivered");
        assert!(
            emitter.states.lock().unwrap().is_empty(),
            "no state changes"
        );
    }
}
