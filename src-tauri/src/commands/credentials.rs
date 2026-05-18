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
pub async fn test_credentials(state: tauri::State<'_, AppState>) -> Result<(), String> {
    test_credentials_impl(&state).await
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

/// Validate the stored Authorization Key by performing a real OAuth
/// handshake. On success, caches the resulting [`SaluteAuth`] in state
/// so the next `synthesize_document` call reuses it (and its token
/// cache).
pub(crate) async fn test_credentials_impl(state: &AppState) -> Result<(), String> {
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
        AppState::new(client, conn)
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

        let err = test_credentials_impl(&state).await.unwrap_err();
        assert!(
            err.contains("no credentials configured"),
            "expected 'no credentials configured', got: {err}"
        );
    }
}
