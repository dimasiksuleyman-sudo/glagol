use tauri::Manager;

// SaluteSpeech API client (OAuth + sync synthesis).
pub mod salute;

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

// Tauri commands exposed to the frontend.
pub mod commands;

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

            app.manage(state::AppState::new(http_client.clone(), conn));
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
