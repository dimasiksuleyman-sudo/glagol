//! Text processing utilities.
//!
//! Currently exposes [`chunker`] for splitting long UTF-8 text into pieces
//! small enough for the SaluteSpeech sync API (4000-char limit per request).
//!
//! A `preprocessor` module (URL replacement, abbreviation expansion, HTML
//! entity decoding, etc.) is planned for Sprint 3 and will run BEFORE
//! the chunker on the pipeline.

pub mod chunker;
