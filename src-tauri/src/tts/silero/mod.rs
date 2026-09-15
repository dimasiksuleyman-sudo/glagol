//! Optional, verified CPU runtime and an owned offline worker process.
pub mod catalog;
mod runtime;
mod worker;
use crate::stt::local::download;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};
pub use worker::Worker;

pub const MODEL_ID: &str = "silero-v5_5_ru";
pub const LICENSE: &str = include_str!("../../../../docs/third-party/Silero-LICENSE.txt");
pub const LICENSE_HASH: &str = "1349a4b6148492b44f629e64eed676612e234fe9a839e4f3b277c1482c8849f1";
pub const VOICES: &[&str] = &["aidar", "baya", "kseniya", "xenia", "eugene"];
const WORKER_IDLE_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const VERIFICATION_MAX_AGE: i64 = 30 * 24 * 60 * 60;
const VERIFICATION_STAMP: &str = "verification.json";

#[derive(Deserialize, Serialize)]
struct FileFingerprint {
    bytes: u64,
    modified_ns: u128,
}

#[derive(Deserialize, Serialize)]
struct VerificationStamp {
    schema_version: u8,
    model: String,
    verified_at: i64,
    runtime_manifest_sha256: String,
    model_file: FileFingerprint,
    python_exe: FileFingerprint,
    python_dll: FileFingerprint,
}

fn fingerprint(path: &Path) -> Result<FileFingerprint, String> {
    let metadata = std::fs::metadata(path).map_err(|_| "missing file".to_string())?;
    if !metadata.is_file() {
        return Err("not a file".into());
    }
    let modified_ns = metadata
        .modified()
        .map_err(|_| "missing timestamp".to_string())?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "invalid timestamp".to_string())?
        .as_nanos();
    Ok(FileFingerprint {
        bytes: metadata.len(),
        modified_ns,
    })
}

fn runtime_manifest_sha256() -> String {
    format!("{:x}", Sha256::digest(include_bytes!("runtime-files.json")))
}

#[derive(Default)]
struct VerificationCache {
    ready: AtomicBool,
    gate: tokio::sync::Mutex<()>,
    error: Mutex<Option<String>>,
}

impl VerificationCache {
    async fn run<F, Fut>(&self, verify: F) -> Result<bool, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<bool, String>>,
    {
        if self.ready.load(Ordering::Acquire) {
            return Ok(false);
        }
        let _gate = self.gate.lock().await;
        if self.ready.load(Ordering::Acquire) {
            return Ok(false);
        }
        if let Some(error) = self.error.lock().unwrap().clone() {
            return Err(error);
        }
        match verify().await {
            Ok(performed) => {
                self.ready.store(true, Ordering::Release);
                Ok(performed)
            }
            Err(error) => {
                *self.error.lock().unwrap() = Some(error.clone());
                Err(error)
            }
        }
    }

    async fn invalidate(&self) {
        let _gate = self.gate.lock().await;
        self.ready.store(false, Ordering::Release);
        *self.error.lock().unwrap() = None;
    }

