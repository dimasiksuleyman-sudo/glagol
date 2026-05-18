# Glagol — Day 5 Session 1 Master Log

**Period:** May 18, 2026 (Sprint 2 first session)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 2 (Storage + Library) — **Session 1 of ~4**
**Status:** PR #15 logical (GH #17) merged. SQLite foundation laid. Runtime verified.

---

## TL;DR

Sprint 2 первая сессия закрыта. От Sprint 1 closure (76 tests, MVP runtime-verified, `v0.1.0-alpha`) пришли к persistent storage foundation: встроенный SQLite через `rusqlite` 0.39 + миграция v1 со схемой `documents` + repository CRUD + path resolution helpers + AppState extension. **76 → 89 tests**, 0 regressions, runtime sanity passed на Windows 11.

**Day 5 spans 1 session** so far. PR #16 logical (refactor synthesize для persistence) — next session, дата TBD.

Single architectural surprise обнаружен в runtime QA: Tauri 2 `app_local_data_dir()` на Windows dev-unsigned builds резолвится через `bundle.identifier` (`app.glagol.desktop`), не через `productName` (`Glagol`). Documented as known behavior; Sprint 5 MSI installer нормализует автоматически. Не чиним сейчас.

---

## Phase 1 — Pre-implementation architectural Q&A round

После прочтения `kickoff-day-5.md` и всех 5 master logs Day 0-4, прошли через 6 architectural decisions + 1 bonus. Каждый вопрос сопровождался **web-search verification deps freshness** (kickoff явно requested «verify before commit, lessons from Sprint 1 suggest»):

| Q | Topic | Decision | Why |
|---|---|---|---|
| Q1 | SQLite client | `rusqlite` 0.39 (`bundled` + `serde_json`) | plugin-sql exposes SQL to frontend — wrong shape для нашей security posture (SECURITY.md prohibits direct frontend DB access); sqlx async overkill для single-user local DB; bundled SQLite static link = zero system deps для будущего MSI installer |
| Q2 | Migration tooling | `rusqlite_migration` 2.5.0 | Atomic per-migration transactions built-in (own runner easy to forget); `validate()` catches malformed SQL at startup; solved problem with subtle correctness traps |
| Q3 | Test DB strategy | `Connection::open_in_memory()` per test | Matches Sprint 1 mockito + per-Entry pattern (ephemeral, in-process, no FS); Windows SQLite file locking flakiness eliminated by design |
| Q4 | Audio paths в БД | Relative (`{uuid}.wav`, flat layout) | Portable across machine moves / backups; doesn't affect HTML5 playback (asset protocol resolves either way); future Sprint 5 configurable root won't invalidate stored paths |
| Q5 | Persistence flow | Auto-save on success only | Minimal Sprint 2 scope; no stuck rows; no crash recovery surface; `status='ready'` only Sprint 2 (`'error'` / `'synthesizing'` reserved Sprint 4) |
| Q5.5 | Configurable library location | Backend abstraction now, UI Sprint 5 | `paths::audio_cache_root()` = single grep target для будущего Sprint 5 work; allows MVP simplicity сейчас без lock-out future configurability |
| Q6 | HTML5 audio playback | Tauri asset protocol | Streaming + seeking для long files (200+ MB реалистично на 3-hour audio при 200K char SaluteSpeech monthly free tier); blob URL would double memory pressure |

**All 7 decisions locked в чате до написания kickoff'а.** Sprint 1 pattern: Q&A first, kickoff after, no design pivots mid-implementation. Это пятый случай применения паттерна — pattern is established.

### Process note — verification before commit

Web searches покрыли:
- `tauri-plugin-sql` 2.3.2 (Feb 2026, sqlx-based) — verified active, but architecture mismatch
- `rusqlite` 0.39 + `bundled` feature — verified static SQLite source linking works
- `rusqlite_migration` 2.5.0 (Mar 2026, depends rusqlite ^0.39) — verified exact MSRV match, 94.9% test coverage, atomic-per-migration transactions confirmed

Без этих проверок мы бы вошли в Sprint 2 на stale memory из training data. Один из примеров где «verify before commit» рекомендация явно окупается.

---

## Phase 2 — Kickoff written

Файл `kickoff-day-5-session-1.md` (~400 lines) написан в чате, сохранён в `.scratch/`. Структура:

