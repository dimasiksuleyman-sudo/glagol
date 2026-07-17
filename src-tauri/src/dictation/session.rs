//! Dictation session coordination — the glue that turns a global hotkey into a
//! running [`pipeline`](crate::dictation::pipeline) (Sprint 6 PR3).
//!
//! This module owns everything that needs a live Tauri runtime and therefore
//! cannot be exercised by the headless pipeline tests: the `Ctrl+Shift+Space`
//! push-to-talk handler, the tray icon (idle ⇄ recording) and its menu, the
//! close-to-tray behaviour with its one-time notice, and the overlay window's
//! position/visibility. The pure decision points it *can* test in isolation —
//! whether a missing API key is fatal, and the one-time tray-notice flag — are
//! extracted as free functions with unit tests at the bottom.
//!
//! # Push-to-talk (Phase 0 finding)
//!
//! `global-hotkey 0.8.0` synthesises `Released` on Windows by polling
//! `GetAsyncKeyState` on the hotkey's **main key** (Space) after the native
//! `WM_HOTKEY` press; registration sets `MOD_NOREPEAT`, so exactly one
//! `Pressed`/`Released` pair arrives per physical hold. Both handlers are still
//! written to be idempotent — `Pressed` while already busy is ignored (D4), and
//! `Released` after the session ended is a no-op (the stored sender was taken).
//! The pipeline's wall-clock watchdog (D10) is the second line if a `Released`
//! is ever lost.

use std::sync::atomic::Ordering;

use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, PhysicalPosition};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
use tokio::sync::oneshot;

use crate::commands::dictation::{build_stt_client, get_stt_settings_impl, read_insertion_mode};
use crate::db::repository;
use crate::dictation::insert::{ArboardClipboard, EnigoInserter, InsertionMode};
use crate::dictation::pipeline::{
    run_dictation, DictationDeps, DictationEmitter, DictationOpts, DictationOutcome, DictationState,
};
use crate::dictation::DictationPhase;
use crate::state::AppState;
use crate::stt::openai_compat::OpenAiCompatStt;
use crate::stt::validation;

/// Label of the always-present overlay window (created hidden at startup, D5).
pub const OVERLAY_LABEL: &str = "overlay";

/// Tray icon id — used to look the tray up by `app.tray_by_id` for the
/// idle ⇄ recording icon swap (D11).
pub const TRAY_ID: &str = "glagol-tray";

/// Overlay pill size in logical pixels (D5). Kept in sync with the CSS pill in
/// `OverlayPill.tsx`.
pub const OVERLAY_WIDTH: f64 = 224.0;
pub const OVERLAY_HEIGHT: f64 = 40.0;

/// Gap between the pill's bottom edge and the monitor's bottom edge, in logical
/// pixels — keeps the pill clear of the Windows taskbar.
const OVERLAY_BOTTOM_MARGIN: f64 = 80.0;

/// `app_settings` key for the one-time close-to-tray notice flag (D12). Stored
/// as `"1"` once the dialog has been shown; absent/anything-else means pending.
const KEY_TRAY_NOTICE_SHOWN: &str = "tray_notice_shown";

/// Embedded tray icons (D11/D15). 32×32 RGBA PNGs decoded via `Image::from_bytes`
/// (the `image-png` tauri feature). Placeholder mic-on-disc glyphs — blue idle,
/// red recording — replaceable by design assets without code changes.
const TRAY_IDLE_PNG: &[u8] = include_bytes!("../../icons/tray-idle.png");
const TRAY_RECORDING_PNG: &[u8] = include_bytes!("../../icons/tray-recording.png");

// ── Hotkey ──────────────────────────────────────────────────────────────

/// The push-to-talk hotkey: `Control+Shift+Space` (D3). Registered globally, so
/// it is intercepted for the whole lifetime of the app — the honest cost is
/// documented in the PR body (Word never receives its non-breaking space while
/// Glagol runs); configurability lands in PR5.
pub fn dictation_hotkey() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space)
}

/// Dispatch a shortcut event to the press/release handlers (D3/D4). Wired via
/// the global-shortcut plugin's handler in `lib.rs`.
pub fn handle_shortcut(app: &AppHandle, event_state: ShortcutState) {
    match event_state {
        ShortcutState::Pressed => on_pressed(app),
        ShortcutState::Released => on_released(app),
    }
}

