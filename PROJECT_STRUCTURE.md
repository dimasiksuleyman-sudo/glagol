# Структура проекта Glagol

> Карта репозитория: что где лежит и зачем. Файл описывает **фактическую** структуру
> (по состоянию на `v0.1.0-rc.7`), а не план из [CLAUDE.md](CLAUDE.md).
> Для операционных правил ИИ-ассистентов см. [CLAUDE.md](CLAUDE.md), для людей — [CONTRIBUTING.md](CONTRIBUTING.md).

## Что это за проект

**Glagol** — open source desktop-приложение для Windows, которое озвучивает длинные тексты
и документы на русском языке качественными нейросетевыми голосами и складывает аудио в
локальную библиотеку для повторного прослушивания. Работает поверх **SaluteSpeech API** от
Сбера (бесплатно до 200 000 символов в месяц). Проект независимый, распространяется под
лицензией MIT и **не аффилирован со Сбербанком**. Подробнее — в [README.md](README.md).

**Стек:** Tauri 2.x (Rust backend) + React 19 + TypeScript (frontend), Tailwind CSS + shadcn/ui,
SQLite (`rusqlite`), синтез через SaluteSpeech.

## Дерево верхнего уровня

```
glagol/
├── .github/            # GitHub-конфиги: CI, шаблоны issue/PR
├── .vscode/            # Рекомендуемые расширения редактора
├── docs/               # Документация, скриншоты, master-логи по дням
├── public/             # Статические ассеты Vite (иконки)
├── src/                # Frontend — React + TypeScript
├── src-tauri/          # Backend — Rust + конфиг Tauri
├── index.html          # HTML-точка входа Vite
├── package.json        # Frontend-зависимости и npm-скрипты
├── pnpm-lock.yaml      # Залоченные версии frontend (коммитится!)
├── pnpm-workspace.yaml # Конфиг pnpm-воркспейса
├── components.json     # Конфиг генератора компонентов shadcn/ui
├── tsconfig.json       # Конфиг TypeScript (+ tsconfig.node.json)
├── vite.config.ts      # Конфиг сборщика Vite
└── *.md                # Корневая документация (см. ниже)
```

## Корневая документация

| Файл | Назначение |
|---|---|
| `README.md` | Двуязычная витрина проекта: что это, зачем, установка, стек, дорожная карта, дисклеймер |
| `CLAUDE.md` | Операционный мануал для ИИ-ассистентов: инварианты, API-справка, рабочие соглашения |
| `CONTRIBUTING.md` | Руководство для контрибьюторов-людей |
| `CODE_OF_CONDUCT.md` | Кодекс поведения сообщества |
| `SECURITY.md` | Модель угроз, политика раскрытия уязвимостей |
| `CHANGELOG.md` | История изменений (батчами, с Sprint 5) |
| `LICENSE` | Лицензия MIT |
| `USER_GUIDE.md` | Руководство пользователя (агрегирует RU/EN версии) |
| `USER_GUIDE.ru.md` / `USER_GUIDE.en.md` | Руководство пользователя по языкам |
| `PROJECT_STRUCTURE.md` | Этот файл — карта репозитория |

## `.github/` — конфигурация GitHub

```
.github/
├── workflows/
│   └── ci.yml                  # CI-пайплайн: линтеры, тесты, сборка
├── ISSUE_TEMPLATE/
│   ├── bug_report.yml          # Шаблон баг-репорта
│   ├── feature_request.yml     # Шаблон предложения фичи
│   └── config.yml              # Настройки выбора шаблонов
└── PULL_REQUEST_TEMPLATE.md    # Шаблон описания PR
```

## `docs/` — документация

```
docs/
├── day-logs/           # Master-логи по дням/сессиям (публикуются после закрытия спринта)
├── screenshots/        # Скриншоты для README (library/settings/synthesize-page.png)
└── images/             # Прочие изображения документации
```

## `src/` — Frontend (React + TypeScript)

```
src/
├── components/
│   ├── ui/                     # Примитивы shadcn/ui (Button, Card, Input, Select, …)
│   ├── layout/
│   │   └── AppShell.tsx        # Каркас приложения: навигация + расположение страниц
│   ├── settings/
│   │   ├── BackupSection.tsx   # UI резервного копирования/восстановления
│   │   └── UsageSection.tsx    # UI счётчика израсходованных символов
│   └── ScannedPdfDialog.tsx    # Диалог-предупреждение о сканированном (нетекстовом) PDF
├── contexts/
│   └── CredentialsContext.tsx  # Три-стейт состояние наличия/валидности Authorization Key
├── lib/                        # Утилиты фронтенда, граница IPC
│   ├── tauri.ts                # Обёртки над Tauri-командами + inline-типы (единый источник IPC)
│   ├── voices.ts               # Каталог голосов SaluteSpeech
│   ├── format.ts               # Форматирование дат/чисел (ru-RU)
│   ├── pluralize.ts            # Русские три формы множественного числа
│   └── utils.ts                # Общие хелперы (cn и т.п.)
├── pages/                      # Компоненты-маршруты
│   ├── Settings.tsx            # Настройки: ключ, бэкап, счётчик использования
│   ├── Synthesize.tsx          # Ввод текста/загрузка файла → синтез
│   └── Library.tsx             # Библиотека: список, плеер, переименование, экспорт, удаление
├── App.tsx                     # Корневой компонент, роутинг
├── main.tsx                    # Точка входа React
├── index.css                   # Глобальные стили + Tailwind-директивы
└── vite-env.d.ts               # Типы окружения Vite
```

