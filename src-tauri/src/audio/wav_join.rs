//! WAV chunk concatenation for SaluteSpeech audio output.
//!
//! SaluteSpeech returns one WAV file per `synthesize()` call. To produce
//! a single audio file from a long document, we synthesize each chunk
//! independently and then concatenate the resulting WAV bytes here.
//!
//! All chunks from a single document share the same voice and therefore
//! the same WAV format, which makes concatenation straightforward: parse
//! each chunk with `hound::WavReader`, write all samples through a single
//! `hound::WavWriter`, and let hound rewrite the RIFF/data headers with
//! the combined sample count.

use std::io::Cursor;

use hound::{WavReader, WavWriter};

/// Concatenate multiple WAV byte arrays into a single valid WAV file.
///
/// All input chunks MUST have identical format. Format equality is
/// checked via `WavSpec`'s derived `PartialEq`, which compares all
/// four fields: `channels`, `sample_rate`, `bits_per_sample`, and
/// `sample_format`. This is naturally true when all chunks come from
/// SaluteSpeech with the same voice ID.
///
/// # Arguments
///
/// - `chunks` — non-empty slice of WAV byte arrays from `synthesize()`
///
/// # Returns
///
/// `Ok(Vec<u8>)` containing a single valid WAV file with all audio
/// data concatenated, header rewritten with combined sample count.
///
/// # Errors
///
/// - [`WavJoinError::Empty`] if `chunks` is empty
/// - [`WavJoinError::FormatMismatch`] if chunks differ on any of the
///   four `WavSpec` fields (sample rate, bit depth, channels, format)
/// - [`WavJoinError::Invalid`] if a chunk's bytes cannot be parsed,
///   with positional context (e.g., "chunk 3: ...")
/// - [`WavJoinError::Codec`] for low-level `hound` failures during
///   sample iteration or output writing
/// - [`WavJoinError::Io`] on internal buffer write failure (extremely rare)
pub fn join_wav_chunks(chunks: &[Vec<u8>]) -> Result<Vec<u8>, WavJoinError> {
    if chunks.is_empty() {
        return Err(WavJoinError::Empty);
    }

    let first = WavReader::new(Cursor::new(&chunks[0]))
        .map_err(|e| WavJoinError::Invalid(format!("chunk 0: {e}")))?;
    let spec = first.spec();

    for (i, chunk) in chunks.iter().enumerate().skip(1) {
        let reader = WavReader::new(Cursor::new(chunk))
            .map_err(|e| WavJoinError::Invalid(format!("chunk {i}: {e}")))?;
        if reader.spec() != spec {
            return Err(WavJoinError::FormatMismatch(format!(
                "chunk 0: {:?}, chunk {}: {:?}",
                spec,
                i,
                reader.spec()
            )));
        }
    }

    let mut output = Cursor::new(Vec::new());
    let mut writer = WavWriter::new(&mut output, spec)?;

    for chunk in chunks {
        let mut reader = WavReader::new(Cursor::new(chunk))?;
        for sample in reader.samples::<i16>() {
            writer.write_sample(sample?)?;
        }
    }

    writer.finalize()?;
    Ok(output.into_inner())
}

