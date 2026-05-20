//! SaluteSpeech monthly usage counter — Settings page surface.
//!
//! The free tier on the personal SaluteSpeech plan is **200 000
//! characters of synthesis per calendar month**. Glagol counts user
//! synthesis at command-layer (`commands::synthesize`) and stores the
//! running total in `api_usage` (one row per `YYYY-MM`, local
//! timezone). This module exposes the read side: a single Tauri
//! command that reports the current month's usage to the Settings UI.
//!
//! The counter is **advisory**, not enforcement. We never reject a
//! synthesis call because the user hit 200 000 chars — they may be on
//! a paid tier, may have re-upped quota out-of-band, etc. Over-quota
//! values render as `100.0%` filled with the raw `chars_used` shown
//! verbatim so the user sees the real number.

use rusqlite::{Connection, Error as RusqliteError};
use serde::Serialize;

use crate::db::repository;
use crate::state::AppState;

/// SaluteSpeech personal-tier free quota: 200 000 characters of
/// synthesis per calendar month. This is the **denominator** of the
/// Settings progress bar; it is informational only and not enforced
/// on the synthesis path.
pub const SALUTE_FREE_TIER_CHARS_PER_MONTH: u64 = 200_000;

/// Snapshot of the current month's SaluteSpeech consumption.
///
/// Returned by [`get_current_month_usage`] to the frontend; mirrored
/// 1:1 by the `UsageInfo` interface in `src/lib/tauri.ts`. `month` is
/// the `YYYY-MM` key — the frontend formats it for display via the
/// shared Russian month-name helper (also exported by this module so
/// the backend can keep one source of truth for genitive forms).
#[derive(Debug, Clone, Serialize)]
pub struct UsageInfo {
    pub month: String,
    pub chars_used: u64,
    pub chars_limit: u64,
    pub percent_used: f32,
}

/// Read the current month's usage from `conn` and assemble the
/// [`UsageInfo`] the Settings page renders. Pure function — no Tauri
/// state, no clock side-effects beyond a single `Local::now()` to
/// resolve the calendar month.
///
/// A missing `api_usage` row surfaces as `chars_used = 0` (the
/// zero-state, e.g. first launch after a new month rolls over).
pub fn get_current_month_usage_impl(conn: &Connection) -> Result<UsageInfo, RusqliteError> {
    let month = chrono::Local::now().format("%Y-%m").to_string();
    let chars_used: u64 = match repository::get_usage_for_month(conn, &month)? {
        Some(row) => row.chars_used.max(0) as u64,
        None => 0,
    };
    let percent_used = if SALUTE_FREE_TIER_CHARS_PER_MONTH == 0 {
        0.0
    } else {
        (chars_used as f32 / SALUTE_FREE_TIER_CHARS_PER_MONTH as f32 * 100.0).min(100.0)
    };
    Ok(UsageInfo {
        month,
        chars_used,
        chars_limit: SALUTE_FREE_TIER_CHARS_PER_MONTH,
        percent_used,
    })
}

#[tauri::command]
pub async fn get_current_month_usage(
    state: tauri::State<'_, AppState>,
) -> Result<UsageInfo, String> {
    let conn = state
        .db
        .lock()
        .map_err(|e| format!("Не удалось получить блокировку базы данных: {e}"))?;
    get_current_month_usage_impl(&conn).map_err(|e| format!("Не удалось прочитать счётчик: {e}"))
}

/// Russian month name in the form used after the preposition «в»
/// ("в мае", "в июне"). Used by the Settings UI to render
/// "Использовано в мае: 12 345 / 200 000 символов".
///
/// (Name kept from the Sprint 5d kickoff for cross-document traceability;
/// strictly speaking the returned forms are prepositional, not genitive.)
///
/// Panics on inputs outside `1..=12`. `chrono::Local::now().month()`
/// is documented to return `1..=12` so the panic is defensive against
/// future refactors that might compute the month differently, not a
/// real runtime possibility today.
pub fn russian_month_genitive(month: u32) -> &'static str {
    match month {
        1 => "январе",
        2 => "феврале",
        3 => "марте",
        4 => "апреле",
        5 => "мае",
        6 => "июне",
        7 => "июле",
        8 => "августе",
        9 => "сентябре",
        10 => "октябре",
        11 => "ноябре",
        12 => "декабре",
        other => unreachable!("month out of range 1..=12: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::repository;
    use crate::db::test_connection;

    /// Seed `conn` so the *current* month has `chars` used. Tests
    /// resolve the month the same way the impl does (`Local::now()`)
    /// so seeding stays in lock-step regardless of when the suite
    /// runs.
    fn seed_current_month(conn: &Connection, chars: i64) {
        let month = chrono::Local::now().format("%Y-%m").to_string();
        repository::record_usage(conn, &month, chars, 1_700_000_000_000).expect("seed");
    }

    #[test]
    fn get_current_month_usage_impl_computes_percent_correctly() {
        // Table-driven: each row seeds a fresh in-memory DB with the
        // given `chars_used` (or none for the 0 case) and asserts the
        // computed percentage. The 250k row guards the cap at 100.0;
        // an over-quota user must still see the bar pinned to full,
        // never a 125% overflow.
        let cases: &[(Option<i64>, u64, f32)] = &[
            (None, 0, 0.0),
            (Some(100_000), 100_000, 50.0),
            (Some(150_000), 150_000, 75.0),
            (Some(200_000), 200_000, 100.0),
            (Some(250_000), 250_000, 100.0),
        ];

        for (seed, expected_chars, expected_percent) in cases {
            let conn = test_connection();
            if let Some(chars) = seed {
                seed_current_month(&conn, *chars);
            }
            let info = get_current_month_usage_impl(&conn).expect("impl ok");
            assert_eq!(
                info.chars_used, *expected_chars,
                "chars_used mismatch for seed {seed:?}"
            );
            assert!(
                (info.percent_used - expected_percent).abs() < f32::EPSILON,
                "percent mismatch for seed {seed:?}: got {}, expected {expected_percent}",
                info.percent_used,
            );
            assert_eq!(info.chars_limit, SALUTE_FREE_TIER_CHARS_PER_MONTH);
        }
    }

    #[test]
    fn russian_month_genitive_all_months() {
        let cases = [
            (1, "январе"),
            (2, "феврале"),
            (3, "марте"),
            (4, "апреле"),
            (5, "мае"),
            (6, "июне"),
            (7, "июле"),
            (8, "августе"),
            (9, "сентябре"),
            (10, "октябре"),
            (11, "ноябре"),
            (12, "декабре"),
        ];
        for (month, expected) in cases {
            assert_eq!(
                russian_month_genitive(month),
                expected,
                "month {month} should map to {expected}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "month out of range")]
    fn russian_month_genitive_panics_on_invalid_input() {
        // Defensive contract: any future refactor that passes 0 or
        // 13+ must trip the unreachable! at compile-test time, not
        // ship a silent fallback to "январе".
        let _ = russian_month_genitive(0);
    }
}
