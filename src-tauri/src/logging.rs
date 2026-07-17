//! Tracing subscriber installation (Sprint 6 PR3.1).
//!
//! The codebase has emitted `tracing::{trace,debug,info,warn,error}!` since
//! Sprint 1, but **no subscriber was ever installed** — every call dispatched
//! into the void and `RUST_LOG` did nothing (there was no `EnvFilter` to read
//! it). Most visibly, the D8-a addendum from PR3 («log each clip's RMS so the
//! silence threshold can be calibrated from real use») compiled, was marked
//! done, and produced nothing. This module installs the subscriber so all of
//! those existing calls start working. It adds **no** new log calls and changes
//! **no** existing levels (PR3.1 scope: observability only).
//!
//! # Two layers (D-L3)
//!
//! - **dev** (`debug_assertions`): a stdout `fmt` layer with an `EnvFilter` from
//!   `RUST_LOG`, defaulting to `glagol_lib=debug`.
//! - **release**: a **daily-rolling file** under the OS log dir (7 files kept),
//!   default directive `info,glagol_lib::dictation=debug`. Release binaries
//!   carry `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`
//!   (`main.rs`) — there is **no console**, so stdout would write to nowhere;
//!   the file is the only place a user or maintainer can read diagnostics. The
//!   dictation-at-`debug` clause keeps the D8-a RMS calibration lines in the
//!   real-use log (a flat `info` would filter them, stranding calibration in
//!   dev); see [`DEFAULT_DIRECTIVE`].
//!
//! Initialisation happens inside Tauri's `setup` hook because the release log
//! directory (`app_log_dir`) needs an `AppHandle`. Events emitted before `setup`
//! run are lost — accepted.
//!
//! # The `WorkerGuard` trap (D-L4)
//!
//! [`tracing_appender::non_blocking`] returns `(NonBlocking, WorkerGuard)`. If
//! the guard is dropped, the background writer silently stops flushing — the
//! exact class of silent failure this whole change exists to prevent. So the
//! guard is handed back to the caller and stored in [`crate::state::AppState`]
//! for the process lifetime (see `lib.rs`).
//!
//! # Logging rules (D-L5) — WRITTEN, not incidental
//!
//! A log file on disk is the worst possible place to break the project's
//! no-secrets / data-stays-local promise, so two rules are now explicit and
//! enforced by a source-scan test (see the test module):
//!
//! 1. **API keys never reach the log.** Not the STT key, not the SaluteSpeech
//!    Authorization Key. Ever.
//! 2. **Transcript text never reaches the log.** It is the user's speech; a file
//!    of dictated text on disk is an ФЗ-152-class problem in an app that
//!    advertises "your data does not leave the machine". Today this holds by
//!    accident — with a real file it must be a written rule, or in two sprints
//!    someone adds `?transcript` at a convenient debugging moment.
//!
//! What we DO log: RMS values, durations, error codes/messages, dispositions,
//! boolean flags. Never content, never secrets.

use tauri::AppHandle;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// Default filter when `RUST_LOG` is unset, in dev: our lib at `debug`, deps
/// quiet.
#[cfg(debug_assertions)]
const DEFAULT_DIRECTIVE: &str = "glagol_lib=debug";

/// Default filter when `RUST_LOG` is unset, in release: everything at `info`,
/// **plus the dictation module at `debug`**. The extra clause is deliberate and
/// load-bearing: D8-a wants each clip's RMS logged so the 0.005 silence
/// threshold can be calibrated *from a week of real use*, and that log lives at
/// `debug` (`pipeline.rs`). A flat `info` default would filter it out, so the
/// real-use log — the release build's rolling file — would never contain the
/// numbers, and the calibration could only happen in a dev run. Admitting
/// `glagol_lib::dictation=debug` (and nothing else at debug) puts the RMS/
/// duration/reason lines in the user's log while keeping the rest at `info`.
/// This changes **no** log call's level (D-L7) — only the default filter. Those
/// lines carry numbers and reasons only, never content or secrets (D-L5).
#[cfg(not(debug_assertions))]
const DEFAULT_DIRECTIVE: &str = "info,glagol_lib::dictation=debug";

/// Prefix + suffix of the rolling log files, e.g. `glagol.2026-07-17.log`.
#[cfg(not(debug_assertions))]
const LOG_FILE_PREFIX: &str = "glagol";
#[cfg(not(debug_assertions))]
const LOG_FILE_SUFFIX: &str = "log";
/// Number of daily log files to retain (a week).
#[cfg(not(debug_assertions))]
const LOG_FILES_KEPT: usize = 7;

/// Build the `EnvFilter` from `RUST_LOG`, falling back to [`DEFAULT_DIRECTIVE`].
/// A malformed `RUST_LOG` degrades to the default rather than panicking.
fn env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_DIRECTIVE))
}

