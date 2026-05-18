//! Tauri commands driving the synthesis pipeline and the resulting
//! file write.
//!
//! `synthesize_document` runs the full backend pipeline
//! (`chunker → loop synthesize → wav_join`) and emits progress events
//! through a [`tauri::ipc::Channel`]. `write_wav_file` is a thin
//! `tokio::fs::write` wrapper used by the frontend after the user picks
//! a destination via the `dialog.save()` plugin.
//!
//! Each Tauri command is a thin wrapper over an `*_impl` function that
//! takes plain `&AppState` and an arbitrary `impl Fn(ProgressEvent)`
//! callback. Tests target the impls because `tauri::State` and
//! `tauri::ipc::Channel` cannot be constructed outside a running Tauri
//! runtime.

use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Channel;

use crate::audio::wav_join::join_wav_chunks;
use crate::salute::auth::SaluteAuth;
use crate::salute::errors::SaluteError;
use crate::salute::synthesize::{SynthesisClient, UnknownVoiceId, VoiceId};
use crate::secrets::keyring;
use crate::state::AppState;
use crate::text::chunker::{chunk_text, DEFAULT_MAX_CHARS};

/// Progress events emitted by [`synthesize_document`] over a
/// [`tauri::ipc::Channel`].
///
/// Serialised as a discriminated union tagged on `kind` with
/// camel-case payload field names, which matches how a TypeScript
/// caller would naturally model the type:
///
/// ```ts
/// type ProgressEvent =
///   | { kind: "chunked"; total: number }
///   | { kind: "synthesizingChunk"; current: number; total: number }
///   | { kind: "joining" };
/// ```
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProgressEvent {
    /// Text has been split into `total` chunks; synthesis is about to
    /// start.
    Chunked { total: usize },
    /// Chunk `current` (1-indexed) is about to be sent to SaluteSpeech.
    /// Emitted *before* the network call.
    #[serde(rename_all = "camelCase")]
    SynthesizingChunk { current: usize, total: usize },
    /// All chunks have been synthesized; joining the WAV blocks.
    Joining,
}