/// `Pressed`: start a dictation session unless one is already running (D4).
///
/// Synchronous and fast — it only claims the phase, stores the release sender,
/// makes the pill appear, and swaps the tray icon, then spawns the async
/// pipeline. A second `Pressed` (autorepeat is suppressed by `MOD_NOREPEAT`, but
/// a genuine double-press during processing is possible) finds a non-`Idle`
/// phase and is dropped at `trace` — no toast, no spam (D4).
fn on_pressed(app: &AppHandle) {
    let state = app.state::<AppState>();

    // Claim the session, or bail if one is already active (D4).
    {
        let mut phase = state
            .dictation
            .lock()
            .expect("dictation phase mutex poisoned");
        if !matches!(*phase, DictationPhase::Idle) {
            let current = *phase;
            tracing::trace!(phase = ?current, "hotkey Pressed ignored — dictation already active");
            return;
        }
        *phase = DictationPhase::Recording;
    }

    // Claim a unique session token so this session's teardown only resets shared
    // state if no newer session has started (see `AppState::dictation_generation`).
    let token = state
        .dictation_generation
        .fetch_add(1, Ordering::SeqCst)
        .wrapping_add(1);

    // Store the release signal the pipeline awaits; `Released` fires it.
    let (release_tx, release_rx) = oneshot::channel();
    {
        *state
            .dictation_stop
            .lock()
            .expect("dictation_stop mutex poisoned") = Some(release_tx);
    }

    // Pill appears instantly (lowest latency — the pipeline's `recording` event
    // fills in content a few ms later); tray shows the recording state.
    position_overlay(app);
    show_overlay(app);
    set_tray_recording(app, true);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_session(app, release_rx, token).await;
    });
}

/// `Released`: end the in-flight recording by firing the stored release signal.
/// A no-op if no session is active (the sender was already taken) — this makes a
/// duplicate `Released` or an out-of-order event harmless.
fn on_released(app: &AppHandle) {
    let state = app.state::<AppState>();
    let sender = state
        .dictation_stop
        .lock()
        .expect("dictation_stop mutex poisoned")
        .take();
    if let Some(tx) = sender {
        // Err means the pipeline already returned (watchdog/error) — fine.
        let _ = tx.send(());
    }
}

/// The async body of one session: build the provider from persisted settings +
/// keyring, run the pipeline, record advisory usage, and always reset the tray
/// and phase on the way out.
async fn run_session(app: AppHandle, release_rx: oneshot::Receiver<()>, token: u64) {
    let state = app.state::<AppState>();

    let (provider, language, mode) = match build_provider(&state) {
        Ok(triple) => triple,
        Err(message) => {
            // No key / bad config — surface it in the pill and stand down. The
            // recorder was never opened.
            app.emit_state(DictationState::Error { message });
            finish_session(&app, &state, token);
            return;
        }
    };

    // The two delivery seams (D8): zero-field unit structs holding no OS handle,
    // so they cost nothing to construct here and open a fresh `Enigo` /
    // `arboard::Clipboard` per call inside the pipeline's `spawn_blocking`.
    let inserter = EnigoInserter;
    let clipboard = ArboardClipboard;
    let opts = DictationOpts::new(None, language, mode);
    let outcome = run_dictation(
        DictationDeps {
            recorder: &state.recorder,
            provider: &provider,
            inserter: &inserter,
            clipboard: &clipboard,
            emitter: &app,
            phase: &state.dictation,
        },
        opts,
        release_rx,
    )
    .await;

    // Advisory recognition-seconds accounting (D13) — never blocks the user.
    if let DictationOutcome::Delivered { duration_ms, .. } = outcome {
        record_recognition_usage(&state, duration_ms);
    }

    finish_session(&app, &state, token);
}

/// Reset per-session state after the pipeline exits (or fails to start): tray
/// back to idle, phase to `Idle`, and drop any leftover release sender.
///
/// Guarded by the session `token`: if a newer `Pressed` has already bumped the
/// generation counter (it slipped into the gap after the pipeline set `Idle`),
/// this stale teardown does nothing — the new session owns the shared state now
/// (D10-class hazard, defused). In the build-provider-failure path the phase is
/// still `Recording`, so no newer session can exist and the guard always passes.
fn finish_session(app: &AppHandle, state: &AppState, token: u64) {
    if state.dictation_generation.load(Ordering::SeqCst) != token {
        return;
    }
    set_tray_recording(app, false);
    {
        *state
            .dictation
            .lock()
            .expect("dictation phase mutex poisoned") = DictationPhase::Idle;
    }
    {
        *state
            .dictation_stop
            .lock()
            .expect("dictation_stop mutex poisoned") = None;
    }
}

