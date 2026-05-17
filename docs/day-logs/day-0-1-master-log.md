# Glagol — Day 0-1 Master Log

**Period:** May 12, 2026 (single intense session)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Status at end of Day 1:** Sprint 0 complete, Tauri scaffold merged into main

---

## TL;DR

За одну сессию прошли от пустой идеи «как бы это бесплатно слушать длинные тексты на русском» до:
- Работающего Tauri 2 + React 19 + TypeScript desktop-приложения локально
- Полностью настроенного GitHub-репозитория с 8/8 community standards
- Проверенного end-to-end интеграционного pipeline'а с SaluteSpeech API (от OAuth до синтезированного WAV)
- Merge commit'а #1 в истории main

Главный технический риск проекта — TLS-сертификат НУЦ Минцифры — снят на самом раннем этапе. Это эзотерика, на которой многие российские интеграции буксуют неделями.

---

## Day 0 — Discovery and Foundation

### Цель дня

Решить вопрос: технически реализуема ли идея «open source Tauri-приложения для озвучки длинных русских текстов через SaluteSpeech» на текущем стеке, и заложить фундамент репозитория.

### Что было сделано

#### 1. Исследование и стратегия (research-phase)

- Глубокое исследование Tauri 2.x best practices 2025-2026
- Анализ конкурентов: Speechify ($139/год), NaturalReader ($99-199/год), Balabolka (free, но устаревший), SaluteSpeech App от Сбера (без библиотеки и кэша), Yandex Browser TTS
- Подтверждена ниша: «русскоязычный Speechify, бесплатный, с офлайн-кэшем длинных документов»
- Решено имя проекта: **Glagol** (старорусское «говорить»)
- Лицензия: **MIT**
- Копирайт-холдер: **Glagol Contributors**

#### 2. SaluteSpeech API — research

Изучены ключевые параметры (с подтверждением через developers.sber.ru):
- Бесплатный лимит **`SALUTE_SPEECH_PERS`**: 200 000 символов/мес синтеза (не «12 000 запросов», как было в первоначальном ТЗ — это была фактическая ошибка)
- Sync API: `POST /rest/v1/text:synthesize`, ≤ 4000 символов/запрос
- Async API: до 1 000 000 символов/запрос (только OPUS, без SSML)
- OAuth endpoint: `https://ngw.devices.sberbank.ru:9443/api/v2/oauth` (порт 9443!)
- Token lifetime: 30 минут
- Параллелизм: 5 потоков для физлиц (берём 3 для запаса)
- TLS: сертификат **НУЦ Минцифры РФ** — главный технический риск

#### 3. Tech stack — финальный выбор

| Слой | Решение |
|---|---|
| Framework | Tauri 2.x |
| Backend | Rust stable ≥1.77 |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind + shadcn/ui (для будущих спринтов) |
| State | Zustand (для будущих спринтов) |
| Package manager | pnpm |
| HTTP | reqwest + rustls + cert pin |
| Database | SQLite via tauri-plugin-sql |
| Secrets | keyring-rs (НЕ Stronghold — он deprecated) |
| PDF | pdfium-render |
| DOCX | docx-rs |
| Markdown | pulldown-cmark |
| Audio | hound (WAV), symphonia (decode) |

#### 4. Установка окружения на Windows 11

| Компонент | Установлено |
|---|---|
| Node.js | v22.20.0 |
| npm | 11.6.2 (для одной команды установки pnpm) |
| pnpm | 11.1.1 |
| Rust + Cargo | 1.95.0 |
| MSVC v143 Build Tools | через Visual Studio Installer (~5.46 GB) |
| Windows 11 SDK | 10.0.26100.7705 |
| Git | 2.51.0 (был установлен ранее) |
| Сертификат НУЦ Минцифры | установлен в системный truststore + сохранён локально |
| GitHub Desktop | установлен ранее, подключен к dimasiksuleyman-sudo |

