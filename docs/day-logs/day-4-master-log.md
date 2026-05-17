# Glagol — Day 4 Master Log

**Period:** May 17, 2026 (two sessions, ~5 hours total)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 1 (SaluteSpeech client) — PR #5, #6a, hotfix #9 of 6
**Status at end of Day 4:** Sprint 1 **90% complete**. Backend pipeline closed (text → chunker → synthesize → wav_join). Secrets storage in place. UI + commands remain (PR #6b, planned for Day 5).

---

## TL;DR

День завершил backend часть Sprint 1. Три merged PR + один hotfix:

- **PR #5 / GitHub #7 (Session 1, гибрид):** `audio/wav_join.rs` — 297 LOC, 10 тестов. Pure-функция склейки WAV через `hound`. Финал backend pipeline'а.
- **PR #6a / GitHub #8 (Session 2, chat):** `secrets/keyring.rs` — 250 LOC, 7 тестов. Wincred wrapper для SaluteSpeech Authorization Key. Реализовали в чате руками (security-критично).
- **PR #6 split:** монолитный PR #6 из дорожной карты разрезали на **#6a (keyring, сегодня) + #6b (commands + UI, завтра)**. Security изоляция + меньший review surface.
- **Hotfix #9:** main был сломан после merge PR #8 — два файла модуля `secrets/` молча проигнорировались `.gitignore`. Hotfix восстановил.

Test count на main: **55 passed**, 0 failed. Backend полностью замкнут.

---

## Pre-session prep

- Сохранены `docs/day-logs/day-2-master-log.md` и `docs/day-logs/day-3-master-log.md`
- Подготовлен `docs/day-logs/kickoff-day-4.md` (спека PR #5 v2 после 5 правок в чате)
- main свежий, ветка для PR #5 готова

---

## Session 1 — PR #5 / GitHub #7 (WAV join)

### Pre-implementation discussion (5 находок в спеке)

Перед отправкой kickoff'а в CC прошлись через спеку, нашли 5 проблем — все исправлены до старта CC:

| # | Находка | Тяжесть | Решение |
|---|---|---|---|
| 1 | Suggested code не компилировался: `?` на `hound::Error` без `From`-impl | **БЛОКЕР** | Добавили `Codec(#[from] hound::Error)` в `WavJoinError` |
| 2 | Leftover `use crate::salute::errors::SaluteError;` в API contract | nit | Убрали |
| 3 | Memory estimate "300 MB peak" преувеличен в ~7× | nit | Скорректировано до ~42 MB peak (50K-char doc), streaming deferred до 500K+ |
| 4 | Test #2 и #10 описаны как "bit-for-bit", но hound может перестроить header | важно | Уточнено: «sample-level roundtrip, NOT raw bytes» |
| 5 | Code review checklist упоминал только 3 поля WavSpec, без sample_format | nit | Расширено до 4 полей через `WavSpec` PartialEq derive |

Этот этап (как и в Day 2-3) **сэкономил час дебага** в CC. Финальная спека ушла в CC с 5-вариантным `WavJoinError` и явными test rules.

### CC сессия — облачный, не локальный

Впервые использовали **облачный CC** (claude.ai/code) вместо локального worktree. Различия:

- Branch name жёстко назначен системой: `claude/implement-wav-join-2bmTM` (не наш `feat/audio-wav-join`)
- Нет `gh` CLI — PR создаётся через `mcp__github__create_pull_request`
- Нет worktree concept'а — контейнер эфемерный, self-destructs по inactivity
- `git worktree remove` неприменим — cleanup happens сам

Решение: оставили designated branch, использовали MCP tools для PR creation. CC корректно остановился перед `mcp__github__merge_pull_request` для ручного review.

### 3 раунда диалога CC

**Раунд 1 — CC задал 3 вопроса:**
- (a) Как проверить test #4 (header data chunk size)? Раздумывал между WavReader::len() vs raw byte parse. → A1 raw byte parse через `windows(4).position()` + `u32::from_le_bytes`. Test #4 теперь покрывает другой failure mode (забытый `writer.finalize()` или corrupt RIFF), не пересекается с test #3.
- (b) Test #6 — какой chunk index? → Single invalid chunk → `Invalid("chunk 0: ...")`, assertion `msg.starts_with("chunk 0:")`.
- (c) Cargo версия `"3.5"` vs `"3.5.1"`? → `"3.5"` как в спеке, Cargo подтянет latest 3.x.

**Раунд 2 — CC дополнил риски:** `Io` вариант почти dead code (hound оборачивает io::Error внутри hound::Error), clippy capture syntax `{i}` для format!. Принято.

**Раунд 3 — CC реализовал + 10 тестов + quality gates:**
- 297 LOC в `wav_join.rs`
- 10 тестов, все passing
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` — все clean
- Total 48 passing (38 prior + 10 new)

### Code review chat-side — 3 finding'а

| Finding | Severity | Решение |
|---|---|---|
| `pub use wav_join::{...}` re-export в `audio/mod.rs` создавал асимметрию с `salute/mod.rs` и `text/mod.rs` | MEDIUM | Удалили re-export, callers идут через `audio::wav_join::join_wav_chunks` |
| `FormatMismatch` использовал positional format `{:?}` vs capture `{i}` в Invalid | LOW | Deferred — оставили как есть |
| Test #10 с `i16::MIN`/`i16::MAX` boundary values — CC проактив | POSITIVE | Похвалили |

CC применил Finding 1 через `git commit --amend --no-edit` + `git push --force-with-lease`. Sanity check через web_fetch на GitHub diff подтвердил clean state.

### Merge

PR #5 → GitHub PR #7. Squash-merge через MCP. Container self-destructed по inactivity. **45 минут общего time** против plan'а 90 минут — почти в два раза быстрее, благодаря pre-discussion phase.

---

## Session 2 — PR #6a / GitHub #8 (keyring)

### Decision: split монолитного PR #6

Изначально PR #6 был задуман как «keyring + commands + minimal Settings UI». На практике решили **разрезать**:

- **PR #6a (Session 2, сегодня):** только `secrets/keyring.rs`. Security-критично → руки в чате, не CC.
- **PR #6b (Day 5):** commands + UI. ~400-700 LOC → CC, 2-3 сессии.

Аргументы за split:
1. **Security isolation** — Day 3 master log зафиксировал: «keyring руками в чате, не через CC». Split удерживает это.
2. **Review surface** — 180 LOC keyring + 400-700 LOC commands/UI = слишком много для одного review.
3. **Скорость** — keyring простой, мы его выкатили за ~2.5 часа, дальше PR #6b отдельной фокусной задачей.

### Pre-implementation discussion (7 вопросов к API)

Перед написанием кода прошлись через 7 архитектурных вопросов:

| Q | Тема | Решение |
|---|---|---|
| Q1 | Service name + username | `"Glagol"` / `"salutespeech_auth_key"` (brand-readable в Credential Manager UI) |
| Q2 | Singleton vs multi-profile | **Singleton.** Мульти-аккаунты против TOS Сбера — бан. Если пользователь хочет больше — купит 1M символов за 180₽. |
| Q3 | Pre-encoded base64 vs split ID+Secret | **Pre-encoded.** Sber console уже выдаёт готовый base64, юзеру не надо комбинировать. |
| Q4 | Error surface | **Structured.** 3 варианта: `NotFound`, `Backend(String)`, `Internal(String)`. Callers различают first-run от platform failure. |
| Q5 | Free functions vs struct | **Free functions.** С mock-фичей не нужна инъекция. |
| Q6 | Module path | `secrets/keyring.rs` (как в дорожной карте). |
| Q7 | Test coverage | **7 тестов** (set/get/delete happy + nonexistent + overwrite + empty + long key). |

### Реализация — 2 итерации после API surprises

**Surprise #1 — `features = ["mock"]` не существует в keyring 3.x.**

Моя первая рекомендация в Cargo.toml была:
```toml
[dependencies]
keyring = "3"

[dev-dependencies]
keyring = { version = "3", features = ["mock"] }
```

`cargo check` выдал:
```
package `glagol` depends on `keyring` with feature `mock` but
`keyring` does not have that feature.
available features: apple-native, async-io, ..., windows-native, ...
```

Root cause: в keyring **2.x** mock был feature flag. В **3.x** API переделали:
- Default feature set убрали (надо явно указывать backend)
- Mock backend **встроен** в основной крейт, доступен через `keyring::mock` без features
- Features теперь = OS backends (`windows-native`, `apple-native`, etc.)

**Critical bonus issue:** без `features = ["windows-native"]` production использует mock backend как default! Тихий data loss — Authorization Key не сохраняется между запусками.

Фикс:
```toml
[dependencies]
keyring = { version = "3", features = ["windows-native"] }
```

Mock dev-dep удалён целиком.

**Surprise #2 — Mock state per-Entry, не global.**

После фикса features тесты компилировались, но **4 из 7 падали:**
- `test_set_then_get_returns_value` — set работает, get сразу после возвращает None
- `test_overwrite_replaces_value`, `test_delete_existing_returns_ok`, `test_very_long_key_is_accepted` — все падали на втором обращении к Entry

Прошедшие тесты — те, что не делали set+get через **разные** Entry-объекты (например `test_get_without_set` или `test_delete_nonexistent`).

Root cause из docs.rs mock module:
> «There is no persistence other than **in the entry itself**, so getting a password before setting it will always result in a NoEntry error.»

В 2.x mock использовал shared HashMap. В 3.x state живёт **внутри** `Entry`. Мой `set_in()` создавал Entry, делал set_password, Entry drop'ался при выходе из функции — state потерян.

Фикс — рефакторинг internal API:
- Internal helpers принимают `&Entry`: `set_with(&entry, key)`, `get_with(&entry)`, `delete_with(&entry)`
- Public API создаёт Entry внутри через `auth_key_entry()` helper, потом вызывает internal
- В тестах: каждый тест создаёт ОДИН Entry через `test_entry()` и переиспользует на set/get/delete

В production это behavior-neutral: Wincred всегда резолвит `Entry::new(SERVICE, USER)` на ту же OS-запись. Split нужен только тестам.

### Verification

После двух фиксов:
- `cargo fmt --check` — clean
- `cargo clippy --all-targets -- -D warnings` — clean
- `cargo test` — **55 passed, 0 failed, 0 ignored** (48 prior + 7 new)

### Merge

PR #6a через GitHub Desktop как PR #8 на GitHub. Я не делал full paste-back review — пользователь сам прогнал acceptance criteria и замержил. Для security-критичного модуля это допустимо потому что архитектурная фаза (Q1-Q7 + 2 round'а surprises) была подробная.

### Hotfix #9 — silent file drop

После merge PR #8 пользователь заметил: **локально папка `src-tauri/src/secrets/` есть, в репо нет.** PR #8 на GitHub содержал только 3 файла (Cargo.toml, Cargo.lock, lib.rs) из 5 заявленных.

Root cause — `.gitignore` строка 65:
```
secrets/
```

Без leading slash → матчит **любую** папку `secrets` в дереве, включая наш Rust-модуль `src-tauri/src/secrets/`. GitHub Desktop при commit'е молча проигнорировал два файла. PR review (мой) этого не поймал, потому что я доверял output `git diff --stat` вместо проверки GitHub Files tab после merge.

Состояние main после PR #8 было **сломанным**: `pub mod secrets;` в `lib.rs` ссылался на несуществующие файлы. Свежий clone провалил бы `cargo check`.

Hotfix через PR #9:
- `.gitignore`: добавлены explicit negation patterns:
  ```
  !src-tauri/src/secrets/
  !src-tauri/src/secrets/**
  ```
- Восстановлены 2 файла: `mod.rs`, `keyring.rs`

3 файла изменено, 267 строк добавлено. Squash-merge как `eca8924` на main.

---

## Текущая структура репозитория (после Day 4)

```
src-tauri/
├── assets/
│   └── russiantrustedca.pem            (Day 2)
├── src/
│   ├── lib.rs                          ← +pub mod secrets;
│   ├── main.rs                         (unchanged from scaffold)
│   ├── salute/                         (Day 2-3)
│   │   ├── mod.rs
│   │   ├── errors.rs
│   │   ├── http.rs
│   │   ├── auth.rs
│   │   └── synthesize.rs
│   ├── text/                           (Day 3)
│   │   ├── mod.rs
│   │   └── chunker.rs
│   ├── audio/                          ← NEW (Day 4 Session 1)
│   │   ├── mod.rs
│   │   └── wav_join.rs
│   └── secrets/                        ← NEW (Day 4 Session 2)
│       ├── mod.rs
│       └── keyring.rs
└── Cargo.toml                          ← +hound 3.5, +keyring 3 (windows-native)
```

### API surface (что доступно вне crate'а после Day 4)

```rust
use glagol_lib::salute::http::{build_client, new_rquid};
use glagol_lib::salute::auth::SaluteAuth;
use glagol_lib::salute::synthesize::{SynthesisClient, VoiceId};
use glagol_lib::salute::errors::{SaluteError, SaluteResult};
use glagol_lib::text::chunker::{chunk_text, DEFAULT_MAX_CHARS};
use glagol_lib::audio::wav_join::{join_wav_chunks, WavJoinError};
use glagol_lib::secrets::keyring::{
    set_auth_key, get_auth_key, delete_auth_key,
    KeyringError, KeyringResult,
};
```

### End-to-end сценарий (технически работает после Day 4)

```rust
// User configures (PR #6b will add UI for this)
set_auth_key("base64_auth_key_from_sber_console").unwrap();

// Synthesize a long document
let auth_key = get_auth_key().unwrap().expect("not configured");
let client = build_client().unwrap();
let auth = SaluteAuth::new(client.clone(), auth_key);
let synth = SynthesisClient::new(client);

let long_text = std::fs::read_to_string("document.txt").unwrap();
let chunks = chunk_text(&long_text, DEFAULT_MAX_CHARS);

let mut wav_chunks = Vec::new();
let token = auth.get_token().await.unwrap();
for chunk in chunks {
    let wav = synth.synthesize(&token, &chunk, VoiceId::Natalia).await.unwrap();
    wav_chunks.push(wav);
}

let final_wav = join_wav_chunks(&wav_chunks).unwrap();
std::fs::write("output.wav", final_wav).unwrap();
```

После PR #6b — это всё через UI и Tauri commands, без ручной композиции.

### Test coverage после Day 4

| Модуль | Тестов | Покрытие |
|---|---|---|
| `salute::http` | 3 | Cert loading, RqUID generation |
| `salute::auth` | 8 | Constructors + OAuth flows + caching |
| `salute::synthesize` | 9 | Voice mapping + success + 4 error paths + Retry-After |
| `text::chunker` | 18 | All edge cases incl. UTF-8 safety |
| `audio::wav_join` | 10 | Empty/single/multi-chunk + format check + roundtrip + boundary samples |
| `secrets::keyring` | 7 | Set/get/delete + nonexistent + overwrite + validation + long values |
| **Total** | **55** | **All passing, 0 failed** |

---

## Lessons learned — Day 4

### Технические

1. **`hound::WavSpec` PartialEq derives 4 fields, не 3.** Format equality covers `channels`, `sample_rate`, `bits_per_sample`, `sample_format` одной проверкой. Спека изначально упоминала только 3 поля — потенциальная регрессия если ручной field-by-field check вместо PartialEq.

2. **`hound::Error` не `std::io::Error`.** Они разные типы, `#[from] std::io::Error` НЕ покрывает hound errors. В `thiserror` enum нужны отдельные variants для каждого.

3. **`writer.finalize()` обязательно ПЕРЕД `output.into_inner()`.** Без этого WAV header не пересчитает data chunk size — файл будет битый. Поймает test #7 (re-read output via WavReader).

4. **`hound` rewrites WAV headers on write.** Поэтому byte-identity input vs output для single-chunk случая НЕ гарантирована — нужно сравнивать на уровне samples и WavSpec, не raw bytes.

5. **keyring-rs 3.x: BREAKING CHANGES vs 2.x:**
   - Default feature set убран. Без explicit OS backend (`windows-native`) production использует mock → silent data loss.
   - Mock больше не feature flag, встроен в основной крейт.
   - **Mock state per-Entry, не global.** Тесты должны держать один Entry на тест и переиспользовать. Паттерн «unique service name per test» из 2.x не работает.

6. **`include_bytes!` пути относительно файла.** Запомнил с Day 2, в audio модуле не было embedded data, в secrets тоже — но Day 5 при добавлении новых embedded assets в Tauri config надо помнить.

7. **`!negation` в `.gitignore` работает.** Чтобы un-ignore subpath под более общим правилом — `!path/` + `!path/**` (двойной pattern: первый — для папки, второй — для содержимого).

### Процессные

1. **Pre-implementation discussion критически важна — paid off дважды.** На PR #5 поймали 5 проблем в спеке до CC (включая compile блокер). На PR #6a поймали 2 keyring 3.x surprises только потому, что архитектурная фаза была подробная и я мог быстро адаптироваться. Без pre-discussion — оба раза ушло бы значительно больше дебага.

2. **Имена модулей сверять с `.gitignore` БЕФОРЕ закладки в спеку.** Я выбрал имя `secrets/` потому что оно в дорожной карте Day 0-1, не проверил конфликт с типичным `.gitignore`-правилом. Это создало hotfix PR #9. Урок: для любого нового top-level имени папки — `git check-ignore` перед commit'ом первого файла.

3. **Post-merge sanity check обязателен.** После любого merge — открыть Files tab на GitHub и подтвердить, что diff = local diff. `git push` ≠ «файлы в репо», если gitignored — push молча скипает. Добавляю в чек-лист для всех будущих PR.

4. **`git status` paste-back перед commit.** На review-этапе я просил `git diff --stat`, но не `git status`. Status показал бы untracked файлы. Добавляю в чек-лист.

5. **Облачный CC принципиально не отличается от локального** в нашем workflow. Branch name назначен системой — не проблема. `gh` отсутствует — есть MCP. Worktree cleanup происходит сам через container destruction. Pattern работает.

6. **Split монолитных PR — must для security-критичных модулей.** PR #6 был задуман монолитом keyring + commands + UI. Разрезали, выкатили security часть руками за 2.5 часа, оставили UI на отдельный PR. Бесконечно лучше, чем 700 LOC одним merge.

7. **Когда CC находит проблему в спеке (Variant B на Day 3, Sample type на Day 4) — это работа, не баг.** Хороший AI-агент задаёт вопросы и спорит. Если ИИ молча кивает на противоречивую спеку — это hallucination-prone, не «послушность».

### Про hotfix-flow

8. **Hotfix через отдельный PR > direct commit на main.** PR #9 закрыл регрессию через стандартный review-flow. Direct commit (как было с `de3b066 Update .gitignore` ранее) ломает протокол. На v1.0 этот паттерн надо будет затвердеть в CONTRIBUTING.md.

9. **Diagnostics через `git check-ignore -v <file>` и `Select-String .gitignore "pattern"`.** Простой workflow для разбора почему файл не в репо. Запомнил.

---

## Progress against roadmap

### Sprint 1 progress

```
[XXXXXXXXXXXXXXXXXXX-] 90%

PR #2 → GitHub #2:  ✅ DONE  — Auth + http foundation       (Day 2)
PR #3 → GitHub #3:  ✅ DONE  — Synthesize sync API           (Day 3 S1)
PR #4 → GitHub #4:  ✅ DONE  — Text chunker                  (Day 3 S2)
PR #5 → GitHub #7:  ✅ DONE  — WAV join                     ← Day 4 S1
PR #6a → GitHub #8: ✅ DONE  — Secrets keyring              ← Day 4 S2
Hotfix → GitHub #9: ✅ DONE  — Restore secrets files        ← Day 4 cleanup
PR #6b:             ⏳ TODO  — Tauri commands + minimal UI  ← Day 5 (2-3 sessions)
```

### Overall project progress

```
Sprint 0 (Setup):       ✅ COMPLETE (Day 0-1)
Sprint 1 (Sber API):    ⏳ 90%  (Day 2-4) ← мы здесь
Sprint 2 (Storage):     ⏸️ NOT STARTED
Sprint 3 (Parsers):     ⏸️ NOT STARTED
Sprint 4 (Player):      ⏸️ NOT STARTED
Sprint 5 (CI + Polish): ⏸️ NOT STARTED
v0.1.0 release:         🎯 Day 13-16 (опережаем график на 2-4 дня)
```

### Velocity update

Темп всё ещё **выше плана.** За Day 4 — 2 PR + 1 hotfix через 3 разных workflow (CC облачный, chat manual, PowerShell direct). Это сложнее, чем 2 одинаковых PR через CC, и тем не менее уложились в день.

Прогноз до v0.1.0: Day 13-16 (план был Day 18-20, ревизия Day 15-18 после Day 3, теперь снова сдвиг влево).

---

## Mapping: внутренний PR # vs GitHub PR #

Внутренняя нумерация Sprint 1 (PR #2-#6) не совпадает с GitHub нумерацией. Фиксирую mapping для будущей навигации по логам:

| Sprint 1 (внутренний) | GitHub PR # | Содержимое |
|---|---|---|
| PR #2 | #2 | salute http + auth |
| PR #3 | #3 | synthesize |
| PR #4 | #4 | text chunker |
| — | #5 | (открыт CC, не merged — забытая ветка) |
| — | #6 | docs(text) cross-link to preprocessor Issue |
| PR #5 | #7 | audio wav_join |
| PR #6a | #8 | secrets keyring |
| — | #9 | hotfix restore secrets files |
| PR #6b | #10 (predicted) | Tauri commands + UI |

GitHub Issues:
- #5 — preprocessor module (открыт после PR #4, Sprint 3 tracking)

---

## Что НЕ было сделано (и почему — это правильно)

- ❌ Master log Day 4 не написан до конца Day 4 — пишется сейчас, перед Session 3. Если бы Session 3 стартанул без этого лога, контекст Day 4 потерялся бы в чат-истории.
- ❌ CC warmup task для PR #6b не получил отчёт — пользователь не успел отправить prompt в CC до hotfix-расследования. Будет сделано в начале Session 3.
- ❌ Integration test против реального Сбера для wav_join — не нужен, mockable boundaries покрыты unit-тестами.
- ❌ Streaming WAV write to disk вместо in-memory — deferred (реалистичный peak ~42 MB, не критично).
- ❌ Documentation site (mdBook) — план на v0.2 после первого user feedback. Текущий README + master logs достаточны.

---

## Reference links

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #7 (wav_join, merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/7
- **PR #8 (keyring, merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/8
- **PR #9 (hotfix, merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/9
- **Day 0-1 log:** `docs/day-logs/day-0-1-master-log.md`
- **Day 2 log:** `docs/day-logs/day-2-master-log.md`
- **Day 3 log:** `docs/day-logs/day-3-master-log.md`
- **Day 4 kickoffs:**
  - `docs/day-logs/kickoff-day-4.md` (PR #5 spec v2)
  - (PR #6a — спека жила в чате, не оформлялась как файл)
- **keyring-rs 3.x mock module docs:** https://docs.rs/keyring/latest/keyring/mock/index.html
- **hound docs:** https://docs.rs/hound/latest/hound/

---

*Day 4 captures backend pipeline completion + first cloud CC + first hotfix.*
*Pickup point: warmup CC for PR #6b, then `kickoff-day-4-session-3.md` for Tailwind + shadcn + 4 commands + Settings page.*
*Last updated: 2026-05-17*
