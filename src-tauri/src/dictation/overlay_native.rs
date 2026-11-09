//! Native diagnostics and recovery hooks for the dictation overlay (GL-006).
//!
//! The overlay is a transparent WebView2 window that spends most of its life
//! hidden. Reports from WebView2 hosts show that after a display, session or
//! GPU transition such a window can stay "visible" to the OS and keep running
//! JavaScript while its pixels never reach the screen; a fresh window fixes it.
//! This module observes the Windows events that precede such failures, the
//! WebView2 `ProcessFailed` event, and the window/controller state at each
//! show. Nothing here reads page content; only flags, codes and geometry are
//! logged.
//!
//! On other platforms the hooks are no-ops so the crate builds for CI.

/// A Windows notification that can invalidate the hidden overlay's rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemEvent {
    /// The console display turned off (`GUID_CONSOLE_DISPLAY_STATE` = 0).
    DisplayOff,
    /// The console display turned on again after being off.
    DisplayOn,
    /// The console display dimmed.
    DisplayDimmed,
    /// The system resumed from sleep or hibernation.
    Resume,
    /// The session was locked.
    SessionLock,
    /// The session was unlocked.
    SessionUnlock,
    /// A remote desktop client connected to this session.
    RemoteConnect,
    /// A remote desktop client disconnected from this session.
    RemoteDisconnect,
    /// The session was connected to the physical console.
    ConsoleConnect,
    /// The session was disconnected from the physical console.
    ConsoleDisconnect,
    /// Display mode or topology changed (`WM_DISPLAYCHANGE`).
    DisplayChange,
}

impl SystemEvent {
    /// Whether the overlay window should be rebuilt before it is used again.
    pub fn requires_rebuild(self) -> bool {
        matches!(
            self,
            Self::DisplayOn
                | Self::Resume
                | Self::SessionUnlock
                | Self::RemoteConnect
                | Self::RemoteDisconnect
                | Self::ConsoleConnect
                | Self::DisplayChange
        )
    }
}

/// Map a `WM_WTSSESSION_CHANGE` code to an event (logon/logoff are ignored).
pub fn session_event(code: u32) -> Option<SystemEvent> {
    match code {
        1 => Some(SystemEvent::ConsoleConnect),
        2 => Some(SystemEvent::ConsoleDisconnect),
        3 => Some(SystemEvent::RemoteConnect),
        4 => Some(SystemEvent::RemoteDisconnect),
        7 => Some(SystemEvent::SessionLock),
        8 => Some(SystemEvent::SessionUnlock),
        _ => None,
    }
}

/// Map a `GUID_CONSOLE_DISPLAY_STATE` value given the previous one. Windows
/// sends the current state right after registration and repeats states, so
/// `DisplayOn` is reported only when the display was off or dimmed before.
pub fn display_event(previous: Option<u32>, state: u32) -> Option<SystemEvent> {
    match state {
        0 if previous != Some(0) => Some(SystemEvent::DisplayOff),
        1 if matches!(previous, Some(0) | Some(2)) => Some(SystemEvent::DisplayOn),
        2 if previous != Some(2) => Some(SystemEvent::DisplayDimmed),
        _ => None,
    }
}

