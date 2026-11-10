//! On-demand model storage and serialized native inference. No network during dictation.
pub mod catalog;
pub mod download;
mod engine;
pub mod receipt;

use crate::stt::{SttError, SttProvider, Transcript};
use catalog::{model, MODELS, RUNTIME, RUNTIME_DIR};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
use tauri::{Emitter, Manager};

pub struct LocalModels {
    pub root: PathBuf,
    pub engine: Mutex<Option<engine::Engine>>,
    pub operation: Arc<tokio::sync::Mutex<()>>,
    pub moonshine: Arc<super::moonshine::Moonshine>,
    cancel: AtomicBool,
    progress: Mutex<Option<Progress>>,
    options: Mutex<EngineOptions>,
    /// Set after a recognition error on the graphics card: the CPU is used
    /// until the options are saved again or Glagol restarts.
    gpu_failed: AtomicBool,
    /// Real-time factors of recent local recognitions (numbers only, memory only).
    recent_rtf: Mutex<VecDeque<f64>>,
}

/// Recordings shorter than this are not used to judge speed (fixed overhead).
const SPEED_SAMPLE_MIN_AUDIO_MS: u64 = 2_000;
/// How many recent recognitions the speed hint looks at.
const SPEED_SAMPLES: usize = 5;

/// Why a setup is recommended for this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    /// A discrete graphics card was found: use it with CTC.
    GraphicsCard,
    /// No discrete card, but the processor has AVX2: CTC on the processor.
    Processor,
    /// Neither: local recognition will be slow; CTC on the processor, or a
    /// server/cloud profile.
    SlowProcessor,
}

/// The recommended Russian local setup for this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Recommendation {
    pub model: &'static str,
    pub device: Device,
    pub kind: RecommendationKind,
}

/// Recommend a setup from hardware facts. CTC is always the model: its
/// decoder is cheap on every device (measured on the user's PC, 2026-11-09).
pub fn recommend(has_avx2: bool, discrete_gpu: bool) -> Recommendation {
    let (device, kind) = if discrete_gpu {
        (Device::Gpu, RecommendationKind::GraphicsCard)
    } else if has_avx2 {
        (Device::Cpu, RecommendationKind::Processor)
    } else {
        (Device::Cpu, RecommendationKind::SlowProcessor)
    };
    Recommendation {
        model: MODELS[0].id,
        device,
        kind,
    }
}

/// Median of the recent real-time factors, if there are enough samples.
fn median(samples: &VecDeque<f64>) -> Option<f64> {
    if samples.len() < 2 {
        return None;
    }
    let mut sorted: Vec<f64> = samples.iter().copied().collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    Some(sorted[sorted.len() / 2])
}

/// Where the Russian (GigaAM) model computes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Device {
    /// Processor — the default and the fallback.
    #[default]
    Cpu,
    /// Graphics card through Vulkan.
    Gpu,
}

/// User choices for the Russian local engine (Settings → Dictation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineOptions {
    pub device: Device,
    /// CPU threads; 0 = automatic (the library's default).
    pub threads: u32,
}

/// `app_settings` keys of [`EngineOptions`].
const DEVICE_SETTING: &str = "stt_local_device";
const THREADS_SETTING: &str = "stt_local_threads";

impl EngineOptions {
    /// Read persisted options; absent or unknown values mean the defaults.
    pub fn read(conn: &rusqlite::Connection) -> rusqlite::Result<Self> {
        let get = |key| crate::db::repository::get_setting(conn, key);
        Ok(Self {
            device: if get(DEVICE_SETTING)?.as_deref() == Some("gpu") {
                Device::Gpu
            } else {
                Device::Cpu
            },
            threads: get(THREADS_SETTING)?
                .and_then(|v| v.parse().ok())
                .filter(|n| *n <= 64)
                .unwrap_or(0),
        })
    }

