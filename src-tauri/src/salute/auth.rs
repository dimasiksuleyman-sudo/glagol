//! OAuth 2.0 client for SaluteSpeech API.
//!
//! SaluteSpeech uses a Client Credentials OAuth flow with the following
//! characteristics:
//!
//! - Auth endpoint: `https://ngw.devices.sberbank.ru:9443/api/v2/oauth`
//!   (note the non-standard port 9443!)
//! - Long-lived `Authorization Key`: a Base64-encoded `client_id:client_secret`
//!   pair provided by Sberbank in the developer console. Persistent secret.
//! - Short-lived `access_token`: a JWT valid for ~30 minutes, obtained
//!   via OAuth and used as a `Bearer` token for synthesis API calls.
//!
//! # Token caching strategy
//!
//! [`SaluteAuth`] caches the access token in a thread-safe [`RwLock`].
//! On every call to [`SaluteAuth::get_token`]:
//!
//! 1. If the cached token expires more than 60 seconds in the future,
//!    return it (no network call).
//! 2. Otherwise, request a fresh token from Sberbank, cache it, return it.
//!
//! The 60-second buffer protects against clock skew and request latency:
//! a token that expires in 5 seconds may already be invalid by the time
//! the synthesis request reaches Sberbank.
//!
//! # Thread safety
//!
//! [`SaluteAuth`] is safe to clone and share across tokio tasks. The
//! internal [`RwLock`] ensures only one task refreshes the token at a
//! time; other tasks reading during a refresh will wait briefly, then
//! see the new token.

use crate::salute::errors::{SaluteError, SaluteResult};
use crate::salute::http;
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Default OAuth endpoint for SaluteSpeech (production).
///
/// Note the non-standard port `9443` — `ngw.devices.sberbank.ru` does not
/// listen on the default HTTPS port 443. Sberbank documents this explicitly.
const DEFAULT_OAUTH_URL: &str = "https://ngw.devices.sberbank.ru:9443/api/v2/oauth";

/// OAuth scope for personal (free tier) usage.
///
/// Other available scopes: `SALUTE_SPEECH_CORP` (corporate, paid),
/// `GIGACHAT_API_PERS`, etc. — but Glagol only needs synthesis access.
const SCOPE_PERS: &str = "SALUTE_SPEECH_PERS";
/// Refresh the token if it expires within this many milliseconds.
///
/// 60 000 ms = 60 seconds. Protects against:
/// - Clock skew between our machine and Sberbank servers
/// - Network latency on synthesis requests (a token expiring in 3 seconds
///   may be invalid by the time our request hits Sberbank's gateway)
const REFRESH_BUFFER_MS: i64 = 60_000;

/// Cached OAuth token with its expiration timestamp.
#[derive(Clone, Debug)]
struct CachedToken {
    /// The actual JWT access token to use in `Authorization: Bearer ...` headers.
    access_token: String,

    /// Token expiration time as Unix timestamp in **milliseconds**.
    /// (Sberbank returns `expires_at` in milliseconds, not seconds.)
    expires_at_ms: i64,
}

/// Raw OAuth response payload from `https://ngw.devices.sberbank.ru:9443/api/v2/oauth`.
///
/// Example successful response:
/// ```json
/// {
///   "access_token": "eyJjdHkiOiJqd3QiLCJlbmMiO...",
///   "expires_at": 1747068024000
/// }
/// ```
#[derive(Deserialize, Debug)]
struct TokenResponse {
    access_token: String,
    /// Unix milliseconds.
    expires_at: i64,
}

/// OAuth client for SaluteSpeech.
///
/// Construct once per app session, share across tokio tasks via [`Arc`]
/// or by clone (the internal HTTP client and lock are cheap to clone).
///
/// # Example (planned, full implementation in next step)
///
/// ```ignore
/// use glagol_lib::salute::{http, auth::SaluteAuth};
///
/// let client = http::build_client()?;
/// let auth = SaluteAuth::new(client, my_auth_key);
/// let token = auth.get_token().await?;
/// // ... use token ...
/// ```
#[derive(Debug)]
pub struct SaluteAuth {
    /// Shared HTTP client (already configured with TLS pinning).
    client: Client,

