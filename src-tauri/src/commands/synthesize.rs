//! Tauri commands driving the synthesis pipeline and the resulting
//! persistence to the local library.
//!
//! `synthesize_document` runs the full backend pipeline
//! (`chunker → loop synthesize → wav_join`), persists the joined WAV
//! through a single `rusqlite::Transaction` (INSERT row → write file →
//! commit, auto-rollback on any failure), and returns the freshly
//! generated `document_id` to the frontend. Audio bytes never cross
//! the IPC boundary anymore — that's what `commands::storage::export_audio`
//! is for when the user explicitly wants a copy on disk.
//!
//! Each Tauri command is a thin wrapper over an `*_impl` function that
//! takes plain `&AppState` plus the path roots it needs, so tests can
//! drive the impl directly without a running Tauri runtime.

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::Connection;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::AppHandle;
use uuid::Uuid;

use crate::audio::wav_join::join_wav_chunks;
use crate::db;
use crate::db::repository::DocumentRecord;
use crate::paths;
use crate::salute::auth::SaluteAuth;
use crate::salute::errors::SaluteError;
use crate::salute::synthesize::{SynthesisClient, UnknownVoiceId, VoiceId};
use crate::secrets::keyring;
use crate::state::AppState;
use crate::text::chunker::{chunk_text, DEFAULT_MAX_CHARS};

/// Maximum characters of the input text retained as the document title.
/// 60 keeps Library list rows readable on a typical 1080p window without
/// truncation, while still being descriptive enough to disambiguate.
const TITLE_CHAR_LIMIT: usize = 60;

/// Source-type tag written for documents originating from the paste
/// textarea. Sprint 3 will add `"file"` for parsed uploads.
const SOURCE_TYPE_PASTE: &str = "paste";

/// Status tag for a fully-synthesised, ready-to-play document. Sprint 4
/// will introduce `"synthesizing"` and `"error"` rows.
const STATUS_READY: &str = "ready";

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
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    text: String,
    voice: String,
    on_progress: Channel<ProgressEvent>,
) -> Result<String, String> {
    let audio_root = paths::audio_cache_root(&app)?;
    synthesize_document_impl(&state, &audio_root, text, voice, move |event| {
        let _ = on_progress.send(event);
    })
    .await
}

/// Full synthesis pipeline: validate input, ensure auth, synthesize
/// each chunk sequentially, join the chunks, then persist (DB row +
/// audio file) inside a single transaction.
///
/// Sequential synthesis by design — Sprint 4 will revisit concurrency
/// once a real playback flow exists.
pub(crate) async fn synthesize_document_impl(
    state: &AppState,
    audio_root: &Path,
    text: String,
    voice: String,
    on_progress: impl Fn(ProgressEvent) + Send,
) -> Result<String, String> {
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
    let joined = join_wav_chunks(&wav_chunks).map_err(|e| e.to_string())?;

    persist_synthesis_result(&state.db, audio_root, &text, voice_id, &joined)
}

