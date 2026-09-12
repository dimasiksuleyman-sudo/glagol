# Структура проекта Glagol

> Карта репозитория и обзор архитектуры: что это за приложение, из чего оно состоит,
> где что лежит и как части связаны между собой. Файл описывает **фактическое**
> состояние кода (версия `0.3.0`), а не план из [CLAUDE.md](CLAUDE.md).
>
> Для операционных правил ИИ-ассистентов см. [CLAUDE.md](CLAUDE.md), для контрибьюторов-людей —
> [CONTRIBUTING.md](CONTRIBUTING.md), для пользователей — [USER_GUIDE.md](USER_GUIDE.md).

Версию приложения меняем согласованно в `package.json`, `src-tauri/tauri.conf.json`,
`src-tauri/Cargo.toml` и записи `glagol` в `src-tauri/Cargo.lock`.
`scripts/check-version.mjs` проверяет совпадение перед frontend/Tauri-сборкой.
Новая функциональность повышает minor, исправления — patch; изменения версии
сопровождаются записью CHANGELOG. Уже установленная копия меняет версию только после обновления.

---

## 1. Что это за проект

**Glagol** — open source desktop-приложение для Windows с двумя голосовыми функциями:

1. **Озвучка текста (TTS).** Вставьте текст или загрузите документ (TXT, Markdown, DOCX, PDF) —
   приложение синтезирует речь качественными русскими нейросетевыми голосами через
   **SaluteSpeech API** от Сбера, склеивает результат в один WAV и кладёт его в **локальную
   библиотеку**, где документ можно переслушать, переименовать, экспортировать или удалить.
2. **Голосовой ввод / диктовка (STT).** Зажмите глобальный хоткей (по умолчанию
   `Ctrl+Shift+Space`), говорите, отпустите — распознанный текст **автоматически вставляется в
   активное окно** (Notepad, Chrome, Word, Telegram) или кладётся в буфер обмена. Распознавание
   работает на компьютере с загружаемой GigaAM v3 либо через **OpenAI-совместимый
   STT-эндпоинт** облачного сервиса или сервера организации.

### Ключевые свойства

| Свойство | Как реализовано |
|---|---|
| **Локальность данных** | Библиотека, метаданные, история диктовки — только на диске пользователя. Наружу уходит лишь то, что физически необходимо синтезировать/распознать |
| **Резюмируемая библиотека** | SQLite-метаданные + WAV-файлы на диске, воспроизведение потоком через Tauri Asset Protocol |
| **Работа из трея** | Окно закрывается в трей, чтобы глобальный хоткей диктовки продолжал работать; вторая копия приложения не запускается (single-instance lock) |
| **Секреты в OS-хранилище** | API-ключи — в Windows Credential Manager (`keyring-rs`), access-токены — только в RAM |
| **Нет телеметрии** | Ни аналитики, ни трекинга; логи пишутся локально и не содержат ни ключей, ни текста расшифровок |

### Деньги и лицензия

Само приложение **бесплатно и открыто** (MIT). За API пользователь платит провайдеру напрямую
по своему ключу: озвучка — по подписке SaluteSpeech, диктовка — по потреблению (копейки за час
аудио) либо на бесплатном тарифе провайдера вроде Groq.

Проект **независимый и не аффилирован с ПАО Сбербанк**. Целевая платформа — **Windows 10/11 x64**;
Linux-сборка существует только чтобы гонять quality gates в CI-контейнерах.

### Стек

Tauri 2.x (Rust backend) + React 19 + TypeScript (frontend), Tailwind CSS 4 + shadcn/ui,
SQLite через `rusqlite` + `rusqlite_migration`, `reqwest` + rustls, `tokio`, `cpal` (микрофон),
`enigo` + `arboard` (вставка текста), `pdfium-render` / `docx-rust` / `pulldown-cmark` (парсеры).
Пакетный менеджер — **pnpm**; сборка релиза — NSIS-инсталлятор.

---

## 2. Карта репозитория

