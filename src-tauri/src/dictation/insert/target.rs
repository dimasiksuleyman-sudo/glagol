//! Foreground window captured before dictation can show its overlay.

#[cfg(any(windows, test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowIdentity {
    handle: usize,
    thread: u32,
    process: u32,
}

#[cfg(any(windows, test))]
impl WindowIdentity {
    fn matches(self, current: Option<Self>) -> bool {
        current == Some(self)
    }
}

/// A session's starting window, identified without retaining an OS handle.
#[derive(Clone, Copy, Default)]
pub struct PasteOrigin(#[cfg(windows)] Option<WindowIdentity>);

#[cfg(any(windows, test))]
fn is_shell_executable(path: &str) -> bool {
    let name = path.rsplit(['\\', '/']).next().unwrap_or_default();
    [
        "SearchHost.exe",
        "SearchApp.exe",
        "SearchUI.exe",
        "StartMenuExperienceHost.exe",
        "ShellExperienceHost.exe",
    ]
    .iter()
    .any(|shell| name.eq_ignore_ascii_case(shell))
}

#[cfg(any(windows, test))]
fn is_shell_class(class: &str) -> bool {
    matches!(
        class,
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Progman" | "WorkerW"
    )
}

#[cfg(windows)]
mod windows_target {
    use super::{is_shell_class, is_shell_executable, PasteOrigin, WindowIdentity};
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetShellWindow, GetWindowThreadProcessId,
    };

    fn foreground() -> Option<(HWND, WindowIdentity)> {
        let mut process = 0;
        // SAFETY: GetForegroundWindow takes no arguments. The PID output points
        // to writable storage for the synchronous GetWindowThreadProcessId call.
        let (window, thread) = unsafe {
            let window = GetForegroundWindow();
            if window.is_invalid() {
                return None;
            }
            (window, GetWindowThreadProcessId(window, Some(&mut process)))
        };
        (thread != 0 && process != 0).then_some((
            window,
            WindowIdentity {
                handle: window.0 as usize,
                thread,
                process,
            },
        ))
    }

    fn shell_window(window: HWND, process: u32) -> Option<bool> {
        let mut class = [0u16; 256];
        // SAFETY: class is a writable buffer and window is a borrowed HWND.
        let class_len = unsafe { GetClassNameW(window, &mut class) } as usize;
        // SAFETY: GetShellWindow takes no arguments. Its HWND is only compared.
        if window == unsafe { GetShellWindow() }
            || is_shell_class(&String::from_utf16_lossy(&class[..class_len]))
        {
            return Some(true);
        }
        if class_len == 0 {
            return None;
        }

        // SAFETY: Only query permission is requested. The returned process
        // handle is owned by this function and closed after the query.
        let handle =
            unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process) }.ok()?;
        let mut path = [0u16; 1024];
        let mut length = path.len() as u32;
        // SAFETY: path and length are valid writable buffers for the duration of
        // the query. No handle is used after CloseHandle.
        let result = unsafe {
            let result = QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_FORMAT(0),
                PWSTR(path.as_mut_ptr()),
                &mut length,
            );
            let _ = CloseHandle(handle);
            result
        };
        result.ok()?;
        Some(is_shell_executable(&String::from_utf16_lossy(
            &path[..length as usize],
        )))
    }

    impl PasteOrigin {
        /// Read the foreground window at the shortcut press.
        pub fn capture() -> Self {
            Self(foreground().map(|(_, identity)| identity))
        }

        pub(in crate::dictation) fn belongs_to_current_process(self) -> bool {
            self.0
                .is_some_and(|origin| origin.process == std::process::id())
        }

        pub(in crate::dictation) fn is_window(self, handle: usize) -> bool {
            self.0.is_some_and(|origin| origin.handle == handle)
        }

        fn check(self) -> Result<(HWND, WindowIdentity), String> {
            let origin = self.0.ok_or("no foreground window at dictation start")?;
            if let Some((window, current)) = foreground() {
                if origin.matches(Some(current)) {
                    return Ok((window, current));
                }
            }
            Err("foreground window changed since dictation started".into())
        }

        pub(in crate::dictation) fn ensure_current(self) -> Result<(), String> {
            self.check().map(|_| ())
        }

        pub(in crate::dictation) fn validate(self) -> Result<(), String> {
            let (window, identity) = self.check()?;
            match shell_window(window, identity.process) {
                Some(false) => Ok(()),
                Some(true) => Err("foreground window is Windows shell".into()),
                None => Err("could not identify foreground window owner".into()),
            }
        }
    }
}

#[cfg(not(windows))]
impl PasteOrigin {
    /// Non-Windows insertion does not use a Win32 foreground window.
    pub fn capture() -> Self {
        Self()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_original_window_and_owner_match() {
        let target = WindowIdentity {
            handle: 1,
            thread: 2,
            process: 3,
        };
        assert!(target.matches(Some(target)));
        assert!(!target.matches(None));
        assert!(!target.matches(Some(WindowIdentity {
            handle: 4,
            ..target
        })));
        assert!(!target.matches(Some(WindowIdentity {
            thread: 4,
            ..target
        })));
        assert!(!target.matches(Some(WindowIdentity {
            process: 4,
            ..target
        })));
    }

    #[test]
    fn search_and_start_are_blocked_without_excluding_file_explorer() {
        assert!(is_shell_executable(r"C:\Windows\SystemApps\SEARCHHOST.EXE"));
        assert!(is_shell_executable("StartMenuExperienceHost.exe"));
        assert!(is_shell_executable("ShellExperienceHost.exe"));
        assert!(is_shell_executable("SearchApp.exe"));
        assert!(is_shell_executable("SearchUI.exe"));
        assert!(is_shell_class("Shell_TrayWnd"));
        assert!(is_shell_class("Shell_SecondaryTrayWnd"));
        assert!(!is_shell_executable("explorer.exe"));
        assert!(!is_shell_class("CabinetWClass"));
        assert!(!is_shell_executable("FakeSearchHost.exe"));
    }
}