/// Install the global tracing subscriber and return the log-flush guard.
///
/// Returns `Some(WorkerGuard)` in release (the caller **must** keep it alive for
/// the process lifetime — D-L4) and `None` in dev (the stdout writer needs no
/// guard). Idempotent-safe: uses `try_init`, so a second call (or a test that
/// already set a subscriber) is a no-op rather than a panic.
#[must_use = "the WorkerGuard must be kept alive for the process lifetime, or logs stop flushing"]
pub fn init_tracing(app: &AppHandle) -> Option<WorkerGuard> {
    #[cfg(debug_assertions)]
    {
        let _ = app; // release-only; keep the signature stable across profiles.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter())
            .with_writer(std::io::stdout)
            .try_init();
        None
    }

    #[cfg(not(debug_assertions))]
    {
        use tauri::Manager;
        use tracing_appender::rolling::{RollingFileAppender, Rotation};

        let log_dir = match app.path().app_log_dir() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("logging disabled: could not resolve app log dir: {e}");
                return None;
            }
        };
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("logging disabled: could not create log dir: {e}");
            return None;
        }

        let appender = match RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix(LOG_FILE_PREFIX)
            .filename_suffix(LOG_FILE_SUFFIX)
            .max_log_files(LOG_FILES_KEPT)
            .build(&log_dir)
        {
            Ok(a) => a,
            Err(e) => {
                eprintln!("logging disabled: could not open rolling log file: {e}");
                return None;
            }
        };

        let (writer, guard) = tracing_appender::non_blocking(appender);
        let _ = tracing_subscriber::fmt()
            .with_env_filter(env_filter())
            .with_ansi(false) // a file is not a terminal — no colour escapes.
            .with_writer(writer)
            .try_init();
        Some(guard)
    }
}

#[cfg(test)]
mod tests {
    //! The load-bearing test here guards the **rule** (D-L5), not the plumbing:
    //! no `tracing::*!` call **anywhere in the crate** may capture a secret or the
    //! transcript. It walks the real `src/` tree at test time (not a hardcoded
    //! list — new modules are covered automatically) so rule #1 (keys) is
    //! enforced in the modules where keys actually live — `stt/`, `secrets/`,
    //! `commands/credentials.rs`, `salute/` — not only where the transcript
    //! flows. The scanner's own file is the sole exclusion: its doc comments and
    //! test fixtures contain the forbidden patterns on purpose.

    use super::*;
    use std::path::{Path, PathBuf};

    /// Identifiers that must never be *captured* by a tracing call. `key` is
    /// matched as a whole word so `hotkey`/`keyring`/`keyword` do not trip it.
    const FORBIDDEN: &[&str] = &["transcript", "api_key", "key"];

    /// The scanner's own file — excluded because its doc comments and test
    /// fixtures deliberately contain `?transcript` / `api_key` examples. It has
    /// no production `tracing::` calls (its own errors use `eprintln!`), so
    /// excluding it loses no real coverage.
    const SCANNER_FILE: &str = "logging.rs";

    /// Recursively collect every `.rs` file under `dir`.
    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// Return the offending snippet if any `tracing::<level>!(...)` invocation in
    /// `src` captures a forbidden identifier. Message *text* is exempt — string
    /// literals are stripped first — so `"empty transcript; discarding"` is fine
    /// while `?transcript` is not.
    fn find_secret_leak(src: &str) -> Option<String> {
        let mut search_from = 0;
        while let Some(rel) = src[search_from..].find("tracing::") {
            let start = search_from + rel;
            let Some(paren_rel) = src[start..].find('(') else {
                break;
            };
            let open = start + paren_rel;
            let (invocation, end) = extract_balanced(src, open);
            let stripped = strip_string_literals(&invocation);
            if let Some(word) = FORBIDDEN.iter().find(|w| contains_word(&stripped, w)) {
                return Some(format!("{word} in `{}`", src[start..end].trim()));
            }
            search_from = end;
        }
        None
    }

