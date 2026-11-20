# Слежение за моделями (model-watch)

Цель: замечать улучшения и снятие моделей, от которых зависит Глагол, и
переходить на новые версии осознанно — с замером и выпуском. Основание:
пользователь 2026-11-20: «Локальные модели мы должны проверять сами время от
времени… может они улучшились?» и заметка про облачные модели.

Принцип: Глагол скачивает только закреплённые нами артефакты (URL + SHA-256),
поэтому сам ничего не обновляет. Новые версии замечаем мы, сравниваем на
одинаковых записях и выпускаем с новой версией Глагола.

## Что закреплено и где следить

| Компонент | Закреплено | Где следить | Файл в коде |
|---|---|---|---|
| GigaAM v3 e2e RNNT/CTC GGUF (RU диктовка) | HF `handy-computer/gigaam-v3-e2e-*-gguf` @ f719d70 / 075dff8 (2026-06-28) | HF commits; `ai-sage/GigaAM-v3`, `ai-sage/GigaAM-Multilingual` | `src-tauri/src/stt/local/` |
| transcribe.cpp (движок GigaAM) | v0.2.3, сборка cpu-vulkan | github.com/handy-computer/transcribe.cpp/releases | `src-tauri/src/stt/local/` |
| Moonshine Small Streaming EN | HF `moonshine-ai/moonshine-voice-assets` @ 0bf2f2e, `quantized_26_08_21` | HF tree `model/small-streaming-en`, `model/medium-streaming-en` | `src-tauri/src/stt/moonshine/catalog.rs` |
| moonshine-voice (библиотека) | PyPI 0.1.5 | pypi.org/project/moonshine-voice | `src-tauri/src/stt/moonshine/catalog.rs` |
| Silero TTS RU / EN | `v5_5_ru.pt`, `v3_en.pt` | models.silero.ai, github.com/snakers4/silero-models | `src-tauri/src/tts/silero/catalog.rs` |
| Qwen3-TTS (кандидат) | не закреплён | см. [tts-qwen](../tts-qwen/README.md) | — |
| Облачная ИИ-нормализация | по умолчанию `google/gemini-3.5-flash-lite` (RouterAI) | `GET /models` провайдера | `src-tauri/src/text/ai_normalization.rs` |

## Как проверяем

- **Ежемесячно, 1-го числа утром (Москва)** запланированная облачная сессия
  сверяет таблицу с источниками и присылает сводку «что вышло». Она ничего
  не обновляет и не коммитит.
- Кандидат на обновление → шаг серии: замер на одинаковых записях (скорость,
  ошибки, пунктуация, размер), решение пользователя, затем обновление каталога
  (URL + SHA-256) и выпуск.

## Шаги

- **W1. Замер Moonshine:** новая сборка Small `quantized_26_11_07` и библиотека
  0.1.6, а также Medium Streaming против текущей Small `26_08_21` на одних
  записях (скорость на ноутбуке и ПК, ошибки).
- **W2. GigaAM Multilingual Large CTC:** когда появится конвертация под
  transcribe.cpp — замер против GigaAM v3 e2e (ошибки, пунктуация, скорость,
  размер). По карточке модели: RU WER 3,0 % FLEURS; вероятно, без пунктуации.
- **W3. transcribe.cpp новее v0.2.3:** проверить выпуски и что изменилось
  (скорость, Vulkan), обновить при пользе.
