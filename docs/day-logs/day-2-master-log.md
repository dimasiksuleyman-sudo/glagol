# Glagol — Day 2 Master Log

**Period:** May 13, 2026 (single session)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 1 (SaluteSpeech client) — PR #2 of ~5
**Status at end of Day 2:** PR #2 merged. SaluteSpeech HTTP foundation + OAuth complete with 11 passing unit tests.

---

## TL;DR

За одну сессию (~3 часа) написали полнофункциональный фундамент SaluteSpeech-клиента на Rust:
- TLS pinning с встроенным сертификатом НУЦ Минцифры РФ
- OAuth 2.0 client credentials flow с thread-safe token caching
- Strongly-typed обработка ошибок (Auth / RateLimited / Api / Network / etc.)
- 11 unit-тестов с покрытием всех веток (включая mockito для HTTP-моков)
- Zero warnings в `cargo clippy -- -D warnings`, чистый `cargo fmt`

Темп: на верхней границе плана дорожной карты. PR #2 — самый сложный архитектурно из Sprint 1 (после него synthesize/chunker/wav_join будут заметно проще).

---

## Pre-session prep (вечером перед сессией)

- Сохранён `docs/day-logs/day-0-1-master-log.md`
- Сохранена дорожная карта `docs/roadmap/initial-roadmap-2026-05.md`
- Создан `docs/day-logs/kickoff-day-2.md` со спецификацией PR #2
- Создана папка `.scratch/` (gitignored) для личных заметок

## Day 2 — Sprint 1 PR #2 implementation

### Цель дня

Написать полный SaluteSpeech HTTP + OAuth foundation с тестами. Получить в результате один merged PR в main.

### Pre-implementation discussion (5 вопросов к спеке)

Перед написанием кода — критически прошлись по спеке из kickoff-day-2.md, нашли **5 проблем**:

1. **Mockito и HTTPS:** mockito отдаёт только HTTP. Если URL OAuth-эндпоинта зашит константой, моки не смогут перехватить запросы. **Решение:** `with_base_url()` конструктор для тест-инъекции.
2. **Имя крейта в integration test:** Tauri 2 scaffold создаёт `[lib] name = "glagol_lib"`. **Решение:** использовать `glagol_lib::salute::...` в integration tests.
3. **`pub mod errors` забыт в спеке `mod.rs`:** опечатка автора. **Решение:** добавили `pub mod errors;` в `mod.rs`.
4. **RqUID — caller-side через `http::new_rquid()`:** оставлен этот паттерн (явнее для debug, RqUID логируется при каждом запросе).
5. **Tracing setup:** макросы пишут в no-op без subscriber. Subscriber отложен до Sprint 5. Уровни: `debug!` на cache hit, `info!` на refresh, `warn!` на ошибку.

Это **критически важный этап** — без него мы бы запустили `cargo check` с failing тестами и потратили время на дебаг. Read-before-write.

### Implementation — 14 шагов

#### Шаги 1-2: Environment

- VS Code 1.120.0 установлен (вчера не было)
- Расширения: rust-analyzer, Even Better TOML, Tauri, GitLens
- Сертификат `russian_trusted_root_ca.cer` (RSA, 3 КБ, не ГОСТ) скопирован в `src-tauri/assets/russiantrustedca.pem`
- Проверен формат: `-----BEGIN CERTIFICATE-----` — PEM, конвертация не нужна
- Создана ветка `feat/salute-http-foundation`

#### Шаги 3-4: Cargo.toml + module structure

- Cargo.toml метаданные: description, authors=Glagol Contributors, license=MIT, repository, readme
- Добавлены deps: reqwest (rustls-tls,json,stream), tokio (full), serde, serde_json, thiserror, anyhow, uuid (v4), base64, chrono, tracing
- Dev-deps: mockito, tokio (full,test-util)
- `cargo check` — 3:47, all green
- Создана папка `src-tauri/src/salute/`

#### Шаг 5: `errors.rs` — error types

- `SaluteError` enum через `thiserror::Error` derive
- 8 вариантов ошибок: Network, Auth, Api, RateLimited, TokenExpired, Certificate, InvalidResponse, Internal
- `SaluteResult<T>` type alias
- `#[from] reqwest::Error` для автоматической конвертации в `Network`

#### Шаг 6: `mod.rs` — module declarations

- `pub mod errors;` (затем добавлены http и auth по мере создания)
- Doc-комментарии модульного уровня с архитектурным обзором

#### Шаг 7: Register in `lib.rs`

- Добавлено `pub mod salute;` в lib.rs
- `cargo check` — 4 секунды, all green

#### Шаги 8-9: `http.rs` — TLS pinning foundation

