use super::engine::Engine;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, Write},
    path::Path,
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

const LIMIT: usize = 512_000;
#[derive(Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Begin,
    Audio { samples: Vec<f32> },
    Finish,
}

pub struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    pub last_used: Instant,
}
impl Worker {
    pub async fn start(root: &Path, cancel: &AtomicBool) -> Result<Self, String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        #[cfg(test)]
        let executable = std::env::var_os("GLAGOL_MOONSHINE_TEST_EXE")
            .map(std::path::PathBuf::from)
            .unwrap_or(executable);
        let mut command = Command::new(executable);
        command
            .arg("--moonshine-worker")
            .arg(root)
            .arg(std::process::id().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .spawn()
            .map_err(|_| "Could not start the Moonshine worker.")?;
        let input = child.stdin.take().ok_or("Missing worker input")?;
        let output = BufReader::new(child.stdout.take().ok_or("Missing worker output")?);
        let mut worker = Self {
            child,
            input,
            output,
            last_used: Instant::now(),
        };
        worker.response(cancel).await?;
        Ok(worker)
    }
    pub async fn request(
        &mut self,
        request: Request,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let bytes = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        if bytes.len() >= LIMIT {
            return Err("Recognition message exceeds the limit.".into());
        }
        // The whole transaction is bounded, including pipe writes if the child hangs.
        tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(90), async {
                self.input.write_all(&bytes).await.map_err(|_| "The Moonshine worker exited.")?;
                self.input.write_all(b"\n").await.map_err(|_| "The Moonshine worker exited.")?;
                self.input.flush().await.map_err(|_| "The Moonshine worker exited.")?;
                self.response(cancel).await
            }) => result.map_err(|_| "Moonshine did not respond within 90 seconds.".to_string())?,
            _ = cancelled(cancel) => Err("Recognition cancelled.".into()),
        }
    }
    async fn response(&mut self, cancel: &AtomicBool) -> Result<String, String> {
        self.response_with_timeout(cancel, Duration::from_secs(90))
            .await
    }
    async fn response_with_timeout(
        &mut self,
        cancel: &AtomicBool,
        timeout: Duration,
    ) -> Result<String, String> {
        let response = async {
            let mut bytes = Vec::new();
            (&mut self.output)
                .take((LIMIT + 1) as u64)
                .read_until(b'\n', &mut bytes)
                .await
                .map_err(|_| "The Moonshine worker exited.")?;
            if bytes.len() > LIMIT || !bytes.ends_with(b"\n") {
                return Err("Invalid Moonshine worker response.".into());
            }
            let value: serde_json::Value =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid Moonshine worker response.")?;
            if value["ok"] != true {
                return Err("The Moonshine worker could not process this recording.".into());
            }
            self.last_used = Instant::now();
            Ok(value["text"].as_str().unwrap_or("").to_owned())
        };
        tokio::select! {
            result = tokio::time::timeout(timeout, response) => result.map_err(|_| "Moonshine did not respond within 90 seconds.".to_string())?,
            _ = cancelled(cancel) => Err("Recognition cancelled.".into()),
        }
    }
    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }
}
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