    /// Persist options after validation.
    pub fn write(self, conn: &rusqlite::Connection) -> Result<(), String> {
        if self.threads > 64 {
            return Err("Too many threads.".into());
        }
        let now = chrono::Utc::now().timestamp_millis();
        let device = match self.device {
            Device::Cpu => "cpu",
            Device::Gpu => "gpu",
        };
        crate::db::repository::set_setting(conn, DEVICE_SETTING, device, now)
            .and_then(|_| {
                crate::db::repository::set_setting(
                    conn,
                    THREADS_SETTING,
                    &self.threads.to_string(),
                    now,
                )
            })
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

/// The engine as currently loaded, for the settings screen.
#[derive(Serialize)]
pub struct EngineState {
    pub options: EngineOptions,
    /// Logical CPUs, the upper bound offered for threads.
    pub logical_cpus: usize,
    /// Threads used when `options.threads` is 0.
    pub default_threads: usize,
    /// Backend of the loaded model (`CPU`, `Vulkan0`, …), if one is loaded.
    pub backend: Option<String>,
    /// The graphics card was requested but the CPU is used.
    pub gpu_fallback: bool,
    /// A discrete Vulkan graphics card seen by the runtime (after its first load).
    pub gpu_name: Option<String>,
    /// The processor has AVX2.
    pub has_avx2: bool,
    /// The runtime's devices are known (startup probe or a model load done).
    pub hardware_known: bool,
    /// The recommended setup for this computer.
    pub recommendation: Recommendation,
    /// Median wait per second of speech over recent recognitions (≥2 needed).
    pub recent_rtf: Option<f64>,
}

#[derive(Clone, Serialize)]
pub struct Progress {
    pub model_id: String,
    pub stage: String,
    pub downloaded: u64,
    pub total: u64,
}

#[derive(Serialize)]
pub struct ModelStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub description: String,
    pub language: &'static str,
    pub provider: &'static str,
    pub runtime_bytes: u64,
    pub download_bytes: u64,
    pub bytes: u64,
    pub installed: bool,
    pub partial_bytes: u64,
}

#[derive(Serialize)]
pub struct LocalStatus {
    pub supported: bool,
    pub runtime_bytes: u64,
    pub runtime_installed: bool,
    pub models: Vec<ModelStatus>,
    pub progress: Option<Progress>,
}

impl LocalModels {
    pub fn new(root: PathBuf) -> Self {
        let operation = Arc::new(tokio::sync::Mutex::new(()));
        let moonshine = Arc::new(super::moonshine::Moonshine::new(
            root.join("moonshine-0.1.5"),
            operation.clone(),
        ));
        Self {
            root,
            engine: Mutex::new(None),
            operation,
            moonshine,
            cancel: AtomicBool::new(false),
            progress: Mutex::new(None),
            options: Mutex::new(EngineOptions::default()),
            gpu_failed: AtomicBool::new(false),
            recent_rtf: Mutex::new(VecDeque::new()),
        }
    }

    /// Apply options; the next [`prepare`](Self::prepare) reopens the model if
    /// they differ from the loaded session. Clears an earlier GPU failure.
    pub fn set_options(&self, options: EngineOptions) {
        *self.options.lock().unwrap() = options;
        self.gpu_failed.store(false, Ordering::Relaxed);
    }

    /// Options and the loaded backend for the settings screen.
    pub fn engine_state(&self) -> EngineState {
        let options = *self.options.lock().unwrap();
        let (backend, fell_back) = self
            .engine
            .try_lock()
            .ok()
            .and_then(|slot| {
                slot.as_ref()
                    .map(|e| (Some(e.backend.clone()), e.fell_back))
            })
            .unwrap_or((None, false));
        EngineState {
            options,
            logical_cpus: crate::diagnostics::logical_cpus(),
            default_threads: crate::diagnostics::stt_default_threads(),
            backend,
            gpu_fallback: options.device == Device::Gpu
                && (fell_back || self.gpu_failed.load(Ordering::Relaxed)),
            gpu_name: engine::discrete_gpu(),
            has_avx2: crate::diagnostics::has_avx2(),
            hardware_known: engine::runtime_loaded(),
            recommendation: recommend(
                crate::diagnostics::has_avx2(),
                engine::discrete_gpu().is_some(),
            ),
            recent_rtf: self.recent_rtf.lock().ok().and_then(|s| median(&s)),
        }
    }

    /// Remember the speed of one recognition for the settings hint.
    fn record_speed(&self, audio_ms: u64, rtf: f64) {
        if audio_ms < SPEED_SAMPLE_MIN_AUDIO_MS || !rtf.is_finite() {
            return;
        }
        if let Ok(mut samples) = self.recent_rtf.lock() {
            if samples.len() == SPEED_SAMPLES {
                samples.pop_front();
            }
            samples.push_back(rtf);
        }
    }

