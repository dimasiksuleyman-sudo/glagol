//! Dictation session coordination — the glue that turns a global hotkey into a
//! running [`pipeline`](crate::dictation::pipeline) (Sprint 6 PR3).
//!
//! This module owns everything that needs a live Tauri runtime and therefore
//! cannot be exercised by the headless pipeline tests: the `Ctrl+Shift+Space`
//! push-to-talk handler, the tray icon (idle ⇄ recording) and its menu, the
//! close-to-tray behaviour with its one-time notice, and the overlay window's
//! position/visibility and its rebuild after the system resumes from sleep. The pure decision points it *can* test in isolation —
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

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{
    AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};
use tokio::sync::oneshot;

use crate::commands::dictation::{
    read_device_setting, read_history_enabled, read_insertion_mode, read_remove_fillers,
};
use crate::db::repository;
use crate::dictation::insert::{ArboardClipboard, EnigoInserter, InsertionMode};
use crate::dictation::pipeline::{
    run_dictation, DictationDeps, DictationEmitter, DictationOpts, DictationOutcome,
    DictationState, Disposition,
};
use crate::dictation::DictationPhase;
use crate::state::AppState;
use crate::stt::validation;
use crate::stt::SttBackend;

/// Label of the overlay window created hidden at startup (D5). A window rebuilt
/// after sleep is labelled `overlay-N` (see [`overlay_label`]); the capability
/// and `main.tsx` accept both forms.
pub const OVERLAY_LABEL: &str = "overlay";

/// How many times the overlay window was rebuilt. `0` is the startup window.
static OVERLAY_GENERATION: AtomicU64 = AtomicU64::new(0);

/// The overlay must be rebuilt before it is trusted again: set after sleep,
/// display/session transitions and WebView2 process failures (GL-006).
static OVERLAY_STALE: AtomicBool = AtomicBool::new(false);

/// When the overlay was last hidden (Unix ms); 0 while it is shown.
static OVERLAY_HIDDEN_SINCE_MS: AtomicU64 = AtomicU64::new(0);

/// A pill hidden this long is rebuilt while idle (GL-006: after ~25 minutes of
/// idle time a WebView2 overlay was seen drawing frames that never reached the
/// screen, with no system event to react to).
const OVERLAY_IDLE_REBUILD: Duration = Duration::from_secs(10 * 60);

/// Delay before checking that the shown pill actually reached the screen.
const PRESENCE_CHECK_DELAY: Duration = Duration::from_millis(350);

/// Delay before checking a window rebuilt during dictation: its page has to
/// load and fetch the current state first.
const REPAIRED_PRESENCE_CHECK_DELAY: Duration = Duration::from_millis(1500);

/// Dictation session token whose pill was already rebuilt on the spot; at most
/// one immediate repair per session, so a covered pill cannot cause a loop.
static OVERLAY_REPAIRED_TOKEN: AtomicU64 = AtomicU64::new(0);

/// The last state broadcast to the overlay, for a page that starts mid-session.
static OVERLAY_LAST_STATE: Mutex<Option<DictationState>> = Mutex::new(None);

/// Remember the state just broadcast to the overlay (see [`overlay_state`]).
pub fn remember_overlay_state(state: &DictationState) {
    if let Ok(mut last) = OVERLAY_LAST_STATE.lock() {
        *last = Some(state.clone());
    }
}