/// Build the STT provider for a session from persisted settings + the keyring
/// key, plus the resolved recognition language and the auto-insertion mode (D12).
/// Returns a user-facing Russian error when the configuration is unusable — most
/// importantly when a remote endpoint has no API key (D13).
fn build_provider(
    state: &AppState,
) -> Result<(OpenAiCompatStt, Option<String>, InsertionMode), String> {
    let (settings, mode) = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        let settings = get_stt_settings_impl(&conn)
            .map_err(|e| format!("Не удалось прочитать настройки диктовки: {e}"))?;
        let mode = read_insertion_mode(&conn)
            .map_err(|e| format!("Не удалось прочитать режим вставки: {e}"))?;
        (settings, mode)
    };

    let key = crate::secrets::keyring::get_stt_key().map_err(|e| e.to_string())?;
    if requires_missing_key(&settings.base_url, key.is_some()) {
        return Err("Добавьте ключ STT в Настройках, чтобы пользоваться диктовкой.".to_string());
    }

    let proxy = settings.proxy.trim();
    let client = build_stt_client(if proxy.is_empty() { None } else { Some(proxy) })?;
    let provider = OpenAiCompatStt::new(client, &settings.base_url, &settings.model, key);
    Ok((provider, resolve_language(&settings.language), mode))
}

/// Advisory write of recognition seconds to `api_usage` for the current month
/// (D13). Swallows every error to stderr — a stale usage counter must never
/// block the user's dictation (mirrors `commands::synthesize::record_synthesis_usage`).
fn record_recognition_usage(state: &AppState, duration_ms: u32) {
    let seconds = (duration_ms as i64) / 1000;
    if seconds <= 0 {
        return;
    }
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let Ok(conn) = state.db.lock() else {
        eprintln!("advisory usage skipped: db mutex poisoned");
        return;
    };
    if let Err(e) = repository::record_recognition_usage(&conn, &month, seconds, now) {
        eprintln!("advisory recognition-usage write failed: {e}");
    }
}

// ── Overlay window ──────────────────────────────────────────────────────

/// Position the overlay at the bottom-centre of the monitor under the cursor
/// (D5), falling back to the primary monitor if the cursor/monitor query fails.
/// A missing overlay window or a failed monitor query is logged and skipped —
/// the pill simply shows wherever it last was rather than crashing dictation.
fn position_overlay(app: &AppHandle) {
    let Some(window) = app.get_webview_window(OVERLAY_LABEL) else {
        tracing::warn!("overlay window missing — cannot position");
        return;
    };

    // Prefer the monitor under the cursor; fall back to primary.
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());

    let Some(monitor) = monitor else {
        tracing::warn!("no monitor available — leaving overlay at last position");
        return;
    };

    let scale = monitor.scale_factor();
    let mon_pos = monitor.position();
    let mon_size = monitor.size();

    // Work in physical pixels (set_position takes physical coordinates).
    let pill_w = OVERLAY_WIDTH * scale;
    let pill_h = OVERLAY_HEIGHT * scale;
    let margin = OVERLAY_BOTTOM_MARGIN * scale;

    let x = mon_pos.x as f64 + (mon_size.width as f64 - pill_w) / 2.0;
    let y = mon_pos.y as f64 + mon_size.height as f64 - pill_h - margin;

    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        tracing::warn!("failed to position overlay: {e}");
    }
}

/// Show the overlay window (created hidden at startup, D5).
fn show_overlay(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = window.show();
    }
}

// ── Tray ────────────────────────────────────────────────────────────────

/// Build the system tray (D11): idle icon, a «Показать / Выход» menu, and the
/// tooltip. Called once from the setup hook.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Показать", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::image::Image::from_bytes(TRAY_IDLE_PNG)?)
        .tooltip("Glagol — диктовка (Ctrl+Shift+Space)")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

/// Swap the tray icon between idle and recording (D11). Best-effort: a missing
/// tray or a decode failure is logged, never fatal.
fn set_tray_recording(app: &AppHandle, recording: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let bytes = if recording {
        TRAY_RECORDING_PNG
    } else {
        TRAY_IDLE_PNG
    };
    match tauri::image::Image::from_bytes(bytes) {
        Ok(icon) => {
            let _ = tray.set_icon(Some(icon));
        }
        Err(e) => tracing::warn!("failed to decode tray icon: {e}"),
    }
}

/// Show + focus the main window (tray «Показать», D11).
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