    /// Learn the processor and graphics card without opening a model, so the
    /// recommendation is ready before the first dictation. Runs only when the
    /// runtime is installed; its files are verified first (a valid receipt of
    /// any model covers them, otherwise the full archive comparison runs).
    pub fn probe_hardware(&self) -> Result<bool, String> {
        if engine::runtime_loaded() {
            return Ok(false);
        }
        if !self
            .root
            .join(RUNTIME_DIR)
            .join(if cfg!(windows) {
                "transcribe.dll"
            } else {
                "libtranscribe.so"
            })
            .is_file()
        {
            return Ok(false);
        }
        let now = chrono::Utc::now().timestamp();
        let reused = MODELS.iter().any(|m| {
            match (
                receipt::current_files(&self.root, m.artifact.file, RUNTIME.file, RUNTIME_DIR),
                receipt::load(&self.root, m.id),
            ) {
                (Some(files), Some(saved)) => receipt::is_valid(&saved, &files, now),
                _ => false,
            }
        });
        if !reused {
            verify_runtime(&self.root)?;
        }
        engine::probe(&self.root.join(RUNTIME_DIR))?;
        Ok(true)
    }

    /// Whether `gpu`/`threads` are what the session should use now.
    fn wanted(&self) -> (bool, u32) {
        let options = *self.options.lock().unwrap();
        (
            options.device == Device::Gpu && !self.gpu_failed.load(Ordering::Relaxed),
            options.threads,
        )
    }

    /// Full SHA-256 check of model and runtime, unless a recent receipt matches.
    /// Returns (verify_model_ms, verify_runtime_ms, reused).
    fn verify_files(&self, m: &catalog::Model) -> Result<(u64, u64, bool), String> {
        let files = receipt::current_files(&self.root, m.artifact.file, RUNTIME.file, RUNTIME_DIR);
        if let (Some(files), Some(saved)) = (&files, receipt::load(&self.root, m.id)) {
            if receipt::is_valid(&saved, files, chrono::Utc::now().timestamp()) {
                return Ok((0, 0, true));
            }
        }
        let started = Instant::now();
        download::verify(&self.root.join(m.artifact.file), &m.artifact).map_err(|_| {
            "Модель отсутствует или повреждена. Скачайте её в настройках диктовки.".to_string()
        })?;
        let verify_model_ms = started.elapsed().as_millis() as u64;
        let runtime_started = Instant::now();
        verify_runtime(&self.root)?;
        let verify_runtime_ms = runtime_started.elapsed().as_millis() as u64;
        if let Some(files) =
            receipt::current_files(&self.root, m.artifact.file, RUNTIME.file, RUNTIME_DIR)
        {
            receipt::save(&self.root, m.id, files);
        }
        Ok((verify_model_ms, verify_runtime_ms, false))
    }
    pub fn status(&self) -> LocalStatus {
        LocalStatus {
            supported: cfg!(all(windows, target_arch = "x86_64")),
            runtime_bytes: RUNTIME.bytes,
            runtime_installed: self.root.join(RUNTIME_DIR).join("transcribe.dll").is_file(),
            models: MODELS
                .iter()
                .map(|m| ModelStatus {
                    id: m.id,
                    name: m.name,
                    description: crate::i18n::error(m.description.into()),
                    language: "ru",
                    provider: "gigaam",
                    runtime_bytes: RUNTIME.bytes,
                    download_bytes: if self.root.join(m.artifact.file).exists() { 0 } else { m.artifact.bytes } + if self.root.join(RUNTIME_DIR).join("transcribe.dll").exists() { 0 } else { RUNTIME.bytes },
                    bytes: m.artifact.bytes,
                    installed: std::fs::metadata(self.root.join(m.artifact.file))
                        .is_ok_and(|s| s.len() == m.artifact.bytes),
                    partial_bytes: std::fs::metadata(
                        self.root.join(format!("{}.part", m.artifact.file)),
                    )
                    .map(|s| s.len())
                    .unwrap_or(0),
                })
                .chain([ModelStatus {
                    id: super::moonshine::catalog::ID,
                    name: "Moonshine Small Streaming",
                    description: crate::preferences::message("English dictation processed while you speak. Text appears after releasing the shortcut.", "Английская диктовка обрабатывается во время речи. Текст появится после отпускания клавиши."),
                    language: "en", provider: "moonshine",
                    bytes: super::moonshine::catalog::FILES.iter().map(|a| a.bytes).sum(),
                    runtime_bytes: super::moonshine::catalog::RUNTIME.bytes,
                    download_bytes: super::moonshine::download_bytes(&self.moonshine.root),
                    installed: super::moonshine::installed(&self.moonshine.root),
                    partial_bytes: super::moonshine::partial_bytes(&self.moonshine.root),
                }]).collect(),
            progress: self.progress.lock().unwrap().clone(),
        }
    }
    pub fn prepare(&self, id: &str) -> Result<(), String> {
        let m = model(id)?;
        let (gpu, threads) = self.wanted();
        let mut slot = self.engine.lock().map_err(|e| e.to_string())?;
        if slot
            .as_ref()
            .is_some_and(|e| e.model_id == id && e.gpu_requested == gpu && e.threads == threads)
        {
            return Ok(());
        }
        let started = Instant::now();
        let (verify_model_ms, verify_runtime_ms, verification_reused) = self.verify_files(m)?;
        // Free old weights before allocating the new model on a 16GB machine.
        *slot = None;
        let now = Instant::now();
        let opened = engine::Engine::open(
            &self.root.join(RUNTIME_DIR),
            &self.root.join(m.artifact.file),
            id,
            gpu,
            threads,
        );
        let opened = match opened {
            Ok(engine) => engine,
            Err(error) => {
                // A file may have changed without its metadata: check fully next time.
                receipt::invalidate(&self.root, id);
                return Err(error);
            }
        };
        tracing::info!(
            model = id,
            elapsed_ms = now.elapsed().as_millis(),
            verify_model_ms,
            verify_runtime_ms,
            verification_reused,
            backend = %opened.backend,
            total_ms = started.elapsed().as_millis() as u64,
            "local STT model loaded"
        );
        *slot = Some(opened);
        Ok(())
    }

