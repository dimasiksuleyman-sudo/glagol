//! The recorder thread: a single blocking loop that owns the `cpal::Stream`
//! and turns captured native-rate audio into a normalized [`PcmAudio`] (D2).
//!
//! [`run_recorder`] is generic over the [`SampleSource`] and [`LevelSink`] seams
//! so the whole state machine is exercised without hardware (a `FakeSource`
//! pushes the same `Samples(..)` the cpal callback would). [`CpalSource`] is the
//! real implementation — the config ladder (D3), the data/error callbacks, and
//! ownership of the non-`Send` stream. The live-microphone gate is the
//! `#[ignore]` test `tests::live_mic` (D14).

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;

use super::{
    downmix_to_mono, f32_to_i16, resample::resample_to_16k, rms, LevelSink, PcmAudio,
    RecorderError, RecorderHandle, RecorderMsg, SampleSource, StartedConfig, StartedInfo,
    LEVEL_WINDOW_MS, MAX_RECORDING_MS, TARGET_SAMPLE_RATE,
};

/// How long the loop blocks waiting for a message before ticking. Keeps the
/// thread responsive to `Shutdown` / a disconnected channel; the RMS emit and
/// cap check are driven by `Samples` arrival, not this timeout.
const RECV_TIMEOUT: Duration = Duration::from_millis(50);

/// Recorder thread phase. The captured buffer / level accumulator live as loop
/// locals; this only tracks *what the thread is doing*, so a stray `Start` is
/// `Busy` (D13) and a post-cap / post-fault `Stop` hands over the right result.
enum Phase {
    /// Not capturing.
    Idle,
    /// Capturing at the given native format.
    Recording { rate: u32, channels: u16 },
    /// The 60 s cap fired: capture stopped, audio already finalized and waiting
    /// for the caller's `Stop`.
    Capped(PcmAudio),
    /// The stream's error callback fired: waiting for `Stop` to report it.
    Faulted(String),
}

/// Spawn the dedicated recorder thread and return a handle to it.
///
/// `make_source` is a factory (not a value) so a **non-`Send`** source such as
/// [`CpalSource`] — it owns a `cpal::Stream`, which is not `Send` on WASAPI —
/// is constructed *on* the recorder thread and never crosses a thread boundary.
/// The thread is named for debuggers/`ps`; it exits on `Shutdown`.
pub fn spawn_recorder<S, L, F>(make_source: F, sink: L) -> RecorderHandle
where
    F: FnOnce() -> S + Send + 'static,
    S: SampleSource,
    L: LevelSink,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = RecorderHandle::new(tx.clone());
    std::thread::Builder::new()
        .name("glagol-recorder".into())
        .spawn(move || {
            let source = make_source();
            run_recorder(rx, tx, source, sink);
        })
        .expect("failed to spawn glagol-recorder thread");
    handle
}

/// Resample the captured mono buffer to 16 kHz and convert to `i16` (D7).
fn finalize_buffer(mono: &[f32], native_rate: u32, truncated: bool) -> PcmAudio {
    let resampled = resample_to_16k(mono, native_rate);
    let samples = f32_to_i16(&resampled);
    PcmAudio::from_samples_16k(samples, truncated)
}

