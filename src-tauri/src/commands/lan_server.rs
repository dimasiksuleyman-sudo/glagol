//! Settings and control of the LAN recognition server (GL-010).
//!
//! Settings live in `app_settings`; the access key lives in the OS keyring and
//! crosses IPC only when the user asks for the connection code.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::db::repository;
use crate::lan_server::{self, auth, awake, GigaAm, Server};
use crate::state::AppState;
use crate::stt::local::LocalModels;

const KEY_ENABLED: &str = "lan_server_enabled";
const KEY_PORT: &str = "lan_server_port";
const KEY_KEEP_AWAKE: &str = "lan_server_keep_awake";
/// The Russian model the host dictates with; the server serves the same one.
const KEY_RU_MODEL: &str = "stt_local_model_ru";
const DEFAULT_RU_MODEL: &str = "gigaam-v3-e2e-ctc";

/// The running server, its last start error and the keep-awake request.
#[derive(Default)]
pub struct LanServerState {
    server: tokio::sync::Mutex<Option<Server>>,
    error: Mutex<Option<String>>,
    awake: Mutex<Option<awake::Guard>>,
}

/// Saved server settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanServerSettings {
    /// Serve recognition to other computers.
    pub enabled: bool,
    /// TCP port.
    pub port: u16,
    /// Keep the computer from sleeping while serving.
    pub keep_awake: bool,
}

/// Read the saved settings; absent or damaged values fall back to defaults
/// (off, port 8765, keep awake).
pub fn read_settings(conn: &Connection) -> rusqlite::Result<LanServerSettings> {
    let flag = |key: &str, default: bool| -> rusqlite::Result<bool> {
        Ok(match repository::get_setting(conn, key)?.as_deref() {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        })
    };
    Ok(LanServerSettings {
        enabled: flag(KEY_ENABLED, false)?,
        port: repository::get_setting(conn, KEY_PORT)?
            .and_then(|p| p.parse::<u16>().ok())
            .filter(|p| valid_port(*p))
            .unwrap_or(lan_server::DEFAULT_PORT),
        keep_awake: flag(KEY_KEEP_AWAKE, true)?,
    })
}

fn put(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    repository::set_setting(conn, key, value, chrono::Utc::now().timestamp_millis())
        .map(|_| ())
        .map_err(|e| format!("Не удалось сохранить настройки сервера: {e}"))
}

/// Ports below 1024 need administrator rights; 8766 is reserved for LAN
/// discovery.
pub fn valid_port(port: u16) -> bool {
    port >= 1024 && port != 8766
}

fn settings(app: &AppHandle) -> Result<LanServerSettings, String> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    read_settings(&conn).map_err(|e| format!("Не удалось прочитать настройки сервера: {e}"))
}

fn ru_model(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let model = state
        .db
        .lock()
        .ok()
        .and_then(|conn| repository::get_setting(&conn, KEY_RU_MODEL).ok().flatten());
    model.unwrap_or_else(|| DEFAULT_RU_MODEL.to_string())
}

/// What the settings screen shows.
#[derive(Debug, Clone, Serialize)]
pub struct LanServerInfo {
    /// Local GigaAM runs on this platform (Windows x64).
    pub supported: bool,
    /// The saved switch.
    pub enabled: bool,
    /// The server is listening.
    pub running: bool,
    /// Saved port.
    pub port: u16,
    /// Saved keep-awake switch.
    pub keep_awake: bool,
    /// This computer's LAN address, if it has a private IPv4 address.
    pub address: Option<String>,
    /// Display name of the served Russian model.
    pub model_name: Option<String>,
    /// The served model is installed.
    pub model_ready: bool,
    /// Counters of the running server.
    pub status: Option<lan_server::Status>,
    /// Why the server is not running although it is enabled.
    pub error: Option<String>,
}

async fn info(app: &AppHandle) -> Result<LanServerInfo, String> {
    let saved = settings(app)?;
    let state = app.state::<LanServerState>();
    let status = state.server.lock().await.as_ref().map(Server::status);
    let model_id = ru_model(app);
    let models = app.state::<Arc<LocalModels>>();
    let local = models.status();
    let model_ready = local
        .models
        .iter()
        .any(|m| m.id == model_id && m.language == "ru" && m.installed);
    Ok(LanServerInfo {
        supported: local.supported,
        enabled: saved.enabled,
        running: status.is_some(),
        port: saved.port,
        keep_awake: saved.keep_awake,
        address: lan_server::net::primary_ipv4().map(|ip| ip.to_string()),
        model_name: crate::stt::local::catalog::model(&model_id)
            .ok()
            .map(|m| m.name.to_string()),
        model_ready,
        status,
        error: state.error.lock().ok().and_then(|e| e.clone()),
    })
}