    /// Recognize with the loaded model. A recognition error on the graphics
    /// card switches this session to the CPU and retries once.
    fn recognize(
        &self,
        id: &str,
        pcm: &[f32],
    ) -> Result<(String, engine::RunStats, u64, u64, String), String> {
        let started = Instant::now();
        self.prepare(id)?;
        let prepare_ms = started.elapsed().as_millis() as u64;
        let now = Instant::now();
        let mut guard = self.engine.lock().map_err(|e| e.to_string())?;
        let lock_wait_ms = now.elapsed().as_millis() as u64;
        let engine = guard
            .as_mut()
            .filter(|e| e.model_id == id)
            .ok_or("Модель переключена. Повторите диктовку.")?;
        match engine.transcribe(pcm) {
            Ok((text, stats)) => Ok((
                text,
                stats,
                prepare_ms,
                lock_wait_ms,
                engine.backend.clone(),
            )),
            Err(_) if engine.gpu_requested && !engine.fell_back => {
                tracing::warn!(model = id, backend = %engine.backend, "local STT graphics card failed; retrying on CPU");
                self.gpu_failed.store(true, Ordering::Relaxed);
                *guard = None;
                drop(guard);
                self.prepare(id)?;
                let mut guard = self.engine.lock().map_err(|e| e.to_string())?;
                let engine = guard
                    .as_mut()
                    .filter(|e| e.model_id == id)
                    .ok_or("Модель переключена. Повторите диктовку.")?;
                let (text, stats) = engine
                    .transcribe(pcm)
                    .inspect_err(|_| receipt::invalidate(&self.root, id))?;
                Ok((
                    text,
                    stats,
                    prepare_ms,
                    lock_wait_ms,
                    engine.backend.clone(),
                ))
            }
            Err(error) => {
                receipt::invalidate(&self.root, id);
                Err(error)
            }
        }
    }
}

fn verify_runtime(root: &Path) -> Result<(), String> {
    download::verify(&root.join(RUNTIME.file), &RUNTIME)
        .map_err(|_| "Скачайте движок локальной диктовки в настройках.".to_string())?;
    let archive = std::fs::File::open(root.join(RUNTIME.file)).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        validate_entry(&path)?;
        if entry.header().entry_type().is_dir() {
            continue;
        }
        if !entry.header().entry_type().is_file() {
            return Err("Недопустимый файл в архиве движка.".into());
        }
        let mut expected = Vec::new();
        entry
            .read_to_end(&mut expected)
            .map_err(|e| e.to_string())?;
        let disk = std::fs::read(root.join(path))
            .map_err(|_| "Файлы движка повреждены. Повторите загрузку.".to_string())?;
        if Sha256::digest(&expected) != Sha256::digest(&disk) {
            return Err("Файлы движка повреждены. Повторите загрузку.".into());
        }
    }
    Ok(())
}