// ── Close-to-tray (D12) ─────────────────────────────────────────────────

/// Intercept the main window's close request: hide to the tray instead of
/// exiting so the global hotkey keeps working (D12). The first time this
/// happens, show a one-time explanatory dialog (via the existing dialog plugin,
/// no new dependency) and persist the `tray_notice_shown` flag.
pub fn hide_main_to_tray(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    maybe_show_tray_notice(app);
}

/// Show the close-to-tray notice once, then mark it shown. The DB read/write is
/// best-effort — a failure just means the notice may appear again, never a crash.
fn maybe_show_tray_notice(app: &AppHandle) {
    // Read the flag in a tight scope so no `State` guard is held across the
    // dialog call below.
    let pending = {
        let state = app.state::<AppState>();
        let Ok(conn) = state.db.lock() else {
            return;
        };
        tray_notice_pending(&conn).unwrap_or(false)
    };
    if !pending {
        return;
    }

    app.dialog()
        .message("Глагол продолжит работать в трее. Иконка — в области уведомлений. Диктовка по Ctrl+Shift+Space остаётся доступной.")
        .title("Глагол свернулся в трей")
        .kind(MessageDialogKind::Info)
        .show(|_| {});

    let now = chrono::Utc::now().timestamp_millis();
    let state = app.state::<AppState>();
    let Ok(conn) = state.db.lock() else {
        return;
    };
    if let Err(e) = mark_tray_notice_shown(&conn, now) {
        eprintln!("failed to persist tray_notice_shown: {e}");
    }
}

// ── Pure helpers (unit-tested) ──────────────────────────────────────────

/// Whether the configured endpoint needs an API key we do not have (D13). A
/// remote provider requires a Bearer key; a local whisper server
/// (`localhost`/`127.0.0.1`/`::1`) is valid without one.
pub(crate) fn requires_missing_key(base_url: &str, has_key: bool) -> bool {
    !has_key && !validation::is_local_endpoint(base_url)
}

/// Map the STT language setting to the transcribe argument: `auto`/empty →
/// `None` (no language field), otherwise the pinned code. Owned `String` because
/// it outlives the borrowed settings inside the async session.
fn resolve_language(language: &str) -> Option<String> {
    match language {
        "auto" | "" => None,
        other => Some(other.to_string()),
    }
}

/// Whether the one-time close-to-tray notice still needs to be shown (D12).
/// Pending unless the flag has been explicitly stamped `"1"`.
pub(crate) fn tray_notice_pending(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(repository::get_setting(conn, KEY_TRAY_NOTICE_SHOWN)?.as_deref() != Some("1"))
}

/// Persist the close-to-tray notice as shown (D12).
pub(crate) fn mark_tray_notice_shown(conn: &Connection, updated_at: i64) -> rusqlite::Result<()> {
    repository::set_setting(conn, KEY_TRAY_NOTICE_SHOWN, "1", updated_at)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_connection;

    #[test]
    fn requires_missing_key_only_for_remote_without_key() {
        // Remote endpoint, no key → dictation must refuse and ask for a key.
        assert!(requires_missing_key("https://api.aitunnel.ru/v1", false));
        // Remote endpoint with a key → fine.
        assert!(!requires_missing_key("https://api.aitunnel.ru/v1", true));
        // Local whisper server, no key → fine (keyless local server is valid).
        assert!(!requires_missing_key("http://localhost:8000/v1", false));
        assert!(!requires_missing_key("http://127.0.0.1:9000/v1", false));
    }

    #[test]
    fn resolve_language_maps_auto_and_empty_to_none() {
        assert_eq!(resolve_language("auto"), None);
        assert_eq!(resolve_language(""), None);
        assert_eq!(resolve_language("ru"), Some("ru".to_string()));
        assert_eq!(resolve_language("en"), Some("en".to_string()));
    }

    #[test]
    fn tray_notice_is_pending_then_shown_once() {
        let conn = test_connection();
        // Fresh install: the notice is pending.
        assert!(tray_notice_pending(&conn).unwrap());

        mark_tray_notice_shown(&conn, 1_700_000_000_000).unwrap();
        // After marking, it is no longer pending — the dialog shows exactly once.
        assert!(!tray_notice_pending(&conn).unwrap());

        // Idempotent: marking again keeps it shown.
        mark_tray_notice_shown(&conn, 1_700_000_010_000).unwrap();
        assert!(!tray_notice_pending(&conn).unwrap());
    }
}