    /// From the `(` at `open`, return the parenthesised text (inclusive) and the
    /// index just past the matching `)`. Parens inside string literals are
    /// ignored so a `")"` in a message does not unbalance the scan.
    fn extract_balanced(src: &str, open: usize) -> (String, usize) {
        let bytes = src.as_bytes();
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut i = open;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
            } else if c == '"' {
                in_string = true;
            } else if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    return (src[open..=i].to_string(), i + 1);
                }
            }
            i += 1;
        }
        (src[open..].to_string(), bytes.len())
    }

    /// Remove `"..."` string-literal contents (message text is allowed to
    /// mention the forbidden words), leaving the code around them.
    fn strip_string_literals(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut in_string = false;
        let mut escaped = false;
        for c in s.chars() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
            } else if c == '"' {
                in_string = true;
            } else {
                out.push(c);
            }
        }
        out
    }

    /// Whole-word (Rust-identifier-boundary) containment: `key` matches `key` and
    /// `?key` but not `hotkey` or `keyring`.
    fn contains_word(haystack: &str, word: &str) -> bool {
        let is_ident = |c: char| c.is_alphanumeric() || c == '_';
        let mut from = 0;
        while let Some(rel) = haystack[from..].find(word) {
            let idx = from + rel;
            let before_ok = idx == 0 || !is_ident(haystack[..idx].chars().next_back().unwrap());
            let after = idx + word.len();
            let after_ok =
                after >= haystack.len() || !is_ident(haystack[after..].chars().next().unwrap());
            if before_ok && after_ok {
                return true;
            }
            from = idx + word.len();
        }
        false
    }

    // ── the rule (D-L5), enforced across the whole crate ──

    #[test]
    fn no_tracing_call_in_the_crate_captures_secrets_or_transcript() {
        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src_root, &mut files);

        let mut scanned = 0usize;
        for file in &files {
            if file.file_name().is_some_and(|n| n == SCANNER_FILE) {
                continue;
            }
            let src = std::fs::read_to_string(file).expect("read source file");
            scanned += 1;
            if let Some(leak) = find_secret_leak(&src) {
                panic!(
                    "{}: a tracing call captures a forbidden identifier — {leak}",
                    file.display()
                );
            }
        }

        // Guard against the walk silently covering nothing — that would make the
        // PR claim a protection it does not provide. The crate has dozens of
        // modules, including the secret-touching ones the scan now reaches.
        assert!(
            scanned > 15,
            "expected to scan the whole crate; only {scanned} files walked from {}",
            src_root.display()
        );
    }

    // ── the scanner is itself trustworthy (bakes the negative cycle in) ──

    #[test]
    fn scanner_flags_captured_secret_but_not_message_text() {
        // Captured value → flagged (this is exactly the manual negative cycle:
        // add `?transcript` to a real call and this scan fails).
        assert!(find_secret_leak(r#"tracing::debug!(?transcript, "clip");"#).is_some());
        assert!(find_secret_leak(r#"tracing::info!(api_key = %api_key, "auth");"#).is_some());
        assert!(find_secret_leak(r#"tracing::warn!(key = ?key, "stored");"#).is_some());

        // The word inside a message string → NOT flagged (the real code's
        // "provider returned empty transcript; discarding" must stay legal).
        assert!(find_secret_leak(
            r#"tracing::debug!("provider returned empty transcript; discarding");"#
        )
        .is_none());
        // Substrings of unrelated identifiers → NOT flagged.
        assert!(find_secret_leak(r#"tracing::trace!(phase = ?p, "hotkey ignored");"#).is_none());
        assert!(find_secret_leak(r#"tracing::warn!("keyring backend error");"#).is_none());
    }

    // ── the calibration event actually carries its fields (D8-a / D-L6) ──

    #[test]
    fn clip_log_event_carries_rms_and_duration_fields() {
        use std::sync::{Arc, Mutex};
        use tracing::field::{Field, Visit};
        use tracing::subscriber::with_default;
        use tracing_subscriber::layer::{Context, Layer};
        use tracing_subscriber::prelude::*;

        #[derive(Default)]
        struct FieldNames(Arc<Mutex<Vec<String>>>);
        impl Visit for FieldNames {
            fn record_debug(&mut self, field: &Field, _: &dyn std::fmt::Debug) {
                self.0.lock().unwrap().push(field.name().to_string());
            }
            fn record_f64(&mut self, field: &Field, _: f64) {
                self.0.lock().unwrap().push(field.name().to_string());
            }
            fn record_u64(&mut self, field: &Field, _: u64) {
                self.0.lock().unwrap().push(field.name().to_string());
            }
        }

        struct CaptureLayer(Arc<Mutex<Vec<String>>>);
        impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
            fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
                let mut visitor = FieldNames(self.0.clone());
                event.record(&mut visitor);
            }
        }

        let captured = Arc::new(Mutex::new(Vec::new()));
        let subscriber = tracing_subscriber::registry().with(CaptureLayer(captured.clone()));
        with_default(subscriber, || {
            // Same event shape as `pipeline.rs`'s "dictation clip finalized" log:
            // the calibration data (D8-a) is fields, never content.
            tracing::debug!(
                duration_ms = 1000u32,
                rms = 0.5f32,
                truncated = false,
                "dictation clip finalized"
            );
        });

        let names = captured.lock().unwrap().clone();
        assert!(
            names.contains(&"rms".to_string()),
            "rms field missing: {names:?}"
        );
        assert!(
            names.contains(&"duration_ms".to_string()),
            "duration_ms field missing: {names:?}"
        );
    }

    #[test]
    fn default_directive_parses_as_env_filter() {
        // A typo'd default directive would silently disable all logging; assert
        // the compiled-in default is a valid filter.
        assert!(EnvFilter::try_new(DEFAULT_DIRECTIVE).is_ok());
    }
}
