//! Keep the server computer from sleeping while it serves (optional).
//!
//! Windows keeps a `SetThreadExecutionState` request only for the thread that
//! made it, so a small dedicated thread holds it until the guard is dropped.
//! The display may still turn off; only system sleep is prevented.

/// While alive, the computer does not go to sleep on idle.
pub struct Guard {
    stop: Option<std::sync::mpsc::Sender<()>>,
}

/// Ask Windows not to sleep until the returned guard is dropped. A no-op on
/// other platforms.
pub fn keep_awake() -> Guard {
    let (stop, stopped) = std::sync::mpsc::channel::<()>();
    let spawned = std::thread::Builder::new()
        .name("lan-server-awake".into())
        .spawn(move || {
            #[cfg(windows)]
            {
                use windows::Win32::System::Power::{
                    SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
                };
                // SAFETY: SetThreadExecutionState only records a power request
                // for this thread; it takes flags by value and has no memory
                // effects.
                unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED) };
                let _ = stopped.recv();
                // SAFETY: as above; clears this thread's request.
                unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
            }
            #[cfg(not(windows))]
            let _ = stopped.recv();
        });
    if let Err(e) = spawned {
        tracing::warn!("failed to start keep-awake thread: {e}");
    }
    Guard { stop: Some(stop) }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
