//! Independent interface and speech preferences, migrated without touching keys.
use crate::db::repository::{get_setting, set_setting};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};

static RUSSIAN_UI: AtomicBool = AtomicBool::new(false);

pub fn language() -> Language {
    if RUSSIAN_UI.load(Ordering::Relaxed) {
        Language::Ru
    } else {
        Language::En
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    En,
    Ru,
}
impl Language {
    /// Stable persisted language tag.
    pub fn tag(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Ru => "ru",
        }
    }
    fn parse(value: &str) -> Self {
        if value == "ru" {
            Self::Ru
        } else {
            Self::En
        }
    }
}

/// Select user-facing text at the native boundary. English is the fallback.
pub fn message(english: &str, russian: &str) -> String {
    if RUSSIAN_UI.load(Ordering::Relaxed) {
        russian
    } else {
        english
    }
    .into()
}

#[derive(Serialize)]
pub struct Preferences {
    pub ui_language: Language,
    pub onboarding_stage: String,
    pub upgraded: bool,
    pub stt_language: Language,
    pub tts_language: Language,
    pub tts_voice_en: String,
    pub tts_voice_ru: String,
    pub legacy_voice_migrated: bool,
}

fn read(conn: &Connection, key: &str, default: &str) -> rusqlite::Result<String> {
    Ok(get_setting(conn, key)?.unwrap_or_else(|| default.into()))
}
fn put(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    set_setting(conn, key, value, chrono::Utc::now().timestamp_millis()).map(|_| ())
}

/// Called once after schema migration; `existing` is captured before opening DB.
pub fn initialize(conn: &Connection, existing: bool) -> rusqlite::Result<()> {
    if get_setting(conn, "preferences_v1")?.is_some() {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    put(&tx, "preferences_v1", "1")?;
    put(&tx, "ui_language", "en")?;
    put(&tx, "onboarding_stage", "language")?;
    put(
        &tx,
        "preferences_upgraded",
        if existing { "1" } else { "0" },
    )?;
    // Resolve 0.4.1's implicit cloud mode before changing defaults for fresh DBs.
    if get_setting(&tx, "stt_mode")?.is_none() {
        put(&tx, "stt_mode", if existing { "cloud" } else { "local" })?;
    }
    let old_model = read(&tx, "stt_local_model", "gigaam-v3-e2e-ctc")?;
    put(&tx, "stt_local_model_ru", &old_model)?;
    put(&tx, "stt_local_model_en", "moonshine-small-streaming-en")?;
    put(
        &tx,
        "stt_local_language",
        if existing { "ru" } else { "en" },
    )?;
    put(&tx, "tts_language", if existing { "ru" } else { "en" })?;
    put(&tx, "tts_voice_ru", "xenia")?;
    put(&tx, "tts_voice_en", "en_0")?;
    tx.commit()
}

/// Read persisted preferences, without inferring speech language from the UI.
pub fn get(conn: &Connection) -> rusqlite::Result<Preferences> {
    Ok(Preferences {
        ui_language: Language::parse(&read(conn, "ui_language", "en")?),
        onboarding_stage: read(conn, "onboarding_stage", "language")?,
        upgraded: read(conn, "preferences_upgraded", "1")? == "1",
        stt_language: Language::parse(&read(conn, "stt_local_language", "ru")?),
        tts_language: Language::parse(&read(conn, "tts_language", "ru")?),
        tts_voice_en: read(conn, "tts_voice_en", "en_0")?,
        tts_voice_ru: read(conn, "tts_voice_ru", "xenia")?,
        legacy_voice_migrated: read(conn, "legacy_voice_migrated", "0")? == "1",
    })
}

pub fn set_stt_language(conn: &Connection, language: Language) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    let model = read(
        &tx,
        &format!("stt_local_model_{}", language.tag()),
        if language == Language::En {
            "moonshine-small-streaming-en"
        } else {
            "gigaam-v3-e2e-ctc"
        },
    )?;
    put(&tx, "stt_local_language", language.tag())?;
    put(&tx, "stt_local_model", &model)?;
    tx.commit()
}

/// Set native message language after loading/saving preferences.
pub fn activate(language: Language) {
    RUSSIAN_UI.store(language == Language::Ru, Ordering::Relaxed);
}

/// First choice seeds speech languages only for a fresh installation.
pub fn choose_ui(conn: &Connection, language: Language) -> rusqlite::Result<Preferences> {
    let before = get(conn)?;
    let tx = conn.unchecked_transaction()?;
    put(&tx, "ui_language", language.tag())?;
    if before.onboarding_stage == "language" {
        if !before.upgraded {
            put(&tx, "stt_local_language", language.tag())?;
            put(&tx, "tts_language", language.tag())?;
            let model = read(
                &tx,
                &format!("stt_local_model_{}", language.tag()),
                "gigaam-v3-e2e-ctc",
            )?;
            put(&tx, "stt_local_model", &model)?;
        }
        put(&tx, "onboarding_stage", "speech")?;
    }
    tx.commit()?;
    get(conn)
}

