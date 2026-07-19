use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

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

// Structured-logging subscriber installation (Sprint 6 PR3.1).
pub mod logging;

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

/// Emit dictation state-machine transitions (D7) as a `dictation-state`
/// broadcast. `AppHandle` is the production [`dictation::pipeline::DictationEmitter`];
/// the overlay window `listen()`s and drives the pill. Broadcast (not
/// `ipc::Channel`) for the same reason as the level meter: the consumer is a
/// separate window that never `invoke`s, so there is no channel to bind.
impl dictation::pipeline::DictationEmitter for tauri::AppHandle {
    fn emit_state(&self, state: dictation::pipeline::DictationState) {
        let _ = self.emit(dictation::pipeline::DICTATION_STATE_EVENT, state);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let http_client = salute::http::build_client()
        .expect("failed to build HTTP client (embedded НУЦ Минцифры cert may be malformed)");

    tauri::Builder::default()
        // Single-instance lock (v0.2.1). MUST be registered FIRST so it runs
        // before `global-shortcut` (and every other plugin) can act: a second
        // launch has to be intercepted *before* it tries to grab the process-
        // global hotkey, or it steals the combo from the copy the user is
        // already using. The callback runs inside the ORIGINAL, already-running
        // process (the second one exits) — so it brings the existing window
        // forward from the tray rather than starting anything new (D2). argv/cwd
        // are unused: Glagol takes no file arguments.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            dictation::session::show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Global push-to-talk hotkey (Sprint 6 PR3). One handler dispatches the
        // single registered shortcut's Pressed/Released to the dictation session
        // (D3/D4). Registered from Rust in `setup`; the webview binds nothing.
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    dictation::session::handle_shortcut(app, event.state);
                })
                .build(),
        )
        .setup(move |app| {
            // Install the tracing subscriber first so every `tracing::*!` call
            // from here on is actually recorded (Sprint 6 PR3.1). In release the
            // returned WorkerGuard must outlive the process or the rolling-file
            // writer stops flushing (D-L4) — it is parked in AppState below.
            let log_guard = crate::logging::init_tracing(app.handle());

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

            // Park the log-flush guard for the process lifetime (D-L4).
            app.state::<state::AppState>().set_log_guard(log_guard);

            // ── Dictation surface (Sprint 6 PR3) ──────────────────────────
            //
            // Create the overlay window once, hidden (D5): building a WebView
            // costs hundreds of ms, which would be a stall at the exact moment
            // the user starts speaking. From here it is only show/hide +
            // reposition. `transparent` + always-on-top + `skipTaskbar` +
            // `focused(false)` so the pill floats over the target app without
            // stealing its keyboard focus.
            WebviewWindowBuilder::new(
                app,
                dictation::session::OVERLAY_LABEL,
                WebviewUrl::App("index.html".into()),
            )
            .title("Glagol overlay")
            .inner_size(
                dictation::session::OVERLAY_WIDTH,
                dictation::session::OVERLAY_HEIGHT,
            )
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .resizable(false)
            .shadow(false)
            .visible(false)
            .build()?;

            // System tray with the idle icon + «Показать / Выход» menu (D11).
            dictation::session::build_tray(app.handle())?;

            // Register the global push-to-talk hotkey (D3/D7). Prefer the user's
            // saved hotkey (falls back to the default if unset or unparseable) so
            // a hotkey changed via Settings persists across restarts. A failure
            // here (some other app already owns the combo) is logged, not fatal —
            // the rest of Glagol (TTS) must still run.
            let hotkey = {
                let state = app.state::<state::AppState>();
                let conn = state
                    .db
                    .lock()
                    .expect("db mutex poisoned during startup hotkey read");
                commands::dictation::startup_hotkey(&conn)
            };
            if let Err(e) = app.global_shortcut().register(hotkey) {
                tracing::warn!("failed to register dictation hotkey {hotkey:?}: {e}");
            }

            // Close-to-tray (D12): intercept the main window's close so the
            // hotkey keeps working after the user "closes" the window.
            if let Some(main) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        dictation::session::hide_main_to_tray(&handle);
                    }
                });
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
            commands::dictation::get_dictation_settings,
            commands::dictation::set_dictation_setting,
            commands::dictation::set_dictation_hotkey,
            commands::dictation::list_dictations,
            commands::dictation::clear_dictation_history,
            commands::dictation::get_recognitions_minutes,
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
