//! Text normalization for speech (series tts-normalization, step N1): numbers,
//! money, percentages, dates and abbreviations become words before the text
//! reaches a speech engine. Offline, deterministic, engine-independent.

pub mod en;
pub mod ru;
pub mod ru_numbers;
