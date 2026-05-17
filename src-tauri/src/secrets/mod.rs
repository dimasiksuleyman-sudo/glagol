//! Secrets storage for Glagol.
//!
//! Currently provides Windows Credential Manager access for the
//! SaluteSpeech Authorization Key via `secrets::keyring`. Future
//! modules may add encrypted local cache or session token storage —
//! see SECURITY.md.
//!
//! All Glagol secrets live under service `"Glagol"` in the OS
//! credential store. End users can audit and remove them through
//! Windows' Credential Manager
//! (`control /name Microsoft.CredentialManager`).

pub mod keyring;