/// The current dictation state for an overlay page that has just started, or
/// `None` when no dictation is running. A window rebuilt during dictation
/// (GL-006) uses it to show the state it missed.
#[tauri::command]
pub fn overlay_state(app: AppHandle) -> Option<DictationState> {
    if dictation_idle(&app) {
        return None;
    }
    OVERLAY_LAST_STATE.lock().ok().and_then(|last| last.clone())
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Whether a pill hidden since `hidden_since_ms` is due for an idle rebuild.
fn idle_rebuild_due(hidden_since_ms: u64, now_ms: u64) -> bool {
    hidden_since_ms != 0
        && now_ms.saturating_sub(hidden_since_ms) >= OVERLAY_IDLE_REBUILD.as_millis() as u64
}

/// Mark the overlay for rebuild at the next idle poll, with the reason.
fn mark_overlay_stale(reason: &str) {
    if !OVERLAY_STALE.swap(true, Ordering::SeqCst) {
        tracing::info!(reason, "overlay marked for rebuild");
    }
}

/// Interval of the sleep/resume watcher thread.
const RESUME_POLL: Duration = Duration::from_secs(3);

/// Wall-clock advance over one [`RESUME_POLL`] beyond which the system is taken
/// to have been asleep. Threads are frozen during sleep, so after resume the
/// wall clock is far ahead of the interval the watcher actually slept.
const RESUME_JUMP_MARGIN: Duration = Duration::from_secs(10);

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
/// (the `image-png` tauri feature). Idle uses the Glagol app icon; recording
/// uses the distinct circular red microphone indicator.
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
/// makes the preparation pill appear, then spawns the async
/// pipeline. A second `Pressed` (autorepeat is suppressed by `MOD_NOREPEAT`, but
/// a genuine double-press during processing is possible) finds a non-`Idle`
/// phase and is dropped at `trace` — no toast, no spam (D4).
fn on_pressed(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state
        .db
        .lock()
        .ok()
        .and_then(|conn| crate::preferences::get(&conn).ok())
        .is_none_or(|prefs| prefs.onboarding_stage != "complete")
    {
        return;
    }

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

    // Acknowledge the hotkey without claiming capture is ready. The pipeline
    // announces Recording only after the recorder receives its first packet.
    app.emit_state(DictationState::Starting);
    position_overlay(app);
    show_overlay(app);

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

    let (provider, device, language, mode, remove_fillers) = match build_provider(&app, &state) {
        Ok(setup) => setup,
        Err(message) => {
            // No key / bad config — surface it in the pill and stand down. The
            // recorder was never opened.
            app.emit_state(DictationState::Error { message });
            finish_session(&app, &state, token);
            return;
        }
    };

    log_dictation_context(&app);

    // The two delivery seams (D8): zero-field unit structs holding no OS handle,
    // so they cost nothing to construct here and open a fresh `Enigo` /
    // `arboard::Clipboard` per call inside the pipeline's `spawn_blocking`.
    let inserter = EnigoInserter;
    let clipboard = ArboardClipboard;
    // Pass the pinned device (D6); the recorder falls back to the system default
    // and reports it if the device is gone.
    let opts = DictationOpts::new(device, language, mode).with_remove_fillers(remove_fillers);
    let recorder = SessionRecorder {
        recorder: &state.recorder,
        provider: &provider,
    };
    let outcome = run_dictation(
        DictationDeps {
            recorder: &recorder,
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

    // Advisory writes — never block the user (best-effort, logged on failure).
    // Recognition-seconds accounting is always-on (D4/D13); the transcript is
    // written to history only when the opt-in toggle is on (D4).
    match &outcome {
        DictationOutcome::Delivered {
            duration_ms,
            text,
            disposition,
            ..
        } => {
            record_recognition_usage(&state, *duration_ms);
            record_dictation_history(
                &state,
                text,
                dictation_status(*disposition),
                *duration_ms as i64,
                None,
            );
        }
        DictationOutcome::Failed { message } => {
            // An error still earns a history row so the user can see the failed
            // attempt (D2). No recognition seconds — nothing was recognised.
            record_dictation_history(
                &state,
                "",
                "error",
                0,
                Some(crate::i18n::error(message.clone())),
            );
        }
        // D2: a discarded clip (silence / accidental tap / empty transcript) has
        // nothing to record and is never written.
        DictationOutcome::Discarded(_) => {}
    }

    finish_session(&app, &state, token);
}

/// Record the conditions that can slow recognition (0.6.2 diagnostics): Windows
/// may throttle a process whose window is hidden or unfocused, and a loaded or
/// busy Silero worker competes for CPU. Flags only — no content.
fn log_dictation_context(app: &AppHandle) {
    let window = app.get_webview_window("main");
    let visible = window
        .as_ref()
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    let focused = window
        .as_ref()
        .and_then(|w| w.is_focused().ok())
        .unwrap_or(false);
    let minimized = window
        .as_ref()
        .and_then(|w| w.is_minimized().ok())
        .unwrap_or(false);
    let (tts_loaded, tts_busy) = app
        .try_state::<crate::tts::silero::models::Models>()
        .map(|models| {
            let loaded = [&models.ru, &models.en].iter().any(|m| {
                m.worker
                    .try_lock()
                    .map(|worker| worker.is_some())
                    .unwrap_or(true)
            });
            (loaded, models.ru.operation.try_lock().is_err())
        })
        .unwrap_or((false, false));
    tracing::debug!(
        window_visible = visible,
        window_focused = focused,
        window_minimized = minimized,
        tts_worker_loaded = tts_loaded,
        tts_operation_busy = tts_busy,
        "dictation context"
    );
}

/// Persisted `dictations.status` for a delivery disposition (D2). `Delivered`
/// only ever carries `Pasted`/`Clipboard`; the `Discarded` arm is unreachable
/// from that path (a discarded clip is filtered out before delivery) and maps to
/// the clipboard label defensively.
fn dictation_status(disposition: Disposition) -> &'static str {
    match disposition {
        Disposition::Pasted => "pasted",
        Disposition::Clipboard => "clipboard",
        Disposition::Discarded => "clipboard",
    }
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
/// key, plus the pinned device (D6), the resolved recognition language, and the
/// auto-insertion mode (D12). Returns a user-facing Russian error when the
/// configuration is unusable — most importantly when a remote endpoint has no
/// API key (D13).
/// Provider, pinned device, language, insertion mode and hesitation-sound removal.
type SessionSetup = (
    SttBackend,
    Option<String>,
    Option<String>,
    InsertionMode,
    bool,
);

struct SessionRecorder<'a> {
    recorder: &'a super::RecorderHandle,
    provider: &'a SttBackend,
}
impl super::pipeline::RecorderControl for SessionRecorder<'_> {
    async fn start(
        &self,
        device: Option<String>,
    ) -> Result<super::StartedInfo, super::RecorderError> {
        let stream = self
            .provider
            .begin_stream()
            .await
            .map_err(super::RecorderError::Streaming)?;
        self.recorder.start_streaming(device, stream).await
    }
    async fn stop(&self) -> Result<super::PcmAudio, super::RecorderError> {
        self.recorder.stop().await
    }
}

fn build_provider(app: &AppHandle, state: &AppState) -> Result<SessionSetup, String> {
    let (provider, settings, mode, device, remove_fillers) = {
        let conn = state
            .db
            .lock()
            .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
        let (provider, settings) = crate::commands::speech::build_backend(app, &conn)?;
        let mode = read_insertion_mode(&conn)
            .map_err(|e| format!("Не удалось прочитать режим вставки: {e}"))?;
        let device = read_device_setting(&conn)
            .map_err(|e| format!("Не удалось прочитать устройство ввода: {e}"))?;
        let remove_fillers = read_remove_fillers(&conn)
            .map_err(|e| format!("Не удалось прочитать настройки диктовки: {e}"))?;
        (provider, settings, mode, device, remove_fillers)
    };

    Ok((
        provider,
        device,
        resolve_language(&settings.language),
        mode,
        remove_fillers,
    ))
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

/// Write one dictation to the history table (D2/D4/D5) — **only if the opt-in
/// `dictation_history_enabled` toggle is on** (D4). When it is off the transcript
/// never touches disk; this returns before the insert rather than filtering in
/// the UI. Advisory: every failure is swallowed to stderr so a history write can
/// never break the user's dictation (mirrors [`record_recognition_usage`]). The
/// insert prunes the table to the 200-row cap in one transaction (D5,
/// [`repository::insert_dictation`]).
fn record_dictation_history(
    state: &AppState,
    text: &str,
    status: &str,
    duration_ms: i64,
    error_message: Option<String>,
) {
    let created_at = chrono::Utc::now().timestamp_millis();
    let Ok(mut conn) = state.db.lock() else {
        eprintln!("advisory dictation-history skipped: db mutex poisoned");
        return;
    };

    // D4 gating: opt-in means the text never touches disk when the toggle is off.
    match read_history_enabled(&conn) {
        Ok(true) => {}
        Ok(false) => return,
        Err(e) => {
            eprintln!("advisory dictation-history skipped: cannot read toggle: {e}");
            return;
        }
    }

    let record = repository::NewDictation {
        created_at,
        duration_ms,
        text: text.to_string(),
        status: status.to_string(),
        error_message,
    };
    if let Err(e) = repository::insert_dictation(&mut conn, &record) {
        eprintln!("advisory dictation-history write failed: {e}");
    }
}

// ── Overlay window ──────────────────────────────────────────────────────

/// Position the overlay at the bottom-centre of the monitor under the cursor
/// (D5), falling back to the primary monitor if the cursor/monitor query fails.
/// A missing overlay window or a failed monitor query is logged and skipped —
/// the pill simply shows wherever it last was rather than crashing dictation.
fn position_overlay(app: &AppHandle) {
    let Some(window) = current_overlay(app) else {
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
    tracing::debug!(
        monitor = monitor.name().map(String::as_str).unwrap_or(""),
        monitors = app.available_monitors().map(|m| m.len()).unwrap_or(0),
        monitor_x = mon_pos.x,
        monitor_y = mon_pos.y,
        monitor_width = mon_size.width,
        monitor_height = mon_size.height,
        scale,
        x = x as i64,
        y = y as i64,
        "overlay positioned"
    );
}

/// Show the overlay window (created hidden at startup, D5). Re-asserts
/// always-on-top first and checks the native WebView2 state afterwards.
///
/// GL-006 diagnostics: the pill can vanish while dictation keeps working, so
/// every show records whether the window exists, accepted `show()`, reports
/// itself visible and where it is. The pill's own reports
/// ([`report_overlay_event`]) then tell whether its page received the state and
/// painted a frame.
fn show_overlay(app: &AppHandle) {
    show_overlay_checked_after(app, PRESENCE_CHECK_DELAY);
}

/// [`show_overlay`] with the delay before its on-screen presence check.
fn show_overlay_checked_after(app: &AppHandle, check_delay: Duration) {
    let generation = OVERLAY_GENERATION.load(Ordering::SeqCst);
    let Some(window) = current_overlay(app) else {
        tracing::warn!(generation, "overlay window missing — cannot show");
        return;
    };
    if let Err(e) = window.set_always_on_top(true) {
        tracing::warn!(generation, "failed to re-assert overlay always-on-top: {e}");
    }
    let shown = window.show();
    OVERLAY_HIDDEN_SINCE_MS.store(0, Ordering::SeqCst);
    super::overlay_native::check_after_show(&window);
    spawn_presence_check(app, window.clone(), generation, check_delay);
    if let Err(e) = &shown {
        tracing::warn!(generation, "failed to show overlay: {e}");
    }
    let position = window.outer_position().ok();
    let size = window.outer_size().ok();
    tracing::debug!(
        generation,
        show_ok = shown.is_ok(),
        visible = window.is_visible().unwrap_or(false),
        minimized = window.is_minimized().unwrap_or(false),
        x = position.map(|p| p.x).unwrap_or(i32::MIN),
        y = position.map(|p| p.y).unwrap_or(i32::MIN),
        width = size.map(|s| s.width).unwrap_or(0),
        height = size.map(|s| s.height).unwrap_or(0),
        "overlay shown"
    );
}

/// After the pill is shown, read a few screen pixels where it should be. If the
/// pill is not there, the frame never reached the screen (GL-006): log it,
/// rebuild the window on the spot once per dictation and show it again, and
/// keep it marked for a rebuild after the dictation if that fails.
fn spawn_presence_check(app: &AppHandle, window: WebviewWindow, generation: u64, delay: Duration) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("overlay-presence-check".into())
        .spawn(move || {
            std::thread::sleep(delay);
            // A tap may already have hidden the pill, or the window was replaced;
            // then there is nothing to check.
            if dictation_idle(&app)
                || OVERLAY_GENERATION.load(Ordering::SeqCst) != generation
                || !window.is_visible().unwrap_or(false)
            {
                return;
            }
            let (Ok(position), Ok(size), Ok(scale)) = (
                window.outer_position(),
                window.outer_size(),
                window.scale_factor(),
            ) else {
                return;
            };
            let (inside, corner) =
                super::overlay_native::presence_points(position.x, position.y, size.width, scale);
            let mut points = inside.clone();
            points.push(corner);
            let Some(pixels) = super::overlay_native::read_screen_pixels(&points) else {
                tracing::debug!(generation, "overlay presence check: screen not readable");
                return;
            };
            let (corner_pixel, inside_pixels) = pixels.split_last().expect("corner sampled");
            let presence = super::overlay_native::judge_presence(inside_pixels, *corner_pixel);
            let brightness = |p: &(u8, u8, u8)| p.0.max(p.1).max(p.2);
            let inside_max = inside_pixels.iter().map(brightness).max().unwrap_or(0);
            match presence {
                super::overlay_native::Presence::NotPresented => {
                    tracing::warn!(
                        generation,
                        inside_max,
                        corner = brightness(corner_pixel),
                        "overlay shown but not presented on screen"
                    );
                    mark_overlay_stale("pill not presented on screen");
                    request_overlay_repair(&app);
                }
                _ => tracing::debug!(
                    generation,
                    ?presence,
                    inside_max,
                    corner = brightness(corner_pixel),
                    "overlay presence check"
                ),
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("failed to start overlay presence check: {e}");
    }
}

/// What the overlay page reports about itself (GL-006 diagnostics).
#[derive(Clone, Copy, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayEvent {
    /// The page script started (first load or a WebView reload).
    Mounted,
    /// A `dictation-state` event reached the page.
    StateReceived,
    /// A frame was painted after the state arrived.
    FramePainted,
    /// No frame was painted within the page's deadline after the state arrived.
    FrameMissing,
    /// The page hid its window after a terminal state.
    Hidden,
    /// An uncaught script error or rejected promise in the page.
    ScriptError,
}

/// Dictation states the overlay may name; anything else is dropped.
const OVERLAY_STATES: &[&str] = &["starting", "recording", "processing", "done", "error"];

/// Sanitize page-supplied fields: unknown state names become empty and the
/// delay is capped at ten minutes.
fn overlay_report_fields(state: Option<&str>, delay_ms: Option<u64>) -> (&'static str, u64) {
    let state = state
        .and_then(|s| OVERLAY_STATES.iter().copied().find(|known| *known == s))
        .unwrap_or("");
    (state, delay_ms.map(|d| d.min(600_000)).unwrap_or(0))
}

/// Log a report from the overlay page. Only an allow-listed event, state name
/// and a bounded delay are accepted — the page cannot write free text to the log.
#[tauri::command]
pub fn report_overlay_event(
    window: WebviewWindow,
    event: OverlayEvent,
    state: Option<String>,
    delay_ms: Option<u64>,
) {
    let (state, delay_ms) = overlay_report_fields(state.as_deref(), delay_ms);
    let label = window.label();
    let current = overlay_label(OVERLAY_GENERATION.load(Ordering::SeqCst));
    match event {
        OverlayEvent::FrameMissing | OverlayEvent::ScriptError => tracing::warn!(
            window = label,
            current = label == current,
            ?event,
            state,
            delay_ms,
            "overlay page report"
        ),
        _ => tracing::debug!(
            window = label,
            current = label == current,
            ?event,
            state,
            delay_ms,
            "overlay page report"
        ),
    }
}

/// Hide the calling overlay window unless a new dictation already started.
/// Replaces the page's own delayed `hide()`, which could race a new `show()`.
#[tauri::command]
pub fn hide_overlay(app: AppHandle, window: WebviewWindow) {
    if !dictation_idle(&app) {
        tracing::debug!(
            window = window.label(),
            "overlay hide skipped: dictation active"
        );
        return;
    }
    if let Err(e) = window.hide() {
        tracing::warn!(window = window.label(), "failed to hide overlay: {e}");
    }
    OVERLAY_HIDDEN_SINCE_MS.store(unix_ms(), Ordering::SeqCst);
}

/// Label of the overlay window for a rebuild `generation`: `overlay` for the
/// startup window, `overlay-N` afterwards. A fresh label is needed because a
/// destroyed window releases its label only asynchronously.
pub fn overlay_label(generation: u64) -> String {
    if generation == 0 {
        OVERLAY_LABEL.to_string()
    } else {
        format!("{OVERLAY_LABEL}-{generation}")
    }
}

/// The overlay window currently in use, if it exists.
fn current_overlay(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(&overlay_label(OVERLAY_GENERATION.load(Ordering::SeqCst)))
}

/// Build a hidden overlay window (D5): building a WebView costs hundreds of ms,
/// which would be a stall at the exact moment the user starts speaking, so it
/// is built ahead of time and later only shown/hidden and repositioned.
/// `transparent` + always-on-top + `skipTaskbar` + `focused(false)` so the pill
/// floats over the target app without stealing its keyboard focus.
pub fn build_overlay(app: &AppHandle, label: &str) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Glagol overlay")
        .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .resizable(false)
        .shadow(false)
        .visible(false)
        // GL-006: a second "page load" for the same window means WebView2
        // reloaded the pill (for example after its renderer process failed).
        .on_page_load(|window, payload| {
            tracing::info!(
                window = window.label(),
                event = ?payload.event(),
                "overlay page load"
            );
        })
        .build()?;
    let window_label = window.label().to_string();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::Destroyed => {
            tracing::info!(window = %window_label, "overlay window destroyed")
        }
        tauri::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            tracing::info!(window = %window_label, scale_factor, "overlay scale changed")
        }
        tauri::WindowEvent::Focused(true) => {
            tracing::debug!(window = %window_label, "overlay took focus")
        }
        _ => {}
    });
    super::overlay_native::install_process_failed(&window, || {
        mark_overlay_stale("webview2 process failed")
    });
    // A new window starts hidden: its idle period starts now.
    OVERLAY_HIDDEN_SINCE_MS.store(unix_ms(), Ordering::SeqCst);
    tracing::info!(window = label, "overlay window built");
    Ok(window)
}

