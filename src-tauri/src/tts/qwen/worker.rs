//! Hidden worker process (the same EXE with `--qwen-worker`): loads the
//! verified runtime, synthesizes one chunk per request into a private WAV.
//! No HTTP port, bounded stdio JSON, killed on cancel, timeout or parent exit.
use super::engine::{Engine, Failure, Options};
use crate::tts::{Capabilities, TtsBackend};
use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

/// Longest accepted request line (text of one chunk plus JSON).
const REQUEST_LIMIT: usize = 16 * 1024;
/// Longest accepted response line.
const RESPONSE_LIMIT: usize = 1024;
/// Model loading on a slow processor.
const READY_TIMEOUT: Duration = Duration::from_secs(180);
/// One chunk on a slow processor (≈ 15× slower than real time).
const CHUNK_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Characters per request: long enough for natural intonation.
pub const MAX_INPUT_CHARS: usize = 400;

/// Device settings, from fastest to safest. A device failure moves to the
/// next profile, which is remembered for this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Best available device (GPU), defaults.
    Gpu,
    /// GPU with FP16 clamping and a short decode chunk (old or 2 GB cards).
    SafeGpu,
    /// Processor only.
    Cpu,
}
impl Profile {
    fn options(self) -> Options<'static> {
        match self {
            Profile::Gpu => Options {
                device: None,
                clamp_fp16: false,
                codec_chunk_sec: 24.0,
            },
            Profile::SafeGpu => Options {
                device: None,
                clamp_fp16: true,
                codec_chunk_sec: 6.0,
            },
            Profile::Cpu => Options {
                device: Some("CPU"),
                clamp_fp16: false,
                codec_chunk_sec: 24.0,
            },
        }
    }
    fn tag(self) -> &'static str {
        match self {
            Profile::Gpu => "gpu",
            Profile::SafeGpu => "safe-gpu",
            Profile::Cpu => "cpu",
        }
    }
    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "gpu" => Some(Profile::Gpu),
            "safe-gpu" => Some(Profile::SafeGpu),
            "cpu" => Some(Profile::Cpu),
            _ => None,
        }
    }
    /// The next safer profile, if any.
    pub fn next(self) -> Option<Self> {
        match self {
            Profile::Gpu => Some(Profile::SafeGpu),
            Profile::SafeGpu => Some(Profile::Cpu),
            Profile::Cpu => None,
        }
    }
    /// The remembered profile of this computer (default: GPU).
    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(root.join("device-profile.txt"))
            .ok()
            .and_then(|s| Self::from_tag(s.trim()))
            .unwrap_or(Profile::Gpu)
    }
    fn save(self, root: &Path) {
        if std::fs::write(root.join("device-profile.txt"), self.tag()).is_err() {
            tracing::warn!("Qwen device profile could not be saved");
        }
    }
}

pub struct Worker {
    root: PathBuf,
    provider: &'static str,
    language: &'static str,
    profile: Profile,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    directory: PathBuf,
    cancel: Arc<AtomicBool>,
    pub last_used: Instant,
}

impl Worker {
    /// Start a worker for `language` (`russian`/`english`) with the remembered
    /// device profile.
    pub async fn start(
        root: &Path,
        cancel: Arc<AtomicBool>,
        provider: &'static str,
        language: &'static str,
    ) -> Result<Self, String> {
        let mut profile = Profile::load(root);
        loop {
            match Self::spawn(root, cancel.clone(), provider, language, profile).await {
                Ok(worker) => return Ok(worker),
                // Loading failed on the GPU (often memory): try a safer profile.
                Err(error) if error == "init" => match profile.next() {
                    Some(next) => {
                        tracing::warn!(
                            from = profile.tag(),
                            to = next.tag(),
                            "Qwen worker could not load; trying a safer device profile"
                        );
                        profile = next;
                        profile.save(root);
                    }
                    None => {
                        return Err(crate::preferences::message(
                            "Qwen3-TTS could not load the model on this computer.",
                            "Qwen3-TTS не смог загрузить модель на этом компьютере.",
                        ))
                    }
                },
                Err(error) => return Err(error),
            }
        }
    }