/// Start the server with the saved settings if it is not running.
pub async fn start(app: &AppHandle) -> Result<(), String> {
    let saved = settings(app)?;
    let state = app.state::<LanServerState>();
    let mut slot = state.server.lock().await;
    if slot.is_some() {
        return Ok(());
    }
    let key = tauri::async_runtime::spawn_blocking(auth::load_or_create_key)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Не удалось получить ключ доступа сервера: {e}"))?;
    let handle = app.clone();
    let recognizer = GigaAm {
        models: app.state::<Arc<LocalModels>>().inner().clone(),
        model_source: Box::new(move || ru_model(&handle)),
    };
    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, saved.port));
    match Server::start(addr, key, Arc::new(recognizer)).await {
        Ok(server) => {
            *slot = Some(server);
            *state.error.lock().map_err(|e| e.to_string())? = None;
            *state.awake.lock().map_err(|e| e.to_string())? =
                saved.keep_awake.then(awake::keep_awake);
            Ok(())
        }
        Err(e) => {
            let message = if e.kind() == std::io::ErrorKind::AddrInUse {
                format!(
                    "Порт {} уже занят другой программой. Выберите другой порт.",
                    saved.port
                )
            } else {
                format!("Не удалось запустить сервер: {e}")
            };
            tracing::warn!(port = saved.port, kind = ?e.kind(), "LAN server failed to start");
            *state.error.lock().map_err(|e| e.to_string())? = Some(message.clone());
            Err(message)
        }
    }
}

/// Stop the server if it runs and release the keep-awake request.
pub async fn stop(app: &AppHandle) {
    let state = app.state::<LanServerState>();
    let server = state.server.lock().await.take();
    if let Some(server) = server {
        server.stop();
    }
    if let Ok(mut awake) = state.awake.lock() {
        *awake = None;
    }
    if let Ok(mut error) = state.error.lock() {
        *error = None;
    };
}

async fn restart_if_running(app: &AppHandle) -> Result<(), String> {
    let running = app.state::<LanServerState>().server.lock().await.is_some();
    if running {
        stop(app).await;
        start(app).await?;
    }
    Ok(())
}

/// Start the server at launch when the user turned it on.
pub fn start_at_launch(app: &AppHandle) {
    let enabled = settings(app).map(|s| s.enabled).unwrap_or(false);
    if !enabled {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = start(&app).await {
            tracing::warn!("LAN server not started at launch: {e}");
        }
    });
}

#[tauri::command]
pub async fn get_lan_server(app: AppHandle) -> Result<LanServerInfo, String> {
    info(&app).await.map_err(crate::i18n::error)
}

#[tauri::command]
pub async fn set_lan_server_enabled(
    app: AppHandle,
    enabled: bool,
) -> Result<LanServerInfo, String> {
    (async {
        if enabled {
            start(&app).await?;
        } else {
            stop(&app).await;
        }
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            put(&conn, KEY_ENABLED, if enabled { "true" } else { "false" })?;
        }
        info(&app).await
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub async fn set_lan_server_port(app: AppHandle, port: u16) -> Result<LanServerInfo, String> {
    (async {
        if !valid_port(port) {
            return Err("Порт должен быть от 1024 до 65535 (кроме 8766).".to_string());
        }
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            put(&conn, KEY_PORT, &port.to_string())?;
        }
        restart_if_running(&app).await?;
        info(&app).await
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub async fn set_lan_server_keep_awake(
    app: AppHandle,
    enabled: bool,
) -> Result<LanServerInfo, String> {
    (async {
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            put(
                &conn,
                KEY_KEEP_AWAKE,
                if enabled { "true" } else { "false" },
            )?;
        }
        let state = app.state::<LanServerState>();
        let running = state.server.lock().await.is_some();
        *state.awake.lock().map_err(|e| e.to_string())? =
            (running && enabled).then(awake::keep_awake);
        info(&app).await
    })
    .await
    .map_err(crate::i18n::error)
}

/// The connection code with the access key, for the user to copy.
#[tauri::command]
pub async fn lan_server_connection_code(app: AppHandle) -> Result<String, String> {
    (async {
        let saved = settings(&app)?;
        let address = lan_server::net::primary_ipv4()
            .ok_or("У этого компьютера нет адреса в локальной сети.")?;
        let key = tauri::async_runtime::spawn_blocking(auth::load_or_create_key)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("Не удалось получить ключ доступа сервера: {e}"))?;
        Ok(lan_server::connection_code(
            &address.to_string(),
            saved.port,
            &key,
        ))
    })
    .await
    .map_err(crate::i18n::error)
}

/// Replace the access key; computers with the old code are refused.
#[tauri::command]
pub async fn regenerate_lan_server_key(app: AppHandle) -> Result<LanServerInfo, String> {
    (async {
        tauri::async_runtime::spawn_blocking(auth::regenerate_key)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| format!("Не удалось создать новый ключ: {e}"))?;
        restart_if_running(&app).await?;
        info(&app).await
    })
    .await
    .map_err(crate::i18n::error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_to_off_and_survive_bad_values() {
        let conn = crate::db::test_connection();
        assert_eq!(
            read_settings(&conn).unwrap(),
            LanServerSettings {
                enabled: false,
                port: 8765,
                keep_awake: true
            }
        );
        put(&conn, KEY_ENABLED, "true").unwrap();
        put(&conn, KEY_PORT, "9000").unwrap();
        put(&conn, KEY_KEEP_AWAKE, "false").unwrap();
        assert_eq!(
            read_settings(&conn).unwrap(),
            LanServerSettings {
                enabled: true,
                port: 9000,
                keep_awake: false
            }
        );
        put(&conn, KEY_PORT, "80").unwrap();
        assert_eq!(read_settings(&conn).unwrap().port, 8765, "privileged port");
        put(&conn, KEY_PORT, "garbage").unwrap();
        assert_eq!(read_settings(&conn).unwrap().port, 8765);
    }

    #[test]
    fn ports_below_1024_and_the_discovery_port_are_refused() {
        assert!(valid_port(8765));
        assert!(valid_port(65535));
        assert!(!valid_port(80));
        assert!(!valid_port(8766));
    }
}
