use tauri::{Emitter, Manager};

// SaluteSpeech API client (OAuth + sync synthesis).
pub mod salute;

// Speech-to-text (STT) client for the Dictation feature (Sprint 6).
pub mod stt;

// Text processing utilities (chunking, future: preprocessing).
pub mod text;

// Audio utilities (WAV chunk concatenation).
pub mod audio;

// Secrets storage via OS-native credential manager.
pub mod secrets;

// Local SQLite database (connection management, migrations, repository).
pub mod db;

// Filesystem path resolution helpers (audio cache root, database path).
pub mod paths;

// File parsers for the Synthesize page file picker (TXT/MD/DOCX/PDF).
pub mod parser;

// Library backup/restore via zip archive (Sprint 5c).
pub mod backup;

// Shared Tauri application state.
pub mod state;

// Dictation microphone recorder (Sprint 6 PR2): capture → 16 kHz mono S16LE.
pub mod dictation;

// Tauri commands exposed to the frontend.
pub mod commands;

/// Broadcast event name for microphone RMS levels (D11). Kept in lock-step with
/// `DICTATION_LEVEL_EVENT` in `src/lib/tauri.ts`. Emitted at ~20 Hz while
/// recording; the overlay (PR3) subscribes. kebab-case per project convention
/// (`synthesis-completed`, `backup-progress`).
const DICTATION_LEVEL_EVENT: &str = "dictation-level";

/// Payload of the [`DICTATION_LEVEL_EVENT`] broadcast: one linear RMS value in
/// `0.0..=1.0`. Scaling/smoothing is the overlay's job (D6/D11).
#[derive(Clone, serde::Serialize)]
struct LevelPayload {
    level: f32,
}

/// Emit RMS levels as a Tauri broadcast. `AppHandle` is `Clone + Send + Sync +
/// 'static`, so it satisfies [`dictation::LevelSink`] directly and can be moved
/// onto the recorder thread. We use `app.emit()` (broadcast), not
/// `ipc::Channel`, because the level consumer is a separate overlay window that
/// never `invoke`s — there is no call to bind a channel to (see the PR body).
impl dictation::LevelSink for tauri::AppHandle {
    fn level(&self, rms: f32) {
        let _ = self.emit(DICTATION_LEVEL_EVENT, LevelPayload { level: rms });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let http_client = salute::http::build_client()
        .expect("failed to build HTTP client (embedded НУЦ Минцифры cert may be malformed)");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Resolve the database path and eagerly initialise the connection.
            // Failure here is fatal: silently continuing with a broken DB would
            // corrupt every subsequent write.
            let db_path = crate::paths::database_path(app.handle())?;
            let conn = crate::db::init_database(&db_path)?;

            // Ensure the audio cache directory exists. Synthesis writes
            // straight into it without a per-call mkdir, so the directory
            // must be present before the first synthesis command lands.
            let audio_root = crate::paths::audio_cache_root(app.handle())?;
            std::fs::create_dir_all(&audio_root)
                .map_err(|e| format!("Failed to create audio cache directory: {e}"))?;

            // Spawn the dedicated recorder thread (Sprint 6 PR2). `CpalSource`
            // is built on the recorder thread itself (the factory closure) — its
            // `cpal::Stream` is not `Send` on WASAPI and must never cross a
            // thread boundary. The `AppHandle` is the level sink. No stream is
            // opened until a `Start` arrives, so this is idle (≈0% CPU) at rest.
            let recorder = crate::dictation::recorder::spawn_recorder(
                crate::dictation::recorder::CpalSource::new,
                app.handle().clone(),
            );

            app.manage(state::AppState::new(http_client.clone(), conn, recorder));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::credentials::set_credentials,
            commands::credentials::test_credentials,
            commands::credentials::delete_credentials,
            commands::synthesize::synthesize_document,
            commands::storage::get_audio_path,
            commands::storage::export_audio,
            commands::storage::list_documents,
            commands::storage::delete_document,
            commands::storage::update_document_title,
            commands::file::read_and_parse_file,
            commands::backup::create_backup,
            commands::backup::validate_backup,
            commands::backup::restore_backup,
            commands::backup::relaunch_app,
            commands::usage::get_current_month_usage,
            commands::dictation::get_stt_settings,
            commands::dictation::save_stt_settings,
            commands::dictation::set_stt_key,
            commands::dictation::delete_stt_key,
            commands::dictation::has_stt_key,
            commands::dictation::test_stt_key,
            commands::dictation::list_audio_input_devices,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Drain and stop the recorder thread on exit so it is not left as a
            // detached zombie (D2/lib.rs). Best-effort — the process is leaving.
            if let tauri::RunEvent::Exit = event {
                app_handle.state::<state::AppState>().recorder.shutdown();
            }
        });
}
