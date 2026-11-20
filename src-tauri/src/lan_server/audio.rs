//! Turn an uploaded WAV file into the 16 kHz mono samples GigaAM expects.

use std::io::Cursor;

/// Longest accepted recording (the local engine's own limit).
pub const MAX_SECONDS: usize = 10 * 60;
const TARGET_RATE: u32 = 16_000;

/// Decode PCM WAV (16-bit integer or 32-bit float, any rate, any channel
/// count) to mono 16 kHz `f32`. Other formats are refused with a message for
/// the client.
pub fn decode_wav(bytes: &[u8]) -> Result<Vec<f32>, &'static str> {
    let mut reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|_| "audio must be a WAV file (PCM 16-bit or float)")?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate < 8_000 || spec.sample_rate > 192_000 {
        return Err("unsupported WAV channel count or sample rate");
    }
    let channels = spec.channels as usize;
    let frames = reader.duration() as usize;
    if frames > MAX_SECONDS * spec.sample_rate as usize {
        return Err("recording is longer than 10 minutes");
    }
    let interleaved: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (hound::SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|s| s.map(|s| s as f32 / 32768.0))
            .collect::<Result<_, _>>(),
        (hound::SampleFormat::Float, 32) => reader.samples::<f32>().collect::<Result<_, _>>(),
        _ => return Err("WAV must be PCM 16-bit or 32-bit float"),
    }
    .map_err(|_| "WAV data is truncated or damaged")?;
    let mono: Vec<f32> = interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect();
    if mono.is_empty() {
        return Err("recording is empty");
    }
    let pcm = crate::dictation::resample::resample_to_16k(&mono, spec.sample_rate)
        .map_err(|_| "could not resample the recording")?;
    if pcm.len() > MAX_SECONDS * TARGET_RATE as usize {
        return Err("recording is longer than 10 minutes");
    }
    Ok(pcm)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(spec: hound::WavSpec, frames: usize, value: f32) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut out, spec).unwrap();
            for _ in 0..frames * spec.channels as usize {
                match spec.sample_format {
                    hound::SampleFormat::Int => w.write_sample((value * 32767.0) as i16).unwrap(),
                    hound::SampleFormat::Float => w.write_sample(value).unwrap(),
                }
            }
            w.finalize().unwrap();
        }
        out.into_inner()
    }

    fn spec(rate: u32, channels: u16, float: bool) -> hound::WavSpec {
        hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: if float { 32 } else { 16 },
            sample_format: if float {
                hound::SampleFormat::Float
            } else {
                hound::SampleFormat::Int
            },
        }
    }

    #[test]
    fn dictation_wav_passes_through() {
        let pcm = decode_wav(&wav(spec(16_000, 1, false), 16_000, 0.5)).unwrap();
        assert_eq!(pcm.len(), 16_000);
        assert!((pcm[100] - 0.5).abs() < 0.01);
    }

    #[test]
    fn stereo_and_other_rates_are_converted() {
        let pcm = decode_wav(&wav(spec(48_000, 2, true), 48_000, 0.25)).unwrap();
        assert!((15_900..=16_100).contains(&pcm.len()), "{}", pcm.len());
        let pcm = decode_wav(&wav(spec(44_100, 1, false), 4_410, 0.1)).unwrap();
        assert!((1_500..=1_700).contains(&pcm.len()), "{}", pcm.len());
    }

    #[test]
    fn other_formats_are_refused() {
        assert!(decode_wav(b"ID3 not a wav").is_err());
        assert!(
            decode_wav(&wav(spec(16_000, 1, false), 0, 0.0)).is_err(),
            "empty"
        );
        let s24 = hound::WavSpec {
            bits_per_sample: 24,
            ..spec(16_000, 1, false)
        };
        let mut out = Cursor::new(Vec::new());
        {
            let mut w = hound::WavWriter::new(&mut out, s24).unwrap();
            w.write_sample(1000i32).unwrap();
            w.finalize().unwrap();
        }
        assert!(decode_wav(&out.into_inner()).is_err(), "24-bit");
        let long = wav(spec(8_000, 1, false), 8_000 * (MAX_SECONDS + 1), 0.0);
        assert_eq!(
            decode_wav(&long),
            Err("recording is longer than 10 minutes")
        );
    }
}