fn validate_entry(path: &Path) -> Result<(), String> {
    if path
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || !path.starts_with(RUNTIME_DIR)
    {
        return Err("Недопустимый путь в архиве движка.".into());
    }
    Ok(())
}

fn install_runtime(root: &Path) -> Result<(), String> {
    download::verify(&root.join(RUNTIME.file), &RUNTIME)?;
    if verify_runtime(root).is_ok() {
        return Ok(());
    }
    let staging = root.join("runtime-staging");
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let archive = std::fs::File::open(root.join(RUNTIME.file)).map_err(|e| e.to_string())?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        validate_entry(&entry.path().map_err(|e| e.to_string())?)?;
        if !entry.header().entry_type().is_file() && !entry.header().entry_type().is_dir() {
            return Err("Недопустимый тип файла в архиве.".into());
        }
        if !entry.unpack_in(&staging).map_err(|e| e.to_string())? {
            return Err("Недопустимый путь в архиве.".into());
        }
    }
    let target = root.join(RUNTIME_DIR);
    if target.exists() {
        std::fs::remove_dir_all(&target)
            .map_err(|e| format!("Перезапустите Глагол для восстановления движка: {e}"))?;
    }
    std::fs::rename(staging.join(RUNTIME_DIR), target).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir(&staging);
    verify_runtime(root)
}

/// The discrete Vulkan graphics card known to the runtime, if any.
pub fn discrete_gpu_name() -> Option<String> {
    engine::discrete_gpu()
}

/// Options and the loaded backend of the Russian local engine.
#[tauri::command]
pub fn local_engine_state(state: tauri::State<'_, Arc<LocalModels>>) -> EngineState {
    state.engine_state()
}