```
glagol/
├── .github/            # CI-пайплайн, шаблоны issue/PR
├── .vscode/            # Рекомендуемые расширения редактора
├── docs/               # Скриншоты + master-логи по дням разработки
├── public/             # Статические ассеты Vite
├── src/                # Frontend — React + TypeScript (~2 100 строк)
├── src-tauri/          # Backend — Rust + конфигурация Tauri (~16 000 строк)
├── index.html          # HTML-точка входа Vite (грузится и главным окном, и оверлеем)
├── package.json        # Frontend-зависимости и скрипты
├── pnpm-lock.yaml      # Залоченные версии frontend (коммитится!)
├── pnpm-workspace.yaml # Конфиг pnpm-воркспейса
├── components.json     # Конфиг генератора компонентов shadcn/ui
├── tsconfig.json       # Конфиг TypeScript (+ tsconfig.node.json)
├── vite.config.ts      # Конфиг сборщика Vite (алиас @/ → src/)
└── *.md                # Корневая документация (см. ниже)
```

### Корневая документация

| Файл | Назначение |
|---|---|
| `README.md` | Двуязычная витрина проекта: что это, зачем, установка, стек, дорожная карта, дисклеймер |
| `USER_GUIDE.md` | Точка входа в руководство пользователя (выбор языка) |
| `USER_GUIDE.ru.md` / `USER_GUIDE.en.md` | Руководство пользователя; редактируются **всегда парой** |
| `CLAUDE.md` | Операционный мануал для ИИ-ассистентов: инварианты, справочник по API, рабочие соглашения |
| `CONTRIBUTING.md` | Руководство для контрибьюторов-людей |
| `CODE_OF_CONDUCT.md` | Кодекс поведения сообщества |
| `SECURITY.md` | Модель угроз и политика раскрытия уязвимостей |
| `CHANGELOG.md` | История пользовательских изменений (Keep a Changelog + SemVer) |
| `LICENSE` | Лицензия MIT |
| `PROJECT_STRUCTURE.md` | Этот файл — карта репозитория и обзор архитектуры |

---

## 3. `src/` — Frontend (React + TypeScript)

```
src/
├── components/
│   ├── ui/                     # Примитивы shadcn/ui: button, card, input, label, select,
│   │                           #   textarea, switch, radio-group, progress, separator,
│   │                           #   skeleton, alert-dialog, sonner (тосты)
│   ├── layout/
│   │   └── AppShell.tsx        # Каркас: боковое меню (Озвучить / Библиотека / Диктовка /
│   │                           #   Настройки) + <Outlet /> + общий стек тостов
│   ├── dictation/
│   │   ├── OverlayPill.tsx     # «Пилюля» поверх всех окон: состояние диктовки + индикатор уровня
│   │   ├── HotkeyEditor.tsx    # Перехват нажатой комбинации + ручной ввод, откат при конфликте
│   │   ├── DevicePicker.tsx    # Выбор микрофона (системный по умолчанию)
│   │   └── DictationHistory.tsx# Список последних расшифровок с раскрытием и копированием
│   ├── settings/
│   │   ├── DictationSection.tsx# Три режима STT, загрузка моделей, профили сервера и облака
│   │   ├── UsageSection.tsx    # Счётчик символов SaluteSpeech за текущий месяц
│   │   └── BackupSection.tsx   # Создание/восстановление резервной копии библиотеки
│   └── ScannedPdfDialog.tsx    # Предупреждение о сканированном (нетекстовом) PDF
├── contexts/
│   └── CredentialsContext.tsx  # Три-стейт статуса Authorization Key (unknown/valid/invalid)
├── lib/
│   ├── tauri.ts                # ЕДИНСТВЕННАЯ граница IPC: обёртки над всеми Tauri-командами,
│   │                           #   имена событий и inline-типы (отдельного types.ts нет)
│   ├── voices.ts               # Каталог голосов SaluteSpeech
│   ├── format.ts               # Форматирование дат/чисел/длительностей под ru-RU
│   ├── pluralize.ts            # Русские три формы множественного числа (включая 11–14)
│   └── utils.ts                # cn() и прочие мелкие хелперы
├── pages/
│   ├── Synthesize.tsx          # Ввод текста / выбор файла → выбор голоса → синтез, счётчик символов
│   ├── Library.tsx             # Список документов, плеер, скорость 0.5x–2x, переименование,
│   │                           #   экспорт в WAV, удаление
│   ├── Dictation.tsx           # Режим вставки, хоткей, микрофон, история, «надиктовано всего»
│   └── Settings.tsx            # Authorization Key + разделы Диктовка (STT) / Использование / Бэкап
├── App.tsx                     # Таблица маршрутов (react-router-dom), «/» → «/synthesize»
├── main.tsx                    # Точка входа: ветвление по label окна — main → App, overlay → OverlayPill
├── index.css                   # Tailwind 4 + глобальные стили и токены темы
└── vite-env.d.ts               # Типы окружения Vite
```

