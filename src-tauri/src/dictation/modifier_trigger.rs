//! Push-to-talk on holding Ctrl+Win, without a third key (GL-009).
//!
//! `RegisterHotKey` (the global-shortcut plugin) needs an ordinary key, so a
//! modifier-only chord is read from a low-level keyboard hook on Windows (see
//! [`imp`]). The decision logic lives in [`ModifierTrigger`], a pure state
//! machine that runs on every platform and is unit-tested.
//!
//! Rules (agreed 2026-11-19):
//! - Ctrl and Win held together, with no other key pressed since the first of
//!   them went down, start a dictation after [`ARM_DELAY`] — quick Windows
//!   shortcuts such as Ctrl+Win+D never flash the pill;
//! - releasing Ctrl or Win stops the dictation as usual;
//! - any other key while the dictation runs cancels it silently (the chord was
//!   a Windows shortcut), and the chord does not re-arm until both keys are up;
//! - it works alongside the ordinary dictation hotkey, which stays the fallback
//!   for elevated windows that a non-elevated hook cannot observe.
//!
//! Privacy: only whether Ctrl, Win or "some other key" changed is used; key
//! codes are never stored or logged.

use std::time::{Duration, Instant};

/// How long Ctrl+Win must be held alone before a dictation starts.
pub const ARM_DELAY: Duration = Duration::from_millis(200);

/// The key classes the trigger distinguishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Left or right Ctrl.
    Ctrl,
    /// Left or right Windows key.
    Win,
    /// Any other key, including Shift and Alt.
    Other,
}

/// Classify a Windows virtual-key code.
pub fn classify(vk: u32) -> Key {
    match vk {
        // VK_CONTROL, VK_LCONTROL, VK_RCONTROL
        0x11 | 0xA2 | 0xA3 => Key::Ctrl,
        // VK_LWIN, VK_RWIN
        0x5B | 0x5C => Key::Win,
        _ => Key::Other,
    }
}

/// What the dictation session should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Start a dictation (like the hotkey's `Pressed`).
    Press,
    /// Stop recording and deliver (like the hotkey's `Released`).
    Release,
    /// Stop recording and discard: the chord was a Windows shortcut.
    Cancel,
}

/// The Ctrl+Win state machine. Feed it every key transition and call
/// [`ModifierTrigger::on_tick`] at [`ModifierTrigger::deadline`].
#[derive(Debug, Default)]
pub struct ModifierTrigger {
    ctrl: bool,
    win: bool,
    /// Another key was pressed while Ctrl or Win was held: no dictation until
    /// both are released.
    tainted: bool,
    /// When Ctrl+Win became held alone; the dictation starts at
    /// `armed_since + ARM_DELAY`.
    armed_since: Option<Instant>,
    /// A dictation started by this trigger is running.
    active: bool,
}

impl ModifierTrigger {
    /// A key went down (`down = true`) or up. Auto-repeat downs are harmless.
    pub fn on_key(&mut self, key: Key, down: bool, now: Instant) -> Option<Action> {
        match (key, down) {
            (Key::Ctrl | Key::Win, true) => {
                if key == Key::Ctrl {
                    self.ctrl = true;
                } else {
                    self.win = true;
                }
                if self.ctrl && self.win && !self.tainted && !self.active {
                    self.armed_since.get_or_insert(now);
                }
                None
            }
            (Key::Ctrl | Key::Win, false) => {
                if key == Key::Ctrl {
                    self.ctrl = false;
                } else {
                    self.win = false;
                }
                self.armed_since = None;
                if !self.ctrl && !self.win {
                    self.tainted = false;
                }
                if std::mem::take(&mut self.active) {
                    Some(Action::Release)
                } else {
                    None
                }
            }
            (Key::Other, true) => {
                if !self.ctrl && !self.win {
                    return None; // ordinary typing
                }
                self.tainted = true;
                self.armed_since = None;
                if std::mem::take(&mut self.active) {
                    Some(Action::Cancel)
                } else {
                    None
                }
            }
            (Key::Other, false) => None,
        }
    }