- Architectural decisions всех 7 — marked «do not re-litigate»
- Scope: explicit IN/OUT lists с rationale
- Dependencies: exact versions + features rationale
- Schema: full SQL block с type/value contracts (vocabularies, NULL semantics, ID format conventions)
- File-by-file breakdown с code snippets для каждого нового файла
- Pre-answered Q&A: 5 likely CC questions с ответами (Result vs panic, Mutex choice, UUID timing, created_at ownership, status writes в этом PR)
- Quality gates: 7 checks
- Workflow: 4 phases с sanity checkpoints + paste-back-first rule + wait-for-merge-it rule
- Out-of-scope clarifications: explicit «if you reach for X — stop and ask» list

Также написана **пометка в блокнот для Sprint 5** (configurable library location feature) — не GitHub issue, локальная заметка с grep marker `audio_cache_root`.

---

## Phase 3 — CC implementation

CC прошёл **all 4 phases в single pass** без surfacing questions. Deviation from suggested phase-based reporting, но defensible:
- All quality gates clean at end
- No design questions surfaced
- Pre-answered Q&A held up под implementation

### Files created (4)

| File | LOC | Tests |
|---|---|---|
| `paths.rs` | 41 | 0 (Tauri runtime-only, manual QA covers) |
| `db/mod.rs` | 67 | 1 (init_database integration через `std::env::temp_dir`) |
| `db/migrations.rs` | 89 | 4 |
| `db/repository.rs` | 207 | 8 |
| **Total new Rust** | **404** | **13** |

### Files modified (5)

- `Cargo.toml` — +5 lines (`rusqlite` + `rusqlite_migration`; `uuid` was уже present в tree)
- `Cargo.lock` — auto-updated
- `state.rs` — +15 lines (`db: Mutex<Connection>` field + extended `AppState::new(client, db)` constructor)
- `lib.rs` — `mod db; mod paths;`, `use tauri::Manager;`, AppState init moved into `.setup()` closure with eager migration apply
- `commands/credentials.rs`, `commands/synthesize.rs` — only `fresh_state()` test helpers updated to pass `crate::db::test_connection()` (no production command code touched, out-of-scope rule respected)

### Deviations from spec (all approved)

1. **Field name `salute_auth` vs `auth`.** My spec error — не verified actual field name в state.rs перед написанием kickoff'а. CC сохранил existing name. Lesson: спец-ссылки на existing fields формулировать менее prescriptive, или `view` файл before mention.

2. **`AppState::new(http_client, db)` constructor вместо struct-literal init.** Better than spec. Existing constructor kept fields private (no `pub` markers needed); cascade в `fresh_state()` test helpers получился локализованным (только test-only code). I would have approved if I'd known constructor existed.

3. **`http_client` clone в setup closure** — reqwest::Client internally Arc-shared, clone cheap. Single clone at app start, no per-command overhead. Pragmatic choice.

### Branch / commit

- Branch: `claude/setup-sqlite-migrations-Je9rX`
- Single commit: `b4fba73` (squash будет clean)
- Commit message соответствует kickoff spec (10-строчный summary + scope clarification footer)

---

## Phase 4 — Paste-back review

### Quality gates

| Check | Result |
|---|---|
| `cargo check` | clean |
| `cargo fmt --check` | clean (after one autofix pass) |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | **89 passed / 0 failed / 0 ignored** (76 baseline + 13 new) |
| `pnpm tsc --noEmit` | only pre-existing tsconfig deprecation warning |

### Test breakdown (13 new)

- `db::migrations` (4):
  - `migrations_validate_successfully`
  - `apply_migrations_to_empty_db_creates_documents_table`
  - `apply_migrations_is_idempotent`
  - `apply_migrations_creates_index`
- `db::repository` (8):
  - `insert_then_get_returns_same_record`
  - `get_returns_none_for_missing_id`
  - `insert_duplicate_id_fails`
  - `list_all_returns_most_recent_first`
  - `list_all_returns_empty_when_no_rows`
  - `delete_existing_returns_one`
  - `delete_nonexistent_returns_zero`
  - `optional_fields_persist_as_none`
- `db::tests` (1):
  - `init_database_creates_file_and_applies_schema`

Approved для PR creation в чате.

---

## Phase 5 — PR #17 (logical #15) creation

CC создал PR на GitHub. **Discovered:** GitHub присвоил **#17**, не #15. Два номера (#15, #16) исчезли между Sprint 1 closure и Sprint 2 start — вероятно через GitHub Desktop draft activity или web UI которые открыли-закрыли PR'ы без merge. Это history note, не блокер.