**Важное про два окна.** Главное окно и оверлей диктовки грузят **один и тот же бандл**
(`index.html`). `main.tsx` смотрит на `getCurrentWindow().label`: для `overlay` монтируется
только `OverlayPill` — без роутера, без AppShell, без контекста учётных данных, с прозрачным
фоном.

---

## 4. `src-tauri/` — Backend (Rust)

```
src-tauri/
├── src/
│   ├── main.rs                 # Точка входа бинарника; в release — windows_subsystem = "windows"
│   ├── lib.rs                  # Сборка Tauri-приложения: плагины (single-instance первым!),
│   │                           #   setup-хук (логи → БД → аудио-кэш → поток рекордера → оверлей →
│   │                           #   трей → хоткей → close-to-tray), регистрация 28 команд
│   ├── state.rs                # AppState: HTTP-клиент, кэш SaluteAuth, Mutex<Connection>,
│   │                           #   флаг валидности STT-ключа, хендл рекордера, фаза диктовки,
│   │                           #   oneshot-сигнал остановки, счётчик сессий, guard логов
│   ├── paths.rs                # Единственный источник путей ФС (аудио-кэш, файл БД)
│   ├── logging.rs              # Установка tracing-subscriber: dev → stdout, release → daily-rolling файл
│   │
│   ├── commands/               # Граница IPC. Каждая команда — тонкая обёртка над *_impl()
│   │   ├── credentials.rs      # set/test/delete Authorization Key (cache-first + force-bypass)
│   │   ├── synthesize.rs       # synthesize_document → document_id, прогресс + учёт символов
│   │   ├── storage.rs          # list/get_audio_path/delete/export/update_title
│   │   ├── file.rs             # read_and_parse_file (лимиты размера + диспетчер по расширению)
│   │   ├── backup.rs           # create/validate/restore_backup + relaunch_app
│   │   ├── usage.rs            # get_current_month_usage
│   │   ├── speech.rs           # Профили local/server/cloud, раздельные ключи, выбор backend
│   │   └── dictation.rs        # Настройки STT и диктовки, ключ, список микрофонов,
│   │                           #   хоткей, история, минуты распознавания
│   │
│   ├── salute/                 # Клиент SaluteSpeech (TTS)
│   │   ├── http.rs             # Общий HTTP-клиент: встроенный корневой сертификат, RqUID, ретраи
│   │   ├── auth.rs             # OAuth-флоу, кэш access-токена (30 мин, refresh за 60 с до истечения)
│   │   ├── synthesize.rs       # POST /rest/v1/text:synthesize
│   │   └── errors.rs           # SaluteError
│   │
│   ├── stt/                    # Клиент распознавания речи (диктовка)
│   │   ├── mod.rs              # Трейт SttProvider, Transcript, SttError, промпт-словарь
│   │   ├── openai_compat.rs    # POST /audio/transcriptions (multipart) + GET /models (проба)
│   │   ├── validation.rs       # Валидация base_url и прокси: https обязателен, http — только loopback
│   │   ├── local/              # Каталог, загрузка с продолжением, проверка SHA-256,
│   │   │                       #   transcribe.cpp C ABI, GigaAM CTC/RNNT на CPU в spawn_blocking
│   │   └── wav.rs              # Упаковка PCM в WAV в памяти + генератор «0.5 с тишины» для пробы
│   │
│   ├── dictation/              # Весь путь «хоткей → микрофон → текст в окне»
│   │   ├── mod.rs              # PcmAudio (16 кГц/моно/S16LE), RecorderHandle, DictationPhase,
│   │   │                       #   константы: кап 60 с, окно RMS 50 мс
│   │   ├── recorder.rs         # Выделенный OS-поток с cpal::Stream (не Send на WASAPI);
│   │   │                       #   одна очередь команд/сэмплов/ошибок, downmix + RMS + i16
│   │   ├── resample.rs         # Оффлайн-ресемплинг нативной частоты → 16 кГц (rubato)
│   │   ├── pipeline.rs         # Асинхронный сценарий: запись → отсев тишины/касаний →
│   │   │                       #   STT → доставка текста; watchdog на 60 с, события состояния
│   │   ├── insert.rs           # Чистый планировщик вставки + исполнитель через сеамы
│   │   │                       #   TextInserter (enigo) и ClipboardAccess (arboard)
│   │   └── session.rs          # Всё, что требует живого Tauri: обработчик хоткея, трей
│   │                           #   (idle ⇄ recording), позиция/видимость оверлея, close-to-tray
│   │
│   ├── parser/                 # Парсеры входных документов
│   │   ├── mod.rs              # ParsedDocument + ParseError + диспетчер по расширению
│   │   ├── txt.rs              # BOM → UTF-8 strict → фолбэк Windows-1251
│   │   ├── md.rs               # pulldown-cmark; блоки кода → «фрагмент кода»
│   │   ├── docx.rs             # docx-rust: параграфы + таблицы построчно
│   │   └── pdf.rs              # pdfium-render с динамической привязкой; сканы помечаются
│   │
│   ├── text/
│   │   ├── chunker.rs          # Нарезка под лимит API (цель ≤3500 символов, по границам предложений)
│   │   └── preprocessor.rs     # Гуманизация URL, email, аббревиатур, чисел и дат
│   │
│   ├── audio/
│   │   └── wav_join.rs         # Потоковая склейка WAV с нормализацией заголовка
│   │
│   ├── backup/                 # Zip-снапшот библиотеки
│   │   ├── create.rs           # manifest.json + glagol.db + audio_cache/
│   │   ├── restore.rs          # Восстановление с защитой от zip-slip и повторной проверкой (TOCTOU)
│   │   ├── mod.rs              # BackupManifest и типы
│   │   └── error.rs            # BackupError / BackupResult
│   │
│   ├── db/
│   │   ├── mod.rs              # init_database + test_connection() для тестов
│   │   ├── migrations.rs       # Раннер rusqlite_migration + схема (append-only!)
│   │   └── repository.rs       # CRUD-функции: документы, usage, настройки, диктовки
│   │
│   └── secrets/
│       └── keyring.rs          # Windows Credential Manager: сервис «Glagol»,
│                               #   SaluteSpeech, legacy STT, привязанные к адресу ключи cloud/server
├── assets/
│   └── russiantrustedca.pem    # Корневой сертификат НУЦ Минцифры (коммитится — нужен для TLS Сбера)
├── icons/                      # Иконки приложения + tray-idle.png / tray-recording.png
├── capabilities/
│   └── default.json            # Разрешения Tauri 2 для окон main и overlay
├── resources/                  # Пусто в git; сюда build.rs кладёт pdfium для NSIS-бандла
├── build.rs                    # Codegen Tauri + скачивание/кэширование Pdfium, PDFIUM_LIBRARY_PATH
├── Cargo.toml                  # Зависимости с развёрнутыми обоснованиями пинов версий
├── Cargo.lock                  # Залочен и коммитится!
└── tauri.conf.json             # Окно, CSP, asset protocol scope, NSIS-бандл, версия
```