#[cfg(windows)]
mod imp {
    use super::{display_event, session_event, SystemEvent};
    use std::sync::{Mutex, OnceLock};
    use tauri::WebviewWindow;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2ProcessFailedEventArgs2, COREWEBVIEW2_PROCESS_FAILED_KIND,
        COREWEBVIEW2_PROCESS_FAILED_REASON,
    };
    use webview2_com::ProcessFailedEventHandler;
    use windows::core::{w, Interface, BOOL};
    use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Power::{RegisterPowerSettingNotification, POWERBROADCAST_SETTING};
    use windows::Win32::System::RemoteDesktop::{
        WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
    };
    use windows::Win32::System::SystemServices::GUID_CONSOLE_DISPLAY_STATE;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowLongPtrW,
        RegisterClassW, TranslateMessage, DEVICE_NOTIFY_WINDOW_HANDLE, GWL_EXSTYLE, MSG,
        PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_POWERSETTINGCHANGE, WINDOW_EX_STYLE,
        WM_DISPLAYCHANGE, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE, WNDCLASSW, WS_EX_TOPMOST,
        WS_OVERLAPPED,
    };

    type Callback = Box<dyn Fn(SystemEvent) + Send + Sync>;
    static CALLBACK: OnceLock<Callback> = OnceLock::new();
    static DISPLAY_STATE: Mutex<Option<u32>> = Mutex::new(None);

    fn emit(event: SystemEvent) {
        if let Some(callback) = CALLBACK.get() {
            callback(event);
        }
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_DISPLAYCHANGE => emit(SystemEvent::DisplayChange),
            WM_WTSSESSION_CHANGE => {
                if let Some(event) = session_event(wparam.0 as u32) {
                    emit(event);
                }
            }
            WM_POWERBROADCAST => match wparam.0 as u32 {
                PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => emit(SystemEvent::Resume),
                PBT_POWERSETTINGCHANGE if lparam.0 != 0 => {
                    // SAFETY: for PBT_POWERSETTINGCHANGE, lParam points to a
                    // POWERBROADCAST_SETTING valid for this call. The display
                    // state payload is one DWORD; it is read unaligned because
                    // `Data` is declared as a one-byte array.
                    let (is_display, state) = unsafe {
                        let setting = &*(lparam.0 as *const POWERBROADCAST_SETTING);
                        (
                            setting.PowerSetting == GUID_CONSOLE_DISPLAY_STATE
                                && setting.DataLength >= 4,
                            std::ptr::read_unaligned(setting.Data.as_ptr() as *const u32),
                        )
                    };
                    if is_display {
                        if let Ok(mut previous) = DISPLAY_STATE.lock() {
                            let event = display_event(*previous, state);
                            *previous = Some(state);
                            if let Some(event) = event {
                                emit(event);
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
        // SAFETY: forwarding the unmodified message to the default procedure.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// Start a thread with a hidden top-level window that receives display,
    /// power and session notifications. Failures are logged; dictation does
    /// not depend on this watcher.
    pub fn spawn_system_watch(on_event: impl Fn(SystemEvent) + Send + Sync + 'static) {
        if CALLBACK.set(Box::new(on_event)).is_err() {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name("overlay-system-watch".into())
            .spawn(|| {
                // SAFETY: standard Win32 window-class registration and message
                // loop on this dedicated thread. The class name and window
                // procedure are 'static; the window is never shown, lives for
                // the process lifetime and is only used to receive broadcast,
                // power-setting and session notifications.
                unsafe {
                    let instance = match GetModuleHandleW(None) {
                        Ok(module) => module.into(),
                        Err(e) => {
                            tracing::warn!("overlay system watch: module handle unavailable: {e}");
                            return;
                        }
                    };
                    let class = WNDCLASSW {
                        lpfnWndProc: Some(wndproc),
                        hInstance: instance,
                        lpszClassName: w!("GlagolSystemWatch"),
                        ..Default::default()
                    };
                    if RegisterClassW(&class) == 0 {
                        tracing::warn!("overlay system watch: window class registration failed");
                        return;
                    }
                    let hwnd = match CreateWindowExW(
                        WINDOW_EX_STYLE(0),
                        w!("GlagolSystemWatch"),
                        w!("Glagol system watch"),
                        WS_OVERLAPPED,
                        0,
                        0,
                        0,
                        0,
                        None,
                        None,
                        Some(instance),
                        None,
                    ) {
                        Ok(hwnd) => hwnd,
                        Err(e) => {
                            tracing::warn!("overlay system watch: window creation failed: {e}");
                            return;
                        }
                    };
                    let display_registration = RegisterPowerSettingNotification(
                        HANDLE(hwnd.0),
                        &GUID_CONSOLE_DISPLAY_STATE,
                        DEVICE_NOTIFY_WINDOW_HANDLE,
                    );
                    let session_registration =
                        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
                    tracing::info!(
                        display_notifications = display_registration.is_ok(),
                        session_notifications = session_registration.is_ok(),
                        "overlay system watch started"
                    );
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                        let _ = TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("failed to start overlay system watch: {e}");
        }
    }

    /// Log WebView2 process failures for `window` and report them through
    /// `on_failed` (called on the main thread).
    pub fn install_process_failed(window: &WebviewWindow, on_failed: impl Fn() + Send + 'static) {
        let label = window.label().to_string();
        let installed = window.with_webview(move |webview| {
            // SAFETY: Tauri runs this closure on the main thread with a live
            // controller. The handler is registered on the CoreWebView2 of this
            // window, and its arguments are only read during the callback.
            unsafe {
                let core = match webview.controller().CoreWebView2() {
                    Ok(core) => core,
                    Err(e) => {
                        tracing::warn!(window = %label, "overlay WebView2 core unavailable: {e}");
                        return;
                    }
                };
                let window_label = label.clone();
                let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
                    let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND(-1);
                    let mut reason = COREWEBVIEW2_PROCESS_FAILED_REASON(-1);
                    let mut exit_code = 0i32;
                    if let Some(args) = args {
                        let _ = args.ProcessFailedKind(&mut kind);
                        if let Ok(args2) = args.cast::<ICoreWebView2ProcessFailedEventArgs2>() {
                            let _ = args2.Reason(&mut reason);
                            let _ = args2.ExitCode(&mut exit_code);
                        }
                    }
                    tracing::warn!(
                        window = %window_label,
                        kind = kind.0,
                        reason = reason.0,
                        exit_code,
                        "overlay WebView2 process failed"
                    );
                    on_failed();
                    Ok(())
                }));
                let mut token = 0i64;
                match core.add_ProcessFailed(&handler, &mut token) {
                    Ok(()) => {
                        tracing::debug!(window = %label, "overlay ProcessFailed handler installed")
                    }
                    Err(e) => {
                        tracing::warn!(window = %label, "overlay ProcessFailed handler failed: {e}")
                    }
                }
            }
        });
        if let Err(e) = installed {
            tracing::warn!("overlay ProcessFailed handler not scheduled: {e}");
        }
    }

    /// Log what Windows and WebView2 think of the overlay right after `show()`
    /// and nudge the controller: a WebView2 controller left invisible is made
    /// visible again, and the moved parent window is reported to it.
    pub fn check_after_show(window: &WebviewWindow) {
        let (topmost, cloaked) = match window.hwnd() {
            // SAFETY: `hwnd` belongs to a live Tauri window; both calls only
            // read window state into local storage of the stated size.
            Ok(hwnd) => unsafe {
                let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
                let mut cloaked = 0u32;
                let read = DwmGetWindowAttribute(
                    hwnd,
                    DWMWA_CLOAKED,
                    &mut cloaked as *mut u32 as *mut core::ffi::c_void,
                    std::mem::size_of::<u32>() as u32,
                );
                (
                    style & WS_EX_TOPMOST.0 as isize != 0,
                    if read.is_ok() { cloaked as i64 } else { -1 },
                )
            },
            Err(_) => (false, -1),
        };
        let label = window.label().to_string();
        let scheduled = window.with_webview(move |webview| {
            // SAFETY: Tauri runs this closure on the main thread with a live
            // controller; out-parameters are local and correctly sized.
            unsafe {
                let controller = webview.controller();
                let mut visible = BOOL(0);
                let visible_read = controller.IsVisible(&mut visible).is_ok();
                let mut bounds = RECT::default();
                let _ = controller.Bounds(&mut bounds);
                let corrected = visible_read && !visible.as_bool() && controller.SetIsVisible(true).is_ok();
                let notified = controller.NotifyParentWindowPositionChanged().is_ok();
                tracing::debug!(
                    window = %label,
                    topmost,
                    cloaked,
                    controller_visible = visible.as_bool(),
                    corrected,
                    notified,
                    bounds_w = bounds.right - bounds.left,
                    bounds_h = bounds.bottom - bounds.top,
                    "overlay native state"
                );
                if corrected {
                    tracing::warn!(window = %label, "overlay WebView2 controller was invisible after show; made visible");
                }
            }
        });
        if let Err(e) = scheduled {
            tracing::warn!("overlay native state check not scheduled: {e}");
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SystemEvent;
    use tauri::WebviewWindow;

    /// No-op outside Windows.
    pub fn spawn_system_watch(_on_event: impl Fn(SystemEvent) + Send + Sync + 'static) {}
    /// No-op outside Windows.
    pub fn install_process_failed(_window: &WebviewWindow, _on_failed: impl Fn() + Send + 'static) {
    }
    /// No-op outside Windows.
    pub fn check_after_show(_window: &WebviewWindow) {}
}

pub use imp::{check_after_show, install_process_failed, spawn_system_watch};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_on_is_reported_only_after_off_or_dim() {
        assert_eq!(
            display_event(None, 1),
            None,
            "initial state after registration"
        );
        assert_eq!(display_event(Some(1), 0), Some(SystemEvent::DisplayOff));
        assert_eq!(display_event(Some(0), 0), None, "repeated off");
        assert_eq!(display_event(Some(0), 1), Some(SystemEvent::DisplayOn));
        assert_eq!(display_event(Some(1), 2), Some(SystemEvent::DisplayDimmed));
        assert_eq!(display_event(Some(2), 1), Some(SystemEvent::DisplayOn));
        assert_eq!(display_event(Some(1), 1), None);
        assert_eq!(display_event(Some(1), 7), None, "unknown state");
    }

    #[test]
    fn session_codes_map_and_rebuild_only_on_returning_events() {
        assert_eq!(session_event(8), Some(SystemEvent::SessionUnlock));
        assert_eq!(session_event(7), Some(SystemEvent::SessionLock));
        assert_eq!(session_event(3), Some(SystemEvent::RemoteConnect));
        assert_eq!(session_event(5), None, "logon is ignored");
        assert!(SystemEvent::SessionUnlock.requires_rebuild());
        assert!(SystemEvent::DisplayOn.requires_rebuild());
        assert!(SystemEvent::DisplayChange.requires_rebuild());
        assert!(!SystemEvent::DisplayOff.requires_rebuild());
        assert!(!SystemEvent::SessionLock.requires_rebuild());
        assert!(!SystemEvent::DisplayDimmed.requires_rebuild());
    }
}
