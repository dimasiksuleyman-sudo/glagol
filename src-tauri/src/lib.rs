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
            app.manage(state::AppState::new(http_client.clone(), conn));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::credentials::set_credentials,
            commands::credentials::test_credentials,
            commands::credentials::delete_credentials,
            commands::synthesize::synthesize_document,
            commands::synthesize::write_wav_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
