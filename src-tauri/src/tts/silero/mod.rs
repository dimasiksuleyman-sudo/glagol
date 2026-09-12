//! Optional, verified CPU runtime and an owned offline worker process.
pub mod catalog;
mod runtime;
mod worker;
use crate::stt::local::download;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Emitter, Manager};
pub use worker::Worker;

pub const MODEL_ID: &str = "silero-v5_5_ru";
pub const LICENSE: &str = include_str!("../../../../docs/third-party/Silero-LICENSE.txt");
pub const LICENSE_HASH: &str = "1349a4b6148492b44f629e64eed676612e234fe9a839e4f3b277c1482c8849f1";
pub const VOICES: &[&str] = &["aidar", "baya", "kseniya", "xenia", "eugene"];

#[derive(Clone, Serialize)]
pub struct Progress {
    pub stage: String,
    pub downloaded: u64,
    pub total: u64,
}
#[derive(Serialize)]
pub struct Status {
    pub supported: bool,
    pub installed: bool,
    pub accepted: bool,
    pub model_bytes: u64,
    pub runtime_bytes: u64,
    pub disk_bytes: u64,
    pub partial_bytes: u64,
    pub license: &'static str,
    pub license_hash: &'static str,
    pub progress: Option<Progress>,
    pub error: Option<String>,
}

