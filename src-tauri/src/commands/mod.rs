//! Tauri commands exposed to the frontend.
//!
//! All commands return `Result<T, String>` — error types from underlying
//! modules (`KeyringError`, `SaluteError`, `WavJoinError`, `io::Error`)
//! are converted to plain strings at this boundary because Tauri's IPC
//! layer serializes errors to strings on the JavaScript side anyway.

pub mod backup;
pub mod credentials;
pub mod file;
pub mod storage;
pub mod synthesize;
pub mod usage;
