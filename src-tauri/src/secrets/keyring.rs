//! Windows Credential Manager wrapper for storing the SaluteSpeech
//! Authorization Key (the `base64(client_id:client_secret)` blob Sber
//! displays in the developer console).
//!
//! Production code uses the real `keyring-rs` backend (Wincred on
//! Windows, enabled via the `windows-native` feature). Unit tests use
//! the always-available `keyring::mock` backend with one important
//! caveat: in keyring 3.x, mock state lives **inside the `Entry`
//! object itself**, not in a shared store. Therefore unit tests below
//! create one `Entry` per test and reuse it across set/get/delete —
//! see the test module comment for details.
//!
//! NOTE on naming: this module is called `keyring` after the
//! `keyring-rs` crate it wraps. References inside this file to
//! `keyring::Entry` and `keyring::Error` resolve to the external
//! crate; the module's own scope does not collide.

use keyring::Entry;
use thiserror::Error;

/// Single hard-coded service name shown to the user in Credential
/// Manager. Bundle-id-style names like "app.glagol.desktop" would also
/// work, but "Glagol" is far more readable when a user audits stored
/// credentials through the OS UI.
const SERVICE: &str = "Glagol";

/// Username slot for the SaluteSpeech Authorization Key. Glagol stores
/// exactly one credential per installation today (single-user app);
/// adding more secrets in the future means adding more `USERNAME_*`
/// constants, NOT a profile system.
const USERNAME_AUTH_KEY: &str = "salutespeech_auth_key";

/// Errors that can occur when reading or writing secrets.
#[derive(Error, Debug)]
pub enum KeyringError {
    /// The requested credential does not exist in the OS keyring.
    /// This is the normal first-run state and most callers should
    /// treat it as "no configuration yet" rather than a fault.
    #[error("no credential found")]
    NotFound,

    /// The OS keyring backend returned an unexpected error
    /// (platform failure, ambiguous match, encoding issue, etc.).
    /// User-facing messaging should suggest checking Credential
    /// Manager and reporting via SECURITY.md if the issue persists.
    #[error("keyring backend error: {0}")]
    Backend(String),

    /// Input validation failure inside this module
    /// (empty key, whitespace-only key, etc.). Indicates a bug in
    /// the caller — production UI should validate before invoking.
    #[error("internal error: {0}")]
    Internal(String),
}

pub type KeyringResult<T> = Result<T, KeyringError>;

// ───────────────────────────────────────────────────────────────
// Public API — single SaluteSpeech Authorization Key per install.
// ───────────────────────────────────────────────────────────────

/// Store the SaluteSpeech Authorization Key in the OS keyring.
///
/// Overwrites any existing value silently. The key should be the
/// `base64(client_id:client_secret)` string Sber displays in the
/// developer console — Glagol does NOT decode or split it.
///
/// # Errors
/// - [`KeyringError::Internal`] if `key` is empty or whitespace-only
/// - [`KeyringError::Backend`] on platform failure (Wincred error)
pub fn set_auth_key(key: &str) -> KeyringResult<()> {
    let entry = auth_key_entry()?;
    set_with(&entry, key)
}

/// Retrieve the SaluteSpeech Authorization Key from the OS keyring.
///
/// Returns `Ok(None)` if no key has been stored yet (normal first-run
/// state); callers should NOT treat this as an error.
///
/// # Errors
/// - [`KeyringError::Backend`] on platform failure
pub fn get_auth_key() -> KeyringResult<Option<String>> {
    let entry = auth_key_entry()?;
    get_with(&entry)
}

/// Remove the SaluteSpeech Authorization Key from the OS keyring.
///
/// # Errors
/// - [`KeyringError::NotFound`] if no key was stored — lets the caller
///   surface "nothing to delete" cleanly without a panic
/// - [`KeyringError::Backend`] on platform failure
pub fn delete_auth_key() -> KeyringResult<()> {
    let entry = auth_key_entry()?;
    delete_with(&entry)
}

