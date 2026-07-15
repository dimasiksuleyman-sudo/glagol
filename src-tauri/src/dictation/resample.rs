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

use super::TARGET_SAMPLE_RATE;

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
/// Returns samples in `[-1.0, 1.0]` at 16 kHz. The identity path (source
/// already 16 kHz) returns a copy without touching rubato. On the (practically
/// impossible) resampler-construction failure the input is returned unresampled
/// rather than dropping the recording.
pub fn resample_to_16k(mono: &[f32], from_rate: u32) -> Vec<f32> {
    // Identity path: already at the target rate (D8).
    if from_rate == TARGET_SAMPLE_RATE {
        return mono.to_vec();
    }
    if mono.is_empty() {
        return Vec::new();
    }

    // Ideal converted length — the trim target that strips the zero-pad tail.
    let expected_len =
        (mono.len() as f64 * TARGET_SAMPLE_RATE as f64 / from_rate as f64).ceil() as usize;

    let mut resampler = match FftFixedIn::<f32>::new(
        from_rate as usize,
        TARGET_SAMPLE_RATE as usize,
        CHUNK_SIZE_IN,
        SUB_CHUNKS,
        1, // mono
    ) {
        Ok(r) => r,
        // Construction only fails on nonsensical rates. Degrade to the
        // unresampled buffer rather than losing the recording.
        Err(_) => return mono.to_vec(),
    };

    let mut out: Vec<f32> = Vec::with_capacity(expected_len + CHUNK_SIZE_IN);

    // 1. Every full fixed-size input chunk.
    let mut pos = 0usize;
    loop {
        let need = resampler.input_frames_next();
        if pos + need > mono.len() {
            break;
        }
        let processed = resampler
            .process(&[&mono[pos..pos + need]], None)
            .expect("resampler.process on a full chunk is infallible for valid rates");
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
            .expect("resampler.process on the zero-padded tail is infallible");
        out.extend_from_slice(&processed[0]);
    }

    // 3. Flush the filter's delay line — without this the end of the last word
    //    is eaten (trap 2). `process_partial(None)` drains remaining frames.
    for _ in 0..MAX_FLUSH_ROUNDS {
        let processed = resampler
            .process_partial::<&[f32]>(None, None)
            .expect("resampler flush is infallible");
        if processed[0].is_empty() {
            break;
        }
        out.extend_from_slice(&processed[0]);
    }

    // 4. Trim to the ideal length: drops the zero-pad-induced tail while
    //    keeping the flushed real frames that complete the final word.
    out.truncate(expected_len);
    out
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
        let out = resample_to_16k(&input, 16_000);
        assert_eq!(out, input, "16 kHz input must pass through untouched");
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(resample_to_16k(&[], 48_000).is_empty());
    }

    #[test]
    fn resamples_48k_to_16k_expected_length() {
        // 1.0 s at 48 kHz -> exactly 16 000 frames at 16 kHz.
        let input = sine(1000.0, 48_000, 48_000);
        let out = resample_to_16k(&input, 48_000);
        assert_eq!(out.len(), 16_000, "48k->16k of 1.0 s must be 16 000 frames");
    }

    #[test]
    fn resamples_44100_to_16k_expected_length() {
        // 1.0 s at 44.1 kHz -> 16 000 frames (ceil).
        let input = sine(1000.0, 44_100, 44_100);
        let out = resample_to_16k(&input, 44_100);
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
        let out = resample_to_16k(&input, 48_000);
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
    /// 3. Comment out the `for _ in 0..MAX_FLUSH_ROUNDS { … }` flush block.
    /// 4. Re-run: the resampler withholds its ~`output_delay()` trailing frames,
    ///    so `out` is ~128 frames short of 16 000 and assertion (a) fails.
    /// 5. Restore the flush; the test passes again.
    ///
    /// Verified manually via that comment-out cycle during PR2 development.
    #[test]
    fn flush_keeps_the_tail_of_the_last_word() {
        let input = sine(1000.0, 48_000, 48_000);
        let out = resample_to_16k(&input, 48_000);

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