    async fn spawn(
        root: &Path,
        cancel: Arc<AtomicBool>,
        provider: &'static str,
        language: &'static str,
        profile: Profile,
    ) -> Result<Self, String> {
        let started = Instant::now();
        // Only private worker directories with UUID names; no library/user paths.
        for entry in std::fs::read_dir(root)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name
                .strip_prefix("worker-")
                .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok())
                && entry
                    .file_type()
                    .is_ok_and(|t| t.is_dir() && !t.is_symlink())
            {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
        let directory = root.join(format!("worker-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut command = Command::new(executable);
        command
            .arg("--qwen-worker")
            .arg(root)
            .arg(&directory)
            .arg(std::process::id().to_string())
            .arg(language)
            .arg(profile.tag())
            // ggml finds its backend DLLs in the current directory.
            .current_dir(root.join(super::catalog::RUNTIME_DIR))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command.spawn().map_err(|_| {
            crate::preferences::message(
                "Could not start Qwen3-TTS. Repair it in Settings.",
                "Не удалось запустить Qwen3-TTS. Восстановите его в настройках.",
            )
        })?;
        let input = child.stdin.take().ok_or("Missing worker input")?;
        let output = BufReader::new(child.stdout.take().ok_or("Missing worker output")?);
        let mut worker = Self {
            root: root.to_path_buf(),
            provider,
            language,
            profile,
            child,
            input,
            output,
            directory,
            cancel,
            last_used: Instant::now(),
        };
        let ready = worker.response(READY_TIMEOUT).await?;
        if ready["ready"] != true {
            return Err(if ready["init"] == true {
                "init".into()
            } else {
                crate::preferences::message(
                    "Qwen3-TTS could not load its files. Repair it in Settings.",
                    "Qwen3-TTS не смог загрузить свои файлы. Восстановите его в настройках.",
                )
            });
        }
        tracing::info!(
            profile = profile.tag(),
            elapsed_ms = started.elapsed().as_millis(),
            "Qwen worker ready"
        );
        Ok(worker)
    }

    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }

    async fn response(&mut self, timeout: Duration) -> Result<serde_json::Value, String> {
        let cancel = self.cancel.clone();
        let operation = async {
            let mut bytes = Vec::new();
            (&mut self.output)
                .take(RESPONSE_LIMIT as u64 + 1)
                .read_until(b'\n', &mut bytes)
                .await
                .map_err(|_| "worker exited".to_string())?;
            if bytes.len() > RESPONSE_LIMIT || !bytes.ends_with(b"\n") {
                return Err("worker exited".into());
            }
            serde_json::from_slice(&bytes).map_err(|_| "worker exited".into())
        };
        tokio::select! {
            result = tokio::time::timeout(timeout, operation) => result.unwrap_or_else(|_| Err("timeout".into())),
            _ = async { while !cancel.load(Ordering::Relaxed) { tokio::time::sleep(Duration::from_millis(100)).await; } } => Err("Озвучка отменена.".into()),
        }
        .map_err(|error: String| match error.as_str() {
            "timeout" => crate::preferences::message(
                "Qwen3-TTS did not finish a fragment in 15 minutes.",
                "Qwen3-TTS не озвучил фрагмент за 15 минут.",
            ),
            "worker exited" => crate::preferences::message(
                "Qwen3-TTS stopped unexpectedly.",
                "Qwen3-TTS неожиданно завершился.",
            ),
            _ => error,
        })
    }

    async fn request(&mut self, text: &str, voice: &str) -> Result<serde_json::Value, String> {
        let request = serde_json::json!({"text": text, "voice": voice}).to_string() + "\n";
        if request.len() > REQUEST_LIMIT {
            return Err("Слишком длинный фрагмент текста.".into());
        }
        self.input
            .write_all(request.as_bytes())
            .await
            .map_err(|_| "Qwen3-TTS неожиданно завершился.".to_string())?;
        self.input.flush().await.map_err(|e| e.to_string())?;
        self.response(CHUNK_TIMEOUT).await
    }

    /// Replace this worker by one with a safer profile (remembered).
    async fn degrade(&mut self) -> Result<bool, String> {
        let Some(next) = self.profile.next() else {
            return Ok(false);
        };
        tracing::warn!(
            from = self.profile.tag(),
            to = next.tag(),
            "Qwen device failure; switching to a safer device profile"
        );
        next.save(&self.root);
        self.shutdown().await;
        let replacement = Self::start(
            &self.root.clone(),
            self.cancel.clone(),
            self.provider,
            self.language,
        )
        .await?;
        let old = std::mem::replace(self, replacement);
        drop(old);
        Ok(true)
    }
}

impl TtsBackend for Worker {
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            provider: self.provider,
            voices: super::VOICES,
            max_input_chars: MAX_INPUT_CHARS,
            sample_rate: 24000,
        }
    }
    fn synthesize_chunk<'a>(
        &'a mut self,
        text: &'a str,
        voice: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<PathBuf>, String>> + Send + 'a>,
    > {
        Box::pin(async move {
            loop {
                if self.cancel.load(Ordering::Relaxed) {
                    return Err("Озвучка отменена.".into());
                }
                let response = self.request(text, voice).await?;
                self.last_used = Instant::now();
                if response["empty"] == true {
                    return Ok(None);
                }
                if response["ok"] == true {
                    let path = self.directory.join("chunk.wav");
                    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 48_000_000 {
                        return Err("Слишком большой фрагмент аудио.".into());
                    }
                    return Ok(Some(path));
                }
                if response["device"] == true && self.degrade().await? {
                    continue;
                }
                return Err(crate::preferences::message(
                    "Qwen3-TTS could not synthesize a text fragment.",
                    "Qwen3-TTS не смог озвучить фрагмент текста.",
                ));
            }
        })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    text: String,
    voice: String,
}

