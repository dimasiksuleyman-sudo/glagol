//! Privacy-safe application timing events used by release QA.

use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Instant,
};

/// Process-start clock shared with the main webview.
pub struct StartupMetrics {
    started: Instant,
    ui_reported: AtomicBool,
}

impl StartupMetrics {
    /// Start measuring before Tauri begins initialization.
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            ui_reported: AtomicBool::new(false),
        }
    }

    fn take_ui_ready_ms(&self) -> Option<u128> {
        self.ui_reported
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| self.started.elapsed().as_millis())
    }
}

impl Default for StartupMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Record the first painted main-window frame once per process.
#[tauri::command]
pub fn report_ui_ready(state: tauri::State<'_, StartupMetrics>) {
    if let Some(elapsed_ms) = state.take_ui_ready_ms() {
        tracing::info!(elapsed_ms, "application UI ready");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_ready_is_recorded_once() {
        let metrics = StartupMetrics::new();
        assert!(metrics.take_ui_ready_ms().is_some());
        assert!(metrics.take_ui_ready_ms().is_none());
    }
}