/// Save engine options (graphics card / CPU threads). When Russian local
/// dictation is active the model is reopened right away, so a graphics-card
/// failure shows up here rather than on the next dictation.
#[tauri::command]
pub async fn set_local_engine_options(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<LocalModels>>,
    options: EngineOptions,
) -> Result<EngineState, String> {
    (async {
        let app_state = app.state::<crate::state::AppState>();
        if *app_state.dictation.lock().map_err(|e| e.to_string())?
            != crate::dictation::DictationPhase::Idle
        {
            return Err("Wait for the current dictation to finish.".into());
        }
        let _operation = state
            .operation
            .try_lock()
            .map_err(|_| "Дождитесь текущей операции с моделью.".to_string())?;
        let profile = {
            let conn = app_state.db.lock().map_err(|e| e.to_string())?;
            options.write(&conn)?;
            crate::commands::speech::read_profile(&conn, None)?
        };
        state.set_options(options);
        tracing::info!(device = ?options.device, threads = options.threads, "local STT engine options changed");
        let models = state.inner().clone();
        if profile.mode == crate::commands::speech::Mode::Local && model(&profile.model).is_ok() {
            tauri::async_runtime::spawn_blocking(move || models.prepare(&profile.model))
                .await
                .map_err(|e| e.to_string())??;
        } else {
            tauri::async_runtime::spawn_blocking(move || {
                *models.engine.lock().map_err(|e| e.to_string())? = None;
                Ok::<_, String>(())
            })
            .await
            .map_err(|e| e.to_string())??;
        }
        Ok(state.engine_state())
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub fn local_models_status(state: tauri::State<'_, Arc<LocalModels>>) -> LocalStatus {
    state.status()
}

#[tauri::command]
pub fn cancel_model_download(state: tauri::State<'_, Arc<LocalModels>>) {
    state.cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub async fn download_local_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<LocalModels>>,
    id: String,
) -> Result<(), String> {
    (async {
        if !cfg!(all(windows, target_arch = "x86_64")) {
            return Err("Локальные модели пока поддерживаются на Windows x64.".into());
        }
        let _operation = state
            .operation
            .try_lock()
            .map_err(|_| "Дождитесь текущей операции с моделью.".to_string())?;
        state.cancel.store(false, Ordering::Relaxed);
        if id == super::moonshine::catalog::ID {
            state.moonshine.close().await;
            let result = super::moonshine::install(
                &app,
                &state.moonshine.root,
                &state.cancel,
                |stage, downloaded, total| {
                    let p = Progress {
                        model_id: id.clone(),
                        stage: stage.into(),
                        downloaded,
                        total,
                    };
                    *state.progress.lock().unwrap() = Some(p.clone());
                    let _ = app.emit("local-model-progress", p);
                },
            )
            .await;
            *state.progress.lock().unwrap() = None;
            let _ = app.emit("local-models-changed", ());
            return result;
        }
        let m = *model(&id)?;
        // Download/repair always ends with a full check on the next load.
        for known in MODELS {
            receipt::invalidate(&state.root, known.id);
        }
        std::fs::create_dir_all(&state.root).map_err(|e| e.to_string())?;
        std::fs::write(
            state.root.join("GigaAM-LICENSE.txt"),
            include_bytes!("../../../../docs/third-party/GigaAM-LICENSE.txt"),
        )
        .map_err(|e| e.to_string())?;
        // Reserve enough for the model, archive and unpacked runtime. Partials are reusable.
        let remaining = m.artifact.bytes.saturating_sub(
            std::fs::metadata(state.root.join(format!("{}.part", m.artifact.file)))
                .map(|s| s.len())
                .unwrap_or(0),
        );
        if fs2::available_space(&state.root).map_err(|e| e.to_string())? < remaining + 200_000_000 {
            return Err(
                "Недостаточно места: освободите не менее 500 МБ и повторите загрузку.".into(),
            );
        }
        let total = RUNTIME.bytes + m.artifact.bytes;
        let last_emit = Mutex::new(Instant::now() - std::time::Duration::from_secs(1));
        let emit = |stage: &str, downloaded| {
            let p = Progress {
                model_id: id.clone(),
                stage: stage.into(),
                downloaded,
                total,
            };
            *state.progress.lock().unwrap() = Some(p.clone());
            let mut last = last_emit.lock().unwrap();
            if downloaded == total || last.elapsed() >= std::time::Duration::from_millis(100) {
                let _ = app.emit("local-model-progress", p);
                *last = Instant::now();
            }
        };
        emit("downloading", 0);
        let result = async {
            let client = reqwest::Client::builder()
                .https_only(true)
                .connect_timeout(std::time::Duration::from_secs(20))
                .read_timeout(std::time::Duration::from_secs(30))
                .user_agent("Glagol local-model downloader")
                .build()
                .map_err(|e| e.to_string())?;
            download::fetch(&client, &RUNTIME, &state.root, &state.cancel, |n| {
                emit("downloading", n)
            })
            .await?;
            download::fetch(&client, &m.artifact, &state.root, &state.cancel, |n| {
                emit("downloading", RUNTIME.bytes + n)
            })
            .await?;
            if state.cancel.load(Ordering::Relaxed) {
                return Err("Загрузка отменена.".into());
            }
            emit("verifying", total);
            let root = state.root.clone();
            tauri::async_runtime::spawn_blocking(move || install_runtime(&root))
                .await
                .map_err(|e| e.to_string())??;
            tracing::info!(model = %id, "local STT download verified");
            Ok(())
        }
        .await;
        *state.progress.lock().unwrap() = None;
        let _ = app.emit("local-models-changed", ());
        result
    })
    .await
    .map_err(crate::i18n::error)
}

#[tauri::command]
pub async fn remove_local_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<LocalModels>>,
    id: String,
) -> Result<(), String> {
    (async {
        let _operation = state
            .operation
            .try_lock()
            .map_err(|_| "Дождитесь текущей операции с моделью.".to_string())?;
        let app_state = app.state::<crate::state::AppState>();
        {
            let conn = app_state.db.lock().map_err(|e| e.to_string())?;
            let profile = crate::commands::speech::read_profile(&conn, None)?;
            if profile.mode == crate::commands::speech::Mode::Local && profile.model == id {
                return Err("Сначала выберите другую модель или режим диктовки.".into());
            }
        }
        if id == super::moonshine::catalog::ID {
            state.moonshine.close().await;
            super::moonshine::remove(&state.moonshine.root)?;
            let _ = app.emit("local-models-changed", ());
            return Ok(());
        }
        let m = *model(&id)?;
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut engine = state.engine.lock().map_err(|e| e.to_string())?;
            if engine.as_ref().is_some_and(|e| e.model_id == id) {
                *engine = None;
            }
            receipt::invalidate(&state.root, m.id);
            for file in [
                m.artifact.file.to_string(),
                format!("{}.part", m.artifact.file),
            ] {
                match std::fs::remove_file(state.root.join(file)) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.to_string()),
                }
            }
            Ok::<_, String>(())
        })
        .await
        .map_err(|e| e.to_string())??;
        let _ = app.emit("local-models-changed", ());
        Ok(())
    })
    .await
    .map_err(crate::i18n::error)
}

