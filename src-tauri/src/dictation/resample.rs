//! Offline resampling of a captured mono buffer down to 16 kHz (D8).
//!
//! Runs once, on finalization, over the whole recording — quality under
//! Whisper is more than sufficient and `FftFixedIn` is faster than the sinc
//! resamplers. Two subtleties are load-bearing and both are implemented here:
//!
//! 1. **Zero-padded tail.** The last partial input chunk is padded up to a full
//!    fixed chunk so its frames actually enter the resampler; the output is
//!    then trimmed back to the ideal length, which also removes the silence the
//!    padding introduced. Without the trim you get a click / stub of silence at
//!    the end.
//! 2. **Filter-latency flush.** An FFT resampler holds its last ~`output_delay()`
//!    frames inside the filter when the input ends. A final `process_partial(None)`
//!    drain pushes them out; without it the end of the last word is swallowed —
//!    and the end of a dictated phrase carries the meaning («…в четверг» →
//!    «…в чет»).
//!
//! When the source is already 16 kHz the resampler is never constructed
//! ([identity path](resample_to_16k)).
//!
//! ## Deviation note (kickoff D1 vs D8)
//!
//! The kickoff pinned `rubato = 3.0.0`, but its pseudocode targets the
//! `FftFixedIn` + `process_partial` + `output_delay` API that rubato dropped in
//! 1.0.0. We use `rubato = 0.16.2` — the newest release that still has exactly
//! that API — so the design below is D8 verbatim rather than a reinterpretation
//! over 3.0's `audioadapter` buffers. See `Cargo.toml`.

use rubato::{FftFixedIn, Resampler};

use super::{RecorderError, TARGET_SAMPLE_RATE};

/// Fixed input chunk size fed to the resampler, in frames.
const CHUNK_SIZE_IN: usize = 1024;
/// FFT sub-chunk hint (efficiency only; the resampler may use a different
/// actual value).
const SUB_CHUNKS: usize = 2;
/// Safety bound on the flush loop — `process_partial(None)` terminates on the
/// first empty batch; this only guards against a pathological non-terminating
/// drain.
const MAX_FLUSH_ROUNDS: usize = 16;

/// Resample a **mono** `f32` buffer captured at `from_rate` down to
/// [`TARGET_SAMPLE_RATE`] (16 kHz).
///
/// Returns samples in `[-1.0, 1.0]` at 16 kHz. Every fallible step is audited
/// (D8-A): the constructor, each `process` / `process_partial`, and flush
/// convergence surface a **loud [`RecorderError`]** — there is no silent
/// degradation path. In particular the old "constructor failed → return the
/// native-rate buffer" fallback is gone: returning native-rate samples labelled
/// 16 kHz would feed Whisper slowed audio, which is worse than failing.
///
/// The identity path (source already 16 kHz) and an empty buffer are infallible
/// in fact; they still return `Ok` so the signature is uniform.
pub fn resample_to_16k(mono: &[f32], from_rate: u32) -> Result<Vec<f32>, RecorderError> {
    // Identity path: already at the target rate — infallible (D8).
    if from_rate == TARGET_SAMPLE_RATE {
        return Ok(mono.to_vec());
    }
    // Empty input — infallible.
    if mono.is_empty() {
        return Ok(Vec::new());
    }

    // Ideal converted length — the trim target that strips the zero-pad tail.
    let expected_len =
        (mono.len() as f64 * TARGET_SAMPLE_RATE as f64 / from_rate as f64).ceil() as usize;

    // Constructor: fails only on a nonsensical rate (e.g. 0). Loud Err — a clip
    // left at the native rate but tagged 16 kHz would silently slow STT (D8-A).
    let mut resampler = FftFixedIn::<f32>::new(
        from_rate as usize,
        TARGET_SAMPLE_RATE as usize,
        CHUNK_SIZE_IN,
        SUB_CHUNKS,
        1, // mono
    )
    .map_err(|e| {
        RecorderError::UnsupportedConfig(format!("resampler init failed for {from_rate} Hz: {e}"))
    })?;

    let mut out: Vec<f32> = Vec::with_capacity(expected_len + CHUNK_SIZE_IN);

    // 1. Every full fixed-size input chunk. `process` errors only on a wrong
    //    channel/frame count, which we control — surface it rather than emit a
    //    silently short clip (D8-A).
    let mut pos = 0usize;
    loop {
        let need = resampler.input_frames_next();
        if pos + need > mono.len() {
            break;
        }
        let processed = resampler
            .process(&[&mono[pos..pos + need]], None)
            .map_err(resample_err)?;
        out.extend_from_slice(&processed[0]);
        pos += need;
    }

    // 2. Zero-pad the trailing partial chunk so its frames enter the resampler
    //    (trap 1). The overall trim in step 4 removes the padding again.
    if pos < mono.len() {
        let need = resampler.input_frames_next();
        let mut last = vec![0.0f32; need];
        let remaining = &mono[pos..];
        last[..remaining.len()].copy_from_slice(remaining);
        let processed = resampler
            .process(&[last.as_slice()], None)
            .map_err(resample_err)?;
        out.extend_from_slice(&processed[0]);
    }

    // 3. Flush the filter's delay line — without this the end of the last word
    //    is eaten (trap 2). `process_partial(None)` keeps emitting the delayed
    //    frames; for `FftFixedIn` it does not signal "done" with an empty batch,
    //    so the empty-break is only an early-out and exhausting MAX_FLUSH_ROUNDS
    //    is harmless — step 4 trims the surplus. Each call is still fallible and
    //    surfaced loudly (D8-A).
    for _ in 0..MAX_FLUSH_ROUNDS {
        let processed = resampler
            .process_partial::<&[f32]>(None, None)
            .map_err(resample_err)?;
        if processed[0].is_empty() {
            break;
        }
        out.extend_from_slice(&processed[0]);
    }

    // 4. Guard against a silently short clip (D8-A): fewer than the ideal number
    //    of frames means the tail was lost (e.g. the flush was removed). That is
    //    a loud Err, never a quiet truncation.
    if out.len() < expected_len {
        return Err(RecorderError::UnsupportedConfig(format!(
            "resample produced {} of {expected_len} frames at {from_rate} Hz (tail lost)",
            out.len()
        )));
    }

    // 5. Trim to the ideal length: drops the zero-pad-induced tail while
    //    keeping the flushed real frames that complete the final word.
    out.truncate(expected_len);
    Ok(out)
}

