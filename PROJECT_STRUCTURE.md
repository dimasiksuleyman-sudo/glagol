# Структура Глагола 0.5.0

Глагол — Tauri 2 / Rust / React 19 / TypeScript приложение Windows x64 с
независимыми EN/RU интерфейсом, диктовкой и озвучкой. Код MIT; Silero v5.5 RU и v3 EN —
CC BY-NC-SA 4.0 для некоммерческого использования. GigaAM и офисный STT-сервер
не требуют Silero. Yandex SpeechKit v3 для коммерческой озвучки — будущий этап.

## Frontend

| Модуль | Ответственность |
|---|---|
| `src/main.tsx`, `App.tsx` | PreferencesProvider, TtsProvider, onboarding, маршруты и оболочка |
| `src/i18n`, `contexts/PreferencesContext.tsx` | Типизированные EN/RU словари, fallback EN, Intl, live broadcast языка |
| `components/Onboarding.tsx`, `DictationSetupCard.tsx`, `*LanguageSwitch.tsx` | Первый выбор языка, необязательные установки, независимые переключатели |
| `pages/Synthesize.tsx` | Текст/файлы, голос, прогресс, отмена, экспорт |
| `pages/Library.tsx`, `components/AudioPlayer.tsx` | Библиотека и локализованный плеер |
| `pages/Dictation.tsx` | Горячая клавиша, микрофон, история |
| `components/settings/DictationSection.tsx` | Local/server/cloud STT, модели и ключи |
| `components/settings/TtsSection.tsx` | Лицензия, скачивание/импорт, восстановление, удаление, голоса |
| `contexts/TtsContext.tsx` | Только локальное состояние TTS; без сети/OAuth при старте |
| `lib/tauri.ts`, `lib/tts.ts` | Типизированные IPC-команды |
| `lib/voices.ts` | Пять RU / четыре EN голоса Silero и прежние имена библиотеки |

Аудио не передаётся через IPC: плеер читает файл через ограниченный asset protocol.

## Rust

| Модуль | Ответственность |
|---|---|
| `lib.rs` | Tauri setup, состояние, команды, трей, hotkey, завершение |
| `state.rs` | SQLite и диктовка; TTS имеет отдельное состояние |
| `preferences.rs`, `commands/preferences.rs` | Миграция старых неявных настроек, UI/STT/TTS языки, голоса, onboarding |
| `i18n` | Native messages на IPC/event границе; пользовательские аргументы сохраняются |
| `commands/synthesize.rs` | Общий последовательный pipeline, временный WAV, транзакция |
| `tts/mod.rs` | TtsBackend: provider, voices, limits, WAV result, cancellation |
| `tts/silero/mod.rs` | Установка, consent, состояние и время жизни worker |
| `tts/silero/models.rs` | RU/EN provider, отдельные consent/receipts и общие runtime/operation mutex |
| `tts/silero/catalog.rs` | Закреплённые HTTPS URL, размеры и SHA-256 |
| `tts/silero/runtime.rs`, `runtime-files.json` | Безопасная распаковка и проверка файлов |
| `tts/silero/worker.rs`, `worker.py` | Скрытый CPU-процесс, bounded stdio JSON, офлайн синтез |
| `commands/tts.rs` | Status/install/cancel/remove/preview; backend consent checks |
| `stt/local` | Необязательные GigaAM/transcribe.cpp; downloader переиспользуется TTS |
| `stt/moonshine` | EN каталог, staging, проверенные DLL, C ABI, скрытый режим того же EXE |
| `dictation/stream_resample.rs` | Потоковый mono 16 kHz вне callback, сохранение границ и flush хвоста |
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
После полной проверки Silero сохраняется техническая отметка на 30 дней с
метаданными ключевых файлов; при старении, изменении файлов или ошибке worker
выполняется новый полный контроль. Экран озвучки заранее поднимает worker.
Worker ограничен двумя CPU-потоками, таймаутом 90 секунд и выгрузкой после
15 минут простоя; он завершается при выходе/падении родителя. HTTP-порт не открывается.

Диктовка: hotkey → подготовка → сигнал записи → отпускание → один финальный текст
→ вставка/буфер/необязательная история. GigaAM/server/cloud сохраняют пакетный путь;
EN во время записи идёт через bounded очередь к Moonshine child. Переполнение
отменяет попытку без обрезанного текста. JSON ограничен 512 000 байт, timeout 90 с,
parent handle завершает child при выходе/падении; unload после 15 минут простоя.
До DLL проверяются все хеши; закреплённый ORT загружается абсолютным путём первым.
STT независим от mutex/моделей/условий Silero.

`%LOCALAPPDATA%\app.glagol.desktop\`:

- `glagol.db`: documents, api_usage, app_settings, dictations. Миграция 5 добавляет
  `documents.provider`; старые записи `salutespeech-legacy`, новые `silero`/`silero-en`.
  Миграция 6 добавляет nullable speech_language; прежние записи остаются NULL.
- `audio_cache`: готовые WAV; `previews` — временное предпрослушивание вне бэкапа.
- `speech_models`: GigaAM и нативный STT-runtime.
- `tts_models`: модель, архивы, runtime, локальное подтверждение условий.

Бэкапы содержат SQLite и готовые WAV. Модели, runtime и согласие Silero в них
не входят. Ключи — в Windows Credential Manager, не в SQLite/архивах.
При обновлении удаляется только `Glagol/salutespeech_auth_key`; ошибки ОС не
блокируют запуск. TLS-сертификат, OAuth, quota UI и клиент SaluteSpeech удалены.

## Сборка, проверки и документация

Рабочий контекст: [AGENTS.md](AGENTS.md) → [docs/STATUS.md](docs/STATUS.md) →
выбранная серия `docs/workstreams/`. Формат — [docs/context/README.md](docs/context/README.md),
процедуры — [docs/runbooks/README.md](docs/runbooks/README.md).
`project.json` и `state.json` — источники статусов; STATUS — генерируемый обзор.
`scripts/runbook-check.mjs` проверяет согласованность, `runbook-check.test.mjs`
проверяет сам гейт на временных Git-репозиториях. Продуктовые SQLite-данные
и пользовательские настройки в эту систему не входят.

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
