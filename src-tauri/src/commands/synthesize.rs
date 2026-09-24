//! Sequential bounded TTS with transactional library persistence.
use crate::{
    db::{self, repository::DocumentRecord},
    paths,
    state::AppState,
    text::chunker::chunk_text,
    tts::{silero::models::Models, TtsBackend},
};
use serde::Serialize;
use std::{
    fs,
    path::Path,
    sync::{atomic::Ordering, Mutex},
    time::Instant,
};
use tauri::{ipc::Channel, Emitter};

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProgressEvent {
    Preparing,
    Chunked { total: usize },
    SynthesizingChunk { current: usize, total: usize },
    Joining,
}
#[tauri::command]
pub async fn synthesize_document(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    models: tauri::State<'_, Models>,
    provider: Option<String>,
    text: String,
    voice: String,
    on_progress: Channel<ProgressEvent>,
) -> Result<String, String> {
    (async {
        let tts = models.select(provider.as_deref())?;
        if text.trim().is_empty() {
            return Err("Введите текст для озвучивания.".into());
        }
        if text.chars().count() > 500_000 {
            return Err("Текст слишком большой: максимум 500 000 символов.".into());
        }
        if !tts.model.voices.contains(&voice.as_str()) {
            return Err("Неизвестный голос.".into());
        }
        let _operation = tts
            .operation
            .try_lock()
            .map_err(|_| "Другая операция озвучки ещё выполняется.".to_string())?;
        let started = Instant::now();
        let chars = text.chars().count();
        let mut preparation_ms = 0;
        let mut synthesis_ms = 0;
        tts.require_consent()?;
        tts.cancel.store(false, Ordering::Relaxed);
        let _ = on_progress.send(ProgressEvent::Preparing);
        let audio_root = paths::audio_cache_root(&app)?;
        let result = async {
            let preparation_started = Instant::now();
            let preparation_result = tts.prepare(&app).await;
            preparation_ms = preparation_started.elapsed().as_millis();
            preparation_result?;
            tts.stage(&app, "synthesizing", 0, 0);

            let synthesis_started = Instant::now();
            let synthesis_result = async {
                let mut slot = tts.worker.lock().await;
                let backend = slot.as_mut().ok_or("Движок не готов")?;
                synthesize_impl(&state.db, &audio_root, &text, &voice, backend, |e| {
                    let _ = on_progress.send(e);
                })
                .await
            }
            .await;
            synthesis_ms = synthesis_started.elapsed().as_millis();
            synthesis_result
        }
        .await;
        if result.is_err() {
            tts.close_worker().await;
            tts.invalidate_verification().await;
        }
        tts.finish(&app, &result.as_ref().map(|_| ()).map_err(Clone::clone));
        match &result {
            Ok(_) => tracing::info!(
                chars,
                preparation_ms,
                synthesis_ms,
                elapsed_ms = started.elapsed().as_millis(),
                "local TTS document synthesis completed"
            ),
            Err(_) => tracing::warn!(
                chars,
                preparation_ms,
                synthesis_ms,
                elapsed_ms = started.elapsed().as_millis(),
                "local TTS document synthesis failed"
            ),
        }
        if let Ok(id) = &result {
            let _ = app.emit(
                "synthesis-completed",
                serde_json::json!({"documentId": id, "charsAdded": 0}),
            );
        }
        result
    })
    .await
    .map_err(crate::i18n::error)
}