#[tauri::command]
pub async fn synthesize_document(
    state: tauri::State<'_, AppState>,
    text: String,
    voice: String,
    on_progress: Channel<ProgressEvent>,
) -> Result<tauri::ipc::Response, String> {
    let bytes = synthesize_document_impl(&state, text, voice, move |event| {
        let _ = on_progress.send(event);
    })
    .await?;

    // Return as binary `Response` instead of `Vec<u8>` so the WAV bytes
    // travel through the Tauri IPC bridge as a raw ArrayBuffer rather
    // than a JSON array of numbers. A 21 MB WAV serialised as JSON
    // would balloon to ~84 MB on the wire and cost seconds of CPU on
    // both sides; `Response` skips the serde layer entirely.
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn write_wav_file(path: String, bytes: Vec<u8>) -> Result<(), String> {
    write_wav_file_impl(&path, &bytes).await
}

/// Full synthesis pipeline: validate input, ensure auth, synthesize
/// each chunk sequentially, then join the chunks into one WAV blob.
///
/// Sequential by design — Sprint 4 will revisit concurrency once a real
/// playback flow exists.
pub(crate) async fn synthesize_document_impl(
    state: &AppState,
    text: String,
    voice: String,
    on_progress: impl Fn(ProgressEvent) + Send,
) -> Result<Vec<u8>, String> {
    if text.trim().is_empty() {
        return Err("text is empty or whitespace-only".to_string());
    }

    let voice_id: VoiceId = voice.parse().map_err(|e: UnknownVoiceId| e.to_string())?;

    let chunks = chunk_text(&text, DEFAULT_MAX_CHARS);
    let total = chunks.len();
    on_progress(ProgressEvent::Chunked { total });

    if chunks.is_empty() {
        return Err("text produced no synthesizable chunks".to_string());
    }

    let auth = get_or_init_auth(state).await?;
    let synth = SynthesisClient::new(state.http_client.clone());

    let mut wav_chunks: Vec<Vec<u8>> = Vec::with_capacity(total);
    for (i, chunk) in chunks.iter().enumerate() {
        on_progress(ProgressEvent::SynthesizingChunk {
            current: i + 1,
            total,
        });

        let mut token = auth.get_token().await.map_err(|e| e.to_string())?;
        let wav = match synth.synthesize(&token, chunk, voice_id).await {
            Ok(bytes) => bytes,
            Err(SaluteError::TokenExpired) => {
                // One retry: drop the cached token, fetch a fresh one,
                // try synthesize once more. A second TokenExpired here
                // means the Authorization Key itself is bad — surface it.
                auth.invalidate().await;
                token = auth.get_token().await.map_err(|e| e.to_string())?;
                synth
                    .synthesize(&token, chunk, voice_id)
                    .await
                    .map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };

        wav_chunks.push(wav);
    }

    on_progress(ProgressEvent::Joining);
    join_wav_chunks(&wav_chunks).map_err(|e| e.to_string())
}

/// Write the given bytes to `path` on disk.
///
/// Called from the frontend with a path the user just chose via
/// `dialog.save()`. We do not validate the path further — the plugin's
/// scope already gates where the picker can land, and Tauri's IPC is
/// local-only.
pub(crate) async fn write_wav_file_impl(path: &str, bytes: &[u8]) -> Result<(), String> {
    tokio::fs::write(path, bytes)
        .await
        .map_err(|e| format!("failed to write {path}: {e}"))
}

/// Get the cached [`SaluteAuth`] from state, or build a fresh one from
/// the keyring if the state slot is empty.
///
/// The mutex guard is released before this function returns: the caller
/// holds only the resulting `Arc<SaluteAuth>` while making network
/// calls, so concurrent commands are never blocked on
/// `state.salute_auth` during synthesis.
async fn get_or_init_auth(state: &AppState) -> Result<Arc<SaluteAuth>, String> {
    let mut guard = state.salute_auth.lock().await;

    if let Some(existing) = guard.as_ref() {
        return Ok(existing.clone());
    }

    let auth_key = keyring::get_auth_key()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no credentials configured".to_string())?;

    let fresh = Arc::new(SaluteAuth::new(state.http_client.clone(), auth_key));
    *guard = Some(fresh.clone());
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    //! Tests cover the early-validation paths of `synthesize_document`
    //! and the I/O behaviour of `write_wav_file`. Paths that reach
    //! SaluteSpeech network calls are not exercised here — those would
    //! require either a mockito-instrumented `SaluteAuth` (out of scope
    //! for this PR's command layer; the underlying auth/synthesize
    //! modules already cover them) or a `tauri::ipc::Channel` mock.

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

    fn noop_progress(_event: ProgressEvent) {}

    #[tokio::test]
    async fn test_synthesize_document_empty_text_rejected() {
        init_mock();
        let state = fresh_state();

        let err = synthesize_document_impl(
            &state,
            String::new(),
            "Nec_24000".to_string(),
            noop_progress,
        )
        .await
        .unwrap_err();

        assert!(
            err.contains("empty") || err.contains("whitespace"),
            "expected empty-text error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_synthesize_document_whitespace_text_rejected() {
        init_mock();
        let state = fresh_state();

        let err = synthesize_document_impl(
            &state,
            "   \n\t   ".to_string(),
            "Nec_24000".to_string(),
            noop_progress,
        )
        .await
        .unwrap_err();

        assert!(
            err.contains("empty") || err.contains("whitespace"),
            "expected whitespace-text error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_synthesize_document_unknown_voice_rejected() {
        init_mock();
        let state = fresh_state();

        let err = synthesize_document_impl(
            &state,
            "Привет, мир!".to_string(),
            "FooBar_9999".to_string(),
            noop_progress,
        )
        .await
        .unwrap_err();

        assert!(
            err.contains("unknown voice"),
            "expected unknown-voice error, got: {err}"
        );
        assert!(
            err.contains("FooBar_9999"),
            "error should cite the bad voice name, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_synthesize_document_no_credentials_errors() {
        init_mock();
        let state = fresh_state();

        // Valid text, valid voice, but keyring is empty under mock backend.
        let err = synthesize_document_impl(
            &state,
            "Привет, мир!".to_string(),
            "Nec_24000".to_string(),
            noop_progress,
        )
        .await
        .unwrap_err();

        assert!(
            err.contains("no credentials configured"),
            "expected 'no credentials configured', got: {err}"
        );
    }

    #[tokio::test]
    async fn test_write_wav_file_creates_file() {
        let tmp_dir = std::env::temp_dir();
        let path = tmp_dir.join(format!(
            "glagol_test_wav_{}.wav",
            uuid::Uuid::new_v4().simple()
        ));
        let path_str = path.to_string_lossy().into_owned();
        let payload: Vec<u8> = (0..256).map(|i| i as u8).collect();

        write_wav_file_impl(&path_str, &payload)
            .await
            .expect("write should succeed");

        let read_back = tokio::fs::read(&path).await.expect("read back ok");
        assert_eq!(read_back, payload);

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_write_wav_file_invalid_path_errors() {
        // Path inside a non-existent directory under the temp root —
        // portable across Linux/macOS/Windows test environments.
        let tmp_dir = std::env::temp_dir();
        let bad_path = tmp_dir
            .join(format!(
                "glagol_does_not_exist_{}",
                uuid::Uuid::new_v4().simple()
            ))
            .join("nested")
            .join("out.wav");
        let path_str = bad_path.to_string_lossy().into_owned();

        let err = write_wav_file_impl(&path_str, b"abc").await.unwrap_err();
        assert!(
            err.contains("failed to write"),
            "expected error prefix, got: {err}"
        );
    }
}