    /// `Authorization Key` from Sberbank developer console.
    /// This is Base64(client_id:client_secret), passed as `Authorization: Basic <key>`.
    auth_key: String,

    /// OAuth scope to request. Defaults to `SALUTE_SPEECH_PERS`.
    scope: String,

    /// OAuth endpoint URL. Defaults to Sberbank production; can be
    /// overridden via [`SaluteAuth::with_base_url`] for tests (e.g. mockito).
    oauth_url: String,

    /// Cached token state. `None` if no token has been obtained yet.
    token_cache: RwLock<Option<CachedToken>>,
}

impl SaluteAuth {
    /// Create a new OAuth client with production defaults.
    ///
    /// # Arguments
    ///
    /// - `client` — pre-built [`reqwest::Client`] with TLS pinning (use
    ///   [`crate::salute::http::build_client`]).
    /// - `auth_key` — Base64-encoded `client_id:client_secret` from the
    ///   Sberbank developer console.
    pub fn new(client: Client, auth_key: String) -> Self {
        Self::with_base_url(client, auth_key, DEFAULT_OAUTH_URL.to_string())
    }

    /// Create a new OAuth client with a custom OAuth endpoint URL.
    ///
    /// This is primarily used in unit tests with `mockito`, which serves
    /// HTTP (not HTTPS) on `127.0.0.1`. Production code should use
    /// [`SaluteAuth::new`] instead.
    pub fn with_base_url(client: Client, auth_key: String, oauth_url: String) -> Self {
        Self {
            client,
            auth_key,
            scope: SCOPE_PERS.to_string(),
            oauth_url,
            token_cache: RwLock::new(None),
        }
    }

    /// Get a valid access token, refreshing from Sberbank if needed.
    ///
    /// Implementation:
    ///
    /// 1. Acquire a **read** lock on the token cache (cheap, non-blocking
    ///    for other readers).
    /// 2. If a cached token exists and its expiry is more than
    ///    `REFRESH_BUFFER_MS` (60 seconds) in the future, return it.
    /// 3. Otherwise, drop the read lock and call [`Self::refresh_token`],
    ///    which acquires a **write** lock and fetches a fresh token.
    ///
    /// The 60-second buffer protects against clock skew and request latency.
    ///
    /// # Errors
    ///
    /// All errors from [`Self::refresh_token`] propagate (network, auth,
    /// rate-limit, etc.).
    pub async fn get_token(&self) -> SaluteResult<String> {
        // Fast path: read cached token under read lock.
        {
            let guard = self.token_cache.read().await;
            if let Some(cached) = guard.as_ref() {
                let now_ms = Utc::now().timestamp_millis();
                let expires_in_ms = cached.expires_at_ms - now_ms;

                if expires_in_ms > REFRESH_BUFFER_MS {
                    debug!(
                        expires_in_secs = expires_in_ms / 1000,
                        "using cached SaluteSpeech token"
                    );
                    return Ok(cached.access_token.clone());
                }
            }
        } // read lock released here

        // Slow path: need to refresh.
        self.refresh_token().await
    }

