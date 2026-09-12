//! Provider-independent TTS contract. The first backend is optional local Silero.
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
