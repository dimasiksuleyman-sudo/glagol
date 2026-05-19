//! Tauri commands for managing the SaluteSpeech Authorization Key.
//!
//! Three commands form a complete CRUD-ish surface for the single
//! credential Glagol stores:
//!
//! - [`set_credentials`] — write the key to the OS keyring and reset
//!   the cached [`SaluteAuth`] so the next call uses the new key.
//! - [`test_credentials`] — perform a real OAuth handshake against
//!   Sberbank to validate the saved key, caching the resulting
//!   [`SaluteAuth`] for subsequent synthesize calls.
//! - [`delete_credentials`] — remove the key from the keyring
//!   (idempotently — already-missing is treated as success) and clear
//!   the cached auth.
//!
//! Each public Tauri command is a thin wrapper over an `*_impl`
//! function that takes plain `&AppState`. Unit tests target the impl
//! functions directly because `tauri::State<'_, T>` cannot be
//! constructed outside a running Tauri runtime.

use std::sync::Arc;

use crate::salute::auth::SaluteAuth;
use crate::salute::errors::SaluteError;
use crate::secrets::keyring::{self, KeyringError};
use crate::state::AppState;

#[tauri::command]
pub async fn set_credentials(
    state: tauri::State<'_, AppState>,
    auth_key: String,
) -> Result<(), String> {
    set_credentials_impl(&state, &auth_key).await
}

#[tauri::command]
pub async fn test_credentials(
    state: tauri::State<'_, AppState>,
    force: bool,
) -> Result<(), String> {
    test_credentials_impl(&state, force).await
}

#[tauri::command]
pub async fn delete_credentials(state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_credentials_impl(&state).await
}

/// Store the SaluteSpeech Authorization Key in the OS keyring and
/// invalidate any cached [`SaluteAuth`].
///
/// The cached auth is reset (not rebuilt) so the next operation
/// requiring credentials picks up the new key from the keyring.
/// Building a fresh `SaluteAuth` here would couple `set_credentials`
/// to the HTTP client and require validating the key against Sberbank
/// — that is the explicit job of [`test_credentials_impl`].
pub(crate) async fn set_credentials_impl(state: &AppState, auth_key: &str) -> Result<(), String> {
    keyring::set_auth_key(auth_key).map_err(|e| e.to_string())?;

    let mut guard = state.salute_auth.lock().await;
    *guard = None;

    Ok(())
}

/// Validate the stored Authorization Key. With `force = false` (the
/// mount-time probe), returns `Ok(())` immediately if we already
/// authenticated this process lifetime — the in-memory
/// [`SaluteAuth`] in `state.salute_auth` is treated as a positive
/// signal and Sberbank is not contacted. With `force = true` (the
/// user-initiated Settings → Test button), the cache is bypassed and
/// a real OAuth handshake is performed; on success the resulting
/// [`SaluteAuth`] is cached so the next `synthesize_document` call
/// reuses it (and its token cache).
///
/// Why cache-first: Sprint 1 always performed the OAuth call. Any
/// transient error on the mount-time probe (Ctrl+R refresh of the
/// dev WebView, page navigation, brief network blip) mapped the
/// frontend `CredentialsContext` to `"invalid"` even though the
/// keyring entry was perfectly valid. Trusting our own
/// process-lifetime cache for the probe path removes that false
/// negative without weakening the explicit-revalidation path.
pub(crate) async fn test_credentials_impl(state: &AppState, force: bool) -> Result<(), String> {
    if !force {
        let guard = state.salute_auth.lock().await;
        if guard.is_some() {
            return Ok(());
        }
        // Guard drops here at end of `if` block before the keyring
        // call below, so the network roundtrip never holds the lock.
    }

    let auth_key = keyring::get_auth_key()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no credentials configured".to_string())?;

    let auth = Arc::new(SaluteAuth::new(state.http_client.clone(), auth_key));

    auth.get_token()
        .await
        .map_err(|e: SaluteError| e.to_string())?;

    let mut guard = state.salute_auth.lock().await;
    *guard = Some(auth);

    Ok(())
}

/// Delete the stored Authorization Key from the OS keyring and clear
/// the cached [`SaluteAuth`].
///
/// Idempotent: a missing keyring entry is treated as success because
/// the caller (typically a "Delete" button) wants the post-condition
/// "no credentials stored" regardless of the prior state.
pub(crate) async fn delete_credentials_impl(state: &AppState) -> Result<(), String> {
    match keyring::delete_auth_key() {
        Ok(()) | Err(KeyringError::NotFound) => {}
        Err(e) => return Err(e.to_string()),
    }

    let mut guard = state.salute_auth.lock().await;
    *guard = None;

    Ok(())
}

#[cfg(test)]
mod tests {
    //! Mock keyring backend is installed process-wide before each test
    //! via the `init_mock()` helper. Because keyring 3.x mock state lives
    //! inside each `Entry` object (NOT in a shared store), the public
    //! `keyring::set_auth_key` / `get_auth_key` cannot round-trip in
    //! tests — every call constructs a new `Entry` and therefore a new,
    //! empty mock store. Tests here therefore exercise only paths that
    //! depend on the keyring being **empty** (the normal first-run
    //! state under the mock backend) and the in-memory state-reset
    //! behaviour of the commands.