/// Persist a freshly-synthesised document: insert the row, write the WAV
/// file, commit. The whole thing runs inside a single rusqlite
/// `Transaction` so any failure auto-rolls back via the `Drop` impl —
/// users never see a row pointing at a file that was never written, nor
/// a file with no row referencing it.
///
/// Synchronous from the moment the mutex is acquired through to the
/// `tx.commit()` call: no `.await` in the critical section. The
/// `std::sync::MutexGuard` isn't `Send`, so the compiler would refuse
/// any future change that violated this invariant.
pub(crate) fn persist_synthesis_result(
    db: &Mutex<Connection>,
    audio_root: &Path,
    text: &str,
    voice: VoiceId,
    wav_bytes: &[u8],
) -> Result<String, String> {
    let document_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().timestamp_millis();
    let relative_audio = format!("{document_id}.wav");
    let absolute_audio = audio_root.join(&relative_audio);

    let title: String = text
        .chars()
        .take(TITLE_CHAR_LIMIT)
        .collect::<String>()
        .trim()
        .to_string();

    let record = DocumentRecord {
        id: document_id.clone(),
        title,
        source_type: SOURCE_TYPE_PASTE.to_string(),
        char_count: text.chars().count() as i64,
        voice: voice.as_api_id().to_string(),
        status: STATUS_READY.to_string(),
        error_message: None,
        created_at,
        audio_path: Some(relative_audio),
        audio_duration_ms: None,
    };

    let mut conn = db.lock().expect("db mutex poisoned");
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    db::repository::insert(&tx, &record).map_err(|e| e.to_string())?;
    fs::write(&absolute_audio, wav_bytes)
        .map_err(|e| format!("failed to write audio file: {e}"))?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(document_id)
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
    //! and the persistence orchestration of `persist_synthesis_result`.
    //! Paths that reach SaluteSpeech network calls are not exercised
    //! here — those would require a mockito-instrumented `SaluteAuth`
    //! (out of scope for this PR's command layer; the underlying
    //! auth/synthesize modules already cover them).

    use super::*;
    use crate::db::repository;
    use crate::db::test_connection;
    use crate::salute::http;
    use std::path::PathBuf;
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

    fn unique_audio_root(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "glagol_synth_{}_{}",
            label,
            Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).expect("create temp audio root");
        dir
    }

    fn noop_progress(_event: ProgressEvent) {}

    #[tokio::test]
    async fn test_synthesize_document_empty_text_rejected() {
        init_mock();
        let state = fresh_state();
        let audio_root = unique_audio_root("empty");

        let err = synthesize_document_impl(
            &state,
            &audio_root,
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

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[tokio::test]
    async fn test_synthesize_document_whitespace_text_rejected() {
        init_mock();
        let state = fresh_state();
        let audio_root = unique_audio_root("whitespace");

        let err = synthesize_document_impl(
            &state,
            &audio_root,
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

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[tokio::test]
    async fn test_synthesize_document_unknown_voice_rejected() {
        init_mock();
        let state = fresh_state();
        let audio_root = unique_audio_root("badvoice");

        let err = synthesize_document_impl(
            &state,
            &audio_root,
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

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[tokio::test]
    async fn test_synthesize_document_no_credentials_errors() {
        init_mock();
        let state = fresh_state();
        let audio_root = unique_audio_root("nocreds");

        // Valid text, valid voice, but keyring is empty under mock backend.
        let err = synthesize_document_impl(
            &state,
            &audio_root,
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

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn persist_synthesis_result_writes_row_and_file_on_success() {
        let db = Mutex::new(test_connection());
        let audio_root = unique_audio_root("persist_ok");
        let payload: Vec<u8> = (0..2048).map(|i| (i % 256) as u8).collect();

        let id = persist_synthesis_result(
            &db,
            &audio_root,
            "Привет, мир! Это короткий тестовый текст для проверки сохранения.",
            VoiceId::Natalia,
            &payload,
        )
        .expect("persist succeeds");

        let conn = db.lock().unwrap();
        let row = repository::get(&conn, &id)
            .expect("query ok")
            .expect("row exists");
        assert_eq!(row.status, "ready");
        assert_eq!(row.source_type, "paste");
        assert_eq!(row.voice, "Nec_24000");
        assert!(row.error_message.is_none());
        let relative = row.audio_path.expect("audio_path set on success");
        assert_eq!(relative, format!("{id}.wav"));

        let on_disk = fs::read(audio_root.join(&relative)).expect("audio file readable");
        assert_eq!(on_disk, payload);

        drop(conn);
        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn persist_synthesis_result_returns_uuid_v4() {
        let db = Mutex::new(test_connection());
        let audio_root = unique_audio_root("persist_uuid");

        let id = persist_synthesis_result(
            &db,
            &audio_root,
            "hello",
            VoiceId::Natalia,
            b"fake-wav-bytes",
        )
        .expect("persist ok");

        let parsed = Uuid::parse_str(&id).expect("returned id is a valid UUID");
        assert_eq!(
            parsed.get_version_num(),
            4,
            "command layer must mint v4 UUIDs (got version {})",
            parsed.get_version_num()
        );

        let _ = fs::remove_dir_all(&audio_root);
    }

    #[test]
    fn persist_synthesis_result_writes_single_row_per_call() {
        // Whatever the input bytes' size, persistence does exactly one
        // INSERT — the chunking loop above lives outside this function,
        // so multi-chunk synthesis still results in a single library row.
        let db = Mutex::new(test_connection());
        let audio_root = unique_audio_root("persist_single");
        let big_payload: Vec<u8> = vec![0xAB; 256 * 1024];

        let id = persist_synthesis_result(
            &db,
            &audio_root,
            "длинный текст",
            VoiceId::Boris,
            &big_payload,
        )
        .expect("persist ok");

        let conn = db.lock().unwrap();
        let all = repository::list_all(&conn).expect("list ok");
        assert_eq!(all.len(), 1, "exactly one row per persist call");
        assert_eq!(all[0].id, id);

        drop(conn);
        let _ = fs::remove_dir_all(&audio_root);
    }
}
