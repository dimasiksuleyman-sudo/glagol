//! In-memory WAV framing for STT uploads.
//!
//! The recorder (PR2) captures raw 16-bit signed little-endian mono PCM. The
//! OpenAI-compatible endpoint wants a real `.wav` file (some providers sniff
//! the RIFF header / filename extension), so we wrap the PCM into a WAV
//! container entirely in memory — no temp file — before uploading.
//!
//! [`wrap_wav_s16le_mono`] is a pure function so it can be unit-tested by
//! parsing its own output back with `hound`. [`silence_wav_s16le_mono`] builds
//! on it to produce the "0.5 s of silence" probe that
//! `commands::dictation` sends when a provider does not expose `/models`.

use std::io::Cursor;

use hound::{SampleFormat, WavSpec, WavWriter};

/// Wrap raw signed-16-bit little-endian **mono** PCM samples into a complete
/// WAV byte buffer (44-byte RIFF/WAVE header + `data` chunk).
///
/// Pure and infallible for in-memory buffers: the only fallible operations are
/// writes to a `Cursor<Vec<u8>>`, which never fail (the vector grows), so the
/// internal `expect`s document invariants rather than guard real error paths.
pub fn wrap_wav_s16le_mono(pcm: &[i16], sample_rate: u32) -> Vec<u8> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec)
            .expect("in-memory WAV writer construction is infallible");
        for &sample in pcm {
            writer
                .write_sample(sample)
                .expect("writing a sample to an in-memory buffer is infallible");
        }
        writer
            .finalize()
            .expect("finalizing an in-memory WAV buffer is infallible");
    }
    cursor.into_inner()
}

/// Build a valid WAV clip of `duration_ms` milliseconds of pure silence at
/// `sample_rate`. Used as the connectivity/credential probe sample when a
/// provider does not answer `GET /models`.
///
/// The sample count rounds up so a non-integer millisecond count still yields
/// at least the requested duration.
pub fn silence_wav_s16le_mono(duration_ms: u32, sample_rate: u32) -> Vec<u8> {
    // ceil(sample_rate * duration_ms / 1000) using integer math.
    let samples = (sample_rate as u64 * duration_ms as u64).div_ceil(1000) as usize;
    let pcm = vec![0i16; samples];
    wrap_wav_s16le_mono(&pcm, sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::WavReader;
    use std::io::Cursor;

    #[test]
    fn wrap_produces_parseable_wav_with_correct_header() {
        let pcm: Vec<i16> = vec![0, 1000, -1000, 32767, -32768, 500];
        let bytes = wrap_wav_s16le_mono(&pcm, 24_000);

        // Byte-level header sanity: RIFF/WAVE/fmt/data magic in place.
        assert_eq!(&bytes[0..4], b"RIFF", "missing RIFF magic");
        assert_eq!(&bytes[8..12], b"WAVE", "missing WAVE magic");
        assert_eq!(&bytes[12..16], b"fmt ", "missing fmt chunk");
        assert!(bytes.windows(4).any(|w| w == b"data"), "missing data chunk");

        // Parse it back with hound and confirm the spec + samples round-trip.
        let mut reader = WavReader::new(Cursor::new(&bytes)).expect("output parses as WAV");
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 24_000);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, SampleFormat::Int);

        let decoded: Vec<i16> = reader
            .samples::<i16>()
            .map(|s| s.expect("sample decodes"))
            .collect();
        assert_eq!(decoded, pcm, "samples must round-trip unchanged");
    }

    #[test]
    fn silence_wav_has_expected_sample_count_and_is_all_zero() {
        // 500 ms at 24 kHz = 12 000 samples.
        let bytes = silence_wav_s16le_mono(500, 24_000);
        let mut reader = WavReader::new(Cursor::new(&bytes)).expect("silence parses as WAV");
        assert_eq!(reader.spec().sample_rate, 24_000);

        let decoded: Vec<i16> = reader
            .samples::<i16>()
            .map(|s| s.expect("sample decodes"))
            .collect();
        assert_eq!(
            decoded.len(),
            12_000,
            "500 ms @ 24 kHz should be 12 000 samples"
        );
        assert!(
            decoded.iter().all(|&s| s == 0),
            "silence must be all zeroes"
        );
    }
}
