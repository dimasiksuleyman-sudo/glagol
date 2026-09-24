//! On-demand model storage and serialized native inference. No network during dictation.
pub mod catalog;
pub mod download;
mod engine;

use crate::stt::{SttError, SttProvider, Transcript};
use catalog::{model, MODELS, RUNTIME, RUNTIME_DIR};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
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
        }
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
        let mut slot = self.engine.lock().map_err(|e| e.to_string())?;
        if slot.as_ref().is_some_and(|e| e.model_id == id) {
            return Ok(());
        }
        download::verify(&self.root.join(m.artifact.file), &m.artifact).map_err(|_| {
            "Модель отсутствует или повреждена. Скачайте её в настройках диктовки.".to_string()
        })?;
        verify_runtime(&self.root)?;
        // Free old weights before allocating the new model on a 16GB machine.
        *slot = None;
        let now = Instant::now();
        *slot = Some(engine::Engine::open(
            &self.root.join(RUNTIME_DIR),
            &self.root.join(m.artifact.file),
            id,
        )?);
        tracing::info!(
            model = id,
            elapsed_ms = now.elapsed().as_millis(),
            "local STT model loaded"
        );
        Ok(())
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
            let mut reader = hound::WavReader::new(Cursor::new(wav_bytes)).map_err(|e| e.to_string())?;
            let spec = reader.spec();
            if spec.channels != 1 || spec.sample_rate != 16_000 || spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int { return Err("Неподдерживаемый формат записи.".into()); }
            let pcm: Vec<f32> = reader.samples::<i16>().map(|s| s.map(|s| s as f32 / 32768.0)).collect::<Result<_, _>>().map_err(|e| e.to_string())?;
            if pcm.len() > 10 * 60 * 16_000 { return Err("Запись слишком длинная для локальной диктовки.".into()); }
            models.prepare(&id)?;
            let now = Instant::now();
            let mut guard = models.engine.lock().map_err(|e| e.to_string())?;
            let engine = guard.as_mut().filter(|e| e.model_id == id).ok_or("Модель переключена. Повторите диктовку.")?;
            let text = engine.transcribe(&pcm)?;
            tracing::info!(model = %id, audio_ms = pcm.len()/16, elapsed_ms = now.elapsed().as_millis(), "local STT completed");
            Ok(Transcript { text })
        }).await.map_err(|e| SttError::Api(e.to_string()))?.map_err(SttError::Api)
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