/// The child never initializes Tauri, opens the database, or registers hotkeys.
pub fn child_main(root: &Path, parent: u32) -> Result<(), String> {
    watch_parent(parent)?;
    let mut engine = Engine::open(root)?;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let reply = |output: &mut std::io::StdoutLock<'_>, text: &str| -> Result<(), String> {
        serde_json::to_writer(&mut *output, &serde_json::json!({"ok": true, "text": text}))
            .map_err(|e| e.to_string())?;
        output
            .write_all(b"\n")
            .and_then(|_| output.flush())
            .map_err(|e| e.to_string())
    };
    reply(&mut output, "")?;
    let mut sample_count = 0usize;
    loop {
        let mut bytes = Vec::new();
        use std::io::Read;
        (&mut input)
            .take((LIMIT + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > LIMIT || !bytes.ends_with(b"\n") {
            return Err("Invalid worker request".into());
        }
        let request: Request =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid worker request")?;
        let text = match request {
            Request::Begin => {
                sample_count = 0;
                engine.begin()?;
                String::new()
            }
            Request::Audio { samples } => {
                sample_count += samples.len();
                if samples.len() > 4096
                    || sample_count > 16000 * 60
                    || samples.iter().any(|s| !s.is_finite() || s.abs() > 1.01)
                {
                    return Err("Invalid worker audio".into());
                }
                engine.audio(&samples)?;
                String::new()
            }
            Request::Finish => engine.finish()?,
        };
        reply(&mut output, &text)?;
    }
}

#[cfg(windows)]
fn watch_parent(parent: u32) -> Result<(), String> {
    // A kernel handle survives PID reuse and detects parent crashes even while
    // native inference is stuck. Open it before loading any downloaded code.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut std::ffi::c_void;
        fn WaitForSingleObject(handle: *mut std::ffi::c_void, timeout: u32) -> u32;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    // SAFETY: only SYNCHRONIZE access is requested; null is checked before use.
    let handle = unsafe { OpenProcess(0x00100000, 0, parent) };
    if handle.is_null() {
        return Err("Parent process is unavailable".into());
    }
    let raw = handle as usize;
    // SAFETY: this thread exclusively owns the live process handle until close.
    std::thread::spawn(move || unsafe {
        WaitForSingleObject(raw as *mut _, u32::MAX);
        CloseHandle(raw as *mut _);
        std::process::exit(0);
    });
    Ok(())
}
#[cfg(not(windows))]
fn watch_parent(_: u32) -> Result<(), String> {
    Err("Moonshine worker requires Windows x64".into())
}

pub enum Packet {
    Audio(Vec<f32>),
    Finish,
}
#[derive(Clone)]
pub struct Input {
    pub sender: tokio::sync::mpsc::Sender<Packet>,
    pub failed: Arc<AtomicBool>,
}
impl Input {
    pub fn push(&self, samples: &[f32]) -> Result<(), String> {
        if self.failed.load(Ordering::Relaxed) {
            return Err("Recognition stopped after an engine or audio queue error.".into());
        }
        for block in samples.chunks(4096) {
            self.send(Packet::Audio(block.to_vec()))?;
        }
        Ok(())
    }
    pub fn finish(&self) -> Result<(), String> {
        self.send(Packet::Finish)
    }
    fn send(&self, packet: Packet) -> Result<(), String> {
        self.sender.try_send(packet).map_err(|_| {
            self.failed.store(true, Ordering::Relaxed);
            "The recognition audio queue overflowed or the engine stopped. Repeat your dictation."
                .into()
        })
    }
}

pub struct Streaming {
    pub input: Input,
    pub result: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<Result<String, String>>>>,
}
impl Drop for Streaming {
    fn drop(&mut self) {
        self.input.failed.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[tokio::test]
    async fn unresponsive_process_times_out_and_is_terminated() {
        let mut child = Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", "Start-Sleep -Seconds 30"])
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut worker = Worker {
            input: child.stdin.take().unwrap(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            last_used: Instant::now(),
        };
        assert!(worker
            .response_with_timeout(&AtomicBool::new(false), Duration::from_millis(50))
            .await
            .unwrap_err()
            .contains("90 seconds"));
        worker.shutdown().await;
        assert!(worker.child.try_wait().unwrap().is_some());
    }
    #[tokio::test]
    #[ignore = "requires pinned Moonshine artifacts, GLAGOL_MOONSHINE_ROOT, GLAGOL_MOONSHINE_TEST_EXE and GLAGOL_MOONSHINE_WAV"]
    async fn native_moonshine_streaming_process() {
        let root = std::path::PathBuf::from(std::env::var("GLAGOL_MOONSHINE_ROOT").unwrap());
        let wav = std::env::var("GLAGOL_MOONSHINE_WAV").unwrap();
        let mut reader = hound::WavReader::open(wav).unwrap();
        assert_eq!(reader.spec().channels, 1);
        let rate = reader.spec().sample_rate;
        let samples: Vec<f32> = reader
            .samples::<i16>()
            .map(|s| s.unwrap() as f32 / 32768.0)
            .collect();
        let cancel = AtomicBool::new(false);
        let started = Instant::now();
        let mut worker = Worker::start(&root, &cancel).await.unwrap();
        eprintln!(
            "Moonshine cold worker ready: {} ms",
            started.elapsed().as_millis()
        );
        for block in [997, 4096] {
            worker.request(Request::Begin, &cancel).await.unwrap();
            let mut resampler =
                crate::dictation::stream_resample::StreamResampler::new(rate).unwrap();
            for chunk in samples.chunks(block) {
                for chunk in resampler.push(chunk).unwrap().chunks(4096) {
                    worker
                        .request(
                            Request::Audio {
                                samples: chunk.to_vec(),
                            },
                            &cancel,
                        )
                        .await
                        .unwrap();
                }
            }
            for chunk in resampler.finish().unwrap().chunks(4096) {
                worker
                    .request(
                        Request::Audio {
                            samples: chunk.to_vec(),
                        },
                        &cancel,
                    )
                    .await
                    .unwrap();
            }
            let release = Instant::now();
            let text = worker.request(Request::Finish, &cancel).await.unwrap();
            eprintln!(
                "Moonshine final latency: {} ms (block {block})",
                release.elapsed().as_millis()
            );
            assert!(!text.trim().is_empty());
            assert!(
                text.to_lowercase().contains("hello"),
                "synthetic beginning was lost"
            );
            assert!(
                text.to_lowercase().contains("listen"),
                "synthetic ending was lost"
            );
        }
        worker.shutdown().await;
        assert!(worker.request(Request::Begin, &cancel).await.is_err());
        drop(worker);
        let mut worker = Worker::start(&root, &cancel).await.unwrap();
        worker.request(Request::Begin, &cancel).await.unwrap();
        worker
            .request(
                Request::Audio {
                    samples: vec![0.0; 4096],
                },
                &cancel,
            )
            .await
            .unwrap();
        assert!(worker
            .request(Request::Finish, &cancel)
            .await
            .unwrap()
            .trim()
            .is_empty());
        cancel.store(true, Ordering::Relaxed);
        assert!(worker.request(Request::Begin, &cancel).await.is_err());
        worker.shutdown().await;
        eprintln!("MOONSHINE NATIVE PROCESS PASS: streaming, warm reuse, silence, crash recovery, cancellation");
    }
}