/// The child never initializes Tauri, opens the database or registers hotkeys.
pub fn child_main(
    root: &Path,
    directory: &Path,
    parent: u32,
    language: &str,
    profile: &str,
) -> Result<(), String> {
    crate::stt::moonshine::worker::watch_parent(parent)?;
    if !matches!(language, "russian" | "english") {
        return Err("Invalid language".into());
    }
    let profile = Profile::from_tag(profile).ok_or("Invalid profile")?;
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let reply = |output: &mut std::io::StdoutLock<'_>, value: serde_json::Value| {
        serde_json::to_writer(&mut *output, &value).map_err(|e| e.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())
    };
    let mut engine = match Engine::open(root, &profile.options()) {
        Ok(engine) => engine,
        Err(error) if error == "init" => {
            return reply(
                &mut output,
                serde_json::json!({"ready": false, "init": true}),
            )
        }
        Err(_) => return reply(&mut output, serde_json::json!({"ready": false})),
    };
    reply(&mut output, serde_json::json!({"ready": true}))?;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let wav = directory.join("chunk.wav");
    loop {
        let mut bytes = Vec::new();
        use std::io::Read;
        (&mut input)
            .take(REQUEST_LIMIT as u64 + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > REQUEST_LIMIT || !bytes.ends_with(b"\n") {
            return Err("Invalid worker request".into());
        }
        let request: Request =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid worker request")?;
        let text = request.text.trim();
        if text.is_empty() || !text.chars().any(char::is_alphanumeric) {
            reply(&mut output, serde_json::json!({"empty": true}))?;
            continue;
        }
        if !super::VOICES.contains(&request.voice.as_str()) {
            reply(&mut output, serde_json::json!({"ok": false}))?;
            continue;
        }
        let value = match engine.synthesize(text, language, &request.voice) {
            Ok(samples) if samples.is_empty() => serde_json::json!({"ok": false, "device": true}),
            Ok(samples) => match write_wav(&wav, &samples) {
                Ok(()) => serde_json::json!({"ok": true}),
                Err(_) => serde_json::json!({"ok": false}),
            },
            Err(Failure::Device) => serde_json::json!({"ok": false, "device": true}),
            Err(Failure::Other) => serde_json::json!({"ok": false}),
        };
        reply(&mut output, value)?;
    }
}

fn write_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 24_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        writer.write_sample(value).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_degrade_to_the_processor_and_are_remembered() {
        assert_eq!(Profile::Gpu.next(), Some(Profile::SafeGpu));
        assert_eq!(Profile::SafeGpu.next(), Some(Profile::Cpu));
        assert_eq!(Profile::Cpu.next(), None);
        let safe = Profile::SafeGpu.options();
        assert!(safe.clamp_fp16 && safe.codec_chunk_sec < 24.0 && safe.device.is_none());
        assert_eq!(Profile::Cpu.options().device, Some("CPU"));
        let root = std::env::temp_dir().join(format!("qwen-profile-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(Profile::load(&root), Profile::Gpu);
        Profile::Cpu.save(&root);
        assert_eq!(Profile::load(&root), Profile::Cpu);
        std::fs::write(root.join("device-profile.txt"), "unknown").unwrap();
        assert_eq!(Profile::load(&root), Profile::Gpu);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn wav_is_16_bit_mono_24_khz() {
        let path = std::env::temp_dir().join(format!("qwen-{}.wav", uuid::Uuid::new_v4()));
        write_wav(&path, &[0.0, 0.5, -2.0]).unwrap();
        let reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        assert_eq!(
            (spec.channels, spec.sample_rate, spec.bits_per_sample),
            (1, 24_000, 16)
        );
        let samples: Vec<i16> = reader.into_samples().map(Result::unwrap).collect();
        assert_eq!(samples, vec![0, 16383, -32767]);
        std::fs::remove_file(path).unwrap();
    }
}