    /// Force a token refresh, ignoring any cached value.
    ///
    /// Makes a `POST` request to the OAuth endpoint with:
    ///
    /// - `Authorization: Basic <auth_key>`
    /// - `RqUID: <new uuid v4>`
    /// - `Content-Type: application/x-www-form-urlencoded`
    /// - body: `scope=SALUTE_SPEECH_PERS`
    ///
    /// Stores the resulting token in the cache and returns it.
    ///
    /// # Errors
    ///
    /// - [`SaluteError::Auth`] on HTTP 401 (invalid Authorization Key).
    /// - [`SaluteError::RateLimited`] on HTTP 429.
    /// - [`SaluteError::Api`] on other 4xx/5xx responses.
    /// - [`SaluteError::Network`] on network/TLS errors.
    /// - [`SaluteError::InvalidResponse`] if the response body cannot be
    ///   parsed as the expected JSON shape.
    pub async fn refresh_token(&self) -> SaluteResult<String> {
        let rquid = http::new_rquid();
        debug!(rquid = %rquid, url = %self.oauth_url, "requesting OAuth token");

        let response = self
            .client
            .post(&self.oauth_url)
            .header("Authorization", format!("Basic {}", self.auth_key))
            .header("RqUID", &rquid)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&[("scope", self.scope.as_str())])
            .send()
            .await
            .map_err(|e| {
                warn!(rquid = %rquid, error = %e, "OAuth network error");
                SaluteError::from(e)
            })?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            warn!(
                rquid = %rquid,
                status = %status,
                body_len = body.len(),
                "OAuth non-success response"
            );
            return Err(match status {
                StatusCode::UNAUTHORIZED => SaluteError::Auth { message: body },
                StatusCode::TOO_MANY_REQUESTS => SaluteError::RateLimited {
                    retry_after_secs: 5,
                },
                _ => SaluteError::Api {
                    status: status.as_u16(),
                    body,
                },
            });
        }

        let token_response: TokenResponse = serde_json::from_str(&body).map_err(|e| {
            SaluteError::InvalidResponse(format!("failed to parse OAuth response: {}", e))
        })?;

        // Update cache under write lock.
        {
            let mut guard = self.token_cache.write().await;
            *guard = Some(CachedToken {
                access_token: token_response.access_token.clone(),
                expires_at_ms: token_response.expires_at,
            });
        }

        info!(
            rquid = %rquid,
            expires_at_ms = token_response.expires_at,
            "OAuth token refreshed successfully"
        );

        Ok(token_response.access_token)
    }

    /// Clear the cached token so the next [`Self::get_token`] call forces
    /// a fresh OAuth request.
    ///
    /// This is the recovery path when SaluteSpeech rejects our access
    /// token with HTTP 401 *before* the cached `expires_at` has passed
    /// (Sberbank can revoke tokens server-side). Without this, the cache
    /// would happily return the same dead token on every retry.
    ///
    /// Cheap: takes a write lock just long enough to assign `None`.
    pub async fn invalidate(&self) {
        let mut guard = self.token_cache.write().await;
        *guard = None;
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::salute::http;

    #[test]
    fn test_new_uses_default_url() {
        let client = http::build_client().expect("client builds");
        let auth = SaluteAuth::new(client, "fake_auth_key".to_string());
        assert_eq!(auth.oauth_url, DEFAULT_OAUTH_URL);
        assert_eq!(auth.scope, SCOPE_PERS);
        assert_eq!(auth.auth_key, "fake_auth_key");
    }

    #[test]
    fn test_with_base_url_overrides_endpoint() {
        let client = http::build_client().expect("client builds");
        let custom_url = "http://localhost:1234/mock/oauth".to_string();
        let auth = SaluteAuth::with_base_url(client, "fake".to_string(), custom_url.clone());
        assert_eq!(auth.oauth_url, custom_url);
    }

    #[tokio::test]
    async fn test_cache_starts_empty() {
        let client = http::build_client().expect("client builds");
        let auth = SaluteAuth::new(client, "fake".to_string());
        let cache = auth.token_cache.read().await;
        assert!(cache.is_none(), "fresh SaluteAuth should have empty cache");
    }
    // ========================================================================
    // OAuth flow tests with mockito (no real network calls to Sberbank)
    // ========================================================================

    /// Helper: build a SaluteAuth wired to a mockito server.
    ///
    /// Returns `(server, auth)`. The server's lifetime must outlive the auth
    /// instance, hence we return both. Drop order matters: drop `auth` first,
    /// then `server` (Rust does this automatically by reverse declaration).
    fn make_test_auth(server: &mockito::Server) -> SaluteAuth {
        let client = http::build_client().expect("client builds");
        let oauth_url = format!("{}/api/v2/oauth", server.url());
        SaluteAuth::with_base_url(client, "test_auth_key_abc".to_string(), oauth_url)
    }

    #[tokio::test]
    async fn test_oauth_success_returns_token() {
        let mut server = mockito::Server::new_async().await;

        // Mockito setup: when POST /api/v2/oauth arrives, respond with valid JSON.
        let _mock = server
            .mock("POST", "/api/v2/oauth")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"access_token":"fake_jwt_token_xyz","expires_at":99999999999999}"#)
            .create_async()
            .await;

        let auth = make_test_auth(&server);
        let result = auth.get_token().await;

        assert!(result.is_ok(), "expected success, got: {:?}", result);
        assert_eq!(result.unwrap(), "fake_jwt_token_xyz");
    }

    #[tokio::test]
    async fn test_oauth_401_returns_auth_error() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/api/v2/oauth")
            .with_status(401)
            .with_body(r#"{"error":"invalid_credentials"}"#)
            .create_async()
            .await;

        let auth = make_test_auth(&server);
        let result = auth.get_token().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::Auth { message } => {
                assert!(
                    message.contains("invalid_credentials"),
                    "auth error body should contain server response, got: {}",
                    message
                );
            }
            other => panic!("expected SaluteError::Auth, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_oauth_429_returns_rate_limited() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/api/v2/oauth")
            .with_status(429)
            .with_body(r#"{"error":"too_many_requests"}"#)
            .create_async()
            .await;

        let auth = make_test_auth(&server);
        let result = auth.get_token().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::RateLimited { retry_after_secs } => {
                assert!(retry_after_secs > 0, "retry_after should be positive");
            }
            other => panic!("expected SaluteError::RateLimited, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_oauth_500_returns_api_error() {
        let mut server = mockito::Server::new_async().await;

        let _mock = server
            .mock("POST", "/api/v2/oauth")
            .with_status(500)
            .with_body("internal server error")
            .create_async()
            .await;

        let auth = make_test_auth(&server);
        let result = auth.get_token().await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SaluteError::Api { status, body } => {
                assert_eq!(status, 500);
                assert!(
                    body.contains("internal server error"),
                    "API error body should contain server response"
                );
            }
            other => panic!("expected SaluteError::Api, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_invalidate_forces_token_refresh() {
        let mut server = mockito::Server::new_async().await;

        // Expect exactly TWO calls: one before invalidate(), one after.
        // Without invalidate the second call would hit the cache and not
        // touch the network.
        let mock = server
            .mock("POST", "/api/v2/oauth")
            .with_status(200)
            .with_body(r#"{"access_token":"reissued_token","expires_at":99999999999999}"#)
            .expect(2)
            .create_async()
            .await;

        let auth = make_test_auth(&server);

        let first = auth.get_token().await.expect("first call succeeds");
        auth.invalidate().await;
        let second = auth
            .get_token()
            .await
            .expect("post-invalidate call succeeds");

        assert_eq!(first, "reissued_token");
        assert_eq!(second, "reissued_token");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_oauth_token_is_cached_between_calls() {
        let mut server = mockito::Server::new_async().await;

        // Mockito setup: expect EXACTLY ONE call. If get_token() is called
        // twice but only one HTTP request goes out, cache works.
        let mock = server
            .mock("POST", "/api/v2/oauth")
            .with_status(200)
            .with_body(r#"{"access_token":"cached_token","expires_at":99999999999999}"#)
            .expect(1) // <-- only one HTTP call allowed
            .create_async()
            .await;

        let auth = make_test_auth(&server);

        let token1 = auth.get_token().await.expect("first call succeeds");
        let token2 = auth.get_token().await.expect("second call succeeds");

        assert_eq!(token1, token2, "cached call should return same token");
        assert_eq!(token1, "cached_token");

        // This will panic if more or fewer HTTP requests happened.
        mock.assert_async().await;
    }
}