/// Run the recorder loop until `Shutdown` (or the command channel disconnects).
///
/// Owns `source` and `sink` for the thread's lifetime. `self_tx` is a clone of
/// the command channel's sender, handed to `source.start` so the cpal callbacks
/// can push `Samples` / `StreamError` back into this same queue. Generic, not
/// `dyn`, per the PR1 seam convention; no `Send` bound on `S` because the source
/// is created and used entirely on this thread.
pub fn run_recorder<S: SampleSource, L: LevelSink>(
    rx: Receiver<RecorderMsg>,
    self_tx: Sender<RecorderMsg>,
    mut source: S,
    sink: L,
) {
    let mut phase = Phase::Idle;
    // Mono samples at the native rate, accumulated across `Samples` batches.
    let mut buffer: Vec<f32> = Vec::new();
    // Rolling window for the RMS level meter (mono, native rate).
    let mut level_acc: Vec<f32> = Vec::new();

    loop {
        match rx.recv_timeout(RECV_TIMEOUT) {
            Ok(RecorderMsg::Start { device, reply }) => {
                if !matches!(phase, Phase::Idle) {
                    let _ = reply.send(Err(RecorderError::Busy));
                    continue;
                }
                buffer.clear();
                level_acc.clear();
                match source.start(device.as_deref(), self_tx.clone()) {
                    Ok(StartedConfig {
                        info,
                        sample_rate,
                        channels,
                    }) => {
                        phase = Phase::Recording {
                            rate: sample_rate,
                            channels,
                        };
                        let _ = reply.send(Ok(info));
                    }
                    Err(e) => {
                        phase = Phase::Idle;
                        let _ = reply.send(Err(e));
                    }
                }
            }

            Ok(RecorderMsg::Samples(interleaved)) => {
                let Phase::Recording { rate, channels } = phase else {
                    // Stale samples arriving after Stop/Cancel/cap — drop them.
                    continue;
                };
                let mono = downmix_to_mono(&interleaved, channels);

                // Level meter: emit one RMS value per full ~50 ms window (D6).
                let window = level_window_samples(rate);
                level_acc.extend_from_slice(&mono);
                while level_acc.len() >= window {
                    let value = rms(&level_acc[..window]);
                    sink.level(value);
                    level_acc.drain(..window);
                }

                buffer.extend_from_slice(&mono);

                // 60 s cap: auto-finalize rather than lose the recording (D9).
                let max_samples = cap_samples(rate);
                if buffer.len() >= max_samples {
                    buffer.truncate(max_samples);
                    source.stop();
                    let pcm = finalize_buffer(&buffer, rate, true);
                    buffer.clear();
                    level_acc.clear();
                    phase = Phase::Capped(pcm);
                }
            }

            Ok(RecorderMsg::StreamError(msg)) => {
                if matches!(phase, Phase::Recording { .. }) {
                    source.stop();
                    buffer.clear();
                    level_acc.clear();
                    phase = Phase::Faulted(msg);
                }
                // A fault outside Recording has nothing to fault — ignore it.
            }

            Ok(RecorderMsg::Stop { reply }) => {
                let result = match std::mem::replace(&mut phase, Phase::Idle) {
                    Phase::Recording { rate, .. } => {
                        source.stop();
                        Ok(finalize_buffer(&buffer, rate, false))
                    }
                    Phase::Capped(pcm) => Ok(pcm),
                    Phase::Faulted(msg) => Err(RecorderError::DeviceLost(msg)),
                    // Stop with nothing recording: a benign race — hand back an
                    // empty clip rather than error the user.
                    Phase::Idle => Ok(PcmAudio::from_samples_16k(Vec::new(), false)),
                };
                buffer.clear();
                level_acc.clear();
                let _ = reply.send(result);
            }

            Ok(RecorderMsg::Cancel) => {
                if matches!(phase, Phase::Recording { .. }) {
                    source.stop();
                }
                buffer.clear();
                level_acc.clear();
                phase = Phase::Idle;
            }

            Ok(RecorderMsg::Shutdown) => {
                if matches!(phase, Phase::Recording { .. }) {
                    source.stop();
                }
                break;
            }

            Err(RecvTimeoutError::Timeout) => {
                // Idle tick — keep listening.
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Number of mono samples in one RMS window at `rate` (≥ 1).
fn level_window_samples(rate: u32) -> usize {
    ((rate as u64 * LEVEL_WINDOW_MS as u64 / 1000) as usize).max(1)
}

/// Number of mono samples equal to the 60 s cap at `rate`.
fn cap_samples(rate: u32) -> usize {
    (rate as u64 * MAX_RECORDING_MS as u64 / 1000) as usize
}

// ── Config ladder (pure, testable) ──────────────────────────────────────

/// A simplified view of a `cpal::SupportedStreamConfigRange` — just the fields
/// the ladder decides on. Decoupled from cpal's type so the ladder logic is
/// unit-testable without hardware.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ConfigCandidate {
    channels: u16,
    min_rate: u32,
    max_rate: u32,
}

impl ConfigCandidate {
    fn contains(&self, rate: u32) -> bool {
        self.min_rate <= rate && rate <= self.max_rate
    }
}

/// The config ladder (D3): prefer a mono range that already covers 16 kHz
/// (no downmix, no resample), then any multi-channel range covering 16 kHz
/// (downmix only), else `None` → the caller falls back to the device default
/// (downmix + resample). Matching is by **range containment**, not exact
/// equality, because some drivers report a `[min, max]` span rather than a
/// discrete list.
fn choose_ladder(candidates: &[ConfigCandidate], target: u32) -> Option<usize> {
    if let Some(i) = candidates
        .iter()
        .position(|c| c.channels == 1 && c.contains(target))
    {
        return Some(i);
    }
    candidates
        .iter()
        .position(|c| c.channels >= 2 && c.contains(target))
}

// ── CpalSource: the real capture backend ────────────────────────────────

/// The production [`SampleSource`] — wraps WASAPI/ALSA via cpal and owns the
/// live `cpal::Stream`. Never crosses a thread boundary (constructed on the
/// recorder thread by [`spawn_recorder`]); the stream is not `Send` on WASAPI.
pub struct CpalSource {
    stream: Option<cpal::Stream>,
}

impl CpalSource {
    /// A source with no stream open yet.
    pub fn new() -> Self {
        Self { stream: None }
    }
}

impl Default for CpalSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the requested device (`None` = system default). A named-but-missing
/// device falls back to the default with `fell_back_to_default = true` — not an
/// error (D10): an unplugged USB headset must not kill dictation.
fn resolve_device(
    host: &cpal::Host,
    requested: Option<&str>,
) -> Result<(cpal::Device, StartedInfo), RecorderError> {
    // cpal 0.18 exposes the device name via `Display`, not a `name()` method.
    if let Some(name) = requested {
        if let Ok(devices) = host.input_devices() {
            for device in devices {
                if device.to_string() == name {
                    return Ok((
                        device,
                        StartedInfo {
                            device_name: name.to_string(),
                            fell_back_to_default: false,
                        },
                    ));
                }
            }
        }
        // Requested device is gone — fall back to the system default (D10).
        let device = host.default_input_device().ok_or(RecorderError::NoDevice)?;
        let device_name = device.to_string();
        return Ok((
            device,
            StartedInfo {
                device_name,
                fell_back_to_default: true,
            },
        ));
    }

    let device = host.default_input_device().ok_or(RecorderError::NoDevice)?;
    let device_name = device.to_string();
    Ok((
        device,
        StartedInfo {
            device_name,
            fell_back_to_default: false,
        },
    ))
}

/// Walk the config ladder (D3) and return a concrete supported config. Falls to
/// the device default both when no range covers 16 kHz and when a driver that
/// *claims* to cover 16 kHz refuses `try_with_sample_rate` (drivers lie about
/// ranges — the ladder is a fallback chain, not a validator).
fn choose_stream_config(
    device: &cpal::Device,
) -> Result<cpal::SupportedStreamConfig, RecorderError> {
    let ranges: Vec<cpal::SupportedStreamConfigRange> = device
        .supported_input_configs()
        .map_err(|e| RecorderError::UnsupportedConfig(e.to_string()))?
        .collect();

    // In cpal 0.18 `SampleRate` is a plain `u32` type alias.
    let candidates: Vec<ConfigCandidate> = ranges
        .iter()
        .map(|r| ConfigCandidate {
            channels: r.channels(),
            min_rate: r.min_sample_rate(),
            max_rate: r.max_sample_rate(),
        })
        .collect();

    if let Some(i) = choose_ladder(&candidates, TARGET_SAMPLE_RATE) {
        if let Some(config) = ranges[i].try_with_sample_rate(TARGET_SAMPLE_RATE) {
            return Ok(config);
        }
        // Driver reported a range it can't actually honor — fall through.
    }

    device
        .default_input_config()
        .map_err(|e| RecorderError::UnsupportedConfig(e.to_string()))
}

/// Map a cpal build failure to the recorder taxonomy.
// NOTE: best-effort heuristic, see kickoff D12. cpal/WASAPI give no clean
// "denied in Privacy settings" signal — a permission refusal surfaces as a
// build failure. Since we only reach here after devices enumerated, we surface
// PermissionDenied so the UI (PR3/PR5) can point at Параметры → Конфиденциальность
// → Микрофон. A reliable winreg ConsentStore detector is deferred to PR5.
fn build_stream_error(_e: cpal::Error) -> RecorderError {
    RecorderError::PermissionDenied
}

impl SampleSource for CpalSource {
    fn start(
        &mut self,
        device: Option<&str>,
        tx: Sender<RecorderMsg>,
    ) -> Result<StartedConfig, RecorderError> {
        let host = cpal::default_host();
        let (dev, info) = resolve_device(&host, device)?;
        let chosen = choose_stream_config(&dev)?;

        // cpal 0.18: `sample_rate()` is a `u32`; `build_input_stream` takes the
        // `StreamConfig` by value.
        let sample_rate = chosen.sample_rate();
        let channels = chosen.channels();
        let fmt = chosen.sample_format();
        let config: cpal::StreamConfig = chosen.config();

        let stream = match fmt {
            SampleFormat::F32 => {
                let data_tx = tx.clone();
                let err_tx = tx;
                dev.build_input_stream(
                    config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        // NOTE (kickoff D5 callback): sending a Vec<f32> allocates
                        // (the Vec + a channel node). Acceptable for dictation —
                        // ~10 ms callbacks, no realtime-DAW constraints; Whispering
                        // does the same. A lock in the callback stays forbidden
                        // (priority inversion); a lock-free ring buffer would be
                        // over-engineering for this task.
                        let _ = data_tx.send(RecorderMsg::Samples(data.to_vec()));
                    },
                    move |e| {
                        let _ = err_tx.send(RecorderMsg::StreamError(e.to_string()));
                    },
                    None,
                )
            }
            SampleFormat::I16 => {
                let data_tx = tx.clone();
                let err_tx = tx;
                dev.build_input_stream(
                    config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mono: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        let _ = data_tx.send(RecorderMsg::Samples(mono));
                    },
                    move |e| {
                        let _ = err_tx.send(RecorderMsg::StreamError(e.to_string()));
                    },
                    None,
                )
            }
            // `SampleFormat` is #[non_exhaustive]; anything else is an error, not
            // a panic (D4). Canonicalization is always via f32.
            other => return Err(RecorderError::UnsupportedConfig(format!("{other:?}"))),
        }
        .map_err(build_stream_error)?;

        stream
            .play()
            .map_err(|e| RecorderError::BuildStream(e.to_string()))?;
        self.stream = Some(stream);

        Ok(StartedConfig {
            info,
            sample_rate,
            channels,
        })
    }

    fn stop(&mut self) {
        // Dropping the stream stops capture and releases the device.
        self.stream = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;

    // ── test doubles ──

    /// A hardware-free source: returns a fixed native format and never pushes
    /// samples itself (the test drives `Samples` through the handle's sender,
    /// which is exactly what a real callback would do).
    struct FakeSource {
        rate: u32,
        channels: u16,
        fail: bool,
    }

    impl FakeSource {
        fn new(rate: u32, channels: u16) -> Self {
            Self {
                rate,
                channels,
                fail: false,
            }
        }
        fn failing() -> Self {
            Self {
                rate: 16_000,
                channels: 1,
                fail: true,
            }
        }
    }

    impl SampleSource for FakeSource {
        fn start(
            &mut self,
            _device: Option<&str>,
            _tx: Sender<RecorderMsg>,
        ) -> Result<StartedConfig, RecorderError> {
            if self.fail {
                return Err(RecorderError::PermissionDenied);
            }
            Ok(StartedConfig {
                info: StartedInfo {
                    device_name: "fake".to_string(),
                    fell_back_to_default: false,
                },
                sample_rate: self.rate,
                channels: self.channels,
            })
        }
        fn stop(&mut self) {}
    }

    #[derive(Clone, Default)]
    struct FakeSink {
        values: Arc<Mutex<Vec<f32>>>,
    }

    impl LevelSink for FakeSink {
        fn level(&self, rms: f32) {
            self.values.lock().unwrap().push(rms);
        }
    }

    impl FakeSink {
        fn values(&self) -> Vec<f32> {
            self.values.lock().unwrap().clone()
        }
    }

    /// Spawn the recorder loop over a (Send) fake source; return the handle and
    /// the thread's join handle.
    fn harness<S: SampleSource + Send + 'static>(
        source: S,
        sink: FakeSink,
    ) -> (RecorderHandle, JoinHandle<()>) {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = RecorderHandle::new(tx.clone());
        let jh = std::thread::spawn(move || run_recorder(rx, tx, source, sink));
        (handle, jh)
    }

    // ── state machine ──

    #[tokio::test]
    async fn start_samples_stop_returns_16k_pcm() {
        let (handle, jh) = harness(FakeSource::new(48_000, 2), FakeSink::default());
        let info = handle.start(None).await.expect("start ok");
        assert!(!info.fell_back_to_default);

        // 0.5 s of stereo at 48 kHz → ~8000 mono samples at 16 kHz.
        let frames = 24_000usize;
        let mut interleaved = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let s = (2.0 * PI * 440.0 * i as f32 / 48_000.0).sin();
            interleaved.push(s);
            interleaved.push(s);
        }
        handle
            .sender()
            .send(RecorderMsg::Samples(interleaved))
            .unwrap();

        let pcm = handle.stop().await.expect("stop ok");
        assert_eq!(pcm.sample_rate, 16_000);
        assert!(!pcm.truncated);
        assert!(
            (pcm.samples.len() as i64 - 8000).abs() < 200,
            "expected ~8000 samples, got {}",
            pcm.samples.len()
        );

        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn start_while_recording_is_busy() {
        let (handle, jh) = harness(FakeSource::new(16_000, 1), FakeSink::default());
        handle.start(None).await.unwrap();
        let err = handle.start(None).await.unwrap_err();
        assert_eq!(err, RecorderError::Busy);
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn cancel_discards_buffer() {
        let (handle, jh) = harness(FakeSource::new(16_000, 1), FakeSink::default());
        handle.start(None).await.unwrap();
        handle
            .sender()
            .send(RecorderMsg::Samples(vec![0.5; 1000]))
            .unwrap();
        handle.cancel();
        let pcm = handle.stop().await.unwrap();
        assert!(pcm.samples.is_empty(), "cancel must discard the buffer");
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn stream_error_reports_device_lost() {
        let (handle, jh) = harness(FakeSource::new(16_000, 1), FakeSink::default());
        handle.start(None).await.unwrap();
        handle
            .sender()
            .send(RecorderMsg::StreamError("unplugged".to_string()))
            .unwrap();
        let err = handle.stop().await.unwrap_err();
        assert_eq!(err, RecorderError::DeviceLost("unplugged".to_string()));
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn cap_truncates_at_60s() {
        let (handle, jh) = harness(FakeSource::new(16_000, 1), FakeSink::default());
        handle.start(None).await.unwrap();
        // 61 s of mono at 16 kHz — one batch past the cap.
        handle
            .sender()
            .send(RecorderMsg::Samples(vec![0.3f32; 16_000 * 61]))
            .unwrap();
        let pcm = handle.stop().await.unwrap();
        assert!(pcm.truncated, "cap must set truncated");
        // Identity resample at 16 kHz → exactly 60 s of samples.
        assert_eq!(pcm.samples.len(), 16_000 * 60);
        assert_eq!(pcm.duration_ms, 60_000);
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn start_error_replies_and_stays_idle() {
        let (handle, jh) = harness(FakeSource::failing(), FakeSink::default());
        let err = handle.start(None).await.unwrap_err();
        assert_eq!(err, RecorderError::PermissionDenied);
        // A failed start must leave us Idle — a second attempt is retried (and
        // fails the same way), not rejected as Busy.
        let err2 = handle.start(None).await.unwrap_err();
        assert_eq!(err2, RecorderError::PermissionDenied);
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn level_sink_receives_rms_per_window() {
        let sink = FakeSink::default();
        let (handle, jh) = harness(FakeSource::new(16_000, 1), sink.clone());
        handle.start(None).await.unwrap();
        // window = 16000 * 50 / 1000 = 800; feed 1600 full-scale → two windows.
        handle
            .sender()
            .send(RecorderMsg::Samples(vec![1.0; 1600]))
            .unwrap();
        let _ = handle.stop().await.unwrap();
        let vals = sink.values();
        assert_eq!(vals.len(), 2, "two full 50 ms windows expected");
        assert!((vals[0] - 1.0).abs() < 1e-6, "full-scale RMS should be 1.0");
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn stop_without_recording_is_empty() {
        let (handle, jh) = harness(FakeSource::new(16_000, 1), FakeSink::default());
        let pcm = handle.stop().await.unwrap();
        assert!(pcm.samples.is_empty());
        assert!(!pcm.truncated);
        handle.shutdown();
        jh.join().unwrap();
    }

    #[tokio::test]
    async fn shutdown_terminates_thread() {
        let (handle, jh) = harness(FakeSource::new(16_000, 1), FakeSink::default());
        handle.shutdown();
        jh.join().expect("thread joins cleanly after Shutdown");
    }

    // ── config ladder ──

    #[test]
    fn ladder_prefers_mono_covering_target() {
        let candidates = [
            ConfigCandidate {
                channels: 2,
                min_rate: 44_100,
                max_rate: 48_000,
            },
            ConfigCandidate {
                channels: 1,
                min_rate: 8_000,
                max_rate: 48_000,
            },
        ];
        assert_eq!(choose_ladder(&candidates, 16_000), Some(1));
    }

    #[test]
    fn ladder_falls_back_to_multichannel_when_no_mono_covers_target() {
        let candidates = [
            ConfigCandidate {
                channels: 2,
                min_rate: 8_000,
                max_rate: 48_000,
            },
            ConfigCandidate {
                // Mono exists but its range excludes 16 kHz.
                channels: 1,
                min_rate: 44_100,
                max_rate: 48_000,
            },
        ];
        assert_eq!(choose_ladder(&candidates, 16_000), Some(0));
    }

    #[test]
    fn ladder_none_when_target_out_of_every_range() {
        let candidates = [ConfigCandidate {
            channels: 2,
            min_rate: 44_100,
            max_rate: 48_000,
        }];
        assert_eq!(choose_ladder(&candidates, 16_000), None);
    }

    // ── live microphone gate (D14) ──

    /// Manual runtime gate — NOT part of the CI test set. Run on Windows during
    /// QA with a working microphone and **speak during the ~2 s window**:
    ///
    /// ```powershell
    /// cargo test --lib recorder::tests::live_mic -- --ignored --nocapture
    /// ```
    #[tokio::test]
    #[ignore = "requires a live microphone; speak during the ~2 s run"]
    async fn live_mic() {
        let sink = FakeSink::default();
        let sink_thread = sink.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = RecorderHandle::new(tx.clone());
        let jh = std::thread::spawn(move || {
            run_recorder(rx, tx, CpalSource::new(), sink_thread);
        });

        let info = handle.start(None).await.expect("microphone starts");
        eprintln!(
            "recording from: {} (fell_back_to_default={})",
            info.device_name, info.fell_back_to_default
        );

        tokio::time::sleep(Duration::from_millis(2000)).await;

        let pcm = handle.stop().await.expect("stop ok");
        handle.shutdown();
        jh.join().unwrap();

        eprintln!(
            "captured {} samples, {} ms, truncated={}",
            pcm.samples.len(),
            pcm.duration_ms,
            pcm.truncated
        );
        assert_eq!(pcm.sample_rate, 16_000, "output must be 16 kHz");
        assert!(
            (1800..=2400).contains(&pcm.duration_ms),
            "duration {} ms outside 1800..2400 tolerance",
            pcm.duration_ms
        );
        assert!(!pcm.truncated, "a 2 s clip must not hit the 60 s cap");

        let as_f32: Vec<f32> = pcm.samples.iter().map(|&s| s as f32 / 32768.0).collect();
        let level = rms(&as_f32);
        assert!(
            level > 0.005,
            "RMS {level} is below the silence floor — was the mic muted or silent?"
        );

        // At least one non-trivial level was emitted during capture.
        assert!(
            sink.values().iter().any(|&v| v > 0.001),
            "expected some non-silent RMS level emits"
        );
    }
}