/// Map a rubato per-chunk failure to the recorder taxonomy (D8-A). Reachable
/// only on an internal contract violation (wrong channel/frame count), which we
/// do not produce — but it is surfaced loudly rather than swallowed.
fn resample_err(e: rubato::ResampleError) -> RecorderError {
    RecorderError::UnsupportedConfig(format!("resample failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Count sign changes — a lightweight, dependency-free proxy for "the
    /// dominant frequency survived resampling". A clean `f` Hz tone over `d`
    /// seconds has ≈ `2 * f * d` zero crossings regardless of sample rate; a
    /// broken resample ratio (aliasing to another frequency) moves this count.
    fn zero_crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count()
    }

    fn sine(freq: f32, rate: u32, samples: usize) -> Vec<f32> {
        (0..samples)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate as f32).sin())
            .collect()
    }

    #[test]
    fn identity_path_returns_copy_unchanged() {
        let input = sine(1000.0, 16_000, 16_000);
        let out = resample_to_16k(&input, 16_000).unwrap();
        assert_eq!(out, input, "16 kHz input must pass through untouched");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(resample_to_16k(&[], 48_000).unwrap().is_empty());
    }

    #[test]
    fn zero_from_rate_is_a_loud_error() {
        // D8-A negative cycle: from_rate = 0 makes the resampler constructor
        // fail, which must surface as a loud Err — never a native-rate clip
        // silently tagged 16 kHz. Real rates succeed.
        assert!(matches!(
            resample_to_16k(&[0.1, 0.2, 0.3], 0),
            Err(RecorderError::UnsupportedConfig(_))
        ));
        assert!(resample_to_16k(&[0.1; 48_000], 48_000).is_ok());
        assert!(resample_to_16k(&[0.1; 44_100], 44_100).is_ok());
    }

    #[test]
    fn resamples_48k_to_16k_expected_length() {
        // 1.0 s at 48 kHz -> exactly 16 000 frames at 16 kHz.
        let input = sine(1000.0, 48_000, 48_000);
        let out = resample_to_16k(&input, 48_000).unwrap();
        assert_eq!(out.len(), 16_000, "48k->16k of 1.0 s must be 16 000 frames");
    }

    #[test]
    fn resamples_44100_to_16k_expected_length() {
        // 1.0 s at 44.1 kHz -> 16 000 frames (ceil).
        let input = sine(1000.0, 44_100, 44_100);
        let out = resample_to_16k(&input, 44_100).unwrap();
        assert_eq!(
            out.len(),
            16_000,
            "44.1k->16k of 1.0 s must be 16 000 frames"
        );
    }

    #[test]
    fn resample_preserves_dominant_frequency() {
        // A 1 kHz tone stays a 1 kHz tone. Count crossings over the clean
        // steady-state middle only — the leading ~170-sample filter warmup adds
        // near-zero noise crossings that would blur the check. 14 400 samples at
        // 16 kHz = 900 cycles of 1 kHz → exactly 1800 crossings.
        let input = sine(1000.0, 48_000, 48_000);
        let out = resample_to_16k(&input, 48_000).unwrap();
        let steady = &out[800..15_200];
        let zc = zero_crossings(steady);
        assert!(
            (1780..=1820).contains(&zc),
            "1 kHz tone should have ~1800 zero crossings in its steady region, got {zc}"
        );
    }

    /// Negative regression cycle for the filter-latency flush (D8, project
    /// convention). This test proves the flush in step 3 of [`resample_to_16k`]
    /// is load-bearing:
    ///
    /// 1. Fix in place (flush present).
    /// 2. This test asserts (a) the output is exactly 16 000 frames and (c) the
    ///    last 50 ms still carries the tone.
    /// 3. Set `MAX_FLUSH_ROUNDS` to `0` (or delete the flush block).
    /// 4. Re-run: the delayed frames are never drained, so `out` is short of
    ///    `expected_len` and the D8-A length guard returns `Err` (tail lost) —
    ///    `unwrap` panics and the test fails. (Before D8-A this was a silently
    ///    short buffer; now it is a loud error.)
    /// 5. Restore the flush; the test passes again.
    ///
    /// Verified manually via that comment-out cycle.
    #[test]
    fn flush_keeps_the_tail_of_the_last_word() {
        let input = sine(1000.0, 48_000, 48_000);
        let out = resample_to_16k(&input, 48_000).unwrap();

        // (a) full length — this is what breaks when the flush is removed.
        assert_eq!(
            out.len(),
            16_000,
            "without the filter-latency flush the output is short of 16 000 frames"
        );

        // (c) the last 50 ms (800 frames at 16 kHz) still carries the tone,
        // i.e. the end of the phrase was not swallowed.
        let tail = &out[out.len() - 800..];
        let tail_rms = super::super::rms(tail);
        assert!(
            tail_rms > 0.5,
            "last 50 ms RMS should be ≈ 0.707 for a full-scale tone, got {tail_rms}"
        );
    }
}
