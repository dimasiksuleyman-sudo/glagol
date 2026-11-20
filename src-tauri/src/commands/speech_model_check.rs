//! Catalog-only checks for saved remote dictation models.
use super::speech::{active_mode, read_profile, remote_provider, Mode};
use crate::{db::repository, preferences::message, state::AppState, stt::SttError};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const STATUS_EVENT: &str = "speech-model-status-changed";

/// Result of checking a saved model against the provider's catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Available,
    Missing,
    Unknown,
    Failed,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct CheckRecord {
    last_attempt: Option<i64>,
    pending: Option<String>,
    last_result: Option<Availability>,
    checked_at: Option<i64>,
    missing: bool,
}

impl CheckRecord {
    fn due(&self, now: i64) -> bool {
        self.last_attempt
            .is_none_or(|last| now.saturating_sub(last) >= WEEK_MS)
    }

    fn begin(&mut self, now: i64) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        self.pending = Some(token.clone());
        self.last_attempt = Some(now);
        token
    }

    fn finish(&mut self, token: &str, result: Availability, at: i64) -> bool {
        if self.pending.as_deref() != Some(token) {
            return false;
        }
        self.pending = None;
        self.last_result = Some(result);
        self.checked_at = Some(at);
        match result {
            Availability::Available => self.missing = false,
            Availability::Missing => self.missing = true,
            Availability::Unknown | Availability::Failed => {}
        }
        true
    }
}

/// Whether the settings screen can check a saved model and show a warning.
#[derive(Default, Serialize)]
pub struct SavedModelStatus {
    pub configured: bool,
    pub missing: bool,
    pub last_result: Option<Availability>,
    pub checked_at: Option<i64>,
}

fn read_record(conn: &Connection, mode: Mode) -> Result<CheckRecord, String> {
    Ok(
        repository::get_setting(conn, &format!("{}_model_check", mode.prefix()))
            .map_err(|e| e.to_string())?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default(),
    )
}

fn write_record(conn: &Connection, mode: Mode, record: &CheckRecord) -> Result<(), String> {
    let json = serde_json::to_string(record).map_err(|e| e.to_string())?;
    repository::set_setting(
        conn,
        &format!("{}_model_check", mode.prefix()),
        &json,
        chrono::Utc::now().timestamp_millis(),
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn configured(conn: &Connection, mode: Mode) -> Result<bool, String> {
    if mode == Mode::Local {
        return Ok(false);
    }
    Ok(
        repository::get_setting(conn, &format!("{}_base_url", mode.prefix()))
            .map_err(|e| e.to_string())?
            .is_some_and(|url| !url.trim().is_empty()),
    )
}

pub(super) fn saved_status(conn: &Connection, mode: Mode) -> Result<SavedModelStatus, String> {
    if !configured(conn, mode)? {
        return Ok(SavedModelStatus::default());
    }
    let record = read_record(conn, mode)?;
    Ok(SavedModelStatus {
        configured: true,
        missing: record.missing,
        last_result: record.last_result,
        checked_at: record.checked_at,
    })
}

pub(super) fn invalidate_warning(conn: &Connection, mode: Mode) -> Result<(), String> {
    let mut record = read_record(conn, mode)?;
    if record.last_result.is_some() || record.pending.is_some() {
        record.missing = false;
        record.pending = None;
        record.last_result = None;
        record.checked_at = None;
        write_record(conn, mode, &record)?;
    }
    Ok(())
}

fn report(outcome: Result<Option<bool>, SttError>) -> Result<Availability, String> {
    match outcome {
        Ok(Some(true)) => Ok(Availability::Available),
        Ok(Some(false)) => Ok(Availability::Missing),
        Ok(None) => Ok(Availability::Unknown),
        Err(SttError::Api(reason))
            if ["HTTP 404:", "HTTP 405:", "HTTP 501:"]
                .iter()
                .any(|prefix| reason.starts_with(prefix)) =>
        {
            Ok(Availability::Unknown)
        }
        Err(SttError::Auth) => Err(message(
            "The provider did not accept the API key. Check the key.",
            "Провайдер не принял ключ API. Проверьте ключ.",
        )),
        Err(SttError::Network(_)) => Err(message(
            "Could not reach the provider. Check its address and the connection.",
            "Не удалось связаться с провайдером. Проверьте адрес и подключение.",
        )),
        Err(_) => Err(message(
            "Could not check the model list. Try again later.",
            "Не удалось проверить список моделей. Попробуйте позже.",
        )),
    }
}

async fn check(app: &AppHandle, manual_mode: Option<Mode>) -> Result<Option<Availability>, String> {
    let state = app.state::<AppState>();
    let (mode, token, provider) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let mode = match manual_mode {
            Some(mode) => mode,
            None => active_mode(&conn)?,
        };
        if !configured(&conn, mode)? {
            return Ok(None);
        }
        let now = chrono::Utc::now().timestamp_millis();
        let mut record = read_record(&conn, mode)?;
        if manual_mode.is_none() && !record.due(now) {
            return Ok(None);
        }
        let profile = read_profile(&conn, Some(mode))?;
        let provider = remote_provider(&conn, &profile);
        // Reserve before I/O. Even a failed network request spends this
        // startup interval, so repeated launches cannot hammer the provider.
        let token = record.begin(now);
        write_record(&conn, mode, &record)?;
        (mode, token, provider)
    };

    let outcome = match provider {
        Ok(provider) => report(provider.model_is_advertised().await),
        Err(error) => Err(error),
    };
    let result = outcome.as_ref().copied().unwrap_or(Availability::Failed);
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let mut record = read_record(&conn, mode)?;
        if !record.finish(&token, result, chrono::Utc::now().timestamp_millis()) {
            return Err(message(
                "The saved profile changed during the check. Try again.",
                "Сохранённый профиль изменился во время проверки. Попробуйте ещё раз.",
            ));
        }
        write_record(&conn, mode, &record)?;
    }
    let _ = app.emit(STATUS_EVENT, mode);
    outcome.map(Some)
}