/// Whether a wall-clock advance of `wall_elapsed` over one `poll` interval
/// means the system was asleep. A clock moved backwards (`None`) is not sleep.
fn resumed_from_sleep(poll: Duration, wall_elapsed: Option<Duration>) -> bool {
    wall_elapsed.is_some_and(|elapsed| elapsed > poll + RESUME_JUMP_MARGIN)
}

/// Watch for the system resuming from sleep and rebuild the overlay window.
///
/// After sleep the hidden transparent WebView2 of the overlay can lose its
/// rendering: dictation keeps working, but the pill stays invisible until the
/// app restarts. A new window fixes it the same way. The rebuild waits one more
/// poll so the graphics stack settles, and never runs during a dictation
/// session. A false positive (the clock was set forward) only rebuilds the
/// hidden window.
pub fn spawn_resume_watch(app: AppHandle) {
    // Display on, unlock, remote/console connect, display change and resume
    // (GL-006). The clock-jump check below stays as a fallback for sleep.
    super::overlay_native::spawn_system_watch(|event| {
        tracing::info!(?event, "system display/session event");
        if event.requires_rebuild() {
            mark_overlay_stale("system event");
        }
    });
    let spawned = std::thread::Builder::new()
        .name("overlay-resume-watch".into())
        .spawn(move || {
            let mut last = SystemTime::now();
            let mut settled = true;
            loop {
                std::thread::sleep(RESUME_POLL);
                let now = SystemTime::now();
                let resumed = resumed_from_sleep(RESUME_POLL, now.duration_since(last).ok());
                last = now;
                if resumed {
                    tracing::info!("system resume detected; overlay will be rebuilt");
                    mark_overlay_stale("clock jump after sleep");
                    // Let the graphics stack settle for one more poll.
                    settled = false;
                    continue;
                }
                if !settled {
                    settled = true;
                    continue;
                }
                if !OVERLAY_STALE.load(Ordering::SeqCst)
                    && idle_rebuild_due(OVERLAY_HIDDEN_SINCE_MS.load(Ordering::SeqCst), unix_ms())
                    && dictation_idle(&app)
                {
                    mark_overlay_stale("hidden for a long idle period");
                }
                if OVERLAY_STALE.load(Ordering::SeqCst) && dictation_idle(&app) {
                    let handle = app.clone();
                    if let Err(e) = app.run_on_main_thread(move || rebuild_overlay(&handle)) {
                        tracing::warn!("failed to schedule overlay rebuild: {e}");
                    }
                }
            }
        });
    if let Err(e) = spawned {
        tracing::warn!("failed to start overlay resume watcher: {e}");
    }
}

