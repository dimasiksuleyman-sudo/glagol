# Glagol — Day 3 Master Log

**Period:** May 16, 2026 (two sessions, ~6 hours total)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 1 (SaluteSpeech client) — PR #3 + PR #4 of ~5
**Status at end of Day 3:** Sprint 1 **75% complete**. SaluteSpeech client functional + text chunker production-ready.

---

## TL;DR

За один день — **два смерженных PR** в main, **38 unit-тестов** в проекте:

- **PR #3 (Session 1, чат):** `salute/synthesize.rs` — sync API синтеза речи, 9 unit-тестов, паттерн с mockito повторён из PR #2.
- **PR #4 (Session 2, гибрид):** `text/chunker.rs` — первый PR с **Claude Code в worktree**. 18 unit-тестов, edge-cases на UTF-8 / merge logic / sentence detection.

Главное достижение Day 3 — **успешный пилот гибридного workflow** (architecture в чате, реализация в CC, review в чате). CC дважды нашёл проблемы в моей спеке до того, как написал код. Это правильное поведение AI-агента, не пассивная печатная машинка.

---

## Pre-session prep

- Сохранён `docs/day-logs/day-2-master-log.md` (вчера)
- Сохранён `docs/day-logs/kickoff-day-3.md` со спекой PR #3
- Локально main синхронизирован с `git pull origin main`
- VS Code открыт, rust-analyzer индексирует `glagol_lib`

---

## Session 1 — PR #3 (Synthesize), in chat

### Цель сессии

Реализовать `salute/synthesize.rs` — sync API клиент для `POST /rest/v1/text:synthesize`. После этого Glagol технически умеет превратить русский текст в WAV (имея `SaluteAuth` + `SynthesisClient`).

### Pre-implementation discussion (5 вопросов к спеке)

Перед написанием кода — критически прошлись по спеке из kickoff-day-3.md, нашли **5 проблем**:

1. **Content-Type: application/text** — нестандартно, но факт от Сбера. Решение: фиксируем буквально + `.match_header()` в тестах.
2. **retry_after_secs fallback** = `DEFAULT_RETRY_AFTER_SECS: u64 = 60`.
3. **Error body — String, без структурного парсинга.** Substring match в тестах.
4. **401 → TokenExpired** (НЕ Auth) — с doc-комментарием объясняющим разницу.
5. **Integration test inline** (рефакторинг tests/common/ когда будет 3-й test).

Бонус: **MINIMAL_WAV_HEADER** — 44 байта const для бинарных тестов.

### Implementation — 7 шагов

#### Шаги 1-2: Git + module declaration

- `git checkout -b feat/salute-synthesize`
- `pub mod synthesize;` в `salute/mod.rs` (урок Day 2 — **перед** созданием файла)

#### Шаг 3: Скелет synthesize.rs

- `VoiceId` enum: Natalia, Boris, Marfa, Taras, Alexandra, Sergey
- `#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]` — для будущей сериализации в React commands (Sprint 6)
- `as_api_id()` метод → "Nec_24000", etc.
- `SynthesisClient` struct с `new()` + `with_base_url()`
- `synthesize()` через `todo!()`
- 3 теста на скелет — passing

#### Шаг 4: Реальная HTTP-логика

- POST с query params (`format=wav16`, `voice=Nec_24000`)
- Headers: `Authorization: Bearer`, `Content-Type: application/text`, `RqUID`
- Тело: raw UTF-8 text
- **Парсинг Retry-After ДО консумирования body** (header теряется после `.text()`)
- Mapping: 401→TokenExpired, 429→RateLimited, others→Api
- Tracing: debug! на request, info! на success, warn! на ошибки

#### Шаг 5: 6 mockito-тестов

- `test_synthesize_success_returns_wav_bytes` — happy path с проверкой `.match_header("content-type", "application/text")` и binary round-trip
- `test_synthesize_401_returns_token_expired` — критично для retry-логики
- `test_synthesize_400_returns_api_error_with_body`
- `test_synthesize_429_uses_retry_after_header` (header=42)
- `test_synthesize_429_without_header_uses_default` (=60)
- `test_synthesize_500_returns_api_error`

#### Шаги 6-7: Quality gates + commit

