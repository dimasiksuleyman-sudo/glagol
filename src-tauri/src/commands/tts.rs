//! Optional TTS choices are independent of dictation.
use crate::{
    paths,
    tts::silero::{models::Models, Status},
};
use std::{path::PathBuf, time::Instant};
#[tauri::command]
pub fn tts_status(
    models: tauri::State<'_, Models>,
    provider: Option<String>,
) -> Result<Status, String> {
    (|| Ok(models.select(provider.as_deref())?.status()))().map_err(crate::i18n::error)
}
#[tauri::command]
pub async fn prepare_tts(
    app: tauri::AppHandle,
    models: tauri::State<'_, Models>,
    provider: Option<String>,
) -> Result<(), String> {
    (async {
        let state = models.select(provider.as_deref())?;
        let started = Instant::now();
        // Startup verification also holds this lock. A warm-up may wait for it;
        // it must not report a spurious operation failure on the first screen.
        let _operation = state.operation.lock().await;
        state
            .cancel
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let result = state.prepare(&app).await;
        match &result {
            Ok(()) => tracing::info!(
                elapsed_ms = started.elapsed().as_millis(),
                "local TTS warmup request completed"
            ),
            Err(_) => tracing::warn!(
                elapsed_ms = started.elapsed().as_millis(),
                "local TTS warmup request failed"
            ),
        }
        state.finish(&app, &result);
        result
    })
    .await
    .map_err(crate::i18n::error)
}
#[tauri::command]
pub async fn install_tts(
    app: tauri::AppHandle,
    models: tauri::State<'_, Models>,
    provider: Option<String>,
    accepted: bool,
    license_hash: String,
    model_path: Option<String>,
) -> Result<(), String> {
    (async {
        let state = models.select(provider.as_deref())?;
        let _operation = state
            .operation
            .try_lock()
            .map_err(|_| "Другая операция озвучки ещё выполняется.".to_string())?;
        state.acknowledge(accepted, &license_hash)?;
        models.close_workers().await;
        models.ru.clear_verification_cache().await;
        models.en.clear_verification_cache().await;
        let result = state.install(&app, model_path.map(PathBuf::from)).await;
        state.finish(&app, &result);
        result
    })
    .await
    .map_err(crate::i18n::error)
}
#[tauri::command]
pub fn cancel_tts(models: tauri::State<'_, Models>) {
    models.ru.stop();
    models.en.stop();
}
#[tauri::command]
pub async fn remove_tts(
    app: tauri::AppHandle,
    models: tauri::State<'_, Models>,
    provider: Option<String>,
) -> Result<(), String> {
    (async {
        let state = models.select(provider.as_deref())?;
        let _operation = state
            .operation
            .try_lock()
            .map_err(|_| "Сначала отмените текущую операцию озвучки.".to_string())?;
        state.close_worker().await;
        state.invalidate_verification().await;
        let result = models.remove_model_files(&state);
        state.finish(&app, &result);
        result
    })
    .await
    .map_err(crate::i18n::error)
}
#[tauri::command]
pub async fn preview_tts(
    app: tauri::AppHandle,
    models: tauri::State<'_, Models>,
    provider: Option<String>,
    voice: String,
) -> Result<String, String> {
    (async {
    let state = models.select(provider.as_deref())?;
    let started = Instant::now();
    let mut preparation_ms = 0;
    let mut synthesis_ms = 0;
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Другая операция озвучки ещё выполняется.".to_string())?;
    state
        .cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let result = async {
        if !state.model.voices.contains(&voice.as_str()) {
            return Err("Неизвестный голос.".into());
        }
        let preparation_started = Instant::now();
        let preparation_result = state.prepare(&app).await;
        preparation_ms = preparation_started.elapsed().as_millis();
        preparation_result?;

        let synthesis_started = Instant::now();
        let synthesis_result = async {
            let mut slot = state.worker.lock().await;
            use crate::tts::TtsBackend;
            let source = slot
                .as_mut()
                .unwrap()
                .synthesize_chunk(
                    if state.model.language == crate::preferences::Language::En { "Hello! This is Glagol. I can read your documents without an internet connection." } else { "Привет! Это Глагол. Теперь я могу озвучивать ваши тексты без интернета." },
                    &voice,
                )
                .await?
                .ok_or("Пустое аудио")?;
            let previews = paths::audio_cache_root(&app)?.join("previews");
            std::fs::create_dir_all(&previews).map_err(|e| e.to_string())?;
            let destination = previews.join("tts-preview.wav");
            std::fs::copy(source, &destination).map_err(|e| e.to_string())?;
            Ok(destination.to_string_lossy().into_owned())
        }
        .await;
        synthesis_ms = synthesis_started.elapsed().as_millis();
        synthesis_result
    }
    .await;
    if result.is_err() {
        state.close_worker().await;
        state.invalidate_verification().await;
    }
    state.finish(&app, &result.as_ref().map(|_| ()).map_err(Clone::clone));
    match &result {
        Ok(_) => tracing::info!(
            preparation_ms,
            synthesis_ms,
            elapsed_ms = started.elapsed().as_millis(),
            "local TTS preview completed"
        ),
        Err(_) => tracing::warn!(
            preparation_ms,
            synthesis_ms,
            elapsed_ms = started.elapsed().as_millis(),
            "local TTS preview failed"
        ),
    }
    result
}).await.map_err(crate::i18n::error)
}