    /// Time passed: start the dictation once Ctrl+Win was held long enough.
    pub fn on_tick(&mut self, now: Instant) -> Option<Action> {
        let armed = self.armed_since?;
        if now.duration_since(armed) < ARM_DELAY {
            return None;
        }
        self.armed_since = None;
        if self.ctrl && self.win && !self.tainted && !self.active {
            self.active = true;
            Some(Action::Press)
        } else {
            None
        }
    }

    /// When [`ModifierTrigger::on_tick`] should be called next, if at all.
    pub fn deadline(&self) -> Option<Instant> {
        self.armed_since.map(|t| t + ARM_DELAY)
    }
}

/// Whether Ctrl+Win dictation can run on this platform.
pub const SUPPORTED: bool = cfg!(windows);

#[cfg(windows)]
mod imp {
    use super::{classify, Action, Key, ModifierTrigger};
    use std::sync::mpsc::{self, RecvTimeoutError, Sender};
    use std::sync::Mutex;
    use std::time::Instant;
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Threading::GetCurrentThreadId;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
        HC_ACTION, KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
        WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    /// Unassigned virtual key used to keep Windows from opening Start when Win
    /// is released after the chord (the "menu mask" technique of AutoHotkey).
    const MENU_MASK_VK: u16 = 0xE8;

    /// Key classes from the hook to the worker; only the class leaves the hook.
    static EVENTS: Mutex<Option<Sender<(Key, bool)>>> = Mutex::new(None);
    /// Thread id of the running hook thread, for [`stop`].
    static HOOK_THREAD: Mutex<Option<u32>> = Mutex::new(None);