---

## 5. Как это работает вместе

### 5.1 Общие правила

- Frontend **никогда** не ходит в сеть и не трогает файловую систему напрямую. Всё — через
  обёртки в `src/lib/tauri.ts`, каждая из которых зовёт Tauri-команду.
- Каждая команда возвращает `Result<T, String>`; внутренние ошибки (`thiserror`) переводятся
  в **дружественный русский текст на границе IPC**, а тесты продолжают проверять структурную
  английскую форму.
- Длительные операции репортят прогресс: `tauri::ipc::Channel<T>` для высокочастотного потока,
  `app.emit()` — для broadcast (`synthesis-completed`, `backup-progress`, `dictation-level`,
  `dictation-state`).
- Аудио-байты **не пересекают IPC**. Синтез возвращает `document_id`; воспроизведение идёт
  через asset protocol, экспорт — серверным `fs::copy`.

### 5.2 Поток озвучки (TTS)

```
Synthesize.tsx
   └─ read_and_parse_file → parser/{txt,md,docx,pdf}.rs        (если выбран файл)
   └─ synthesize_document
        ├─ text/preprocessor.rs   гуманизация URL/email/чисел/дат/аббревиатур
        ├─ text/chunker.rs        нарезка ≤3500 символов по границам предложений
        ├─ salute/auth.rs         OAuth: Authorization Key из keyring → access-токен в RAM
        ├─ salute/synthesize.rs   куски → WAV (через общий salute/http.rs)
        ├─ audio/wav_join.rs      склейка в один WAV с нормализацией заголовка
        ├─ запись файла в audio_cache/{uuid}.wav + строки в documents (одна транзакция)
        └─ commands/usage.rs      прибавка символов в api_usage (best-effort, не ломает синтез)
   → document_id → Library.tsx → get_audio_path → <audio> через asset protocol
```