- `MINCIFRY_ROOT_CERT_PEM` через `include_bytes!("../../assets/russiantrustedca.pem")` — сертификат вкомпилирован в .exe
- `build_client()` — reqwest::Client с rustls-tls, add_root_certificate, timeouts 30s/10s, User-Agent
- `new_rquid()` — генерация UUID v4 для каждого запроса к Сберу
- 3 unit-теста: build_client_succeeds, new_rquid_is_unique, new_rquid_is_valid_uuid_v4
- **Косяк:** забыл добавить `pub mod http;` в mod.rs — `cargo test --lib salute::http` показал «0 tests». Решилось одной строкой. Урок: всегда `pub mod <name>;` ПЕРЕД созданием файла.
- Все 3 теста passing

#### Шаги 10a-10b: `auth.rs` — OAuth client

**Шаг 10a:** скелет с конструкторами, без логики (`get_token()` и `refresh_token()` через `todo!()`).
- `SaluteAuth` struct с RwLock<Option<CachedToken>>
- `new()` и `with_base_url()` конструкторы (последний для тестов)
- `CachedToken {access_token, expires_at_ms}`
- `TokenResponse` через `serde::Deserialize`
- 3 теста на конструкторы и пустой кэш — passing

**Шаг 10b:** реальная OAuth-логика.
- `get_token()` с двойной блокировкой: read-lock fast path, write-lock slow path
- `refresh_token()` — реальный POST с `Authorization: Basic`, `RqUID`, `scope=SALUTE_SPEECH_PERS`
- Обработка статусов: 401 → Auth, 429 → RateLimited, other 4xx/5xx → Api
- `REFRESH_BUFFER_MS = 60_000` — refresh за 60 сек до истечения (clock skew protection)
- Tracing: debug! на cache hit, info! на successful refresh, warn! на errors
- RqUID логируется при каждом запросе для debug

#### Шаг 11: Mockito unit tests

- 5 тестов OAuth flow с mockito local HTTP server
- `make_test_auth()` helper для wiring mockito URL в SaluteAuth
- Тесты: success, 401, 429, 500, **token-is-cached** (через `.expect(1)` mockito assertion)
- Все 8 тестов в auth.rs зелёные

#### Шаг 12: Quality gates