/// Whether no dictation session is in progress.
fn dictation_idle(app: &AppHandle) -> bool {
    matches!(
        *app.state::<AppState>()
            .dictation
            .lock()
            .expect("dictation phase mutex poisoned"),
        DictationPhase::Idle
    )
}

/// Replace the overlay window with a freshly built one. The new window is
/// built before the old one is destroyed, so a failed build keeps the old
/// window and dictation never loses its overlay entirely.
fn rebuild_overlay(app: &AppHandle) {
    if !dictation_idle(app) {
        tracing::debug!("overlay rebuild skipped: dictation in progress");
        return;
    }
    if !OVERLAY_STALE.swap(false, Ordering::SeqCst) {
        return;
    }
    if let Some(next) = replace_overlay(app) {
        tracing::info!(generation = next, "overlay window rebuilt");
    }
}

/// Build the next overlay window, make it current and destroy the old one.
/// Returns the new generation, or `None` (old window kept, still marked
/// stale) when the build failed.
fn replace_overlay(app: &AppHandle) -> Option<u64> {
    let old = current_overlay(app);
    let next = OVERLAY_GENERATION.load(Ordering::SeqCst).wrapping_add(1);
    if let Err(e) = build_overlay(app, &overlay_label(next)) {
        tracing::warn!("failed to rebuild overlay window: {e}");
        OVERLAY_STALE.store(true, Ordering::SeqCst);
        return None;
    }
    OVERLAY_GENERATION.store(next, Ordering::SeqCst);
    if let Some(old) = old {
        if let Err(e) = old.destroy() {
            tracing::warn!("failed to destroy previous overlay window: {e}");
        }
    }
    Some(next)
}