// ───────────────────────────────────────────────────────────────
// Internal helpers — operate on a borrowed `Entry`.
//
// Splitting the Entry construction from the operation lets unit
// tests hold one Entry across set/get/delete (required by mock
// backend semantics — see test module). In production, Wincred
// resolves `Entry::new(SERVICE, USERNAME_AUTH_KEY)` to the same
// underlying credential every time, so creating fresh entries in
// each public API call is equivalent in behavior.
// ───────────────────────────────────────────────────────────────

fn auth_key_entry() -> KeyringResult<Entry> {
    Entry::new(SERVICE, USERNAME_AUTH_KEY).map_err(|e| KeyringError::Backend(e.to_string()))
}

fn set_with(entry: &Entry, key: &str) -> KeyringResult<()> {
    if key.trim().is_empty() {
        return Err(KeyringError::Internal(
            "auth key cannot be empty or whitespace-only".into(),
        ));
    }
    entry
        .set_password(key)
        .map_err(|e| KeyringError::Backend(e.to_string()))
}

fn get_with(entry: &Entry) -> KeyringResult<Option<String>> {
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(KeyringError::Backend(e.to_string())),
    }
}

fn delete_with(entry: &Entry) -> KeyringResult<()> {
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Err(KeyringError::NotFound),
        Err(e) => Err(KeyringError::Backend(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    //! ## Mock backend caveat
    //!
    //! In keyring 3.x the mock credential store keeps its state
    //! **inside the `Entry` object itself**, NOT in a shared global
    //! store. Two `Entry::new(...)` calls with identical service+user
    //! arguments produce two independent mock credentials with empty
    //! state. This differs from the real Wincred backend, where
    //! `Entry::new` always resolves to the same underlying OS record.
    //!
    //! Therefore each test below:
    //! 1. Calls `init_mock()` to install the mock builder process-wide
    //!    (idempotent via `Once`).
    //! 2. Creates ONE `Entry` via `test_entry()` and reuses it across
    //!    all set/get/delete calls within that test.
    //!
    //! Tests are otherwise isolated because each holds its own `Entry`
    //! and mock state cannot leak between them.

    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_mock() {
        INIT.call_once(|| {
            keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        });
    }

    fn test_entry() -> Entry {
        Entry::new("Glagol-Test", "test-user").unwrap()
    }

    #[test]
    fn test_set_then_get_returns_value() {
        init_mock();
        let entry = test_entry();
        set_with(&entry, "secret_value").unwrap();
        assert_eq!(get_with(&entry).unwrap(), Some("secret_value".to_string()));
    }

    #[test]
    fn test_get_without_set_returns_none_not_error() {
        init_mock();
        let entry = test_entry();
        assert_eq!(get_with(&entry).unwrap(), None);
    }

    #[test]
    fn test_delete_existing_returns_ok() {
        init_mock();
        let entry = test_entry();
        set_with(&entry, "value").unwrap();
        delete_with(&entry).unwrap();
        assert_eq!(get_with(&entry).unwrap(), None);
    }

    #[test]
    fn test_delete_nonexistent_returns_not_found() {
        init_mock();
        let entry = test_entry();
        let err = delete_with(&entry).unwrap_err();
        assert!(
            matches!(err, KeyringError::NotFound),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn test_overwrite_replaces_value() {
        init_mock();
        let entry = test_entry();
        set_with(&entry, "first").unwrap();
        set_with(&entry, "second").unwrap();
        assert_eq!(get_with(&entry).unwrap(), Some("second".to_string()));
    }

    #[test]
    fn test_empty_or_whitespace_key_is_rejected() {
        init_mock();
        let entry = test_entry();

        let err = set_with(&entry, "").unwrap_err();
        assert!(
            matches!(err, KeyringError::Internal(_)),
            "empty string should be rejected, got {err:?}"
        );

        let err = set_with(&entry, "   ").unwrap_err();
        assert!(
            matches!(err, KeyringError::Internal(_)),
            "whitespace-only should be rejected, got {err:?}"
        );
    }

    #[test]
    fn test_very_long_key_is_accepted() {
        init_mock();
        let entry = test_entry();
        // 2 KB — well below Wincred's ~2.5 KB blob limit, sanity check
        // that we don't artificially cap value length.
        let long_value = "x".repeat(2048);
        set_with(&entry, &long_value).unwrap();
        assert_eq!(get_with(&entry).unwrap(), Some(long_value));
    }
}