- `cargo fmt --check` — 1 mismatch от длинной строки, исправлено `cargo fmt`
- `cargo clippy -- -D warnings` — 0 warnings
- `cargo test` — **20 passed** (3 http + 8 auth + 9 synthesize), 3 doc-tests ignored
- Commit: `feat(salute): sync synthesis API with 6 unit tests`
- Squash-merge как commit `9744a5d` в main

### Session 1 result

- ✅ PR #3 merged
- ✅ Sprint 1 на 50%
- ✅ 20 unit-тестов всего в проекте

---

## Session 2 — PR #4 (Chunker), гибрид CC + chat

### Цель сессии

Пилотный запуск Claude Code на изолированном модуле. Спецификация в чате со мной, реализация — в CC worktree, code review — в чате.

### Pre-implementation discussion (architecture phase)

#### Edge-cases matrix — 5 базовых вопросов

В чате согласовали правила:

1. **Empty string → `Vec::new()`** (не `vec![""]`)
2. **Резка по `\n\n+`** даже когда параграфы влезают в один чанк (для resume-playback и cache-friendly invalidation)
3. **`trim()` каждого чанка** — чисто в логах, чисто в БД
4. **`chars().count()` для длины** — лимит SaluteSpeech 4000 = символы (подтверждено через Алису + Google)
5. **Hard cut с `tracing::warn!`** при длинном слове >max_chars (не возвращаем Error — rare edge case URL на 4500 chars в PDF не должен блокировать озвучку)

#### Архитектурное решение — preprocessor отдельно

Пользователь предложил **заменять длинные URL на «смотрите URL в документе»**. Решено: **это работа `text::preprocessor` (Sprint 3)**, не chunker.

Принцип: **chunker делает одно дело** — режет текст. Preprocessing (URL replacement, abbreviations, HTML entities, Markdown stripping) — отдельный модуль в pipeline:

```
raw text → preprocessor → clean text → chunker → Vec<String> → synthesize → wav_join → final WAV
```

GitHub Issue про preprocessor открыт после merge (трекинг в Sprint 3).

#### 4 дополнительных edge-cases

- **Markdown code fences:** chunker не знает Markdown, режет как обычный текст. Preprocessor очистит.
- **Эмодзи на границе:** `chars()` уже обеспечивает безопасность графем.
- **Многоточия `…` vs `...`:** оба = sentence terminator. Look-ahead для `...`.
- **Аббревиатуры `т.е.`:** **A + C** — regex «терминатор + пробел + заглавная буква» (покроет 80%) + preprocessor дочистит в Sprint 3.
- **HTML entities из DOCX:** работа парсера в Sprint 3.

### CC pilot — 4 раунда диалога

Полный kickoff с edge-cases matrix + acceptance criteria записан в `docs/day-logs/kickoff-day-3-session-2.md`. Я отправил его в CC.

#### Раунд 1 — CC задал 5 вопросов до кода

1. Подтверждение merge tiny paragraphs
2. `...` vs `…` — обработать оба?
3. Whitespace для sentence boundary — только пробел или любой `is_whitespace()`?
4. Поведение `max_chars == 0`?
5. Hard cut — на границе `max_chars` или геометрическая середина?

**Это правильное поведение AI-агента** — не помчался писать, спросил уточнений. Все 5 ответил, плюс добавил критическое предупреждение про UTF-8 safety (`is_char_boundary`).

#### Раунд 2 — CC нашёл противоречие в моей спеке

После моих уточнений CC обнаружил: правило «short paragraph merge with next» **противоречит** test #5 (`"A.\n\nB." → 2 chunks`).

Решили **Variant B**: мерджим только если короткий параграф **не оканчивается на терминатор** (`.!?…`). Это эвристика «заголовок ≠ заканчивается точкой». Заголовки мерджатся, обычные параграфы — нет.

#### Раунд 3 — CC реализовал + 18 unit-тестов