### 5.3 Поток диктовки (STT)

```
Глобальный хоткей (session.rs)
   ├─ Pressed  → показать «Подготовка микрофона…», запустить pipeline
   │              recorder.rs: cpal-поток на своём OS-потоке → downmix → RMS (~20 Гц) →
   │              resample.rs → 16 кГц/моно/S16LE, жёсткий кап 60 с
   │              первый непустой пакет сохранён → recording в оверлее и трее
   ├─ Released → oneshot-сигнал → финализация клипа (при подготовке — отмена)
   └─ pipeline.rs
        ├─ отсев: короче 300 мс или RMS ниже порога 0.005 → отбрасываем, сети не касаемся
        ├─ stt/wav.rs → упаковка PCM в WAV в памяти
        ├─ выбранный backend:
        │    ├─ stt/local → GigaAM в памяти процесса, без сети
        │    └─ stt/openai_compat.rs → POST /audio/transcriptions (облако/офис, ключ из keyring)
        ├─ insert.rs → план вставки → буфер обмена (arboard) + Ctrl+V (enigo) в spawn_blocking,
        │              либо только буфер — в зависимости от режима вставки
        └─ репозиторий: минуты распознавания всегда; текст расшифровки — только если
                        история включена (по умолчанию выключена)
   → события dictation-state / dictation-level → OverlayPill.tsx
```

Watchdog в `pipeline.rs` гарантирует остановку, даже если событие `Released` потеряно; счётчик
поколений сессий в `AppState` защищает от гонки, когда новое нажатие приходит в момент
завершения предыдущей сессии.

### 5.4 Состояние и данные на диске

| Что | Где |
|---|---|
| Метаданные документов, usage, настройки, история диктовки | `%LOCALAPPDATA%\<bundle>\glagol.db` (SQLite) |
| Аудиофайлы | `%LOCALAPPDATA%\<bundle>\audio_cache\{uuid}.wav` |
| Загруженные модели, движок и незавершённые загрузки | `app_local_data_dir()/speech_models/`, через `paths::local_models_root` |
| Логи (release) | Системный каталог логов приложения, daily-rolling, хранится 7 файлов |
| Authorization Key SaluteSpeech и API-ключ STT | Windows Credential Manager, сервис `Glagol` |
| Access-токен SaluteSpeech | Только в RAM (`AppState`), 30 минут |

`<bundle>` — `app.glagol.desktop` для dev-сборки и `Glagol` для установленной release-сборки.
Пути к базе и аудио-кэшу резолвятся **только** через `paths.rs`; каталог логов берётся из
`app_log_dir` в `logging.rs`.

#### Схема SQLite (append-only миграции, версия в `user_version`)

| Таблица | Появилась | Содержимое |
|---|---|---|
| `documents` | v1 | Одна строка на озвученный документ: заголовок, тип источника, число символов, голос, статус, **относительный** путь к аудио, длительность |
| `api_usage` | v2 | Помесячный расход: `chars_used` (символы TTS) и `recognitions_seconds` (секунды диктовки) |
| `app_settings` | v3 | Key-value для несекретных настроек: `stt_base_url`, `stt_model`, `stt_language`, `stt_proxy`, `stt_provider`, `stt_insertion_mode`, `dictation_hotkey`, `dictation_device`, `dictation_history_enabled` |
| `dictations` | v4 | История диктовки (opt-in, максимум 10 записей в UI): время, длительность, текст, статус `pasted`/`clipboard`/`error` |

Дефолты настроек живут **в коде**, а не в миграции: отсутствующий ключ резолвится в дефолт, так
что дефолт можно поменять патчем без новой миграции. Текущие: хоткей `CmdOrCtrl+Shift+Space`,
провайдер `aitunnel` (`https://api.aitunnel.ru/v1`, модель `whisper-large-v3-turbo`, язык `ru`),
история — выключена, режим вставки — автовставка.