    /// Low-level keyboard hook: forward the key class and pass every event on.
    /// It never swallows input and never logs keys. Injected events (our menu
    /// mask, the paste keystrokes of text insertion) are ignored.
    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lParam points to a
            // KBDLLHOOKSTRUCT valid for the duration of this call.
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            if (info.flags & LLKHF_INJECTED).0 == 0 {
                let message = wparam.0 as u32;
                let down = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;
                let up = message == WM_KEYUP || message == WM_SYSKEYUP;
                if down || up {
                    if let Ok(events) = EVENTS.lock() {
                        if let Some(tx) = events.as_ref() {
                            let _ = tx.send((classify(info.vkCode), down));
                        }
                    }
                }
            }
        }
        // SAFETY: passing the unchanged hook arguments on is required by the
        // hook contract; a null hook handle is allowed.
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    /// Press and release the unassigned mask key while Win is still held.
    fn send_menu_mask() {
        let key = |flags: KEYBD_EVENT_FLAGS| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(MENU_MASK_VK),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [key(KEYBD_EVENT_FLAGS(0)), key(KEYEVENTF_KEYUP)];
        // SAFETY: `inputs` is a valid array of keyboard INPUT records and the
        // size argument is the size of one INPUT, as SendInput requires.
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 {
            tracing::debug!(sent, "Ctrl+Win: Start menu mask not fully sent");
        }
    }

    /// Install the hook (if it is not running) and call `on_action` for every
    /// trigger decision, from a worker thread.
    pub fn start(on_action: impl Fn(Action) + Send + 'static) {
        let mut thread = HOOK_THREAD.lock().expect("hook thread mutex poisoned");
        if thread.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel::<(Key, bool)>();
        *EVENTS.lock().expect("hook events mutex poisoned") = Some(tx);
        let (tid_tx, tid_rx) = mpsc::channel::<Option<u32>>();
        let hook = std::thread::Builder::new()
            .name("ctrl-win-hook".into())
            .spawn(move || {
                // SAFETY: standard WH_KEYBOARD_LL installation on this dedicated
                // thread, which runs the message loop the hook requires; the
                // hook procedure is 'static and is removed before the thread
                // ends.
                unsafe {
                    let module = GetModuleHandleW(None).ok();
                    let hook = match SetWindowsHookExW(
                        WH_KEYBOARD_LL,
                        Some(hook_proc),
                        module.map(Into::into),
                        0,
                    ) {
                        Ok(hook) => hook,
                        Err(e) => {
                            tracing::warn!("Ctrl+Win: keyboard hook not installed: {e}");
                            let _ = tid_tx.send(None);
                            return;
                        }
                    };
                    let _ = tid_tx.send(Some(GetCurrentThreadId()));
                    tracing::info!("Ctrl+Win dictation hook installed");
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).0 > 0 {}
                    let _ = UnhookWindowsHookEx(hook);
                    tracing::info!("Ctrl+Win dictation hook removed");
                }
            });
        if let Err(e) = hook {
            tracing::warn!("failed to start Ctrl+Win hook thread: {e}");
            *EVENTS.lock().expect("hook events mutex poisoned") = None;
            return;
        }
        *thread = tid_rx.recv().ok().flatten();
        if thread.is_none() {
            *EVENTS.lock().expect("hook events mutex poisoned") = None;
            return;
        }
        let worker = std::thread::Builder::new()
            .name("ctrl-win-trigger".into())
            .spawn(move || {
                let mut trigger = ModifierTrigger::default();
                loop {
                    let event = match trigger.deadline() {
                        Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                        None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    let action = match event {
                        Ok((key, down)) => trigger.on_key(key, down, Instant::now()),
                        Err(RecvTimeoutError::Timeout) => trigger.on_tick(Instant::now()),
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    if let Some(action) = action {
                        if action == Action::Press {
                            send_menu_mask();
                        }
                        tracing::debug!(?action, "Ctrl+Win dictation trigger");
                        on_action(action);
                    }
                }
            });
        if let Err(e) = worker {
            tracing::warn!("failed to start Ctrl+Win trigger thread: {e}");
        }
    }

    /// Remove the hook if it is installed. Dropping the event sender ends the
    /// worker; the hook thread unhooks when it reads WM_QUIT.
    pub fn stop() {
        *EVENTS.lock().expect("hook events mutex poisoned") = None;
        let thread = HOOK_THREAD
            .lock()
            .expect("hook thread mutex poisoned")
            .take();
        if let Some(id) = thread {
            // SAFETY: posting WM_QUIT to the hook thread's message queue ends
            // its GetMessageW loop; an already finished thread makes this fail
            // harmlessly.
            let _ = unsafe { PostThreadMessageW(id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Action;
    /// Ctrl+Win dictation is Windows-only.
    pub fn start(_on_action: impl Fn(Action) + Send + 'static) {}
    /// No-op outside Windows.
    pub fn stop() {}
}

pub use imp::{start, stop};

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    #[test]
    fn holding_ctrl_win_starts_after_the_delay_and_release_stops() {
        let t0 = Instant::now();
        let mut t = ModifierTrigger::default();
        assert_eq!(t.on_key(Key::Ctrl, true, t0), None);
        assert_eq!(t.on_key(Key::Win, true, ms(t0, 30)), None);
        assert_eq!(t.deadline(), Some(ms(t0, 230)));
        assert_eq!(t.on_tick(ms(t0, 100)), None, "too early");
        // Auto-repeat while held changes nothing.
        assert_eq!(t.on_key(Key::Win, true, ms(t0, 150)), None);
        assert_eq!(t.on_tick(ms(t0, 230)), Some(Action::Press));
        assert_eq!(t.deadline(), None);
        assert_eq!(t.on_key(Key::Win, true, ms(t0, 400)), None, "repeat");
        assert_eq!(
            t.on_key(Key::Ctrl, false, ms(t0, 3000)),
            Some(Action::Release)
        );
        assert_eq!(t.on_key(Key::Win, false, ms(t0, 3010)), None);
    }

    #[test]
    fn either_order_and_either_release_work() {
        let t0 = Instant::now();
        let mut t = ModifierTrigger::default();
        t.on_key(Key::Win, true, t0);
        t.on_key(Key::Ctrl, true, ms(t0, 10));
        assert_eq!(t.on_tick(ms(t0, 210)), Some(Action::Press));
        assert_eq!(
            t.on_key(Key::Win, false, ms(t0, 900)),
            Some(Action::Release)
        );
        assert_eq!(t.on_key(Key::Ctrl, false, ms(t0, 950)), None);
    }

    #[test]
    fn quick_windows_shortcut_never_starts() {
        // Ctrl+Win+D within the delay: a new virtual desktop, no pill.
        let t0 = Instant::now();
        let mut t = ModifierTrigger::default();
        t.on_key(Key::Ctrl, true, t0);
        t.on_key(Key::Win, true, ms(t0, 20));
        assert_eq!(t.on_key(Key::Other, true, ms(t0, 80)), None);
        assert_eq!(t.deadline(), None);
        assert_eq!(t.on_tick(ms(t0, 500)), None);
        // Still held after the shortcut: no late start either.
        t.on_key(Key::Other, false, ms(t0, 120));
        assert_eq!(t.on_tick(ms(t0, 1000)), None);
    }

    #[test]
    fn another_key_during_dictation_cancels_and_blocks_until_both_released() {
        let t0 = Instant::now();
        let mut t = ModifierTrigger::default();
        t.on_key(Key::Ctrl, true, t0);
        t.on_key(Key::Win, true, t0);
        assert_eq!(t.on_tick(ms(t0, 200)), Some(Action::Press));
        assert_eq!(
            t.on_key(Key::Other, true, ms(t0, 700)),
            Some(Action::Cancel)
        );
        assert_eq!(t.on_key(Key::Other, true, ms(t0, 750)), None, "only once");
        // Releasing one key and pressing it again while the other is held
        // does not restart: the chord is tainted until both are up.
        assert_eq!(t.on_key(Key::Win, false, ms(t0, 800)), None);
        t.on_key(Key::Win, true, ms(t0, 850));
        assert_eq!(t.deadline(), None);
        t.on_key(Key::Win, false, ms(t0, 900));
        t.on_key(Key::Ctrl, false, ms(t0, 950));
        // A clean chord afterwards works again.
        t.on_key(Key::Ctrl, true, ms(t0, 2000));
        t.on_key(Key::Win, true, ms(t0, 2000));
        assert_eq!(t.on_tick(ms(t0, 2200)), Some(Action::Press));
    }

    #[test]
    fn ordinary_shortcuts_and_lone_keys_do_nothing() {
        let t0 = Instant::now();
        let mut t = ModifierTrigger::default();
        // Ctrl+C, then Win pressed while Ctrl is still held.
        t.on_key(Key::Ctrl, true, t0);
        t.on_key(Key::Other, true, ms(t0, 10));
        t.on_key(Key::Other, false, ms(t0, 20));
        t.on_key(Key::Win, true, ms(t0, 30));
        assert_eq!(t.on_tick(ms(t0, 1000)), None);
        t.on_key(Key::Win, false, ms(t0, 1100));
        t.on_key(Key::Ctrl, false, ms(t0, 1100));
        // Win alone (Start menu) and plain typing.
        t.on_key(Key::Win, true, ms(t0, 2000));
        assert_eq!(t.on_tick(ms(t0, 3000)), None);
        assert_eq!(t.on_key(Key::Win, false, ms(t0, 3100)), None);
        assert_eq!(t.on_key(Key::Other, true, ms(t0, 3200)), None);
    }

    #[test]
    fn released_before_the_delay_does_nothing() {
        let t0 = Instant::now();
        let mut t = ModifierTrigger::default();
        t.on_key(Key::Ctrl, true, t0);
        t.on_key(Key::Win, true, t0);
        assert_eq!(t.on_key(Key::Win, false, ms(t0, 100)), None);
        assert_eq!(t.on_tick(ms(t0, 300)), None);
    }

    #[test]
    fn virtual_keys_are_classified() {
        for vk in [0x11, 0xA2, 0xA3] {
            assert_eq!(classify(vk), Key::Ctrl);
        }
        for vk in [0x5B, 0x5C] {
            assert_eq!(classify(vk), Key::Win);
        }
        for vk in [0x10, 0x12, 0x44, 0xE8, 0x20] {
            assert_eq!(classify(vk), Key::Other);
        }
    }
}