- 504 строки в `chunker.rs`
- Каскадная архитектура: paragraph → sentence → word → hard_cut
- Look-ahead для `...` через `char_indices()` + ручной курсор
- UTF-8 safety везде через `char_indices()` / `chars()`
- `hard_cut` через `String::push(ch)` — никаких byte slicing'ов
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` — **38 passed** (20 прошлых + 18 новых)

#### Раунд 4 — code review в чате

Прошёл построчный review кода. Нашёл 3 находки:

| Finding | Severity | Решение |
|---|---|---|
| `split_by_paragraphs` сканирует по байтам | LOW | OK — ASCII whitespace single-byte в UTF-8, safe by definition. Документировать в PR. |
| `is_uppercase()` не работает для китайского/японского | LOW | Out of scope (Glagol = Russian TTS). Не меняем. |
| Множественные пробелы нормализуются | MEDIUM | Добавить doc-comment про intentional normalization в `chunk_paragraph` |

**APPROVE** с одним nitpick (doc-comment). CC сделал extra commit `docs(text): clarify whitespace normalization` в той же ветке.

### Session 2 result

- ✅ PR #4 merged (2 commits в squash)
- ✅ 18 unit-тестов в chunker
- ✅ Sprint 1 на 75%
- ✅ 38 unit-тестов всего в проекте
- ✅ GitHub Issue открыт про `text::preprocessor` для Sprint 3

### Post-merge cleanup

CC оставил worktree `.claude/worktrees/happy-wing-e70231/` на 4.4 GB на диске. Решили:

1. **`.gitignore` обновлён** — добавлен блок `.claude/` (commit `chore(gitignore): exclude Claude Code worktrees`)
2. **`git worktree remove --force`** — корректное удаление, не `rm -rf` (сохраняет git state)
3. **Habit для будущих PR через CC:** part of "definition of done" — убирать за собой

---

## Что мы построили — техническая сводка после Day 3

### Файлы

```
src-tauri/
├── assets/
│   └── russiantrustedca.pem
├── src/
│   ├── lib.rs                       ← pub mod salute, text
│   ├── salute/
│   │   ├── mod.rs                   ← 4 модуля
│   │   ├── errors.rs                ← SaluteError (8 variants)
│   │   ├── http.rs                  ← TLS-pinned client (Day 2)
│   │   ├── auth.rs                  ← OAuth с caching (Day 2)
│   │   └── synthesize.rs            ← sync synthesis API (Day 3 Session 1) ← NEW
│   └── text/                        ← NEW MODULE
│       ├── mod.rs                   ← pub mod chunker
│       └── chunker.rs               ← 504 lines, 18 tests (Day 3 Session 2) ← NEW
└── Cargo.toml                       ← unchanged since Day 2
```

### API surface (что доступно вне crate'а)

```rust
use glagol_lib::salute::http::{build_client, new_rquid};
use glagol_lib::salute::auth::SaluteAuth;
use glagol_lib::salute::synthesize::{SynthesisClient, VoiceId};
use glagol_lib::salute::errors::{SaluteError, SaluteResult};
use glagol_lib::text::chunker::{chunk_text, DEFAULT_MAX_CHARS};
```

### End-to-end сценарий (технически работает уже сейчас)

```rust
let client = build_client()?;
let auth = SaluteAuth::new(client.clone(), my_auth_key);
let synth = SynthesisClient::new(client);

let long_text = "очень длинный текст из документа...";
let chunks = chunk_text(long_text, DEFAULT_MAX_CHARS);

