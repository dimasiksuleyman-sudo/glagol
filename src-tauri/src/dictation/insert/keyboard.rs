//! Wait for the user's shortcut to clear before sending the Windows paste chord.
use std::time::Duration;

const MAX_WAIT: Duration = Duration::from_millis(500);
const QUIET_PERIOD: Duration = Duration::from_millis(20);

#[derive(Debug, PartialEq, Eq)]
enum ReleaseState {
    Waiting,
    Ready,
    TimedOut,
}

#[derive(Default)]
struct ReleaseWait {
    clear_since: Option<Duration>,
}

impl ReleaseWait {
    fn sample(&mut self, elapsed: Duration, any_down: bool) -> ReleaseState {
        if elapsed >= MAX_WAIT {
            return ReleaseState::TimedOut;
        }
        if any_down {
            self.clear_since = None;
        } else if elapsed - *self.clear_since.get_or_insert(elapsed) >= QUIET_PERIOD {
            return ReleaseState::Ready;
        }
        ReleaseState::Waiting
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PasteKey {
    Control,
    V,
}

const PASTE_CHORD: [(PasteKey, bool); 4] = [
    (PasteKey::Control, false),
    (PasteKey::V, false),
    (PasteKey::V, true),
    (PasteKey::Control, true),
];

fn cleanup_start(sent: usize) -> usize {
    match sent {
        2 => 2,     // V and Ctrl were pressed.
        1 | 3 => 3, // Only Ctrl could be left down.
        _ => PASTE_CHORD.len(),
    }
}

#[cfg(windows)]
mod imp {
    use super::{cleanup_start, PasteKey, ReleaseState, ReleaseWait, MAX_WAIT, PASTE_CHORD};
    use crate::dictation::insert::PasteOrigin;
    use std::time::{Duration, Instant};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        VK_V,
    };

    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    #[derive(Clone, Copy, Debug)]
    struct Modifiers {
        ctrl: bool,
        shift: bool,
        alt: bool,
        win: bool,
        start_mask: bool,
    }

    impl Modifiers {
        fn read() -> Self {
            let down = |key: VIRTUAL_KEY| {
                // SAFETY: GetAsyncKeyState accepts a virtual-key identifier. Only
                // the high bit (currently down) is used, never recent keypresses.
                (unsafe { GetAsyncKeyState(i32::from(key.0)) }) < 0
            };
            Self {
                ctrl: down(VK_CONTROL),
                shift: down(VK_SHIFT),
                alt: down(VK_MENU),
                win: down(VK_LWIN) || down(VK_RWIN),
                start_mask: down(VIRTUAL_KEY(
                    crate::dictation::modifier_trigger::MENU_MASK_VK,
                )),
            }
        }

        fn any_down(self) -> bool {
            self.ctrl || self.shift || self.alt || self.win || self.start_mask
        }
    }

    pub(in crate::dictation) fn prepare(origin: PasteOrigin) -> Result<PasteOrigin, String> {
        report_target(origin.validate())?;
        let start = Instant::now();
        let initial = Modifiers::read();
        let mut modifiers = initial;
        let mut wait = ReleaseWait::default();
        let (elapsed, result) = loop {
            let elapsed = start.elapsed();
            if let Err(error) = report_target(origin.ensure_current()) {
                break (elapsed, Err(error));
            }
            match wait.sample(elapsed, modifiers.any_down()) {
                ReleaseState::Ready => break (elapsed, Ok(origin)),
                ReleaseState::TimedOut => {
                    break (elapsed, Err("modifiers did not clear before paste".into()));
                }
                ReleaseState::Waiting => {
                    std::thread::sleep(POLL_INTERVAL.min(MAX_WAIT - elapsed));
                    modifiers = Modifiers::read();
                }
            }
        };
        tracing::debug!(
            target: "glagol_lib::paste_diagnostics",
            ?initial,
            final_modifiers = ?modifiers,
            elapsed_ms = elapsed.as_millis() as u64,
            "paste modifier wait"
        );
        result
    }

    fn report_target(result: Result<(), String>) -> Result<(), String> {
        if let Err(reason) = &result {
            tracing::debug!(target: "glagol_lib::paste_diagnostics", %reason, "paste target rejected");
        }
        result
    }

    pub(in crate::dictation) fn paste(origin: PasteOrigin) -> Result<(), String> {
        // Clipboard retries may have taken time after the wait. Recheck the
        // modifiers and foreground window immediately before dispatch.
        report_target(origin.validate())?;
        let modifiers = Modifiers::read();
        tracing::debug!(
            target: "glagol_lib::paste_diagnostics",
            ?modifiers,
            "paste modifier state before dispatch"
        );
        if modifiers.any_down() {
            return Err("modifiers changed before paste".into());
        }

        let inputs = PASTE_CHORD.map(|(key, up)| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: match key {
                        PasteKey::Control => VK_LCONTROL,
                        PasteKey::V => VK_V,
                    },
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
        report_target(origin.ensure_current())?;
        // SAFETY: a valid array of keyboard INPUT records with the documented
        // size of one INPUT. SendInput enqueues a successful batch in order.
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) } as usize;
        if sent == inputs.len() {
            return Ok(());
        }
        let cleanup = &inputs[cleanup_start(sent)..];
        if !cleanup.is_empty() {
            // SAFETY: a suffix containing only key-up records from the same
            // valid array. Best effort to unstick keys from a partial send.
            unsafe { SendInput(cleanup, std::mem::size_of::<INPUT>() as i32) };
        }
        Err("Windows did not accept the full paste chord".into())
    }
}

#[cfg(windows)]
pub(in crate::dictation) use imp::{paste, prepare};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_wait_requires_a_quiet_period_and_restarts_when_pressed_again() {
        let mut wait = ReleaseWait::default();
        let ms = Duration::from_millis;
        assert_eq!(wait.sample(ms(0), true), ReleaseState::Waiting);
        assert_eq!(wait.sample(ms(50), false), ReleaseState::Waiting);
        assert_eq!(wait.sample(ms(60), true), ReleaseState::Waiting);
        assert_eq!(wait.sample(ms(70), false), ReleaseState::Waiting);
        assert_eq!(wait.sample(ms(89), false), ReleaseState::Waiting);
        assert_eq!(wait.sample(ms(90), false), ReleaseState::Ready);
    }

    #[test]
    fn release_wait_stops_at_the_deadline_if_the_shortcut_has_not_cleared() {
        let mut wait = ReleaseWait::default();
        assert_eq!(
            wait.sample(MAX_WAIT - Duration::from_millis(10), true),
            ReleaseState::Waiting
        );
        assert_eq!(wait.sample(MAX_WAIT, true), ReleaseState::TimedOut);
        assert_eq!(wait.sample(MAX_WAIT, false), ReleaseState::TimedOut);
    }

    #[test]
    fn paste_batch_contains_only_ctrl_and_v_and_cleanup_only_releases_sent_keys() {
        use PasteKey::{Control, V};
        assert_eq!(
            PASTE_CHORD,
            [(Control, false), (V, false), (V, true), (Control, true)]
        );
        assert!(PASTE_CHORD[cleanup_start(0)..].is_empty());
        assert_eq!(&PASTE_CHORD[cleanup_start(1)..], &[(Control, true)]);
        assert_eq!(
            &PASTE_CHORD[cleanup_start(2)..],
            &[(V, true), (Control, true)]
        );
        assert_eq!(&PASTE_CHORD[cleanup_start(3)..], &[(Control, true)]);
        assert!(PASTE_CHORD[cleanup_start(4)..].is_empty());
    }
}