/// Check only the saved model of the profile selected in Dictation settings.
#[tauri::command]
pub async fn check_saved_speech_model(app: AppHandle, mode: Mode) -> Result<Availability, String> {
    check(&app, Some(mode))
        .await
        .map_err(crate::i18n::error)?
        .ok_or_else(|| {
            message(
                "Save a remote dictation profile before checking its model.",
                "Сохраните удалённый профиль диктовки перед проверкой модели.",
            )
        })
}

/// Do one silent check of the active saved remote profile when due.
pub(crate) fn start_at_launch(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = check(&app, None).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_cooldown_survives_a_failed_request_and_app_restart() {
        let conn = crate::db::test_connection();
        let mut record = CheckRecord::default();
        let now = 1_000_000;
        assert!(record.due(now));
        let token = record.begin(now);
        assert!(record.finish(&token, Availability::Failed, now + 10));
        write_record(&conn, Mode::Server, &record).unwrap();

        let restored = read_record(&conn, Mode::Server).unwrap();
        assert_eq!(restored.last_result, Some(Availability::Failed));
        assert_eq!(restored.checked_at, Some(now + 10));
        assert!(!restored.due(now + WEEK_MS - 1));
        assert!(restored.due(now + WEEK_MS));
        assert!(read_record(&conn, Mode::Cloud).unwrap().due(now));
    }

    #[test]
    fn missing_warning_is_retained_until_the_catalog_confirms_availability() {
        let mut record = CheckRecord::default();
        let missing = record.begin(1);
        assert!(record.finish(&missing, Availability::Missing, 1));
        assert!(record.missing);
        let unknown = record.begin(2);
        assert!(record.finish(&unknown, Availability::Unknown, 2));
        assert!(record.missing);
        let available = record.begin(3);
        assert!(record.finish(&available, Availability::Available, 3));
        assert!(!record.missing);
    }

    #[test]
    fn outdated_attempts_cannot_warn_about_an_edited_profile() {
        let conn = crate::db::test_connection();
        let mut record = CheckRecord::default();
        let first = record.begin(1);
        let second = record.begin(2);
        assert!(!record.finish(&first, Availability::Missing, 2));
        write_record(&conn, Mode::Server, &record).unwrap();
        invalidate_warning(&conn, Mode::Server).unwrap();
        let mut edited = read_record(&conn, Mode::Server).unwrap();
        assert!(!edited.finish(&second, Availability::Missing, 3));
        assert!(!edited.missing);
        assert!(!edited.due(3));
    }

    #[test]
    fn a_provider_without_a_catalog_is_inconclusive() {
        assert_eq!(
            report(Err(SttError::Api("HTTP 404: not found".into()))),
            Ok(Availability::Unknown)
        );
        assert_eq!(report(Ok(None)), Ok(Availability::Unknown));
        assert!(report(Err(SttError::Auth)).is_err());
    }

    #[test]
    fn only_saved_remote_profiles_can_be_checked() {
        let conn = crate::db::test_connection();
        for mode in [Mode::Cloud, Mode::Server, Mode::Local] {
            assert!(!saved_status(&conn, mode).unwrap().configured);
        }
        repository::set_setting(&conn, "stt_server_base_url", "http://localhost/v1", 1).unwrap();
        assert!(saved_status(&conn, Mode::Server).unwrap().configured);
        assert!(!saved_status(&conn, Mode::Cloud).unwrap().configured);
        assert!(!saved_status(&conn, Mode::Local).unwrap().configured);
    }
}
