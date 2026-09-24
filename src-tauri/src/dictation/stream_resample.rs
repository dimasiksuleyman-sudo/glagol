//! Stateful 16 kHz mono conversion. Filter delay is removed once, and the last
//! partial input plus the delayed tail is flushed exactly once on key release.
use rubato::{FftFixedIn, Resampler};
pub struct StreamResampler {
    rate: u32,
    converter: Option<FftFixedIn<f32>>,
    pending: Vec<f32>,
    input_frames: usize,
    output_frames: usize,
    skip: usize,
    finished: bool,
}
impl StreamResampler {
    pub fn new(rate: u32) -> Result<Self, String> {
        if rate == 0 {
            return Err("Invalid input sample rate".into());
        }
        let converter = if rate == 16000 {
            None
        } else {
            Some(FftFixedIn::new(rate as usize, 16000, 1024, 2, 1).map_err(|e| e.to_string())?)
        };
        let skip = converter.as_ref().map_or(0, |r| r.output_delay());
        Ok(Self {
            rate,
            converter,
            pending: Vec::new(),
            input_frames: 0,
            output_frames: 0,
            skip,
            finished: false,
        })
    }
    fn take_output(&mut self, samples: &[f32], out: &mut Vec<f32>) {
        let skip = self.skip.min(samples.len());
        self.skip -= skip;
        out.extend(samples[skip..].iter().map(|s| s.clamp(-1.0, 1.0)));
    }
    pub fn push(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        if self.finished {
            return Err("Resampler already finalized".into());
        }
        if samples.iter().any(|s| !s.is_finite()) {
            return Err("Invalid microphone samples".into());
        }
        self.input_frames += samples.len();
        if self.converter.is_none() {
            self.output_frames += samples.len();
            return Ok(samples.iter().map(|s| s.clamp(-1.0, 1.0)).collect());
        }
        self.pending.extend_from_slice(samples);
        let mut out = Vec::new();
        loop {
            let converter = self.converter.as_mut().unwrap();
            let need = converter.input_frames_next();
            if self.pending.len() < need {
                break;
            }
            let converted = converter
                .process(&[&self.pending[..need]], None)
                .map_err(|e| e.to_string())?;
            self.pending.drain(..need);
            self.take_output(&converted[0], &mut out);
        }
        self.output_frames += out.len();
        Ok(out)
    }
    pub fn finish(&mut self) -> Result<Vec<f32>, String> {
        if self.finished {
            return Err("Resampler already finalized".into());
        }
        self.finished = true;
        let expected = (self.input_frames as u64 * 16000).div_ceil(self.rate as u64) as usize;
        let remaining = expected
            .checked_sub(self.output_frames)
            .ok_or("Resampler emitted too many frames")?;
        if self.converter.is_none() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        if !self.pending.is_empty() {
            let converter = self.converter.as_mut().unwrap();
            let mut padded = vec![0.0; converter.input_frames_next()];
            padded[..self.pending.len()].copy_from_slice(&self.pending);
            let converted = converter
                .process(&[padded], None)
                .map_err(|e| e.to_string())?;
            self.take_output(&converted[0], &mut out);
            self.pending.clear();
        }
        for _ in 0..16 {
            if out.len() >= remaining {
                break;
            }
            let converted = self
                .converter
                .as_mut()
                .unwrap()
                .process_partial::<&[f32]>(None, None)
                .map_err(|e| e.to_string())?;
            self.take_output(&converted[0], &mut out);
        }
        if out.len() < remaining {
            return Err("Resampler could not flush the recording tail".into());
        }
        out.truncate(remaining);
        self.output_frames += out.len();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_preserve_beginning_end_and_exact_length() {
        for rate in [16000, 44100, 48000, 96000, 192000] {
            let input: Vec<f32> = (0..rate as usize + 117)
                .map(|i| {
                    if i < 500 || i > rate as usize - 500 {
                        0.5
                    } else {
                        0.0
                    }
                })
                .collect();
            let mut reference = None;
            for size in [1, 997, 4096, 999999] {
                let mut r = StreamResampler::new(rate).unwrap();
                let mut output = Vec::new();
                for chunk in input.chunks(size) {
                    output.extend(r.push(chunk).unwrap());
                }
                output.extend(r.finish().unwrap());
                assert_eq!(
                    output.len(),
                    (input.len() as u64 * 16000).div_ceil(rate as u64) as usize
                );
                assert!(
                    output[..20].iter().any(|s| *s > 0.3),
                    "beginning lost at {rate}"
                );
                assert!(
                    output[output.len() - 20..].iter().any(|s| *s > 0.3),
                    "tail lost at {rate}"
                );
                if let Some(expected) = &reference {
                    assert_eq!(&output, expected, "block boundary altered audio at {rate}");
                } else {
                    reference = Some(output);
                }
                assert!(r.finish().is_err());
                assert!(r.push(&[0.0]).is_err());
            }
        }
    }
}
