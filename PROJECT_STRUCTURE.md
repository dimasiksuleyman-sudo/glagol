# Структура Глагола 0.4.0

Глагол — Tauri 2 / Rust / React 19 / TypeScript приложение Windows x64 с
независимыми диктовкой и озвучкой. Код MIT; отдельно скачиваемая Silero v5.5 —
CC BY-NC-SA 4.0 для некоммерческого использования. GigaAM и офисный STT-сервер
не требуют Silero. Yandex SpeechKit v3 для коммерческой озвучки — будущий этап.

## Frontend

| Модуль | Ответственность |
|---|---|
| `src/main.tsx`, `App.tsx` | TtsProvider, маршруты и оболочка |
| `pages/Synthesize.tsx` | Текст/файлы, голос, прогресс, отмена, экспорт |
| `pages/Library.tsx`, `components/player` | Библиотека и плеер |
| `pages/Dictation.tsx` | Горячая клавиша, микрофон, история |
| `components/settings/DictationSection.tsx` | Local/server/cloud STT, модели и ключи |
| `components/settings/TtsSection.tsx` | Лицензия, скачивание/импорт, восстановление, удаление, голоса |
| `contexts/TtsContext.tsx` | Только локальное состояние TTS; без сети/OAuth при старте |
| `lib/tauri.ts`, `lib/tts.ts` | Типизированные IPC-команды |
| `lib/voices.ts` | Пять голосов Silero и названия старых голосов библиотеки |

Аудио не передаётся через IPC: плеер читает файл через ограниченный asset protocol.

## Rust

| Модуль | Ответственность |
|---|---|
| `lib.rs` | Tauri setup, состояние, команды, трей, hotkey, завершение |
| `state.rs` | SQLite и диктовка; TTS имеет отдельное состояние |
| `commands/synthesize.rs` | Общий последовательный pipeline, временный WAV, транзакция |
| `tts/mod.rs` | TtsBackend: provider, voices, limits, WAV result, cancellation |
| `tts/silero/mod.rs` | Установка, consent, состояние и время жизни worker |
| `tts/silero/catalog.rs` | Закреплённые HTTPS URL, размеры и SHA-256 |
| `tts/silero/runtime.rs`, `runtime-files.json` | Безопасная распаковка и проверка файлов |
| `tts/silero/worker.rs`, `worker.py` | Скрытый CPU-процесс, bounded stdio JSON, офлайн синтез |
| `commands/tts.rs` | Status/install/cancel/remove/preview; backend consent checks |
| `stt/local` | Необязательные GigaAM/transcribe.cpp; downloader переиспользуется TTS |
| `commands/speech.rs` | Независимые профили local/server/cloud |
| `stt/openai_compat.rs` | Совместимый HTTP STT-клиент |
| `dictation` | Recorder, resampling, readiness, hotkey session, clipboard/paste |
| `parser` | TXT/MD/DOCX/PDF, Pdfium |
| `text` | Общие URL/email/сокращения и разбиение; числа/латиница — Silero adapter |
| `db` | Append-only миграции и repository |
| `backup` | ZIP библиотеки, проверка и защитная копия перед восстановлением |
| `secrets/keyring.rs` | Раздельные STT-ключи; узкое удаление старого TTS-ключа |
| `logging.rs` | Tracing без текста, аудио и ключей |
| `paths.rs` | Единый источник путей |

## Потоки и данные

Озвучка: UI → проверка условий → проверка runtime/model → скрытый worker →
фрагменты PCM mono 24 кГц → временный WAV → запись документа и публикация файла.
При ошибке/отмене готовая запись не появляется. Длинное аудио не копится в RAM.
Worker ограничен двумя CPU-потоками, таймаутом 90 секунд и выгрузкой по простою
180 секунд; он завершается при выходе/падении родителя. HTTP-порт не открывается.

Диктовка: hotkey → подготовка микрофона → сигнал записи/уровень → отпускание →
локальное GigaAM или независимый server/cloud клиент → вставка/буфер → история,
если пользователь её включил. Она не использует настройки и условия Silero.

`%LOCALAPPDATA%\app.glagol.desktop\`:

- `glagol.db`: documents, api_usage, app_settings, dictations. Миграция 5 добавляет
  `documents.provider`; старые записи `salutespeech-legacy`, новые `silero`.
- `audio_cache`: готовые WAV; `previews` — временное предпрослушивание вне бэкапа.
- `speech_models`: GigaAM и нативный STT-runtime.
- `tts_models`: модель, архивы, runtime, локальное подтверждение условий.

Бэкапы содержат SQLite и готовые WAV. Модели, runtime и согласие Silero в них
не входят. Ключи — в Windows Credential Manager, не в SQLite/архивах.
При обновлении удаляется только `Glagol/salutespeech_auth_key`; ошибки ОС не
блокируют запуск. TLS-сертификат, OAuth, quota UI и клиент SaluteSpeech удалены.

## Сборка, проверки и документация

- `package.json`, `Cargo.toml`, `Cargo.lock`, `tauri.conf.json`: синхронная версия.
- `scripts/check-version.mjs`: проверка перед frontend build.
- `scripts/silero`: воспроизводимая подготовка и opt-in проверки модели/runtime.
- `installer/hooks.nsh`: пояснение о независимых компонентах; silent install
  не включает Silero. Версионированная иконка исправляет старые ярлыки Windows.
- `docs/local-tts-runtime.md` / `.ru.md`: источники, лицензии и измерения.
- `docs/local-dictation-runtime.md`: STT-артефакты и тесты.
- `docs/day-logs`: исторические отчёты; `CHANGELOG.md`: изменения выпусков.
- `README.md`, `USER_GUIDE.ru.md`, `USER_GUIDE.en.md`, `SECURITY.md`,
  `CONTRIBUTING.md`, `CLAUDE.md`: текущая пользовательская и техническая документация.

Проверки: cargo fmt/clippy/test, TypeScript, Vite и NSIS build. Нативные smoke-тесты
запускаются отдельно с проверенными локальными артефактами; обычный CI не скачивает
модели. Публикация релиза/PR не является частью локальной сборки.
