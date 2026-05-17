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

    // Normalize each chunk so streaming WAV markers (0xFFFFFFxx in
    // RIFF / data size fields, used by SaluteSpeech because it streams
    // synthesized audio before knowing the total length) don't trip
    // up hound's strict size validation. For already-valid WAVs the
    // normalization rewrites the same values back — it's a no-op in
    // effect but keeps the call site single-path.
    let normalized: Vec<Vec<u8>> = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| {
            normalize_streaming_wav(c).map_err(|e| WavJoinError::Invalid(format!("chunk {i}: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let first = WavReader::new(Cursor::new(&normalized[0]))
        .map_err(|e| WavJoinError::Invalid(format!("chunk 0: {e}")))?;
    let spec = first.spec();

    for (i, chunk) in normalized.iter().enumerate().skip(1) {
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

    for chunk in &normalized {
        let mut reader = WavReader::new(Cursor::new(chunk))?;
        for sample in reader.samples::<i16>() {
            writer.write_sample(sample?)?;
        }
    }

    writer.finalize()?;
    Ok(output.into_inner())
}

/// Normalize a WAV byte buffer so strict readers like `hound` can
/// parse it even when the producer (e.g. SaluteSpeech streaming TTS)
/// emits max-`u32` markers in the RIFF and data size fields because
/// the final size wasn't known when the headers were sent.
///
/// Rewrites two `u32` fields, both little-endian per the WAV spec:
/// - **RIFF size** at offset `4..8` → `total_bytes - 8`
/// - **Data chunk size** (the four bytes immediately after the
///   `b"data"` marker) → number of bytes that follow that field
///
/// The data chunk is located by walking the RIFF chunk graph from
/// offset 12, reading each `(id, size)` header pair and skipping over
/// chunks whose `id != b"data"`. Chunks like `fmt `, `LIST`, `bext`,
/// `fact`, and `INFO` are therefore handled correctly even though we
/// only care about their length.
///
/// For inputs that are already valid (declared sizes match actual
/// byte counts), this rewrites the same values back — it's a no-op
/// in effect, keeping the call sites in [`join_wav_chunks`] on a
/// single code path.
///
/// # Errors
///
/// Returns [`WavJoinError::Invalid`] when the buffer is shorter than
/// the minimum WAV header (44 bytes), does not start with `b"RIFF"`,
/// is missing the `b"WAVE"` form marker at bytes `8..12`, or contains
/// no `b"data"` chunk reachable by chunk-graph walking.
pub fn normalize_streaming_wav(bytes: &[u8]) -> Result<Vec<u8>, WavJoinError> {
    if bytes.len() < 44 {
        return Err(WavJoinError::Invalid(format!(
            "WAV buffer too short: {} bytes (minimum 44)",
            bytes.len()
        )));
    }
    if &bytes[0..4] != b"RIFF" {
        return Err(WavJoinError::Invalid(
            "missing RIFF header at offset 0".into(),
        ));
    }
    if &bytes[8..12] != b"WAVE" {
        return Err(WavJoinError::Invalid(
            "missing WAVE form marker at offset 8".into(),
        ));
    }

    let total_len = bytes.len();
    let mut output = bytes.to_vec();

    // Rewrite RIFF size = total_bytes - 8 (RIFF id + size field).
    let real_riff_size = (total_len - 8) as u32;
    output[4..8].copy_from_slice(&real_riff_size.to_le_bytes());

    // Locate the data chunk by walking RIFF chunks from offset 12.
    let data_offset = find_data_chunk_offset(&output)
        .ok_or_else(|| WavJoinError::Invalid("no data chunk found in WAV stream".into()))?;

    // Rewrite data chunk size = bytes after the size field.
    let size_field_start = data_offset + 4;
    let size_field_end = size_field_start + 4;
    let real_data_size = (total_len - size_field_end) as u32;
    output[size_field_start..size_field_end].copy_from_slice(&real_data_size.to_le_bytes());

    Ok(output)
}

/// Walk the RIFF chunk graph starting at offset 12 (just past
/// `RIFF<size>WAVE`) and return the offset of the `b"data"` chunk id,
/// or `None` if no data chunk is reachable.
///
/// Each iteration reads an 8-byte chunk header (4-byte id + 4-byte
/// little-endian `u32` size), inspects the id, and advances past the
/// payload (rounded up to the next even byte per the RIFF spec's
/// word-alignment requirement).
///
/// Defensive against corrupt or streaming-marker sizes in non-data
/// chunks: any size that would push the cursor past the buffer end
/// (or would overflow when added to the current offset) aborts with
/// `None` rather than panicking.
fn find_data_chunk_offset(bytes: &[u8]) -> Option<usize> {
    let mut offset: usize = 12;
    while offset + 8 <= bytes.len() {
        let id = &bytes[offset..offset + 4];
        if id == b"data" {
            return Some(offset);
        }

        let size_bytes: [u8; 4] = bytes[offset + 4..offset + 8].try_into().ok()?;
        let chunk_size = u32::from_le_bytes(size_bytes) as usize;

        // Defensive: a streaming-marker size in a non-data chunk (very
        // unlikely in practice — fmt/LIST/bext all know their length —
        // but possible if the buffer is corrupt) would otherwise push
        // us into UB territory. Bail rather than panic.
        if chunk_size > bytes.len() {
            return None;
        }

        // RIFF chunks are padded to the next even byte if their size
        // is odd. The pad byte is NOT counted in the chunk size field.
        let padded_size = chunk_size + (chunk_size & 1);

        // Overflow-safe advance: offset + 8 + padded_size.
        let next_offset = offset.checked_add(8)?.checked_add(padded_size)?;
        if next_offset > bytes.len() {
            return None;
        }
        offset = next_offset;
    }
    None
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

    /// Build a synthetic 24 kHz / 16-bit / mono PCM WAV with
    /// caller-supplied RIFF and data size bytes. This lets tests
    /// reproduce SaluteSpeech's streaming pattern (0xFFFFFFxx in the
    /// size fields) byte-for-byte, instead of committing a real
    /// 85 KB Sber response into the repo.
    ///
    /// An optional `LIST` chunk with `list_payload.len()`-byte payload
    /// is inserted between `fmt ` and `data` when `list_payload`
    /// is `Some` — used by the unknown-chunk-skip test.
    ///
    /// The returned buffer's actual byte length is honest; only the
    /// size *fields* may be lying about it.
    fn make_streaming_wav(
        riff_size_bytes: [u8; 4],
        data_size_bytes: [u8; 4],
        samples: &[i16],
        list_payload: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&riff_size_bytes);
        out.extend_from_slice(b"WAVE");

        // fmt  chunk: PCM, mono, 24 kHz, 16 bits per sample
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&24_000u32.to_le_bytes()); // sample rate
        out.extend_from_slice(&48_000u32.to_le_bytes()); // byte rate
        out.extend_from_slice(&2u16.to_le_bytes()); // block align
        out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample

        if let Some(payload) = list_payload {
            out.extend_from_slice(b"LIST");
            out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
            out.extend_from_slice(payload);
            // Pad to even alignment if needed.
            if payload.len() % 2 == 1 {
                out.push(0);
            }
        }

        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_size_bytes);
        for &s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
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

    // ====================================================================
    // PR #13 — streaming-WAV normalization
    // ====================================================================

    #[test]
    fn test_normalize_riff_size_replaced() {
        let samples = [0_i16, 100, -100, 200];
        let bytes = make_streaming_wav(
            [0xFF, 0xFF, 0xFF, 0xFF], // streaming RIFF marker
            [0x00, 0x00, 0x00, 0x00], // zeroed data size (any non-real value works)
            &samples,
            None,
        );

        let normalized = normalize_streaming_wav(&bytes).unwrap();

        let real_riff_size = (bytes.len() - 8) as u32;
        assert_eq!(&normalized[4..8], &real_riff_size.to_le_bytes());
    }

    #[test]
    fn test_normalize_data_size_replaced() {
        let samples = [10_i16; 50]; // 50 samples × 2 bytes = 100 bytes of data
        let bytes = make_streaming_wav(
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0xD3, 0xFF, 0xFF, 0xFF], // Sber-shaped streaming data marker
            &samples,
            None,
        );

        let normalized = normalize_streaming_wav(&bytes).unwrap();

        // Data chunk lives right after fmt: offset 12 (WAVE+fmt header) +
        // 8 (fmt id+size) + 16 (fmt payload) = 36. Size field follows.
        let data_offset = 36;
        assert_eq!(&normalized[data_offset..data_offset + 4], b"data");
        let real_data_size = (samples.len() * 2) as u32;
        assert_eq!(
            &normalized[data_offset + 4..data_offset + 8],
            &real_data_size.to_le_bytes()
        );
    }

    #[test]
    fn test_normalize_passthrough_for_valid_wav() {
        // A WAV produced by hound's own writer is byte-perfectly valid.
        // normalize_streaming_wav must rewrite the size fields to the
        // values they already hold, leaving the buffer byte-identical.
        let spec = spec_24k_mono_16();
        let valid = make_test_wav(&[100, 200, 300, 400], spec);
        let valid_clone = valid.clone();

        let normalized = normalize_streaming_wav(&valid).unwrap();

        assert_eq!(normalized, valid_clone);
    }

    #[test]
    fn test_normalize_skips_unknown_chunks_before_data() {
        let samples = [0x00_i16, 0x64];
        let list_payload = [0xDE_u8, 0xAD, 0xBE, 0xEF];
        let bytes = make_streaming_wav(
            [0xFF, 0xFF, 0xFF, 0xFF],
            [0xFF, 0xFF, 0xFF, 0xFF],
            &samples,
            Some(&list_payload),
        );

        let normalized = normalize_streaming_wav(&bytes).unwrap();

        // RIFF(8) + WAVE(4) + fmt header(8) + fmt payload(16) +
        // LIST header(8) + LIST payload(4) = 48.
        let data_offset = 48;
        assert_eq!(&normalized[data_offset..data_offset + 4], b"data");
        let real_data_size = (samples.len() * 2) as u32;
        assert_eq!(
            &normalized[data_offset + 4..data_offset + 8],
            &real_data_size.to_le_bytes()
        );

        // hound must accept the normalized buffer.
        let (spec, decoded) = read_samples(&normalized);
        assert_eq!(spec, spec_24k_mono_16());
        assert_eq!(decoded, samples.to_vec());
    }

    #[test]
    fn test_join_handles_sber_streaming_pattern() {
        // End-to-end: two Sber-shaped WAVs (streaming markers in BOTH
        // RIFF and data size fields, matching the forensic hex dump
        // collected from a live SaluteSpeech response) are joined into
        // a single valid WAV with the expected sample count.
        let samples_a: Vec<i16> = (0..1200).map(|i| i as i16).collect();
        let samples_b: Vec<i16> = (0..800).map(|i| (i as i16).wrapping_mul(3)).collect();
        let wav_a = make_streaming_wav(
            [0xF7, 0xFF, 0xFF, 0xFF],
            [0xD3, 0xFF, 0xFF, 0xFF],
            &samples_a,
            None,
        );
        let wav_b = make_streaming_wav(
            [0xF7, 0xFF, 0xFF, 0xFF],
            [0xD3, 0xFF, 0xFF, 0xFF],
            &samples_b,
            None,
        );

        let joined = join_wav_chunks(&[wav_a, wav_b]).unwrap();

        let (spec, decoded) = read_samples(&joined);
        assert_eq!(spec, spec_24k_mono_16());
        assert_eq!(decoded.len(), samples_a.len() + samples_b.len());

        let mut expected = samples_a.clone();
        expected.extend_from_slice(&samples_b);
        assert_eq!(decoded, expected);
    }
}
