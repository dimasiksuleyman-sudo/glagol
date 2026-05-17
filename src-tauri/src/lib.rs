// SaluteSpeech API client (OAuth + sync synthesis).
pub mod salute;

// Text processing utilities (chunking, future: preprocessing).
pub mod text;

// Audio utilities (WAV chunk concatenation).
pub mod audio;

// Secrets storage via OS-native credential manager.
pub mod secrets;

// Shared Tauri application state.
pub mod state;

// Tauri commands exposed to the frontend.
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let http_client = salute::http::build_client()
        .expect("failed to build HTTP client (embedded НУЦ Минцифры cert may be malformed)");
    let app_state = state::AppState::new(http_client);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
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