/// Backend seam allows offline tests and future providers without changing storage.
pub async fn synthesize_impl(
    db: &Mutex<rusqlite::Connection>,
    audio_root: &Path,
    text: &str,
    voice: &str,
    backend: &mut impl TtsBackend,
    progress: impl Fn(ProgressEvent),
) -> Result<String, String> {
    let caps = backend.capabilities();
    if !caps.voices.contains(&voice) {
        return Err("Неизвестный голос.".into());
    }
    let clean = if backend.capabilities().provider == "silero-en" {
        crate::text::preprocessor::preprocess_english(text)
    } else {
        crate::text::preprocessor::preprocess(text)
    };
    let chunks = chunk_text(&clean, caps.max_input_chars);
    if chunks.is_empty() {
        return Err("Введите текст для озвучивания.".into());
    }
    fs::create_dir_all(audio_root).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let partial = audio_root.join(format!("{id}.wav.part"));
    let destination = audio_root.join(format!("{id}.wav"));
    let result = async {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: caps.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(&partial, spec).map_err(|e| e.to_string())?;
        progress(ProgressEvent::Chunked {
            total: chunks.len(),
        });
        let mut samples = 0u64;
        for (i, chunk) in chunks.iter().enumerate() {
            progress(ProgressEvent::SynthesizingChunk {
                current: i + 1,
                total: chunks.len(),
            });
            if let Some(path) = backend.synthesize_chunk(chunk, voice).await? {
                let (next_writer, count) = tokio::task::spawn_blocking(move || {
                    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
                    if reader.spec() != spec {
                        return Err("Формат аудио не совпадает.".to_string());
                    }
                    let count = reader.duration() as u64;
                    for sample in reader.samples::<i16>() {
                        writer
                            .write_sample(sample.map_err(|e| e.to_string())?)
                            .map_err(|e| e.to_string())?;
                    }
                    Ok((writer, count))
                })
                .await
                .map_err(|e| e.to_string())??;
                writer = next_writer;
                samples += count;
            }
        }
        if samples == 0 {
            return Err("В тексте нет произносимых слов.".into());
        }
        if backend.is_cancelled() {
            return Err("Озвучка отменена.".into());
        }
        progress(ProgressEvent::Joining);
        writer.finalize().map_err(|e| e.to_string())?;
        let record = DocumentRecord {
            id: id.clone(),
            title: text.chars().take(60).collect::<String>().trim().into(),
            source_type: "paste".into(),
            char_count: text.chars().count() as i64,
            voice: voice.into(),
            provider: caps.provider.into(),
            speech_language: match caps.provider {
                "silero" => Some("ru".into()),
                "silero-en" => Some("en".into()),
                _ => None,
            },
            status: "ready".into(),
            error_message: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            audio_path: Some(format!("{id}.wav")),
            audio_duration_ms: Some((samples * 1000 / caps.sample_rate as u64) as i64),
        };
        let mut conn = db.lock().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        db::repository::insert(&tx, &record).map_err(|e| e.to_string())?;
        fs::rename(&partial, &destination).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(id.clone())
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(partial);
        let _ = fs::remove_file(destination);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        root: std::path::PathBuf,
        calls: usize,
        fail: bool,
    }
    impl TtsBackend for Fake {
        fn capabilities(&self) -> crate::tts::Capabilities {
            crate::tts::Capabilities {
                provider: "fake",
                voices: &["voice"],
                max_input_chars: 20,
                sample_rate: 24000,
            }
        }
        fn synthesize_chunk<'a>(
            &'a mut self,
            _: &'a str,
            _: &'a str,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Option<std::path::PathBuf>, String>>
                    + Send
                    + 'a,
            >,
        > {
            Box::pin(async move {
                self.calls += 1;
                if self.fail && self.calls == 2 {
                    return Err("cancelled".into());
                }
                let path = self.root.join("chunk.wav");
                let mut writer = hound::WavWriter::create(
                    &path,
                    hound::WavSpec {
                        channels: 1,
                        sample_rate: 24000,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    },
                )
                .unwrap();
                for _ in 0..2400 {
                    writer.write_sample(123i16).unwrap();
                }
                writer.finalize().unwrap();
                Ok(Some(path))
            })
        }
    }
    #[tokio::test]
    async fn joins_chunks_and_stores_provider_duration() {
        let root = std::env::temp_dir().join(format!("tts-pipeline-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = Mutex::new(db::test_connection());
        let mut backend = Fake {
            root: root.clone(),
            calls: 0,
            fail: false,
        };
        let id = synthesize_impl(
            &db,
            &root,
            "Первое предложение. Второе предложение. Третье предложение.",
            "voice",
            &mut backend,
            |_| {},
        )
        .await
        .unwrap();
        assert!(backend.calls > 1);
        let row = db::repository::get(&db.lock().unwrap(), &id)
            .unwrap()
            .unwrap();
        assert_eq!(row.provider, "fake");
        assert_eq!(row.audio_duration_ms, Some(backend.calls as i64 * 100));
        let reader = hound::WavReader::open(root.join(format!("{id}.wav"))).unwrap();
        assert_eq!(reader.duration(), backend.calls as u32 * 2400);
        drop(reader);
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn failure_leaves_no_library_row_or_partial_audio() {
        let root = std::env::temp_dir().join(format!("tts-cancel-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = Mutex::new(db::test_connection());
        let mut backend = Fake {
            root: root.clone(),
            calls: 0,
            fail: true,
        };
        assert!(synthesize_impl(
            &db,
            &root,
            "Первое предложение. Второе предложение.",
            "voice",
            &mut backend,
            |_| {}
        )
        .await
        .is_err());
        assert!(db::repository::list_all(&db.lock().unwrap())
            .unwrap()
            .is_empty());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