    use super::*;
    use crate::salute::http;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_mock() {
        INIT.call_once(|| {
            ::keyring::set_default_credential_builder(
                ::keyring::mock::default_credential_builder(),
            );
        });
    }

    fn fresh_state() -> AppState {
        let client = http::build_client().expect("client builds");
        let conn = crate::db::test_connection();
        AppState::new(client, conn, crate::config::Config::default())
    }

    async fn seed_state_with_auth(state: &AppState) {
        let client = http::build_client().expect("client builds");
        let auth = Arc::new(SaluteAuth::new(client, "placeholder_key".to_string()));
        let mut guard = state.salute_auth.lock().await;
        *guard = Some(auth);
    }

    #[tokio::test]
    async fn test_set_credentials_resets_cached_auth() {
        init_mock();
        let state = fresh_state();
        seed_state_with_auth(&state).await;

        // Sanity: state has Some before the call.
        assert!(state.salute_auth.lock().await.is_some());

        set_credentials_impl(&state, "valid_base64_key")
            .await
            .expect("valid key should succeed under mock keyring");

        assert!(
            state.salute_auth.lock().await.is_none(),
            "set_credentials must invalidate the cached SaluteAuth"
        );
    }

    #[tokio::test]
    async fn test_set_credentials_empty_key_rejected() {
        init_mock();
        let state = fresh_state();

        let err = set_credentials_impl(&state, "").await.unwrap_err();
        assert!(
            err.contains("empty") || err.contains("whitespace"),
            "expected validation error mentioning empty/whitespace, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_set_credentials_whitespace_key_rejected() {
        init_mock();
        let state = fresh_state();

        let err = set_credentials_impl(&state, "   \n\t  ").await.unwrap_err();
        assert!(
            err.contains("empty") || err.contains("whitespace"),
            "expected validation error mentioning empty/whitespace, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_delete_credentials_idempotent_when_absent() {
        init_mock();
        let state = fresh_state();

        // Mock keyring starts empty for every Entry construction; this
        // call hits the NotFound branch which the command maps to Ok.
        delete_credentials_impl(&state)
            .await
            .expect("delete must be idempotent when nothing is stored");
        assert!(state.salute_auth.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_delete_credentials_resets_state_even_when_keyring_empty() {
        init_mock();
        let state = fresh_state();
        seed_state_with_auth(&state).await;

        delete_credentials_impl(&state).await.expect("ok");

        assert!(
            state.salute_auth.lock().await.is_none(),
            "delete_credentials must clear the cached SaluteAuth slot"
        );
    }

    #[tokio::test]
    async fn test_test_credentials_no_keys_returns_error() {
        init_mock();
        let state = fresh_state();

        // force=true is semantically irrelevant here (cache is empty),
        // but explicit value avoids the implicit-default trap if the
        // signature ever changes again.
        let err = test_credentials_impl(&state, true).await.unwrap_err();
        assert!(
            err.contains("no credentials configured"),
            "expected 'no credentials configured', got: {err}"
        );
    }

    #[tokio::test]
    async fn test_credentials_uses_cache_when_force_false_and_auth_cached() {
        // Mount-time probe contract: if SaluteAuth is already cached,
        // return Ok without touching the network. Removing the
        // cache-first short-circuit in the impl makes this test fail
        // (the placeholder key seeded below would force a real OAuth
        // call to the hardcoded Sberbank URL, which then errors out).
        init_mock();
        let state = fresh_state();
        seed_state_with_auth(&state).await;

        test_credentials_impl(&state, false)
            .await
            .expect("force=false with cached auth must succeed via cache");

        assert!(
            state.salute_auth.lock().await.is_some(),
            "cached auth must remain in place after a cache-hit probe"
        );
    }

    #[tokio::test]
    async fn test_credentials_skips_cache_when_force_true() {
        // User-initiated revalidation must bypass the cache. The
        // keyring is empty under the mock backend (per-Entry state
        // isn't shared with seed_state_with_auth's placeholder), so
        // the full path falls through to `no credentials configured`.
        // Reaching that error at all proves the cache check was
        // skipped — otherwise we'd have returned Ok on the cache hit.
        init_mock();
        let state = fresh_state();
        seed_state_with_auth(&state).await;

        let err = test_credentials_impl(&state, true).await.unwrap_err();
        assert!(
            err.contains("no credentials configured"),
            "force=true must bypass cache and hit the keyring path, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_credentials_full_oauth_path_when_no_cache_and_force_false() {
        // The cache-first branch must only short-circuit when the
        // cache is populated. With an empty `salute_auth`, force=false
        // still has to proceed to the keyring + OAuth flow.
        init_mock();
        let state = fresh_state();
        // Deliberately no seed_state_with_auth — salute_auth is None.

        let err = test_credentials_impl(&state, false).await.unwrap_err();
        assert!(
            err.contains("no credentials configured"),
            "force=false with empty cache must proceed to keyring path, got: {err}"
        );
    }
}
