//! SaluteSpeech API client.
//!
//! This module provides an async client for the SaluteSpeech REST API
//! by PJSC Sberbank, used by Glagol to synthesize Russian text into audio.
//!
//! # Architecture
//!
//! - [`http`] — shared HTTP client with TLS pinning to the embedded
//!   НУЦ Минцифры root certificate.
//! - [`auth`] — OAuth 2.0 client with thread-safe token caching.
//! - [`errors`] — error types returned by all operations.
//!
//! # Example (planned, not yet implemented)
//!
//! ```ignore
//! let client = http::build_client()?;
//! let auth = auth::SaluteAuth::new(client.clone(), auth_key);
//! let token = auth.get_token().await?;
//! // Use token for synthesis API calls (added in PR #3)
//! ```

pub mod auth;
pub mod errors;
pub mod http;