/// TTS state and files are separate from dictation and database backups.
pub struct Silero {
    pub root: PathBuf,
    pub operation: tokio::sync::Mutex<()>,
    pub worker: tokio::sync::Mutex<Option<Worker>>,
    pub cancel: Arc<AtomicBool>,
    progress: Mutex<Option<Progress>>,
    error: Mutex<Option<String>>,
}
impl Silero {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            operation: tokio::sync::Mutex::new(()),
            worker: tokio::sync::Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Mutex::new(None),
            error: Mutex::new(None),
        }
    }
    pub fn accepted(&self) -> bool {
        std::fs::read_to_string(self.root.join("acknowledgement.json"))
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .is_some_and(|v| v["model"] == MODEL_ID && v["license_sha256"] == LICENSE_HASH)
    }
    pub fn require_consent(&self) -> Result<(), String> {
        if self.accepted() {
            Ok(())
        } else {
            Err("Откройте настройки озвучки и подтвердите условия некоммерческого использования Silero.".into())
        }
    }
    pub fn acknowledge(&self, accepted: bool, license_hash: &str) -> Result<(), String> {
        if !accepted || license_hash != LICENSE_HASH {
            return Err("Подтвердите актуальные условия Silero перед загрузкой.".into());
        }
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let data = serde_json::json!({"model": MODEL_ID, "license_sha256": LICENSE_HASH, "accepted_at": chrono::Utc::now().to_rfc3339()});
        std::fs::write(self.root.join("acknowledgement.json"), data.to_string())
            .map_err(|e| e.to_string())
    }
    pub fn status(&self) -> Status {
        let runtime_bytes = catalog::ARTIFACTS.iter().map(|a| a.bytes).sum();
        let partial_bytes = catalog::ARTIFACTS
            .iter()
            .chain(std::iter::once(&catalog::MODEL))
            .map(|a| {
                std::fs::metadata(self.root.join(format!("{}.part", a.file)))
                    .map(|m| m.len())
                    .unwrap_or(0)
            })
            .sum();
        Status {
            supported: cfg!(all(windows, target_arch = "x86_64")),
            installed: self.root.join("runtime/python.exe").is_file()
                && self.root.join(catalog::MODEL.file).is_file(),
            accepted: self.accepted(),
            model_bytes: catalog::MODEL.bytes,
            runtime_bytes,
            disk_bytes: 1_900_000_000,
            partial_bytes,
            license: LICENSE,
            license_hash: LICENSE_HASH,
            progress: self.progress.lock().unwrap().clone(),
            error: self.error.lock().unwrap().clone(),
        }
    }
    pub fn stage(&self, app: &tauri::AppHandle, stage: &str, downloaded: u64, total: u64) {
        *self.progress.lock().unwrap() = Some(Progress {
            stage: stage.into(),
            downloaded,
            total,
        });
        let _ = app.emit("tts-status", ());
    }
    pub fn finish(&self, app: &tauri::AppHandle, result: &Result<(), String>) {
        *self.progress.lock().unwrap() = None;
        *self.error.lock().unwrap() = result.as_ref().err().cloned();
        let _ = app.emit("tts-status", ());
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub async fn close_worker(&self) {
        if let Some(mut worker) = self.worker.lock().await.take() {
            worker.shutdown().await;
        }
    }
    pub async fn install(
        self: &Arc<Self>,
        app: &tauri::AppHandle,
        model_path: Option<PathBuf>,
    ) -> Result<(), String> {
        if !cfg!(all(windows, target_arch = "x86_64")) {
            return Err("Локальная озвучка пока поддерживает Windows x64.".into());
        }
        self.require_consent()?;
        self.cancel.store(false, Ordering::Relaxed);
        self.close_worker().await;
        let total = catalog::MODEL.bytes + catalog::ARTIFACTS.iter().map(|a| a.bytes).sum::<u64>();
        let missing = std::iter::once(&catalog::MODEL)
            .chain(catalog::ARTIFACTS.iter())
            .filter(|a| !self.root.join(a.file).exists())
            .map(|a| a.bytes)
            .sum::<u64>();
        if fs2::available_space(&self.root).map_err(|e| e.to_string())? < missing + 1_400_000_000 {
            return Err("Недостаточно места: освободите 1,9 ГБ для компонентов озвучки.".into());
        }
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        if let Some(source) = model_path {
            self.stage(app, "verifying", 0, total);
            let target = self.root.join(catalog::MODEL.file);
            tokio::task::spawn_blocking(move || {
                download::verify(&source, &catalog::MODEL)?;
                // Import only the exact pinned model, never arbitrary PyTorch code.
                if source != target {
                    let partial = target.with_extension("importing");
                    std::fs::copy(&source, &partial).map_err(|e| e.to_string())?;
                    download::verify(&partial, &catalog::MODEL)?;
                    std::fs::rename(partial, target).map_err(|e| e.to_string())?;
                }
                Ok::<_, String>(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }
        let mut complete = 0;
        for artifact in std::iter::once(&catalog::MODEL).chain(catalog::ARTIFACTS.iter()) {
            download::fetch(&client, artifact, &self.root, &self.cancel, |n| {
                self.stage(app, "downloading", complete + n, total)
            })
            .await?;
            complete += artifact.bytes;
        }
        self.stage(app, "verifying", total, total);
        let this = self.clone();
        tokio::task::spawn_blocking(move || runtime::assemble(&this.root, &this.cancel))
            .await
            .map_err(|e| e.to_string())??;
        std::fs::write(self.root.join("Silero-LICENSE.txt"), LICENSE).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub async fn prepare(self: &Arc<Self>, app: &tauri::AppHandle) -> Result<(), String> {
        self.require_consent()?;
        if self.worker.lock().await.is_some() {
            return Ok(());
        }
        self.stage(app, "preparing", 0, 0);
        let this = self.clone();
        tokio::task::spawn_blocking(move || {
            download::verify(&this.root.join(catalog::MODEL.file), &catalog::MODEL)?;
            runtime::verify(&this.root.join("runtime"), &this.cancel)
        })
        .await
        .map_err(|e| e.to_string())??;
        let worker = Worker::start(&self.root, self.cancel.clone()).await?;
        *self.worker.lock().await = Some(worker);
        Ok(())
    }
    pub fn idle_task(app: tauri::AppHandle) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let state = app.state::<Arc<Silero>>();
                if let Ok(_operation) = state.operation.try_lock() {
                    let mut worker = state.worker.lock().await;
                    if worker
                        .as_ref()
                        .is_some_and(|w| w.last_used.elapsed() > Duration::from_secs(180))
                    {
                        *worker = None;
                    }
                };
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn consent_is_explicit_and_model_license_bound() {
        let root = std::env::temp_dir().join(format!("tts-consent-{}", uuid::Uuid::new_v4()));
        let state = Silero::new(root.clone());
        assert!(state.require_consent().is_err());
        assert!(state.acknowledge(false, LICENSE_HASH).is_err());
        assert!(state.acknowledge(true, "old-license").is_err());
        assert!(!root.exists());
        state.acknowledge(true, LICENSE_HASH).unwrap();
        assert!(state.accepted());
        std::fs::write(root.join("acknowledgement.json"), "{}").unwrap();
        assert!(!state.accepted());
        std::fs::remove_dir_all(root).unwrap();
    }
}