- `cargo fmt --check` — 9 mismatches от длинных-строк-в-чате. Исправлено `cargo fmt` (auto-fix).
- `cargo clippy -- -D warnings` — **0 warnings**. Идиоматичный код.
- `cargo test` — **11 passed, 0 failed, 0 ignored**.
- Doc-tests: 2 ignored (наши `/// ```ignore` примеры — норма).

#### Шаг 13-14: Commit + PR + Merge

- 8 changed files: russiantrustedca.pem, Cargo.lock, Cargo.toml, lib.rs, salute/{mod,errors,http,auth}.rs
- Commit: `feat(salute): http client + НУЦ cert + OAuth foundation`
- Squash-merge в main через GitHub web UI
- Ветка `feat/salute-http-foundation` удалена
- Local main pulled, локальная ветка удалена

### Day 2 result

- ✅ PR #2 merged в main
- ✅ 11 unit-тестов, все passing
- ✅ Zero clippy warnings
- ✅ TLS pinning работает (сертификат корректно парсится)
- ✅ OAuth flow покрыт тестами на все 4 типа HTTP-ответов
- ✅ Token caching доказан тестом с mockito `.expect(1)`

---

## Что мы построили — техническая сводка

### Файлы

```
src-tauri/
├── assets/
│   └── russiantrustedca.pem          ← embedded root CA
├── src/
│   ├── lib.rs                        ← pub mod salute;
│   └── salute/
│       ├── mod.rs                    ← module declarations
│       ├── errors.rs                 ← SaluteError enum (8 variants)
│       ├── http.rs                   ← build_client() + new_rquid()
│       └── auth.rs                   ← SaluteAuth + get_token + refresh_token
└── Cargo.toml                        ← updated deps + metadata
```

### API surface (что доступно вне модуля salute)

```rust
use glagol_lib::salute::http::{build_client, new_rquid};
use glagol_lib::salute::auth::SaluteAuth;
use glagol_lib::salute::errors::{SaluteError, SaluteResult};
```

### Test coverage

| Файл | Тестов | Покрытие |
|---|---|---|
| `salute/http.rs` | 3 | Cert loading, RqUID format, uniqueness |
| `salute/auth.rs` | 8 | Constructors (3), OAuth flows (5: success/401/429/500/cache) |
| **Total** | **11** | **All passing, 0 failed** |

### Key architectural decisions

| Решение | Альтернатива | Почему |
|---|---|---|
| `with_base_url()` для тестов | Hardcoded URL | Mockito работает только на HTTP, нужна инъекция |
| `[lib] name = "glagol_lib"` | `name = "glagol"` | Tauri 2.x scaffold default; конфликт с bin name on Windows |
| Caller-side `new_rquid()` | Helper типа `post_with_rquid()` | Явнее в логах, RqUID видно при дебаге; helper при 3+ повторениях |
| `REFRESH_BUFFER_MS = 60_000` | 0 (refresh точно в момент истечения) | Защита от clock skew + network latency |
| `tracing` без subscriber | `println!` или `log!` | Setup в Sprint 5; макросы пишут в no-op, дисциплина с дня 1 |
| RSA сертификат (не ГОСТ) | ГОСТ 2025 | rustls не поддерживает Russian-only crypto |

---

## Lessons learned

### Технические

1. **`include_bytes!` ищет путь относительно файла, где он написан** — `../../assets/...` от `src/salute/http.rs` = `src-tauri/assets/`. Запомнить.
2. **`pub mod <name>;` ПЕРЕД созданием файла модуля.** Иначе rust-analyzer не видит файл, тесты не запускаются (`0 tests`).
3. **`#[derive(Deserialize)]` + serde — магия.** Поля Rust-структуры должны совпадать с JSON по именам (или использовать `#[serde(rename = "...")]`).
4. **`RwLock` vs `Mutex`:** RwLock лучше для «много читателей, один писатель» паттернов (наш кэш токенов). Mutex проще, но тормозит fast-path под нагрузкой.
5. **`mockito::Server::new_async()` — async constructor.** Старые туториалы могут показывать `mockito::Server::new()` — это устарело.
6. **`cargo fmt` без `--check` автофиксит.** Привычка: после каждого редактирования rs-файла `cargo fmt`. Или включить «Format on Save» в VS Code (Settings → format on save).
7. **`-D warnings` в clippy — строгий режим.** Превращает все warnings в ошибки. Заложить в CI с Sprint 5.

### Процессные

1. **Read-before-write — критично.** 5 вопросов к спеке сэкономили час дебага.
2. **One-step-at-a-time реально работает.** За 14 микро-шагов прошли путь от пустой папки до merged PR. Ни одного отката, ни одной потерянной минуты.
3. **VS Code + rust-analyzer — must.** Notepad для Rust — это самосаботаж. Inline-ошибки сэкономили десятки минут на каждом файле.
4. **`mod tests { ... }` внутри файла > отдельный test-файл** для unit-тестов. Тесты рядом с кодом, легче поддерживать.
5. **Squash-merge для feature PR.** Линейная история main, легко читать `git log --oneline`, легко делать `git revert`.

---

## Что НЕ было сделано (и почему — это правильно)

- ❌ Integration test против реального Сбера (`#[ignore]` + env var) — не критично, мы вчера через PowerShell проверили end-to-end. Можно добавить в любой момент позже.
- ❌ Retry с экспоненциальной задержкой для 429/500 — добавим в PR #3 (synthesize), там это критичнее
- ❌ Подписка `tracing-subscriber` для реального логирования — Sprint 5
- ❌ Tauri-команды (`set_credentials`, `synthesize_text`) — PR #6
- ❌ UI для credentials — PR #6
- ❌ Sprint 1 deliverable end-to-end (текст → WAV) — после PR #5

---

## Progress against the roadmap

### Sprint 1 progress

```
[XXXXX-----------] 25% of Sprint 1
PR #2: ✅ DONE  — Auth + http foundation (this PR)
PR #3: ⏳ TODO  — synthesize.rs + integration test
PR #4: ⏳ TODO  — text/chunker.rs
PR #5: ⏳ TODO  — audio/wav_join.rs
PR #6: ⏳ TODO  — secrets/keyring + Tauri commands + minimal UI
```

### Overall project progress

```
Sprint 0 (Setup):     ✅ COMPLETE (Day 0-1)
Sprint 1 (Sber API):  ⏳ 25% (Day 2)
Sprint 2 (Storage):   ⏸️ NOT STARTED
Sprint 3 (Parsers):   ⏸️ NOT STARTED
Sprint 4 (Player):    ⏸️ NOT STARTED
Sprint 5 (CI + Polish): ⏸️ NOT STARTED
v0.1.0 release:       🎯 ~Day 18-20
```

### Time estimate to v0.1.0

| Phase | Sessions remaining | Calendar (1 session/day) |
|---|---|---|
| Sprint 1 rest (PR #3-#6) | 3-4 | Day 3-6 |
| Sprint 2 (Storage + UI) | 3-4 | Day 7-10 |
| Sprint 3 (Parsers) | 2-3 | Day 11-13 |
| Sprint 4 (Player + Cache) | 3 | Day 14-16 |
| Sprint 5 (Polish + CI/CD) | 2 | Day 17-18 |
| **v0.1.0 RELEASE** | | **Day 18-20** |

**Темп пока — на верхней границе плана.** PR #2 был самым сложным архитектурно (TLS, OAuth, caching, mockito). Дальнейшие PR проще.

---

## Reference links

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #2 (merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/2
- **Sprint 1 spec:** `docs/day-logs/kickoff-day-2.md`
- **Initial roadmap:** `docs/roadmap/initial-roadmap-2026-05.md`
- **Sber SaluteSpeech docs:** https://developers.sber.ru/docs/ru/salutespeech
- **mockito:** https://docs.rs/mockito/

---

*Day 2 captures the first real Rust code in the Glagol project.*
*Pickup point: `git checkout main && git pull` then start PR #3.*
*Last updated: 2026-05-13*
