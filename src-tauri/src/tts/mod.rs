//! Provider-independent TTS contract: optional local Silero and Qwen3-TTS.
pub mod qwen;
pub mod silero;
use std::{future::Future, path::PathBuf, pin::Pin};

/// Provider identity and bounded input/output contract, independent of STT.
pub struct Capabilities {
    pub provider: &'static str,
    pub voices: &'static [&'static str],
    pub max_input_chars: usize,
    pub sample_rate: u32,
}

/// A chunk is an on-disk mono PCM WAV, never frontend IPC audio bytes.
pub trait TtsBackend {
    fn is_cancelled(&self) -> bool {
        false
    }
    fn capabilities(&self) -> Capabilities;
    fn synthesize_chunk<'a>(
        &'a mut self,
        text: &'a str,
        voice: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<PathBuf>, String>> + Send + 'a>>;
}

/// The running worker of whichever local engine the selected model uses.
pub enum LocalWorker {
    Silero(silero::Worker),
    Qwen(qwen::worker::Worker),
}
impl LocalWorker {
    /// When the worker last answered a request.
    pub fn last_used(&self) -> std::time::Instant {
        match self {
            LocalWorker::Silero(w) => w.last_used,
            LocalWorker::Qwen(w) => w.last_used,
        }
    }
    /// Terminate the worker process.
    pub async fn shutdown(&mut self) {
        match self {
            LocalWorker::Silero(w) => w.shutdown().await,
            LocalWorker::Qwen(w) => w.shutdown().await,
        }
    }
}
impl TtsBackend for LocalWorker {
    fn is_cancelled(&self) -> bool {
        match self {
            LocalWorker::Silero(w) => w.is_cancelled(),
            LocalWorker::Qwen(w) => w.is_cancelled(),
        }
    }
    fn capabilities(&self) -> Capabilities {
        match self {
            LocalWorker::Silero(w) => w.capabilities(),
            LocalWorker::Qwen(w) => w.capabilities(),
        }
    }
    fn synthesize_chunk<'a>(
        &'a mut self,
        text: &'a str,
        voice: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<PathBuf>, String>> + Send + 'a>> {
        match self {
            LocalWorker::Silero(w) => w.synthesize_chunk(text, voice),
            LocalWorker::Qwen(w) => w.synthesize_chunk(text, voice),
        }
    }
}