## `src-tauri/` — Backend (Rust + конфиг Tauri)

```
src-tauri/
├── src/
│   ├── main.rs                 # Точка входа бинарника (вызывает lib::run)
│   ├── lib.rs                  # Сборка Tauri-приложения: билд HTTP-клиента, setup, регистрация команд
│   ├── paths.rs                # Единственный источник путей ФС (корень аудио-кэша, путь к БД)
│   ├── state.rs                # AppState: Mutex<Connection> + кэш OAuth-токена
│   ├── commands/               # Tauri-команды, доступные фронтенду
│   │   ├── mod.rs
│   │   ├── credentials.rs      # set/test/delete ключа (cache-first + force-bypass)
│   │   ├── synthesize.rs       # synthesize_document → document_id
│   │   ├── storage.rs          # list/get_audio_path/delete/export/update_title
│   │   ├── file.rs             # read_and_parse_file (лимиты размера + диспетчер по расширению)
│   │   ├── backup.rs           # create/validate/restore_backup + relaunch_app
│   │   └── usage.rs            # get_current_month_usage (счётчик символов)
│   ├── salute/                 # Клиент SaluteSpeech
│   │   ├── mod.rs
│   │   ├── auth.rs             # OAuth-флоу со встроенным сертификатом
│   │   ├── synthesize.rs       # Эндпоинт /synthesize
│   │   ├── http.rs             # Общий HTTP-клиент: cert pinning + RqUID + ретраи
│   │   └── errors.rs           # Enum SaluteError
│   ├── parser/                 # Парсеры файлов
│   │   ├── mod.rs              # ParsedDocument + ParseError + диспетчер
│   │   ├── txt.rs              # BOM → UTF-8 → фолбэк Windows-1251
│   │   ├── md.rs              # pulldown-cmark; блоки кода → «фрагмент кода»
│   │   ├── docx.rs            # docx-rust: параграфы + таблицы
│   │   └── pdf.rs             # pdfium-render; сканированные PDF помечаются
│   ├── text/
│   │   ├── mod.rs
│   │   ├── chunker.rs         # Нарезка текста под лимит API (≤3500 символов)
│   │   └── preprocessor.rs    # Гуманизация URL/email/аббревиатур/чисел/дат
│   ├── audio/
│   │   ├── mod.rs
│   │   └── wav_join.rs        # Конкатенация WAV с нормализацией заголовка (стриминг)
│   ├── backup/                # Zip-снапшот библиотеки (Sprint 5c)
│   │   ├── mod.rs             # BackupManifest + типы
│   │   ├── create.rs          # Создание архива (manifest.json + glagol.db + audio_cache/)
│   │   ├── restore.rs         # Восстановление с защитой от zip-slip (TOCTOU-проверки)
│   │   └── error.rs           # BackupError / BackupResult
│   ├── db/                    # Слой SQLite
│   │   ├── mod.rs             # init_database + test_connection()
│   │   ├── migrations.rs      # Раннер rusqlite_migration + схема (append-only)
│   │   └── repository.rs      # CRUD-функции над DocumentRecord
│   └── secrets/
│       ├── mod.rs
│       └── keyring.rs         # Обёртка над Windows Credential Manager (keyring-rs)
├── assets/
│   └── russiantrustedca.pem   # Корневой сертификат НУЦ Минцифры (коммитится — нужен для TLS Сбера)
├── capabilities/
│   └── default.json           # Разрешения Tauri 2 (allowlist сети, asset protocol scope)
├── icons/                     # Иконки приложения (ico/icns/png под разные платформы)
├── resources/                 # Ресурсы, пакуемые в бандл
├── build.rs                   # Скрипт сборки: скачивает pdfium, пробрасывает PDFIUM_LIBRARY_PATH
├── Cargo.toml                 # Зависимости Rust и метаданные крейта
├── Cargo.lock                 # Залоченные версии Rust (коммитится!)
└── tauri.conf.json            # Конфиг Tauri: окно, бандл, bundle.identifier, CSP
```

## Как это работает вместе

1. **Frontend** (`src/`) рендерит три страницы — Настройки, Синтез, Библиотека — и общается с
   backend только через обёртки в `src/lib/tauri.ts`.
2. **Tauri-команды** (`src-tauri/src/commands/`) — единственная граница IPC; каждая возвращает
   `Result<T, String>`.
3. **Синтез:** текст проходит `text/preprocessor.rs` → `text/chunker.rs`, куски уходят в
   `salute/synthesize.rs` через общий `salute/http.rs`, результат склеивается в `audio/wav_join.rs`
   и пишется в аудио-кэш; метаданные — в SQLite (`db/`).
4. **Библиотека:** SQLite (`db/repository.rs`) — источник правды по метаданным; сами WAV-файлы
   лежат на диске в `audio_cache/{uuid}.wav`, воспроизводятся через Tauri Asset Protocol.
5. **Секреты:** Authorization Key хранится в Windows Credential Manager (`secrets/keyring.rs`),
   access-токен живёт только в оперативной памяти (`state.rs`).
6. **Пути** ко всем файловым локациям резолвятся в одном месте — `paths.rs`.

---

*Соответствует состоянию репозитория на v0.1.0-rc.7. При изменении структуры обновляйте этот файл.*