/// Whether a dictation session `token` may still get its one immediate
/// overlay repair; records the claim.
fn claim_overlay_repair(repaired: &AtomicU64, token: u64) -> bool {
    repaired.swap(token, Ordering::SeqCst) != token
}

/// Schedule an immediate rebuild of a pill that did not reach the screen
/// during dictation, at most once per session (GL-006).
fn request_overlay_repair(app: &AppHandle) {
    let token = app
        .state::<AppState>()
        .dictation_generation
        .load(Ordering::SeqCst);
    if !claim_overlay_repair(&OVERLAY_REPAIRED_TOKEN, token) {
        tracing::debug!("overlay repair already used in this dictation");
        return;
    }
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || repair_overlay_now(&handle)) {
        tracing::warn!("failed to schedule overlay repair: {e}");
    }
}

/// Replace the overlay window while dictation runs and show the new one at
/// the same place. Its page fetches the current state with [`overlay_state`].
fn repair_overlay_now(app: &AppHandle) {
    if dictation_idle(app) {
        // The dictation ended meanwhile; the idle watcher rebuilds the window.
        return;
    }
    OVERLAY_STALE.store(false, Ordering::SeqCst);
    let Some(next) = replace_overlay(app) else {
        return;
    };
    tracing::info!(generation = next, "overlay window rebuilt during dictation");
    position_overlay(app);
    show_overlay_checked_after(app, REPAIRED_PRESENCE_CHECK_DELAY);
}