Режим хранится в `stt_mode`; облако сохраняет прежние `stt_*`, офисный профиль использует
`stt_server_*`, локальная модель — `stt_local_model`. Секреты облака и сервера разделены
и привязаны к адресу. Загруженные модели не включены в установщик и резервную копию библиотеки.
Версии, хеши и проверка нативного движка описаны в [local-dictation-runtime.md](docs/local-dictation-runtime.md).

---

## 6. Безопасность (что закреплено в коде)

- **Ключи не хранятся в коде, конфигах и переменных окружения** — только в OS-хранилище.
- **Трафик SaluteSpeech идёт через `salute/http.rs`** — там пиннинг корневого сертификата
  НУЦ Минцифры (вшит через `include_bytes!`), генерация `RqUID` и ретраи. Проверка сертификатов
  никогда не отключается.
- **Сетевой allowlist:** `ngw.devices.sberbank.ru:9443`, `smartspeech.sber.ru` — жёстко в CSP.
  STT-эндпоинт задаёт пользователь, но запрос уходит **из Rust**, а не из webview, поэтому CSP
  его не касается; вместо этого `stt/validation.rs` требует `https://` для внешних хостов и
  разрешает `http://` для loopback. Офисный режим в `commands/speech.rs` также допускает
  частные IP-адреса RFC1918/ULA; интерфейс предупреждает об отсутствии шифрования.
  Загрузка моделей идёт по HTTPS с фиксированных адресов GitHub/Hugging Face; перед
  использованием проверяются размер и SHA-256, архив движка не допускает выход за каталог.
- **Asset protocol scope узкий**, не `**`: только `$APPLOCALDATA/audio_cache/**`.
- **Логи не содержат ни ключей, ни текста расшифровок** — правило зафиксировано в `logging.rs`
  и проверяется тестом, сканирующим исходники.
- **Нет телеметрии, нет `dangerouslySetInnerHTML`, нет eval**, CSP `script-src 'self'`.
- **`unsafe` — только с комментарием `// SAFETY:`** (сейчас в коде его нет).
- Восстановление из архива защищено от zip-slip: каждая запись проверяется на `..`, ведущий
  слэш/бэкслэш и Windows-префикс диска — и на валидации, и повторно при распаковке.

Подробная модель угроз — в [SECURITY.md](SECURITY.md).

---

## 7. Сборка, качество, релиз

```powershell
pnpm install              # один раз после клона
pnpm tauri dev            # Vite + окно Tauri с hot reload
pnpm tauri build          # NSIS-инсталлятор в src-tauri/target/release/bundle/
```

Обязательные проверки перед пушем (они же — гейты в CI):

```powershell
pnpm tsc --noEmit         # типы frontend
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- Тесты Rust лежат **рядом с кодом** в `#[cfg(test)]`-модулях (сейчас их порядка 345). Чистые
  функции и «сеамы» (`SttProvider`, `TextInserter`, `ClipboardAccess`, `SampleSource`,
  `LevelSink`, `DictationEmitter`) позволяют прогонять весь путь диктовки без микрофона, сети,
  буфера обмена и Tauri-рантайма. Тестовый раннер для frontend сейчас не подключён — его гейт
  это `tsc --noEmit`.
- `.github/workflows/ci.yml` гоняет всё это на `windows-latest` плюс полную сборку
  `pnpm tauri build`; NSIS-инсталлятор кладётся в артефакты на 14 дней.
- На Linux для локальных гейтов нужны GTK/WebKit-библиотеки
  (`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`,
  `libasound2-dev` для cpal).
- Версия живёт в трёх местах: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `package.json`.

---

## 8. `.github/` и `docs/`

```
.github/
├── workflows/ci.yml            # Гейты качества + сборка инсталлятора
├── ISSUE_TEMPLATE/             # bug_report.yml, feature_request.yml, config.yml
└── PULL_REQUEST_TEMPLATE.md    # Билингвальный шаблон описания PR

docs/
├── day-logs/                   # Master-логи по дням/сессиям — подробная история решений,
│                               #   публикуются отдельным docs-PR после закрытия спринта
├── screenshots/                # Скриншоты для README (library/settings/synthesize-page.png)
└── images/                     # Прочие изображения документации
```

`docs/day-logs/` — это фактически архив архитектурных решений: почему выбрана та или иная
библиотека, какие версии пинились и по какой причине, какие баги ловились в рантайме. Если
непонятно, откуда взялось решение в коде — ответ, скорее всего, там.

---

*Соответствует состоянию репозитория на версии `0.2.1`. При изменении структуры обновляйте этот файл.*
