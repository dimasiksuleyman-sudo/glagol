//! Optional TTS choices are independent of dictation.
use crate::{
    paths,
    tts::silero::{Silero, Status},
};
use std::{path::PathBuf, sync::Arc};
#[tauri::command]
pub fn tts_status(state: tauri::State<'_, Arc<Silero>>) -> Status {
    state.status()
}
#[tauri::command]
pub async fn install_tts(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Silero>>,
    accepted: bool,
    license_hash: String,
    model_path: Option<String>,
) -> Result<(), String> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Другая операция озвучки ещё выполняется.".to_string())?;
    state.acknowledge(accepted, &license_hash)?;
    let result = state
        .inner()
        .install(&app, model_path.map(PathBuf::from))
        .await;
    state.finish(&app, &result);
    result
}
#[tauri::command]
pub fn cancel_tts(state: tauri::State<'_, Arc<Silero>>) {
    state.stop();
}
#[tauri::command]
pub async fn remove_tts(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Silero>>,
) -> Result<(), String> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Сначала отмените текущую операцию озвучки.".to_string())?;
    state.close_worker().await;
    let root = paths::tts_models_root(&app)?;
    let result = tokio::task::spawn_blocking(move || {
        if root.exists() {
            std::fs::remove_dir_all(root).map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?;
    state.finish(&app, &result);
    result
}
#[tauri::command]
pub async fn preview_tts(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<Silero>>,
    voice: String,
) -> Result<String, String> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| "Другая операция озвучки ещё выполняется.".to_string())?;
    state
        .cancel
        .store(false, std::sync::atomic::Ordering::Relaxed);
    let result = async {
        if !crate::tts::silero::VOICES.contains(&voice.as_str()) {
            return Err("Неизвестный голос.".into());
        }
        state.inner().prepare(&app).await?;
        let mut slot = state.worker.lock().await;
        use crate::tts::TtsBackend;
        let source = slot
            .as_mut()
            .unwrap()
            .synthesize_chunk(
                "Привет! Это Глагол. Теперь я могу озвучивать ваши тексты без интернета.",
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
    if result.is_err() {
        state.close_worker().await;
    }
    state.finish(&app, &result.as_ref().map(|_| ()).map_err(Clone::clone));
    result
}
