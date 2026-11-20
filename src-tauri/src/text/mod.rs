//! Text processing utilities.
//!
//! - [`preprocessor`] humanises mechanical patterns (URLs, emails,
//!   common abbreviations) before synthesis so the audio flows naturally.
//! - [`chunker`] splits long UTF-8 text into pieces small enough for
//!   the selected TTS backend.
//!
//! Pipeline order in `synthesize_impl`: `preprocessor` →
//! `chunker` → sequential synthesis → streamed WAV.

mod ai_fragments;
pub mod ai_normalization;
pub mod brackets;
pub mod chunker;
pub mod dictionary;
pub mod normalize;
pub mod preprocessor;
