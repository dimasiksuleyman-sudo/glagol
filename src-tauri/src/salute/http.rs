//! Shared HTTP client and request utilities for SaluteSpeech API.
//!
//! This module exists because **all** HTTP calls to Sberbank servers
//! must go through a `reqwest::Client` that trusts the **НУЦ Минцифры РФ**
//! root certificate. Sberbank's TLS certificates are signed by this
//! Russian Ministry of Digital Development root CA, which is NOT included
//! in standard Windows/macOS/Linux truststores by default (as of 2026).
//!
//! Instead of asking the user to install the certificate manually, we
//! **embed it directly into the binary** at compile time via [`include_bytes!`]
//! and add it as an extra root certificate to every reqwest client.
//!
//! # Security note
//!
//! The certificate is a **public** document, signed by НУЦ Минцифры. It is
//! safe to commit to a public repository. It only allows the client to
//! **verify** that a server claiming to be Sberbank really is Sberbank;
//! it does not grant any privileged access.
//!
//! See [`SECURITY.md`](../../../SECURITY.md) for the full security model.

use crate::salute::errors::{SaluteError, SaluteResult};
use reqwest::{Certificate, Client};
use std::time::Duration;

/// Embedded НУЦ Минцифры РФ root certificate (PEM format).
///
/// Source: https://www.gosuslugi.ru/crt — `russian_trusted_root_ca.cer`
/// (RSA variant — NOT the ГОСТ 2025 variant, which uses Russian-only
/// cryptography unsupported by rustls).
///
/// Validity: until 2046.
const MINCIFRY_ROOT_CERT_PEM: &[u8] = include_bytes!("../../assets/russiantrustedca.pem");

/// Default timeout for individual HTTP requests (30 seconds).
///
/// SaluteSpeech synthesis can take a few seconds for long texts;
/// 30 seconds is generous for the sync API (max 4000 chars input).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Timeout for the initial TCP + TLS handshake (10 seconds).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Build a configured [`reqwest::Client`] ready to talk to SaluteSpeech.
///
/// The returned client:
///
/// - Trusts the embedded НУЦ Минцифры root certificate (TLS pinning).
/// - Uses pure-Rust TLS (`rustls`) — no OpenSSL dependency on Windows.
/// - Has reasonable timeouts ([`REQUEST_TIMEOUT`] and [`CONNECT_TIMEOUT`]).
/// - Sends a `User-Agent` header identifying Glagol and its version.
///
/// # Errors
///
/// Returns [`SaluteError::Certificate`] if the embedded certificate cannot
/// be parsed (should never happen in a properly built binary — this is
/// a compile-time invariant we enforce via `include_bytes!`).
///
/// Returns [`SaluteError::Network`] if reqwest's builder fails for some
/// other reason (extremely rare).
pub fn build_client() -> SaluteResult<Client> {
    let cert = Certificate::from_pem(MINCIFRY_ROOT_CERT_PEM).map_err(|e| {
        SaluteError::Certificate(format!(
            "failed to parse embedded НУЦ Минцифры root certificate: {}",
            e
        ))
    })?;

    let client = Client::builder()
        .add_root_certificate(cert)
        .use_rustls_tls()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent(format!(
            "Glagol/{} (+https://github.com/dimasiksuleyman-sudo/glagol)",
            env!("CARGO_PKG_VERSION")
        ))
        .build()?;

    Ok(client)
}

/// Generate a fresh `RqUID` (request unique identifier) for a SaluteSpeech
/// API call.
///
/// SaluteSpeech requires every request to carry a unique `RqUID` header
/// (a UUID v4 in standard 8-4-4-4-12 hex format). Sberbank uses this
/// value as their `X-Request-ID` internally — if you need to contact
/// Sberbank support about a failed request, this is what they will ask for.
///
/// Callers should generate a new RqUID **per request**, log it before
/// sending, and include it in any error messages. See [`crate::salute::auth`]
/// for an example.
pub fn new_rquid() -> String {
    uuid::Uuid::new_v4().to_string()
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_client_succeeds() {
        // The embedded certificate must be parseable and the client
        // must build without errors. This is a smoke test that verifies
        // our PEM file is correctly formatted and bundled.
        let result = build_client();
        assert!(result.is_ok(), "build_client() failed: {:?}", result.err());
    }

    #[test]
    fn test_new_rquid_is_unique() {
        // Two consecutive calls must produce different UUIDs.
        let a = new_rquid();
        let b = new_rquid();
        assert_ne!(
            a, b,
            "two consecutive new_rquid() calls returned the same value"
        );
    }

    #[test]
    fn test_new_rquid_is_valid_uuid_v4() {
        // RqUID must be a parseable UUID v4 (length 36, format 8-4-4-4-12).
        let rquid = new_rquid();
        assert_eq!(rquid.len(), 36, "RqUID has unexpected length: {}", rquid);
        let parsed = uuid::Uuid::parse_str(&rquid);
        assert!(parsed.is_ok(), "RqUID is not a valid UUID: {}", rquid);
        // v4 UUIDs have the version digit '4' at position 14.
        assert_eq!(
            rquid.chars().nth(14).unwrap(),
            '4',
            "RqUID is not v4: {}",
            rquid
        );
    }
}