/// Import the old webview voice once. Unknown/localStorage values are ignored.
pub fn migrate_voice(conn: &Connection, voice: Option<&str>) -> rusqlite::Result<()> {
    if get(conn)?.legacy_voice_migrated {
        return Ok(());
    }
    let tx = conn.unchecked_transaction()?;
    if let Some(voice) = voice.filter(|v| crate::tts::silero::VOICES.contains(v)) {
        put(&tx, "tts_voice_ru", voice)?;
    }
    put(&tx, "legacy_voice_migrated", "1")?;
    tx.commit()
}

/// Finish onboarding without downloading anything or accepting a model license.
pub fn finish_onboarding(conn: &Connection) -> rusqlite::Result<()> {
    if get(conn)?.onboarding_stage == "speech" {
        put(conn, "onboarding_stage", "complete")?;
    }
    Ok(())
}

/// Persist an explicit TTS language independently of the interface and dictation.
pub fn set_tts_language(conn: &Connection, language: Language) -> rusqlite::Result<()> {
    put(conn, "tts_language", language.tag())
}

/// Persist a voice only in the selected language's preference slot.
pub fn set_tts_voice(conn: &Connection, language: Language, voice: &str) -> Result<(), String> {
    let voices = if language == Language::En {
        crate::tts::silero::models::EN_VOICES
    } else {
        crate::tts::silero::VOICES
    };
    if !voices.contains(&voice) {
        return Err(message("Unknown voice.", "Неизвестный голос."));
    }
    put(conn, &format!("tts_voice_{}", language.tag()), voice).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn db() -> Connection {
        let mut c = Connection::open_in_memory().unwrap();
        crate::db::migrations::apply_migrations(&mut c).unwrap();
        c
    }
    #[test]
    fn empty_upgrade_keeps_implicit_cloud_and_russian_speech() {
        let c = db();
        initialize(&c, true).unwrap();
        let p = choose_ui(&c, Language::En).unwrap();
        assert_eq!(read(&c, "stt_mode", "").unwrap(), "cloud");
        assert_eq!(p.tts_language, Language::Ru);
        assert_eq!(p.stt_language, Language::Ru);
        assert_eq!(p.onboarding_stage, "speech");
    }
    #[test]
    fn fresh_choice_is_saved_before_restart_and_subsequent_ui_switch_is_independent() {
        let c = db();
        initialize(&c, false).unwrap();
        choose_ui(&c, Language::Ru).unwrap();
        initialize(&c, true).unwrap(); // restart must not reclassify as an upgrade
        let p = choose_ui(&c, Language::En).unwrap();
        assert_eq!(p.tts_language, Language::Ru);
        assert!(!p.upgraded);
        assert_eq!(read(&c, "stt_mode", "").unwrap(), "local");
        finish_onboarding(&c).unwrap();
        assert_eq!(get(&c).unwrap().onboarding_stage, "complete");
    }
    #[test]
    fn upgrade_preserves_profiles_and_migrates_voice_once() {
        let c = db();
        for (k, v) in [
            ("stt_mode", "server"),
            ("stt_server_language", "auto"),
            ("stt_server_base_url", "http://localhost:9000/v1"),
            ("stt_local_model", "gigaam-v3-e2e-rnnt"),
        ] {
            put(&c, k, v).unwrap();
        }
        initialize(&c, true).unwrap();
        choose_ui(&c, Language::En).unwrap();
        assert_eq!(read(&c, "stt_mode", "").unwrap(), "server");
        assert_eq!(read(&c, "stt_server_language", "").unwrap(), "auto");
        assert_eq!(
            read(&c, "stt_server_base_url", "").unwrap(),
            "http://localhost:9000/v1"
        );
        assert_eq!(
            read(&c, "stt_local_model_ru", "").unwrap(),
            "gigaam-v3-e2e-rnnt"
        );
        migrate_voice(&c, Some("eugene")).unwrap();
        migrate_voice(&c, Some("xenia")).unwrap();
        assert_eq!(get(&c).unwrap().tts_voice_ru, "eugene");
    }

    #[test]
    fn all_eight_language_combinations_survive_restart_and_keep_per_language_choices() {
        let c = db();
        initialize(&c, false).unwrap();
        choose_ui(&c, Language::En).unwrap();
        finish_onboarding(&c).unwrap();
        put(&c, "stt_local_model_ru", "gigaam-v3-e2e-rnnt").unwrap();
        set_tts_voice(&c, Language::Ru, "eugene").unwrap();
        set_tts_voice(&c, Language::En, "en_3").unwrap();
        for ui in [Language::En, Language::Ru] {
            for stt in [Language::En, Language::Ru] {
                for tts in [Language::En, Language::Ru] {
                    set_stt_language(&c, stt).unwrap();
                    set_tts_language(&c, tts).unwrap();
                    choose_ui(&c, ui).unwrap();
                    initialize(&c, true).unwrap();
                    let value = get(&c).unwrap();
                    assert_eq!(
                        (value.ui_language, value.stt_language, value.tts_language),
                        (ui, stt, tts)
                    );
                    assert_eq!(value.tts_voice_ru, "eugene");
                    assert_eq!(value.tts_voice_en, "en_3");
                    assert_eq!(
                        read(&c, "stt_local_model", "").unwrap(),
                        if stt == Language::Ru {
                            "gigaam-v3-e2e-rnnt"
                        } else {
                            "moonshine-small-streaming-en"
                        }
                    );
                }
            }
        }
    }
}