let token = auth.get_token().await?;
for chunk in chunks {
    let wav = synth.synthesize(&token, &chunk, VoiceId::Natalia).await?;
    // PR #5: склейка через wav_join
    std::fs::write(format!("chunk_{i}.wav"), wav)?;
}
```

После PR #5 (wav_join) — один файл WAV из любого текста. После PR #6 (keyring + Tauri commands + UI) — реальное приложение, которое пользователь может запустить.

### Test coverage

| Модуль | Тестов | Покрытие |
|---|---|---|
| `salute::http` | 3 | Certificate loading, RqUID generation |
| `salute::auth` | 8 | Constructors, OAuth flows, caching |
| `salute::synthesize` | 9 | Voice mapping, success path, 4 error paths, Retry-After |
| `text::chunker` | 18 | All edge cases incl. UTF-8 boundary safety |
| **Total** | **38** | **0 failed, 0 ignored** |

---

## Lessons learned — Day 3

### Технические

1. **`Retry-After` header нужно парсить ДО `.text()` или `.bytes()`** — иначе response потребляется и headers недоступны.
2. **Mockito `Matcher::AllOf(vec![...])`** работает с UrlEncoded для query params.
3. **Cascading architecture** (paragraph → sentence → word → hard_cut) — clean pattern для иерархической резки. Каждый уровень падает в следующий только для своего edge case.
4. **`std::mem::take(&mut buffer)`** — эффективный flush буфера без аллокации (вместо `clone()` + `clear()`).
5. **`include_bytes!`** для embedded data делает .exe больше, но не требует пользовательской установки сертификата. Tradeoff worth it for UX.

### Процессные

1. **Read-before-write — критично для CC.** Без 5 вопросов CC бы получил противоречивую спеку и наделал багов. **2 раунда дискуссии = 0 минут дебага.**
2. **CC должен останавливаться перед PR creation** — не перед commit + push. Push в feature-branch ≠ merge. Этот момент даёт окно для code review.
3. **Worktree cleanup — часть Definition of Done.** Добавили правило для всех будущих PR через CC.
4. **Co-author в commits (`dimasiksuleyman-sudo and claude`)** — правильная прозрачность для open source. История ясная.
5. **`kickoff-day-N-session-M.md`** — формат для дней с несколькими PR. Один файл = одна сессия.

### Про пилот Claude Code

**Эксперимент прошёл успешно.** CC показал:

- ✅ Не помчался писать без понимания (5 вопросов до кода)
- ✅ Нашёл противоречие в моей же спеке (Variant B)
- ✅ Code quality 8.5/10 (равно профессиональному middle Rust dev)
- ✅ Quality gates прошёл сам, без напоминаний
- ✅ Honest reporting (фактические цифры, диф, ответы на 5 вопросов про реализацию)
- ✅ Остановился перед PR creation по запросу
- ✅ Сделал extra commit для doc-fix по моему code review

**Решение:** Перейти на гибрид с CC для всех будущих PR в Sprint 2-5, **кроме security-критичных модулей** (keyring.rs, всё что касается secrets/tokens — это рукам в чате).

---

## Что НЕ было сделано (и почему — это правильно)

- ❌ Integration test для synthesize.rs против реального Сбера — не критично, mockito покрывает все ветки
- ❌ Concurrency limit через Semaphore для параллельного синтеза чанков — Sprint 4 (когда будет реальный playback)
- ❌ Retry с экспоненциальной задержкой для 429 — Sprint 4 (после первой реальной синтез-сессии увидим, нужно ли)
- ❌ Tauri commands для frontend — PR #6
- ❌ UI для credentials/synthesize — PR #6

---

## Progress against the roadmap

### Sprint 1 progress

```
[XXXXXXXXXXXXXXX-----] 75% of Sprint 1

PR #2: ✅ DONE — Auth + http foundation
PR #3: ✅ DONE — Synthesize sync API   ← Day 3 Session 1
PR #4: ✅ DONE — Text chunker          ← Day 3 Session 2
PR #5: ⏳ TODO — audio/wav_join.rs
PR #6: ⏳ TODO — keyring + Tauri commands + minimal UI
```

### Overall project progress

```
Sprint 0 (Setup):       ✅ COMPLETE (Day 0-1)
Sprint 1 (Sber API):    ⏳ 75% (Day 2-3) ← мы здесь
Sprint 2 (Storage):     ⏸️ NOT STARTED
Sprint 3 (Parsers):     ⏸️ NOT STARTED
Sprint 4 (Player):      ⏸️ NOT STARTED
Sprint 5 (CI + Polish): ⏸️ NOT STARTED
v0.1.0 release:         🎯 ~Day 15-18 (опережаем график на 2-3 дня)
```

### Velocity update

Темп **выше плана**. По исходной дорожной карте Sprint 1 был 4-5 дней — мы за 3 дня прошли 75%. Причины:

- Pre-discussion в чате (matrix edge-cases) сокращает время реализации
- CC пишет тесты в 2-3 раза быстрее ручной работы
- Паттерн с mockito повторяется — PR #3 был «по шаблону PR #2»
- Code review через chat быстрее, чем самостоятельная реализация

**Прогноз до v0.1.0:** Day 15-18 (вместо ранее заявленного Day 18-20).

---

## Reference links

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #3 (merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/3
- **PR #4 (merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/4
- **GitHub Issue про preprocessor:** созданный после PR #4 merge
- **Day 0-1 log:** `docs/day-logs/day-0-1-master-log.md`
- **Day 2 log:** `docs/day-logs/day-2-master-log.md`
- **Sprint 1 specs:** `docs/day-logs/kickoff-day-2.md`, `kickoff-day-3.md`, `kickoff-day-3-session-2.md`

---

*Day 3 marks the successful transition to hybrid AI workflow.*
*Pickup point: `git checkout main && git pull` then start PR #5.*
*Last updated: 2026-05-16*