// ── Tray ────────────────────────────────────────────────────────────────

/// Build the system tray (D11): idle icon, a «Показать / Выход» menu, and the
/// tooltip. Called once from the setup hook.
pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = tray_menu(app)?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(tauri::image::Image::from_bytes(TRAY_IDLE_PNG)?)
        .tooltip(crate::preferences::message(
            "Glagol — dictation",
            "Glagol — диктовка",
        ))
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

fn tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(
        app,
        "show",
        crate::preferences::message("Show Glagol", "Показать Glagol"),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(
        app,
        "quit",
        crate::preferences::message("Quit", "Выход"),
        true,
        None::<&str>,
    )?;
    Menu::with_items(app, &[&show, &quit])
}

pub fn refresh_tray_language(app: &AppHandle) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(tray_menu(app)?))?;
        tray.set_tooltip(Some(crate::preferences::message(
            "Glagol — dictation",
            "Glagol — диктовка",
        )))?;
    }
    Ok(())
}

/// Swap the tray icon between idle and recording (D11). Best-effort: a missing
/// tray or a decode failure is logged, never fatal.
pub(crate) fn set_tray_recording(app: &AppHandle, recording: bool) {
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

/// Show + un-minimize + focus the main window. Used by the tray «Показать»
/// item (D11) and by the single-instance callback (v0.2.1): a second launch
/// must bring the running copy forward from the tray, not silently no-op. The
/// window may be hidden (closed to tray), minimized, or already visible — each
/// step is best-effort so any starting state ends up shown and focused.
pub fn show_main_window(app: &AppHandle) {
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
        .message(crate::preferences::message("Glagol will keep running in the notification area. Your dictation shortcut remains available.", "Glagol продолжит работать в области уведомлений. Горячая клавиша диктовки остаётся доступной."))
        .title(crate::preferences::message("Glagol is running in the tray", "Glagol свернулся в трей"))
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
    fn idle_rebuild_is_due_only_after_ten_hidden_minutes() {
        let ten_min = 10 * 60 * 1000;
        assert!(
            !idle_rebuild_due(0, ten_min * 5),
            "shown pill is never rebuilt"
        );
        assert!(!idle_rebuild_due(1_000, 1_000 + ten_min - 1));
        assert!(idle_rebuild_due(1_000, 1_000 + ten_min));
        assert!(!idle_rebuild_due(5_000, 1_000), "clock moved back");
    }

    #[test]
    fn immediate_overlay_repair_is_claimed_once_per_dictation() {
        let repaired = AtomicU64::new(0);
        assert!(claim_overlay_repair(&repaired, 1));
        assert!(
            !claim_overlay_repair(&repaired, 1),
            "second loss, same dictation"
        );
        assert!(claim_overlay_repair(&repaired, 2), "next dictation");
    }

    #[test]
    fn overlay_remembers_the_last_broadcast_state() {
        remember_overlay_state(&DictationState::Recording);
        assert_eq!(
            OVERLAY_LAST_STATE.lock().unwrap().clone(),
            Some(DictationState::Recording)
        );
    }

    #[test]
    fn overlay_report_accepts_only_known_states_and_bounded_delays() {
        assert_eq!(
            overlay_report_fields(Some("recording"), Some(40)),
            ("recording", 40)
        );
        assert_eq!(overlay_report_fields(Some("secret text"), None), ("", 0));
        assert_eq!(overlay_report_fields(None, Some(u64::MAX)), ("", 600_000));
        let event: OverlayEvent = serde_json::from_str("\"frame_missing\"").unwrap();
        assert!(matches!(event, OverlayEvent::FrameMissing));
        assert!(serde_json::from_str::<OverlayEvent>("\"anything\"").is_err());
    }

    /// A minimal `AppState` for the advisory-history tests: real in-memory DB, no
    /// recorder thread. Mirrors `commands::dictation::tests::fresh_state`.
    fn fresh_state() -> AppState {
        let conn = test_connection();
        AppState::new(conn, crate::dictation::RecorderHandle::disconnected())
    }

    fn dictation_rows(state: &AppState) -> Vec<repository::Dictation> {
        let conn = state.db.lock().unwrap();
        repository::list_dictations(&conn, None).unwrap()
    }

    #[test]
    fn record_dictation_history_gated_by_toggle() {
        // D4 negative cycle: with history OFF the transcript must NOT be written;
        // with it ON, exactly one row lands. Deleting the `Ok(false) => return`
        // gate in `record_dictation_history` makes the OFF assertion fail (a row
        // is written while history is disabled).
        let state = fresh_state();

        // Off (default): nothing is written.
        record_dictation_history(&state, "секрет", "clipboard", 1_500, None);
        assert!(
            dictation_rows(&state).is_empty(),
            "history off must not write the transcript to disk"
        );

        // Turn history on.
        {
            let conn = state.db.lock().unwrap();
            repository::set_setting(&conn, "dictation_history_enabled", "true", 1).unwrap();
        }

        record_dictation_history(&state, "виден", "clipboard", 1_500, None);
        let rows = dictation_rows(&state);
        assert_eq!(rows.len(), 1, "history on must write exactly one row");
        assert_eq!(rows[0].text, "виден");
        assert_eq!(rows[0].status, "clipboard");
    }

    #[test]
    fn record_dictation_history_writes_error_rows_when_on() {
        // A failed dictation earns an error row (D2) — status `error`, the Russian
        // message in `error_message`, empty transcript.
        let state = fresh_state();
        {
            let conn = state.db.lock().unwrap();
            repository::set_setting(&conn, "dictation_history_enabled", "true", 1).unwrap();
        }
        record_dictation_history(
            &state,
            "",
            "error",
            0,
            Some("ключ недействителен".to_string()),
        );
        let rows = dictation_rows(&state);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "error");
        assert_eq!(
            rows[0].error_message,
            Some("ключ недействителен".to_string())
        );
        assert_eq!(rows[0].text, "");
    }

    #[test]
    fn dictation_status_maps_disposition_to_persisted_string() {
        assert_eq!(dictation_status(Disposition::Pasted), "pasted");
        assert_eq!(dictation_status(Disposition::Clipboard), "clipboard");
    }

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

    #[test]
    fn overlay_label_keeps_startup_name_then_numbers_rebuilds() {
        assert_eq!(overlay_label(0), "overlay");
        assert_eq!(overlay_label(1), "overlay-1");
        assert_eq!(overlay_label(42), "overlay-42");
    }

    #[test]
    fn resume_detected_only_on_large_wall_clock_jump() {
        let poll = Duration::from_secs(3);
        // Normal tick and scheduler jitter: not sleep.
        assert!(!resumed_from_sleep(poll, Some(poll)));
        assert!(!resumed_from_sleep(poll, Some(poll + RESUME_JUMP_MARGIN)));
        // Clock moved backwards: not sleep.
        assert!(!resumed_from_sleep(poll, None));
        // Lid closed for a while: the wall clock jumped ahead.
        assert!(resumed_from_sleep(
            poll,
            Some(poll + RESUME_JUMP_MARGIN + Duration::from_secs(1))
        ));
        assert!(resumed_from_sleep(poll, Some(Duration::from_secs(3600))));
    }
}
