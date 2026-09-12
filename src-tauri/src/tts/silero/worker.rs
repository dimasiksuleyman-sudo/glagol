use super::VOICES;
use crate::tts::{Capabilities, TtsBackend};
use std::{
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

pub struct Worker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    directory: PathBuf,
    cancel: Arc<AtomicBool>,
    pub last_used: Instant,
}
impl Worker {
    pub async fn start(root: &Path, cancel: Arc<AtomicBool>) -> Result<Self, String> {
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
        let script = directory.join("worker.py");
        std::fs::write(&script, include_str!("worker.py")).map_err(|e| e.to_string())?;
        let mut command = Command::new(root.join("runtime/python.exe"));
        command
            .args(["-I", "-B", "-u"])
            .arg(script)
            .arg(root.join(super::catalog::MODEL.file))
            .arg(&directory)
            .arg(std::process::id().to_string())
            .current_dir(&directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = command.spawn().map_err(|_| {
            "Не удалось запустить движок озвучки. Восстановите его в настройках.".to_string()
        })?;
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut worker = Self {
            child,
            input,
            output,
            directory,
            cancel,
            last_used: Instant::now(),
        };
        let ready = worker.response().await?;
        if ready["ready"] != true {
            return Err("Движок Silero не смог загрузить модель.".into());
        }
        tracing::info!(
            model = super::MODEL_ID,
            elapsed_ms = started.elapsed().as_millis(),
            "local TTS worker ready"
        );
        Ok(worker)
    }
    pub async fn shutdown(&mut self) {
        let _ = self.child.kill().await;
    }
    async fn response(&mut self) -> Result<serde_json::Value, String> {
        let cancel = self.cancel.clone();
        let operation = async {
            let mut bytes = Vec::new();
            let mut limited = (&mut self.output).take(1025);
            limited
                .read_until(b'\n', &mut bytes)
                .await
                .map_err(|_| "Движок озвучки завершился.".to_string())?;
            if bytes.len() > 1024 || !bytes.ends_with(b"\n") {
                return Err("Некорректный ответ движка озвучки.".into());
            }
            serde_json::from_slice(&bytes).map_err(|_| "Некорректный ответ движка озвучки.".into())
        };
        tokio::pin!(operation);
        let result = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(90), &mut operation) => result.unwrap_or_else(|_| Err("Движок озвучки не ответил за 90 секунд.".into())),
            _ = async { while !cancel.load(Ordering::Relaxed) { tokio::time::sleep(Duration::from_millis(100)).await; } } => Err("Озвучка отменена.".into()),
        };
        result
    }
}
impl TtsBackend for Worker {
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            provider: "silero",
            voices: VOICES,
            max_input_chars: crate::text::chunker::DEFAULT_MAX_CHARS,
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
            if self.cancel.load(Ordering::Relaxed) {
                return Err("Озвучка отменена.".into());
            }
            let request = serde_json::json!({"text": text, "voice": voice}).to_string() + "\n";
            self.input
                .write_all(request.as_bytes())
                .await
                .map_err(|_| "Движок озвучки завершился.".to_string())?;
            self.input.flush().await.map_err(|e| e.to_string())?;
            let response = self.response().await?;
            self.last_used = Instant::now();
            if response["empty"] == true {
                return Ok(None);
            }
            if response["ok"] != true {
                return Err("Не удалось озвучить фрагмент текста.".into());
            }
            let path = self.directory.join("chunk.wav");
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 24_000_000 {
                return Err("Слишком большой фрагмент аудио.".into());
            }
            Ok(Some(path))
        })
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "requires the pinned runtime/model in GLAGOL_TTS_SMOKE_ROOT"]
    async fn native_silero_pipeline_and_cancel() {
        let root = PathBuf::from(std::env::var("GLAGOL_TTS_SMOKE_ROOT").expect("smoke root"));
        let cancel = Arc::new(AtomicBool::new(false));
        if std::env::var_os("GLAGOL_TTS_ASSEMBLE_SMOKE").is_some() {
            super::super::runtime::assemble(&root, &cancel).unwrap();
        }
        super::super::runtime::verify(&root.join("runtime"), &cancel).unwrap();
        let mut worker = Worker::start(&root, cancel.clone()).await.unwrap();
        let audio = root.join(format!("smoke-audio-{}", uuid::Uuid::new_v4()));
        let db = std::sync::Mutex::new(crate::db::test_connection());
        let text = "Сегодня 12 сентября 2026 года. Проверяем Windows и USB. На двери зам+ок, а на горе з+амок. Ты уже готов? ".repeat(6);
        let id = crate::commands::synthesize::synthesize_impl(
            &db,
            &audio,
            &text,
            "xenia",
            &mut worker,
            |_| {},
        )
        .await
        .unwrap();
        let row = crate::db::repository::get(&db.lock().unwrap(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(row.provider, "silero");
        assert!(row.audio_duration_ms.unwrap() > 10_000);
        let wav = hound::WavReader::open(audio.join(format!("{id}.wav"))).unwrap();
        assert_eq!(wav.spec().sample_rate, 24000);
        drop(wav);
        let flag = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            flag.store(true, Ordering::Relaxed);
        });
        let started = Instant::now();
        assert!(worker
            .synthesize_chunk(&"Проверяем отмену длинного фрагмента. ".repeat(7), "xenia")
            .await
            .is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
        worker.child.kill().await.unwrap();
        println!(
            "Silero pipeline: provider, multi-chunk WAV, duration and active cancellation passed"
        );
        std::fs::remove_dir_all(audio).unwrap();
    }
}
