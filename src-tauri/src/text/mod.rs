//! Text processing utilities.
//!
//! - [`preprocessor`] humanises mechanical patterns (URLs, emails,
//!   common abbreviations) before synthesis so the audio flows naturally.
//! - [`chunker`] splits long UTF-8 text into pieces small enough for
//!   the SaluteSpeech sync API (4000-char limit per request).
//!
//! Pipeline order in `synthesize_document_impl`: `preprocessor` →
//! `chunker` → loop synthesize → wav_join.

pub mod chunker;
pub mod preprocessor;
