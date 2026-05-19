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

// Persistent user configuration (`config.json` next to `glagol.db`).
pub mod config;

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
            // Sprint 5b ordering note:
            //   1. Load config from disk (or defaults on first launch).
            //   2. Open + migrate the DB.
            //   3. Manage AppState (now holding config + db + http client).
            //   4. Ensure the audio cache directory exists. The default
            //      path is always created; a user-configured custom path,
            //      if any, is the user's responsibility to keep around
            //      (validated at save-time in `commands::config`).
            //   5. If config has a custom `library_path`, register it
            //      with the asset-protocol scope so the webview can
            //      stream audio from there (the static scope in
            //      `tauri.conf.json` only covers the default path).
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|e| format!("Failed to resolve app_local_data_dir: {e}"))?;

            let config = crate::config::Config::load(&data_dir);

            let db_path = crate::paths::database_path(app.handle())?;
            let conn = crate::db::init_database(&db_path)?;

            app.manage(state::AppState::new(http_client.clone(), conn, config.clone()));

            let audio_root = crate::paths::audio_cache_root(app.handle())?;
            std::fs::create_dir_all(&audio_root)
                .map_err(|e| format!("Failed to create audio cache directory: {e}"))?;

            if let Some(custom) = config.library_path.as_ref() {
                app.asset_protocol_scope()
                    .allow_directory(custom, false)
                    .map_err(|e| {
                        format!(
                            "Failed to register asset-protocol scope for configured library path {}: {e}",
                            custom.display()
                        )
                    })?;
            }

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
            commands::file::read_and_parse_file,
            commands::config::get_library_path,
            commands::config::set_library_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