pub struct LocalProvider {
    pub models: Arc<LocalModels>,
    pub id: String,
}
impl SttProvider for LocalProvider {
    async fn transcribe(
        &self,
        wav_bytes: Vec<u8>,
        _lang: Option<&str>,
    ) -> Result<Transcript, SttError> {
        let models = self.models.clone();
        let id = self.id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut reader =
                hound::WavReader::new(Cursor::new(wav_bytes)).map_err(|e| e.to_string())?;
            let spec = reader.spec();
            if spec.channels != 1
                || spec.sample_rate != 16_000
                || spec.bits_per_sample != 16
                || spec.sample_format != hound::SampleFormat::Int
            {
                return Err("Неподдерживаемый формат записи.".into());
            }
            let pcm: Vec<f32> = reader
                .samples::<i16>()
                .map(|s| s.map(|s| s as f32 / 32768.0))
                .collect::<Result<_, _>>()
                .map_err(|e| e.to_string())?;
            if pcm.len() > 10 * 60 * 16_000 {
                return Err("Запись слишком длинная для локальной диктовки.".into());
            }
            let inference = Instant::now();
            let (text, stats, prepare_ms, lock_wait_ms, backend) = models.recognize(&id, &pcm)?;
            let elapsed_ms = (inference.elapsed().as_millis() as u64).saturating_sub(prepare_ms);
            let audio_ms = (pcm.len() / 16) as u64;
            models.record_speed(audio_ms, elapsed_ms as f64 / audio_ms.max(1) as f64);
            let (_, threads) = models.wanted();
            tracing::info!(
                model = %id,
                backend = %backend,
                audio_ms,
                elapsed_ms,
                rtf = elapsed_ms as f64 / audio_ms.max(1) as f64,
                prepare_ms,
                lock_wait_ms,
                segments = stats.segments,
                mel_ms = stats.mel_ms as u64,
                encode_ms = stats.encode_ms as u64,
                decode_ms = stats.decode_ms as u64,
                threads = if threads == 0 { crate::diagnostics::stt_default_threads() as u32 } else { threads },
                "local STT completed"
            );
            Ok(Transcript { text })
        })
        .await
        .map_err(|e| SttError::Api(e.to_string()))?
        .map_err(SttError::Api)
    }
    async fn list_models(&self) -> Result<Vec<String>, SttError> {
        Ok(vec![self.id.clone()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_paths_cannot_escape_runtime_directory() {
        assert!(validate_entry(Path::new(&format!("{RUNTIME_DIR}/ggml.dll"))).is_ok());
        for path in [
            "../bad.dll",
            "/bad.dll",
            "other/bad.dll",
            "transcribe-native-windows-x86_64-cpu-vulkan/../../bad.dll",
            "C:\\bad.dll",
        ] {
            assert!(validate_entry(Path::new(path)).is_err(), "{path}");
        }
    }

    #[test]
    fn recommendation_prefers_ctc_and_a_discrete_card() {
        let gpu = recommend(false, true);
        assert_eq!(
            (gpu.model, gpu.device, gpu.kind),
            (
                "gigaam-v3-e2e-ctc",
                Device::Gpu,
                RecommendationKind::GraphicsCard
            )
        );
        let gpu_avx2 = recommend(true, true);
        assert_eq!(gpu_avx2.device, Device::Gpu);
        let cpu = recommend(true, false);
        assert_eq!(
            (cpu.device, cpu.kind),
            (Device::Cpu, RecommendationKind::Processor)
        );
        let slow = recommend(false, false);
        assert_eq!(
            (slow.model, slow.device, slow.kind),
            (
                "gigaam-v3-e2e-ctc",
                Device::Cpu,
                RecommendationKind::SlowProcessor
            )
        );
    }

    #[test]
    fn speed_hint_uses_recent_long_recordings_only() {
        let models = LocalModels::new(std::env::temp_dir());
        assert_eq!(models.engine_state().recent_rtf, None);
        models.record_speed(1_000, 5.0); // too short: ignored
        models.record_speed(4_000, 1.1);
        assert_eq!(
            models.engine_state().recent_rtf,
            None,
            "one sample is not enough"
        );
        models.record_speed(5_000, 1.0);
        models.record_speed(6_000, 1.2);
        assert_eq!(models.engine_state().recent_rtf, Some(1.1));
        for _ in 0..5 {
            models.record_speed(5_000, 0.1);
        }
        assert_eq!(
            models.engine_state().recent_rtf,
            Some(0.1),
            "only the last five count"
        );
        models.record_speed(5_000, f64::NAN);
        assert_eq!(models.recent_rtf.lock().unwrap().len(), 5);
    }

    #[test]
    fn engine_options_default_to_cpu_auto_and_round_trip() {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrations::apply_migrations(&mut conn).unwrap();
        assert_eq!(
            EngineOptions::read(&conn).unwrap(),
            EngineOptions::default()
        );
        let gpu = EngineOptions {
            device: Device::Gpu,
            threads: 4,
        };
        gpu.write(&conn).unwrap();
        assert_eq!(EngineOptions::read(&conn).unwrap(), gpu);
        assert!(EngineOptions {
            device: Device::Cpu,
            threads: 65
        }
        .write(&conn)
        .is_err());
        assert_eq!(EngineOptions::read(&conn).unwrap(), gpu);
        let parsed: EngineOptions =
            serde_json::from_str(r#"{"device":"gpu","threads":0}"#).unwrap();
        assert_eq!(parsed.device, Device::Gpu);
        assert!(serde_json::from_str::<EngineOptions>(r#"{"device":"tpu","threads":0}"#).is_err());

        let models = LocalModels::new(std::env::temp_dir());
        models.set_options(gpu);
        assert_eq!(models.wanted(), (true, 4));
        models.gpu_failed.store(true, Ordering::Relaxed);
        assert_eq!(models.wanted(), (false, 4));
        assert!(models.engine_state().gpu_fallback);
        models.set_options(gpu);
        assert_eq!(models.wanted(), (true, 4));
    }

    #[test]
    fn catalog_rejects_arbitrary_paths() {
        for id in ["../model", "C:\\model", "", "whisper"] {
            assert!(model(id).is_err());
        }
        for m in MODELS {
            assert!(m.artifact.url.starts_with("https://"));
            assert_eq!(m.artifact.sha256.len(), 64);
        }
    }

    /// Opt-in real native ABI/model smoke; no download or microphone access.
    /// GLAGOL_LOCAL_SMOKE_ROOT must hold the pinned runtime archive, both GGUFs
    /// and Sber's public example.wav. Run with --ignored --nocapture.
    #[tokio::test]
    #[ignore = "requires downloaded native runtime and GigaAM models"]
    async fn native_gigaam_smoke() {
        let root = PathBuf::from(std::env::var("GLAGOL_LOCAL_SMOKE_ROOT").unwrap());
        install_runtime(&root).unwrap();
        let models = Arc::new(LocalModels::new(root.clone()));
        let wav = std::fs::read(root.join("example.wav")).unwrap();
        for m in MODELS {
            let start = Instant::now();
            models.prepare(m.id).unwrap();
            eprintln!("{} load {:?}", m.id, start.elapsed());
            let provider = LocalProvider {
                models: models.clone(),
                id: m.id.into(),
            };
            for _ in 0..2 {
                let start = Instant::now();
                let result = provider.transcribe(wav.clone(), Some("ru")).await.unwrap();
                eprintln!("{} inference {:?}: {}", m.id, start.elapsed(), result.text);
                assert!(
                    result
                        .text
                        .chars()
                        .filter(|c| ('а'..='я').contains(c))
                        .count()
                        > 20
                );
            }
        }
    }
}