#### 5. SaluteSpeech — end-to-end smoke test

Прошли полный pipeline через PowerShell, **без единой строчки кода**:

1. Регистрация на developers.sber.ru, создан проект SaluteSpeech API
2. Получен Authorization Key
3. **Инцидент:** случайно засветили Authorization Key в чате при первом curl. Ключ перевыпустили в консоли Сбера. Урок: показывать в чатах можно только access_token (живёт 30 мин), Authorization Key — никогда.
4. Установлен сертификат НУЦ Минцифры (главный риск проекта)
5. OAuth-запрос → получен access_token (длина ~1233 символа)
6. **Косяк с Git Bash:** в первой попытке curl от меня были многострочные команды с переносами через `\`, которые Git Bash «съедал» при копипасте. Решение: PowerShell + Invoke-RestMethod
7. POST на `text:synthesize` с UTF-8 кириллицей → получен `hello.wav` размером 215 532 байта
8. Файл проигрался: голос **Натальи** (`Nec_24000`), приятный, естественный

**Это означает:** весь критический pipeline работает на этой машине. Tauri-приложению остаётся только повторить это в Rust.

#### 6. GitHub репозиторий

- Создан репо `dimasiksuleyman-sudo/glagol`
- Public
- Без README/LICENSE/gitignore (по умолчанию — все наши кастомные)
- Подключен Claude Code через web (claude.ai/code) для будущей AI-разработки

#### 7. Community profile — 9 файлов

Заложили production-grade community-стандарт. Каждый файл — отдельный commit в main:

| # | Файл | Размер | Назначение |
|---|---|---|---|
| 1 | `LICENSE` | 1097 B | MIT, copyright Glagol Contributors |
| 2 | `.gitignore` | 4042 B | Кастомный для Tauri+Rust+pnpm, защита секретов |
| 3 | `README.md` | 13065 B | Bilingual RU+EN, бейджи, дисклеймер от Сбера |
| 4 | `SECURITY.md` | 13113 B | Threat model, secrets management, network boundaries |
| 5 | `CONTRIBUTING.md` | 12146 B | Branch/commit conventions, dev setup |
| 6 | `CODE_OF_CONDUCT.md` | 5400 B | Contributor Covenant 2.0 (GitHub template) |
| 7 | `CLAUDE.md` | 14710 B | Operating manual для AI-ассистентов |
| 8 | `.github/ISSUE_TEMPLATE/bug_report.yml` | — | Bilingual bug report form |
| 9 | `.github/ISSUE_TEMPLATE/feature_request.yml` | — | Bilingual feature request form |
| 10 | `.github/ISSUE_TEMPLATE/config.yml` | — | Отключает пустые issues, направляет в Discussions |
| 11 | `.github/PULL_REQUEST_TEMPLATE.md` | — | PR template с security checklist |

**Important issue, обнаружен и исправлен:** README.md был обрезан при первом коммите из-за технического сбоя — последняя видимая строка обрывалась на блок-схеме «Как это работает», без EN-секции и Disclaimer. Восстановлен в 2 частях через Edit, теперь полный (включая Disclaimer от Сбера на двух языках).

#### 8. Результат Day 0

- ✅ Все 8 галочек GitHub Community Standards зелёные
- ✅ Технический риск (cert + OAuth) снят
- ✅ Stack установлен
- ✅ Репо в production-grade состоянии

---

## Day 1 — Tauri Scaffold

### Цель дня

Создать рабочий Tauri 2 + React 19 + TS desktop-каркас в репо, запустить впервые, замерджить в main.

### Что было сделано

#### 1. Локальная синхронизация

- Подтверждён путь клона: `C:\Projects\glagol\`
- Видны все 9 файлов из Day 0 локально
- Изначально файлы не показывались — решилось повторным "Open with GitHub Desktop" из web

#### 2. Tauri scaffolding — through safe path

**Опасный момент:** команда `pnpm create tauri-app@latest .` в непустой папке спросила «Current directory is not empty, do you want to overwrite=y/n». Слово **overwrite** настораживало (могла быть как «дописать к существующему», так и «снести всё»).

**Решение:** ответили `n`, отменили. Создали подпапку `_tauri_temp/`, запустили скаффолдер там — безопасно, без вопросов про overwrite.

**Параметры скаффолдинга:**
- Package name: `glagol`
- Identifier: `app.glagol.desktop`
- Frontend language: TypeScript / JavaScript
- Package manager: pnpm
- UI template: React
- UI flavor: TypeScript

**Результат `_tauri_temp/`:**
```
.vscode/         (настройки редактора)
public/          (статические ассеты)
src/             (React frontend: App.tsx, main.tsx, vite-env.d.ts)
src-tauri/       (Rust backend: lib.rs, main.rs, Cargo.toml, tauri.conf.json)
.gitignore       (Tauri-овский, 277 B — игнорируем, у нас свой)
index.html
package.json
README.md        (Tauri-овский — игнорируем, у нас свой с дисклеймером)
tsconfig.json
tsconfig.node.json
vite.config.ts
```

#### 3. Перенос файлов наверх

PowerShell `Move-Item` для каждой папки и файла (кроме `.gitignore` и `README.md` от Tauri — наши более полные).

Финальная структура `C:\Projects\glagol\`:
- ✅ Наши 9 community-файлов (date 19:17)
- ✅ Новые Tauri-файлы (date 19:35)
- ✅ Без конфликтов

Папка `_tauri_temp/` удалена.

#### 4. pnpm install

73 пакета установлены. Зафиксированы версии:
- @tauri-apps/api 2.11.0
- @tauri-apps/cli 2.11.1
- @tauri-apps/plugin-opener 2.5.4
- react 19.2.6
- react-dom 19.2.6
- @vitejs/plugin-react 4.7.0
- typescript 5.8.3
- vite 7.3.3
- @types/react 19.2.14

**Warning:** `[ERR_PNPM_IGNORED_BUILDS] Ignored build scripts: esbuild@0.27.7` — pnpm 11 по умолчанию блокирует postinstall-скрипты как security-меру. Решено `pnpm approve-builds` → esbuild одобрен → постустановочный скрипт отработал за 485 ms → нативный бинарник для Windows скачан.

#### 5. First Tauri build — `pnpm tauri dev`

- Vite dev server запустился на http://localhost:1420
- Первая компиляция Rust: ~5 минут (скачано ~300 crate'ов Tauri и их зависимостей)
- Окно открылось: «Welcome to Tauri + React» с логотипами Tauri/Vite/React, полем для имени и кнопкой Greet
- Greet работает — frontend → Rust IPC проверен end-to-end

**Это означает:** весь стек (Tauri 2 + Rust 1.95 + React 19 + TypeScript + Vite + pnpm + MSVC + Windows SDK + WebView2) собран и работает на целевой машине.

При закрытии через Ctrl+C — косметическая ошибка `Failed to unregister class Chrome_WidgetWin_0. Error = 1412` — это нормально для WebView2 при принудительном завершении.

#### 6. Commit + PR + Merge

**Git workflow** через GitHub Desktop:
- Создана ветка `feat/initial-scaffold` от main
- GitHub Desktop предложил перенести 39 uncommitted changes на новую ветку — согласились
- Commit message: `feat: scaffold Tauri 2 + React 19 + TypeScript project`
- Опубликована ветка на GitHub
- Открыт **PR #1** на main
- **Сработал наш PR template из `.github/PULL_REQUEST_TEMPLATE.md`** — первый успех нашего community profile
- PR-описание заполнено по template'у (Description, Type of change, Testing steps, Security checklist, Final checklist)
- GitHub: `Able to merge. No conflicts with base branch`
- **Merge commit `2b5bb87`** создан в main
- Ветка `feat/initial-scaffold` удалена с GitHub

#### 7. Результат Day 1

- ✅ Tauri scaffold смерджен в main
- ✅ PR #1 в истории проекта
- ✅ Работающий локальный dev-environment
- ✅ Подтверждение, что весь стек собирается

---

## Финальная структура репозитория (после Day 1)

```
glagol/
├── .github/
│   ├── ISSUE_TEMPLATE/
│   │   ├── bug_report.yml
│   │   ├── config.yml
│   │   └── feature_request.yml
│   └── PULL_REQUEST_TEMPLATE.md
├── .vscode/
│   └── extensions.json          (Tauri-рекомендации)
├── docs/                        (будет создана для документов)
│   ├── day-logs/
│   └── roadmap/
├── public/
│   ├── tauri.svg
│   └── vite.svg
├── src/                         (React frontend)
│   ├── assets/
│   │   └── react.svg
│   ├── App.css
│   ├── App.tsx
│   ├── main.tsx
│   └── vite-env.d.ts
├── src-tauri/                   (Rust backend)
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/                   (иконки для разных платформ)
│   ├── src/
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── build.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── .gitignore                   (4042 B — наш кастомный)
├── CLAUDE.md
├── CODE_OF_CONDUCT.md
├── CONTRIBUTING.md
├── index.html
├── LICENSE
├── package.json
├── pnpm-lock.yaml
├── pnpm-workspace.yaml
├── README.md                    (13065 B — с дисклеймером)
├── SECURITY.md
├── tsconfig.json
├── tsconfig.node.json
└── vite.config.ts
```

---

## Ключевые решения и почему

| Решение | Альтернатива | Почему выбрано |
|---|---|---|
| Tauri 2 | Electron | Бандл в ~10× меньше, native performance |
| Rust backend | Node.js / Python | Memory safety, performance, security |
| keyring-rs | Stronghold plugin | Stronghold deprecated в Tauri v3, keyring использует Windows Credential Manager напрямую |
| pnpm | npm / yarn | Скорость, экономия диска через hardlinks |
| Embedded НУЦ-сертификат | Установка на машину пользователя | Zero-config для конечного пользователя |
| Sync API для MVP | Async API | Проще логика; async для документов >50k символов в Sprint 6 |
| WAV для кэша | OPUS / MP3 | Проще склейка через hound; перекодировка позже |
| SQLite + paths на диске | SQLite BLOBs | BLOBs тормозят, удваивают диск при апдейтах |
| MIT license | Apache-2.0 / MPL | Максимальная permissiveness для портфолио |
| Glagol Contributors | Личное имя | Открывает дверь будущим контрибьюторам |

---

## Чего НЕ было сделано (и почему — это правильно)

- ❌ Не подписали .exe code signing certificate — отложено до 50+ звёзд (SignPath.io Foundation для FOSS, бесплатно)
- ❌ Не настроили auto-updater Ed25519 ключи — будет в Sprint 5
- ❌ Не написали ни одного нашего Rust-модуля — это Sprint 1, начинается следующим
- ❌ Не настроили GitHub Actions CI — будет в Sprint 5
- ❌ Не подключили Tailwind + shadcn/ui — будет в Sprint 2 (UI)
- ❌ Не добавили реальные плагины tauri-plugin-sql, http, dialog — будут по мере необходимости в спринтах

---

## Lessons learned

### Технические

1. **Сертификат НУЦ Минцифры — главный риск российских интеграций.** Прошли его до написания кода — это сэкономит дни в Sprint 1.
2. **Authorization Key vs Access Token** — критическая разница. AK живёт постоянно, AT 30 минут. AK никогда не показывать в чатах/коммитах/скриншотах.
3. **Git Bash на Windows не любит многострочные команды с `\`** — copy-paste ломает их. Решение: PowerShell для всего, что сложнее одной строки.
4. **`pnpm create tauri-app` в непустой папке опасен** — слово overwrite намекает на снос. Безопасный путь: подпапка → перенос → удаление подпапки.
5. **pnpm 11 блокирует postinstall-скрипты по умолчанию** — security feature. Каждый раз надо `pnpm approve-builds` для легитимных пакетов.
6. **Первая Rust-компиляция Tauri — 5 минут** на современной машине. Это нормально. Cargo кэширует, дальше быстрее.
7. **Стандарт Conventional Commits + Bilingual community files** работает в международном open source-проекте; для русско-говорящих контрибьюторов важно RU-зеркало.

### Процессные

1. **Один шаг = один результат** — лучше длинных «делай 1-2-3 разом». Когда ты устаёшь, ошибки в многошаговых инструкциях накапливаются.
2. **Артефакты справа > длинные посты в чате** для файлов >100 строк — обрывы рендеринга чата частая проблема, артефакт стабилен.
3. **Bilingual everything** — README, SECURITY, CONTRIBUTING, issue templates, PR template — заложили двуязычность с первого дня. Поздно добавлять — переписывать.
4. **GitHub Desktop + PR с template'ом > push directly to main** — даже для одиночной разработки. Создаёт правильную дисциплину и красивую историю.
5. **Чек-листы перед коммитами** — `git status` обязательная пауза перед каждым крупным изменением. Один раз спасает от 30-минутного восстановления.

---

## Что дальше — Sprint 1 preview

**Цель Sprint 1:** написать полный SaluteSpeech Rust-клиент с тестами. UI пока остаётся дефолтным React-шаблоном.

**Запланированные модули:**

| Модуль | Назначение | Размер |
|---|---|---|
| `src-tauri/src/salute/auth.rs` | OAuth-флоу с cert pin + token cache | ~150 LOC |
| `src-tauri/src/salute/http.rs` | Shared reqwest client с retries | ~100 LOC |
| `src-tauri/src/salute/synthesize.rs` | Sync synthesis API wrapper | ~100 LOC |
| `src-tauri/src/text/chunker.rs` | Splitting text into ≤3500-char pieces | ~150 LOC |
| `src-tauri/src/audio/wav_join.rs` | Concatenating WAV chunks via hound | ~80 LOC |
| `src-tauri/src/secrets/keyring.rs` | Windows Credential Manager wrapper | ~50 LOC |
| `src-tauri/src/commands/credentials.rs` | Tauri commands: set/test credentials | ~80 LOC |
| `src-tauri/src/commands/synthesize.rs` | Tauri command: synthesize_text | ~100 LOC |

**Запланированные PR'ы:**
- PR #2: Embedded НУЦ-cert + http.rs + auth.rs + unit tests
- PR #3: synthesize.rs + integration test (real API call с тестовыми credentials)
- PR #4: chunker.rs + unit tests (edge cases: пустой текст, абзацы, длинные слова)
- PR #5: wav_join.rs + audio integrity test
- PR #6: secrets/keyring.rs + Tauri commands + минимальный UI для credentials

**Реалистичная оценка:** 2-3 сессии по 2-3 часа на разработчика с Claude Code-assistance.

**Главные риски Sprint 1:**
- Парсинг ошибок 401/429 SaluteSpeech и retry-стратегия
- UTF-8 чанкинг с защитой от резки слов на границах
- WAV header при склейке (sample rate, channels, bits)
- keyring-rs корректно работает на Windows (тестировать на чистой VM)

---

## Контакты и ссылки

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #1 (merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/1
- **Security:** SECURITY.md → security@d99s9 (Yandex)
- **SaluteSpeech docs:** https://developers.sber.ru/docs/ru/salutespeech
- **Tauri docs:** https://tauri.app/

---

*This log captures Day 0 and Day 1 in full. Subsequent days will have their own logs in `docs/day-logs/`.*
*Last updated: 2026-05-12*