    fn mark_ready(&self) {
        *self.error.lock().unwrap() = None;
        self.ready.store(true, Ordering::Release);
    }
}

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
    verification: VerificationCache,
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
            verification: VerificationCache::default(),
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
    fn recent_verification_valid(&self) -> bool {
        let Ok(contents) = std::fs::read_to_string(self.root.join(VERIFICATION_STAMP)) else {
            return false;
        };
        let Ok(stamp) = serde_json::from_str::<VerificationStamp>(&contents) else {
            return false;
        };
        let age = chrono::Utc::now().timestamp() - stamp.verified_at;
        if stamp.schema_version != 1
            || stamp.model != MODEL_ID
            || !(0..=VERIFICATION_MAX_AGE).contains(&age)
            || stamp.runtime_manifest_sha256 != runtime_manifest_sha256()
        {
            return false;
        }
        fingerprint(&self.root.join(catalog::MODEL.file)).is_ok_and(|value| {
            value.bytes == stamp.model_file.bytes
                && value.modified_ns == stamp.model_file.modified_ns
        }) && fingerprint(&self.root.join("runtime/python.exe")).is_ok_and(|value| {
            value.bytes == stamp.python_exe.bytes
                && value.modified_ns == stamp.python_exe.modified_ns
        }) && fingerprint(&self.root.join("runtime/python311.dll")).is_ok_and(|value| {
            value.bytes == stamp.python_dll.bytes
                && value.modified_ns == stamp.python_dll.modified_ns
        })
    }
    fn record_verification(&self) -> Result<(), String> {
        let stamp = VerificationStamp {
            schema_version: 1,
            model: MODEL_ID.into(),
            verified_at: chrono::Utc::now().timestamp(),
            runtime_manifest_sha256: runtime_manifest_sha256(),
            model_file: fingerprint(&self.root.join(catalog::MODEL.file))?,
            python_exe: fingerprint(&self.root.join("runtime/python.exe"))?,
            python_dll: fingerprint(&self.root.join("runtime/python311.dll"))?,
        };
        let destination = self.root.join(VERIFICATION_STAMP);
        let partial = self.root.join(format!("{VERIFICATION_STAMP}.part"));
        std::fs::write(
            &partial,
            serde_json::to_vec(&stamp).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if destination.exists() {
            std::fs::remove_file(&destination).map_err(|e| e.to_string())?;
        }
        std::fs::rename(partial, destination).map_err(|e| e.to_string())
    }
    async fn verify_installed(self: &Arc<Self>) -> Result<bool, String> {
        self.verification
            .run(|| {
                let this = self.clone();
                async move {
                    let started = std::time::Instant::now();
                    let result = tokio::task::spawn_blocking(move || {
                        if this.recent_verification_valid() {
                            return Ok(false);
                        }
                        download::verify(&this.root.join(catalog::MODEL.file), &catalog::MODEL)?;
                        runtime::verify(&this.root.join("runtime"), &this.cancel)?;
                        if this.record_verification().is_err() {
                            tracing::warn!("local TTS verification stamp could not be saved");
                        }
                        Ok(true)
                    })
                    .await
                    .map_err(|e| e.to_string())?;
                    match &result {
                        Ok(true) => tracing::info!(
                            elapsed_ms = started.elapsed().as_millis(),
                            "local TTS integrity verification completed"
                        ),
                        Ok(false) => tracing::info!(
                            elapsed_ms = started.elapsed().as_millis(),
                            "local TTS recent verification accepted"
                        ),
                        Err(_) => tracing::warn!(
                            elapsed_ms = started.elapsed().as_millis(),
                            "local TTS integrity verification failed"
                        ),
                    }
                    result
                }
            })
            .await
    }
    pub async fn invalidate_verification(&self) {
        self.verification.invalidate().await;
        let _ = std::fs::remove_file(self.root.join(VERIFICATION_STAMP));
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
        self.invalidate_verification().await;
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
        if self.record_verification().is_err() {
            tracing::warn!("local TTS verification stamp could not be saved");
        }
        self.verification.mark_ready();
        Ok(())
    }
    pub async fn prepare(self: &Arc<Self>, app: &tauri::AppHandle) -> Result<(), String> {
        let started = std::time::Instant::now();
        self.require_consent()?;
        if self.worker.lock().await.is_some() {
            tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                worker_reused = true,
                verification_reused = true,
                "local TTS preparation completed"
            );
            return Ok(());
        }
        self.stage(app, "preparing", 0, 0);
        let verification_performed = self.verify_installed().await?;
        let worker = match Worker::start(&self.root, self.cancel.clone()).await {
            Ok(worker) => worker,
            Err(_) if !verification_performed => {
                tracing::warn!("local TTS worker startup failed; forcing integrity verification");
                self.invalidate_verification().await;
                self.verify_installed().await?;
                Worker::start(&self.root, self.cancel.clone()).await?
            }
            Err(error) => return Err(error),
        };
        *self.worker.lock().await = Some(worker);
        tracing::info!(
            elapsed_ms = started.elapsed().as_millis(),
            worker_reused = false,
            verification_reused = !verification_performed,
            "local TTS preparation completed"
        );
        Ok(())
    }
    pub fn verification_task(app: tauri::AppHandle) {
        tauri::async_runtime::spawn(async move {
            let state = app.state::<Arc<Silero>>().inner().clone();
            let status = state.status();
            if !status.supported || !status.installed || !status.accepted {
                return;
            }
            let started = std::time::Instant::now();
            let result = state.verify_installed().await;
            match &result {
                Ok(verification_performed) => tracing::info!(
                    elapsed_ms = started.elapsed().as_millis(),
                    verification_performed,
                    "local TTS background verification available"
                ),
                Err(error) => {
                    *state.error.lock().unwrap() = Some(error.clone());
                    tracing::warn!(
                        elapsed_ms = started.elapsed().as_millis(),
                        "local TTS background verification failed"
                    );
                    let _ = app.emit("tts-status", ());
                }
            }
        });
    }
    pub fn idle_task(app: tauri::AppHandle) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let state = app.state::<Arc<Silero>>();
                if let Ok(_operation) = state.operation.try_lock() {
                    let mut worker = state.worker.lock().await;
                    let idle_ms = worker.as_ref().and_then(|worker| {
                        let idle = worker.last_used.elapsed();
                        (idle > WORKER_IDLE_TIMEOUT).then_some(idle.as_millis())
                    });
                    if let Some(idle_ms) = idle_ms {
                        *worker = None;
                        tracing::info!(idle_ms, "local TTS worker unloaded after idle");
                    }
                };
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
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

    #[tokio::test]
    async fn concurrent_verification_is_reused_for_the_process() {
        let cache = Arc::new(VerificationCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let first = {
            let cache = cache.clone();
            let calls = calls.clone();
            async move {
                cache
                    .run(|| async move {
                        calls.fetch_add(1, Ordering::Relaxed);
                        tokio::task::yield_now().await;
                        Ok(true)
                    })
                    .await
            }
        };
        let second = {
            let cache = cache.clone();
            let calls = calls.clone();
            async move {
                cache
                    .run(|| async move {
                        calls.fetch_add(1, Ordering::Relaxed);
                        Ok(true)
                    })
                    .await
            }
        };
        let (first, second) = tokio::join!(first, second);
        assert!(first.is_ok());
        assert!(second.is_ok());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn failed_verification_is_reused_until_invalidation() {
        let cache = VerificationCache::default();
        let calls = AtomicUsize::new(0);
        assert!(cache
            .run(|| async {
                calls.fetch_add(1, Ordering::Relaxed);
                Err("broken".into())
            })
            .await
            .is_err());
        assert!(cache
            .run(|| async {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(true)
            })
            .await
            .is_err());
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        cache.invalidate().await;
        cache
            .run(|| async {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(true)
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn recent_verification_requires_fresh_unchanged_key_files() {
        let root = std::env::temp_dir().join(format!("tts-receipt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        std::fs::write(root.join(catalog::MODEL.file), b"model").unwrap();
        std::fs::write(root.join("runtime/python.exe"), b"python").unwrap();
        std::fs::write(root.join("runtime/python311.dll"), b"dll").unwrap();
        let state = Silero::new(root.clone());

        state.record_verification().unwrap();
        assert!(state.recent_verification_valid());

        std::fs::write(root.join(catalog::MODEL.file), b"changed").unwrap();
        assert!(!state.recent_verification_valid());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verification_older_than_thirty_days_is_not_reused() {
        let root = std::env::temp_dir().join(format!("tts-receipt-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        std::fs::write(root.join(catalog::MODEL.file), b"model").unwrap();
        std::fs::write(root.join("runtime/python.exe"), b"python").unwrap();
        std::fs::write(root.join("runtime/python311.dll"), b"dll").unwrap();
        let state = Silero::new(root.clone());
        state.record_verification().unwrap();

        let path = root.join(VERIFICATION_STAMP);
        let mut stamp: VerificationStamp =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        stamp.verified_at -= VERIFICATION_MAX_AGE + 1;
        std::fs::write(path, serde_json::to_vec(&stamp).unwrap()).unwrap();
        assert!(!state.recent_verification_valid());
        std::fs::remove_dir_all(root).unwrap();
    }
}
