//! Tauri commands exposed to the frontend.
//!
//! All commands return `Result<T, String>` — error types from underlying
//! modules (`KeyringError`, `WavJoinError`, `io::Error`)
//! are converted to plain strings at this boundary because Tauri's IPC
//! layer serializes errors to strings on the JavaScript side anyway.

pub mod backup;
pub mod dictation;
pub mod file;
pub mod speech;
pub mod storage;
pub mod synthesize;
pub mod tts;