**Decision:** PR title оставлен с literal `(PR #15)` для current PR (harmless). Со следующего PR — **drop explicit numbering из title** (GitHub assigns its own номер; title должен быть чисто descriptive). Logical-to-GH mapping для Sprint 2:

| Logical | GH (actual / assumed) |
|---|---|
| Sprint 2 PR #15 | **#17** (merged) |
| Sprint 2 PR #16 | #18 (assumed) |
| Sprint 2 PR #17 | #19 (assumed) |
| Sprint 2 PR #18 | #20 (assumed) |

### web_fetch sanity check на PR body

- ✅ Bilingual format (RU summary + EN technical body) полный
- ✅ All 13 new tests перечислены поимённо в table
- ✅ Security checklist полностью зелёный с reasoning where needed
- ✅ Manual testing steps numbered и runnable
- ✅ Schema встроена inline в body
- ✅ Commit message соответствует kickoff spec
- ✅ Single commit `b4fba73`

### Minor presentation observations (non-blocking)

1. **GitHub auto-linkification.** Literal `#16` и `#17` в body превратились в hover-links на existing issues (#16 = UX toasts) и self (PR #17). Lesson для будущих kickoff'ов: ссылки на logical PR slots писать как `"PR #16 logical (refactor)"` без literal `#16` чтобы избежать collision.

2. **«Generated by Claude Code» footer в PR body.** Kickoff explicitly запретил `co-authored-by: Claude` строки в commit metadata (CC complied — commit clean), но про PR body footer не было. Lesson: kickoff template для следующих PR должен explicitly forbid AI attribution strings в любом месте (commit body, commit trailer, PR body, PR title).

Merge approved в чате.

---

## Phase 6 — Squash-merge & runtime verification

### Squash-merge

Squash-merge выполнен через GitHub web UI. Результат: HEAD на main = **`4e51bb1`** (commit message: `feat: SQLite foundation with rusqlite + rusqlite_migration (#17)`). Branch `claude/setup-sqlite-migrations-Je9rX` auto-deleted GitHub'ом.

### 5-step runtime verification protocol

Sprint 1 lesson (PR #13 streaming WAV bug) — unit tests могут быть green при сломанном end-to-end. Same paranoia applied here с новой инициализацией в setup hook и migration runner который пока не проверен на реальном диске.

| # | Step | Result |
|---|---|---|
| 1 | `cd C:\Projects\glagol && git checkout main && git pull` | `Already up to date` (user уже pulled). HEAD `4e51bb1` confirmed |
| 2 | `cd src-tauri && cargo test` | **89 passed in 0.29s** — `:memory:` strategy подтвердила скорость |
| 3 | `cd .. && pnpm tsc --noEmit` | Clean (даже pre-existing warning не появился в этом проходе) |
| 4 | `pnpm tauri dev` | 52 sec compile (новые deps + bundled SQLite через libsqlite3-sys); app окно открылось без panics; Settings/Synthesize/Library pages работают как Sprint 1; end-to-end synthesis flow passed (текст → synthesis → Save As → WAV проигрался) |
| 5 | Disk verification | `glagol.db` created, **16384 bytes** (4 SQLite pages = consistent с пустой БД + одна таблица + один index) |

---

## Critical discovery: `app_local_data_dir()` resolution на Windows dev builds

В kickoff'е я писал «default: `%LOCALAPPDATA%\Glagol\glagol.db`». **Фактически файл создаётся в `%LOCALAPPDATA%\app.glagol.desktop\glagol.db`.**

### Root cause

Tauri 2's `app_local_data_dir()` на Windows для unsigned dev builds резолвится к `bundle.identifier` (`app.glagol.desktop` из `tauri.conf.json`), а не к `productName` (`Glagol`). При signed bundled release builds (Sprint 5 MSI installer) поведение меняется на `productName`-based path.

### Это не bug

Это **documented Tauri 2 behavior**. Объясняется так: dev builds не имеют bundle identity assertion (нет MSI, нет AppX manifest), поэтому Tauri использует identifier как «fallback» для namespace isolation. Bundled signed builds имеют proper identity и используют productName для user-facing folder name.

### Decision: не чинить сейчас

Рассмотренные альтернативы:
- **Хак `app_local_data_dir().parent().join("Glagol")`** — fragile, может сломаться при breaking changes Tauri internals, smell of fighting framework
- **Менять `bundle.identifier` на `Glagol`** — breaks existing dev installs (потеря credentials в keyring если identifier меняется?), нарушает out-of-scope для PR #15
- **Менять `productName` или подобное** — uncertain how it affects dev resolution
- **Wait for Sprint 5 installer** — automatic resolve, no extra work, consistent с release reality

Выбрали **wait for Sprint 5**. Sprint 5 MSI installer всё равно поправит путь автоматически через productName resolution. Лучше не дробить решение на два сайта (workaround в dev + proper в release).

### To record для future docs

- **README user-facing docs (Sprint 5 publication):** dev-builds кладут data в `%LOCALAPPDATA%\app.glagol.desktop\`. Release-builds (MSI installer) — в `%LOCALAPPDATA%\Glagol\`.
- **Sprint 5 configurable library location issue (Q5.5):** default path в Settings UI должен display `«Glagol»` (productName), не `«app.glagol.desktop»` (identifier). Если в Sprint 5 видим identifier в UI — это сигнал что MSI installer setup не работает корректно.

---

## Stats

| Metric | Value |
|---|---|
| Tests progression | 76 → **89** (+13) |
| New Rust LOC | ~404 (paths.rs + db/* total) |
| New Rust deps direct | 2 (`rusqlite`, `rusqlite_migration`) + 1 already present (`uuid`) |
| Bundle size impact | est. +1.5 MB debug binary (bundled SQLite statically linked) |
| Calendar duration | Day 5 single session, late-night work (3:48 на disk timestamps) |
| Migration runner | `rusqlite_migration` 2.5 |
| Schema versions deployed | 1 (`documents` table v1) |
| Sprint 2 PRs left | 3 (logical #16/#17/#18) |
| Tag | still `v0.1.0-alpha`; `v0.1.0-rc.1` deferred to Sprint 2 closure |

---

## Lessons learned

### Технические

1. **Tauri 2 `app_local_data_dir()` differs dev vs release.** Identifier-based в dev, productName-based в bundled release. Stable behavior of Tauri 2, but cosmetic surprise если документировать «default path» в специях. Future kickoff'ы про paths должны cite either both variants или explicitly note environment.

2. **`:memory:` SQLite tests настолько быстрые, что 89 tests за 0.29s.** Доказательство правоты Q3. Никаких temp file flakiness, никаких Windows SQLite locking issues — ephemeral БД per test работает идеально. Этот паттерн distributes на все будущие DB tests в Sprint 2/3/4.

3. **`bundled` feature на rusqlite — must-have для Windows distribution.** SQLite source compiled statically в binary, zero system deps. MSI installer в Sprint 5 не будет требовать SQLite на user machine. Trade-off — +1.5 MB binary size, что приемлемо для desktop app.

4. **`rusqlite_migration`'s `validate()` — мощный safety net.** Catches malformed SQL at startup, до того как оно achieve контакт с user DB. Worth +1 dep. Если migration #5 будет с typo — `validate()` его поймает на app startup, до migration apply. Это better than discovering broken migration mid-deployment.

### Процессные

1. **Single-pass implementation defensible когда gates clean.** CC прошёл все 4 phases в один проход без вопросов. Sprint 1 паттерн «report after each phase» хорош для PRs с большой surface или design ambiguity. Для foundation PR со 100% pre-answered Q&A — это была корректная оптимизация на скорость. Phase-based reports возвращаются для PR #16 logical где surface больше (command refactor + frontend integration + error semantics).

2. **PR numbering can drift unexpectedly.** GitHub eats numbers через ghost PRs (draft создан-canceled, либо closed без merge). Don't put literal `#N` в PR titles starting next session. Logical numbering held in roadmaps/kickoffs only.

3. **`Generated by Claude Code` footer slipped через kickoff guard.** Kickoff блокировал только `co-authored-by:` строки в commit metadata. Body footer — separate vector. Update kickoff template для PR #16+: explicit list of forbidden AI attribution strings (commit metadata, commit body, PR body, PR title, branch name).

4. **Web search verification of deps freshness saves real time.** Q1/Q2 pre-implementation searches для actual versions (rusqlite 0.39, rusqlite_migration 2.5, tauri-plugin-sql 2.3.2) дали factual data вместо stale memory. Kickoff explicitly requested «verify before commit», и это правильно. Repeat паттерн для каждого Sprint 2 PR где deps freshness relevant.

### Архитектурные

1. **`paths::audio_cache_root` как single grep target.** Sprint 5 customizable library location делается через одну функцию. Grep marker лучше любого TODO comment'а — он сам себя находит при `grep audio_cache_root` в IDE при начале Sprint 5 work.

2. **`std::sync::Mutex<Connection>` vs `tokio::sync::Mutex<Connection>`.** Для sync rusqlite на single-user desktop — std mutex correct. Async lock would force `.await` на каждое DB op без benefit. Mixed strategy с `tokio::sync::Mutex` на `salute_auth` (async HTTP) — pragmatic, не догматичный «всё async или всё sync».

3. **Repository как free functions, not struct.** Matches Sprint 1 паттерн (`text::chunker`, `audio::wav_join` — also free functions; только stateful модули типа `SaluteAuth` получают struct). Single connection threaded через `&Connection` argument — simple, testable, idiomatic Rust. Если в будущем понадобится repository со stateful behavior (caching, batching) — easy to wrap free functions в struct без breaking change для existing callers.

4. **Eager DB init в setup hook (not lazy).** Если migrations fail — app refuses to start с loud error. Silently broken DB корrupting subsequent writes — worse failure mode than no-start. Same доктрина что в Sprint 1 для SaluteAuth: fail loud, fail early.

---

## What's next — Sprint 2 PR #16 logical (GH будет #18)

### Goal

Refactor `commands::synthesize::synthesize_document` для persistence:

- После успешного WAV synthesis: generate UUID + insert `DocumentRecord` + write WAV в `audio_cache/{uuid}.wav` + return `SynthesisResult { audio_bytes, document_id }`
- New Tauri command `get_audio_path(document_id: String) -> Result<String, String>` — resolve relative → absolute через `paths::resolve_audio_path`, returns string для frontend `convertFileSrc`
- Frontend Synthesize page: получает `SynthesisResult`, показывает toast «Сохранено в библиотеку» с link на `/library` (Library page всё ещё placeholder Coming Soon — PR #17 logical заменит реальным content)
- Audio cache directory создание (`fs::create_dir_all(audio_cache_root())`) — впервые в PR #16, не в #15

### Pre-implementation Q&A topics (probable)

1. **Error semantics на partial failure.** Synthesis OK + DB insert OK + `fs::write` failed → rollback DB row или leave с `audio_path=NULL`? Q5 decision был «auto-save on success only, не пишем error rows», но partial failure — edge case в этом decision.
2. **`SynthesisResult` shape.** `audio_bytes` + `document_id` в одном payload через existing binary IPC pattern, или split на два calls?
3. **`get_audio_path` command interface.** Returns absolute path string (frontend converts через `convertFileSrc`) или сразу asset URL? Trade-off: asset URL forces backend to know about asset protocol, path string keeps boundary cleaner.
4. **UUID + `created_at` ownership.** Tauri command generates them (consistent с Sprint 1 «commands are integration points»), или frontend генерирует? UUID through `uuid::Uuid::new_v4()`, `created_at` through `chrono::Utc::now().timestamp_millis()`.
5. **Test coverage strategy.** Какие mocks нужны для partial failure scenarios? `fs::write` failure mock потенциально нужен — но `std::fs` не mockuable trivially. Options: trait abstraction over filesystem, или testing через actual temp dir в integration tests, или skip this edge case в unit tests + cover в manual QA matrix.

### Estimated

1 CC session. Less infrastructure than PR #15, but more interaction surfaces (existing command refactor + new command + frontend integration + error semantics). LOC growth smaller (~150-200 Rust + ~30 TypeScript), но review burden больше (touches Sprint 1 production code path).

---

## Reference

- **Sprint 2 entry doc:** `.scratch/kickoff-day-5.md`
- **Session 1 kickoff:** `.scratch/kickoff-day-5-session-1.md`
- **PR #17 (logical #15):** https://github.com/dimasiksuleyman-sudo/glagol/pull/17
- **Main HEAD at Session 1 closure:** `4e51bb1`
- **Tag closest:** `v0.1.0-alpha` (Sprint 1 closure; rc.1 deferred to Sprint 2 closure после ~3 more PRs)
- **Sprint 5 blocker noted in local notebook:** configurable library location (grep marker `audio_cache_root`)

---

*Session 1 of Sprint 2 captures: Q&A round (7 decisions) + kickoff drafting + CC single-pass implementation + paste-back review + PR creation + runtime verification + critical Tauri 2 path resolution discovery.*
*PR #15 logical (GH #17) merged. Sprint 2: 1 of ~4 PRs complete. Tests 76 → 89.*
*Last updated: 2026-05-18.*