#[derive(thiserror::Error, Debug)]
pub enum WavJoinError {
    #[error("no WAV chunks provided")]
    Empty,
    #[error("WAV chunks have mismatched format: {0}")]
    FormatMismatch(String),
    #[error("invalid WAV data: {0}")]
    Invalid(String),
    #[error("audio codec error: {0}")]
    Codec(#[from] hound::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavSpec};

    const SR_24K: u32 = 24_000;

    fn spec_24k_mono_16() -> WavSpec {
        WavSpec {
            channels: 1,
            sample_rate: SR_24K,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        }
    }

    fn make_test_wav(samples: &[i16], spec: WavSpec) -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        let mut writer = WavWriter::new(&mut output, spec).unwrap();
        for &s in samples {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();
        output.into_inner()
    }

    fn read_samples(bytes: &[u8]) -> (WavSpec, Vec<i16>) {
        let mut reader = WavReader::new(Cursor::new(bytes)).unwrap();
        let spec = reader.spec();
        let samples = reader.samples::<i16>().map(Result::unwrap).collect();
        (spec, samples)
    }

    /// Find the `data` sub-chunk in a RIFF/WAVE byte stream and return
    /// the declared payload size (the `u32` immediately following the
    /// `"data"` marker). Used by `test_combined_header_correct_data_size`
    /// to verify the output header at the raw-byte level, independent
    /// of `WavReader::len()`.
    fn read_data_chunk_size(bytes: &[u8]) -> u32 {
        let marker = b"data";
        let pos = bytes
            .windows(4)
            .position(|w| w == marker)
            .expect("WAV must contain a `data` chunk");
        let size_bytes = &bytes[pos + 4..pos + 8];
        u32::from_le_bytes(size_bytes.try_into().unwrap())
    }

    #[test]
    fn test_empty_chunks_returns_error() {
        let result = join_wav_chunks(&[]);
        assert!(matches!(result, Err(WavJoinError::Empty)));
    }

    #[test]
    fn test_single_chunk_roundtrip_samples() {
        let samples = vec![0_i16, 100, -100, 32_000, -32_000];
        let spec = spec_24k_mono_16();
        let wav = make_test_wav(&samples, spec);

        let joined = join_wav_chunks(&[wav]).unwrap();
        let (out_spec, out_samples) = read_samples(&joined);

        assert_eq!(out_spec, spec);
        assert_eq!(out_samples, samples);
    }

    #[test]
    fn test_two_chunks_concatenate_samples() {
        let a = vec![1_i16, 2, 3];
        let b = vec![4_i16, 5, 6, 7];
        let spec = spec_24k_mono_16();
        let wav_a = make_test_wav(&a, spec);
        let wav_b = make_test_wav(&b, spec);

        let joined = join_wav_chunks(&[wav_a, wav_b]).unwrap();
        let (_, out_samples) = read_samples(&joined);

        let mut expected = a.clone();
        expected.extend_from_slice(&b);
        assert_eq!(out_samples.len(), a.len() + b.len());
        assert_eq!(out_samples, expected);
    }

    #[test]
    fn test_combined_header_correct_data_size() {
        let a = vec![10_i16; 100];
        let b = vec![20_i16; 250];
        let spec = spec_24k_mono_16();
        let wav_a = make_test_wav(&a, spec);
        let wav_b = make_test_wav(&b, spec);

        let joined = join_wav_chunks(&[wav_a.clone(), wav_b.clone()]).unwrap();

        let size_a = read_data_chunk_size(&wav_a);
        let size_b = read_data_chunk_size(&wav_b);
        let size_joined = read_data_chunk_size(&joined);

        assert_eq!(size_joined, size_a + size_b);
        let bytes_per_sample = u32::from(spec.bits_per_sample / 8);
        assert_eq!(size_joined, (a.len() + b.len()) as u32 * bytes_per_sample);
    }

    #[test]
    fn test_format_mismatch_returns_error() {
        let spec_a = spec_24k_mono_16();
        let spec_b = WavSpec {
            sample_rate: 48_000,
            ..spec_a
        };
        let wav_a = make_test_wav(&[1, 2, 3], spec_a);
        let wav_b = make_test_wav(&[4, 5, 6], spec_b);

        let err = join_wav_chunks(&[wav_a, wav_b]).unwrap_err();
        match err {
            WavJoinError::FormatMismatch(msg) => {
                assert!(
                    msg.contains("chunk 1"),
                    "message should cite chunk index: {msg}"
                );
            }
            other => panic!("expected FormatMismatch, got {other:?}"),
        }
    }

    #[test]
    fn test_invalid_wav_returns_error() {
        let garbage = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];

        let err = join_wav_chunks(&[garbage]).unwrap_err();
        match err {
            WavJoinError::Invalid(msg) => {
                assert!(
                    msg.starts_with("chunk 0:"),
                    "message should cite chunk 0: {msg}"
                );
            }
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn test_output_is_valid_wav_per_hound() {
        let spec = spec_24k_mono_16();
        let wav_a = make_test_wav(&[100, 200, 300], spec);
        let wav_b = make_test_wav(&[400, 500], spec);

        let joined = join_wav_chunks(&[wav_a, wav_b]).unwrap();

        let reader = WavReader::new(Cursor::new(&joined)).expect("output must parse as WAV");
        assert_eq!(reader.spec(), spec);
        assert_eq!(reader.len(), 5);
    }

    #[test]
    fn test_total_duration_correct() {
        let spec = spec_24k_mono_16();
        let a = vec![0_i16; 24_000]; // 1.0 s
        let b = vec![0_i16; 12_000]; // 0.5 s
        let wav_a = make_test_wav(&a, spec);
        let wav_b = make_test_wav(&b, spec);

        let joined = join_wav_chunks(&[wav_a, wav_b]).unwrap();
        let reader = WavReader::new(Cursor::new(&joined)).unwrap();

        assert_eq!(reader.duration(), 24_000 + 12_000);
        assert_eq!(reader.duration(), 36_000);
    }

    #[test]
    fn test_24khz_16bit_mono_passthrough() {
        let spec = spec_24k_mono_16();
        let samples: Vec<i16> = (0..2400).map(|i| (i as i16).wrapping_mul(7)).collect();
        let wav = make_test_wav(&samples, spec);

        let joined = join_wav_chunks(&[wav]).unwrap();
        let (out_spec, out_samples) = read_samples(&joined);

        assert_eq!(out_spec.channels, 1);
        assert_eq!(out_spec.sample_rate, 24_000);
        assert_eq!(out_spec.bits_per_sample, 16);
        assert_eq!(out_spec.sample_format, SampleFormat::Int);
        assert_eq!(out_samples, samples);
    }

    #[test]
    fn test_chunks_preserves_sample_values() {
        let spec = spec_24k_mono_16();
        let a = vec![-1_i16, -2, -3, -4];
        let b = vec![5_i16, 6, 7, 8, 9];
        let c = vec![i16::MIN, 0, i16::MAX];
        let wav_a = make_test_wav(&a, spec);
        let wav_b = make_test_wav(&b, spec);
        let wav_c = make_test_wav(&c, spec);

        let joined = join_wav_chunks(&[wav_a, wav_b, wav_c]).unwrap();
        let (_, out_samples) = read_samples(&joined);

        let mut expected = a.clone();
        expected.extend_from_slice(&b);
        expected.extend_from_slice(&c);
        assert_eq!(out_samples, expected);
    }
}
