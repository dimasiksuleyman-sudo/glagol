# Day 7 Session 2 — Master Log

**Date:** Wednesday, May 20, 2026
**Sprint:** Sprint 5d — SaluteSpeech character counter + Sprint 5b/5c polish batch + tooling
**Outcome:** PR #32 merged (single-PR Sprint, no hotfix), closure tag `v0.1.0-rc.7` pushed
**Manual QA:** 14/14 PASSED (10 main scenarios + 5 sub-scenarios)
**Open issues post-Sprint:** 0 (Issue #16 auto-closed via `closes #16` syntax on merge)

---

## Session arc summary

Single session executing full Sprint 5d lifecycle от resume after Day 7 Session 1 pause through closure tag push. Sprint 5d notable as project's first single-PR Sprint with zero hotfix cycles + zero CI failure cycles — Working Agreements protocol matured к peak form.

Eight distinct phases sequentially:

1. Day 7 Session 1 master log publication via direct main commit + Cargo.lock sync (2 commits)
2. Sprint 5d backlog refinement — closed «not-features» (configurable library closed permanently из Sprint 5b; NSIS auto-launch retained per user love; library drag-to-reorder closed как избыточно для MVP)
3. Sprint 5d Q&A round — 8 architectural decisions (D1-D8)
4. 6 phases sequential implementation (PR #32, 5 commits squashed; Phase 6 toolchain-only no commit)
5. Cloud env GTK install discovery (Phase 1) — unlocks full cargo chain locally
6. PR #32 creation through Sprint 5d trust convention (direct create, no preview)
7. Subscribe convention CI monitoring → CI green → merge
8. 14-scenario manual QA → 14/14 PASSED → closure tag `v0.1.0-rc.7` pushed

Plus product-level conversation about backup/restore being the right cross-machine portability solution (replaces configurable library architectural dead end from Sprint 5b). Plus off-the-record discussion of Premium tier monetization concept (Glagol Premium via Sber wholesale + Whisper STT) — explicitly not recorded в this master log per user request.

---

## Pre-Sprint-5d backlog refinement

User explicitly closed three previously-open backlog items in conversation preceding Q&A round:

- **NSIS auto-launch checkbox removal:** «не нужен, зачем, автоланч очень удобно отрабатывает. я им пользуюсь мне нравится» — user feedback signals feature loved as-is. **Closed permanently from backlog.**
- **Library row drag-to-reorder:** «избыточно для mvp пока не надо» — feature complexity не warranted by current user signal. **Closed for MVP; conditional на post-MVP rethink.**
- **Issue #16 (SaluteSpeech error toasts):** «можно сделать» → **promoted to Sprint 5d Tier 2** (was previously Tier 4 deferred).

This is exemplary product discipline. Three concrete decisions captured before Q&A round prevents these items from creeping into Sprint 5d kickoff via «we should also do...» momentum.

---

## Sprint 5d Q&A round outcomes (8 locked decisions)

Pre-implementation Q&A established architectural locks. Compact ack «1+ / 2+ / 3+ / 4+ / 5+ / 6+ / 7+ / 8+» on all locks.

- **D1 (api_usage schema + migration v2):** Single CREATE TABLE с `month` (TEXT PK, `YYYY-MM` format local timezone), `chars_used` (INTEGER), `recognitions_seconds` (INTEGER, reserved for future STT), `updated_at` (INTEGER ms timestamp). No backfill. No index (PK suffices).
- **D2 (counter increment location + timing):** Inline in `synthesize_document_impl`, after successful synthesis, before/after DB persist. `chars().count()` (grapheme-aware), local timezone month boundary. DB failure logs к stderr, не propagates к user (advisory counter).
- **D3 (UI display location + format):** Settings section between credentials and backup. «Использовано в [месяц]: N / 200 000 символов» format с progress bar + percent + footer reset note. Russian month genitive helper.
- **D4 (backend command shape + event-driven refresh):** `get_current_month_usage` Tauri command returning `UsageInfo` struct (snake_case serde). `synthesis-completed` event с camelCase payload triggers frontend re-fetch (canonical-source-of-truth invariant).
- **D5 (Tier 2 batch integration):** Six items batched в Phase 5 — CHANGELOG promotions + backup error dedupe + pluralization helper + CLAUDE.md updates + Issue #16 toasts.
- **D6 (Tier 3 tooling):** pnpm 11.1.1 → 11.1.3 included (local-only, no commit). ESLint + Vitest deferred к Sprint 5e dedicated tooling Sprint.
- **D7 (test strategy):** +7 tests target (160 → 167). Pluralization tested via manual QA (formal test deferred к Sprint 5e Vitest setup).
- **D8 (phase structure):** 6 phases mapping к natural code surface boundaries — DB foundation → commands module → synthesis hook → frontend → polish batch → toolchain.

---

## Phase implementation log

### Phase 1 — DB migration v2 + repository::usage (commit `78582c2`)

**Scope:** Migration v2 + `api_usage` schema + repository functions + 4 tests.

**Files:** 2 modified, +151 LOC.

**Tests added (4):**
- `record_usage_inserts_new_month_row`
- `record_usage_increments_existing_month_row`
- `get_usage_for_month_returns_none_for_missing_month`
- `record_usage_isolates_months` (May vs June independence — guards calendar-boundary semantics)

**Test count:** 160 → 164

**🎯 CRITICAL banked discovery Phase 1:**

CC installed GTK + WebKit dev libs via apt:

```bash
apt-get update
apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev
```

**Cloud env now supports full Rust + Tauri compile chain.** Previously: «cargo gates deferred to Windows CI per Sprint 5a/5b/5c precedent» — every Tauri PR ran rustfmt-only locally, with cargo compilation deferred к CI matrix. **Now:** `cargo check + cargo test + cargo clippy + cargo fmt --check` ВСЕ run locally pre-push.

**Implication:** CI matrix becomes verification mechanism, не discovery mechanism. Sprint 5c PR #30 CI failure cycle (unused imports + missing scope refs) impossible after this — CC catches compilation errors before push.

**Banked Sprint 5d (critical):** «Cloud environment supports full Rust + Tauri compile chain after installing GTK system libs via apt-get. CC MUST run cargo check + test + clippy + fmt locally before push для all Rust changes. CI matrix becomes verification step, not discovery step.»

**Quality signals:**

- **Repository.rs extension over usage submodule** — CC noticed project convention «one repository file holds all free functions taking `&Connection`». Adding `repository/usage.rs` submodule would have forked convention. Section divider comment marks boundary.
- **`UsageRow` struct over `Option<(i64, i64)>` tuple** — named-fields rationale captured: «struct shape mirrors table 1:1, future migration adding a column is single-line change». Banked: «when row data has 2+ fields, named struct over positional tuple для forward-compat.»
- **`record_usage` returns `Result<usize>` (rows_affected) для symmetry с delete/update_title.** Project-wide API consistency maintained.
- **No index on `api_usage.month`** — PK already provides B-tree index. CC noted «adding an index now is YAGNI». SQLite stores PRIMARY KEY columns в covering index automatically.

### Phase 2 — commands::usage module + Russian month helper (commit `41efa5a`)

**Scope:** `commands::usage` module + `get_current_month_usage` Tauri command + `russian_month_genitive` helper + 3 tests.

**Files:** 1 new + 2 modified, +193 LOC.

**Tests added (3):**
- `get_current_month_usage_impl_computes_percent_correctly` — table-driven across (0, 100k, 150k, 200k, 250k) — guards 100% cap on over-quota usage
- `russian_month_genitive_all_months` — every 1..=12 mapping
- `russian_month_genitive_panics_on_invalid_input` — `should_panic` defensive guard on `unreachable!()` arm

**Test count:** 164 → 167 (+3, kickoff target +2; defensible +1 panic test)

**Quality signals:**

- **Linguistic accuracy banked в docstring** — «Returned forms are technically prepositional (used after «в»), not genitive» — function name kept verbatim для traceability с kickoff, but grammar mismatch flagged для future readers. **Banked:** «when convenient programmer label conflicts с technically correct linguistic terminology, keep label + add docstring clarification rather than renaming.»
- **`chars_used.max(0) as u64` defensive clamp** — SQLite stores INTEGER (signed i64), public API exposes `u64`. CC noticed potential corruption scenario где negative value на DB → silent wrap on unsigned cast → user sees absurdly large number. Defensive clamp keeps public contract honest. **Banked:** «when crossing signed→unsigned boundary at public API surface, clamp explicitly.»
- **`should_panic` defensive guard convention** — `russian_month_genitive_panics_on_invalid_input` first project use. Tests `unreachable!()` arm to prevent future refactor from silently substituting fallback value. **Banked:** «when function has `unreachable!()` arm, test that arm с `#[should_panic]`. Future refactor accidentally returning empty string instead of panic would be caught.»

### Phase 3 — Synthesis-time increment hook + event emission (commit `1eddd5b`)

**Scope:** `synthesize_document_impl` extended с `record_synthesis_usage` helper + `synthesis-completed` event emission + 1 integration test.

**Files:** 1 modified, +120/-5 LOC.

**Tests added (1):**
- `record_synthesis_usage_increments_current_month_counter` — unit-tests helper directly (avoids 50-100 LOC mockito harness setup that doesn't exist)

**Test count:** 167 → 168

**Quality signals:**

- **Deviation captured — order swap (under-count vs over-count semantic).** Kickoff D2 placed increment between «audio file written» (step 6) и «documents INSERT» (step 8). Existing `persist_synthesis_result` writes audio inside documents transaction (one atomic block). CC placed increment AFTER atomic persist returns Ok instead. **Effect:** counter under-counts (not over-counts) on rare persist failure — strictly tracks documents that actually landed. **Banked:** «when feature spec dictates ordering that conflicts с existing transaction boundaries, prefer alignment with existing atomicity over literal spec adherence.»
- **Test approach deviation — unit-test helper instead of full impl.** Kickoff suggested full `synthesize_document_impl` end-to-end testing. Would require ~50-100 LOC mockito harness which doesn't exist в project. CC extracted `record_synthesis_usage(db, chars_added)` helper и unit-tested directly. **Banked:** «when test would require mocking framework that doesn't exist в project, extract testable unit и test that directly. Don't add infrastructure just для single test.»
- **`SynthesisOutcome` struct return type** — CC widened impl return от `String` к struct, but Tauri command wrapper unwraps `document_id` для IPC return keeping frontend signature unchanged. **Smart layering:** pure function returns richer info для internal use (event emission), IPC stays minimal. **Banked:** «when refactor exposes new internal data that frontend doesn't strictly need, widen impl return type, keep Tauri command return narrow.»
- **`text.chars().count()` on post-preprocessor text** — matches `DocumentRecord.char_count` field. Counter aligns с stored document character count. User sees «document used N chars» в counter matching `char_count` displayed elsewhere в UI. **Banked:** «when counting characters for usage tracking, count post-preprocessing text (matches stored DocumentRecord.char_count), not pre-preprocessing.»
- **`record_synthesis_usage` returns `()` not `Result<()>`** — signal: «don't bother handling error — it's already logged via eprintln». Pure function takes `&Connection` + `chars_added`, no AppState dependency — testable in isolation. Advisory-write semantic encoded в type signature.
- **`SYNTHESIS_COMPLETED_EVENT` const co-located с emit site** — Sprint 5c convention applied. Frontend imports event name from `tauri.ts` typed wrapper anyway, не нужно shared constants module.

### Phase 4 — Frontend UsageSection.tsx + Settings integration (commit `7e50720`)

**Scope:** `UsageSection.tsx` component + `tauri.ts` typed wrappers + Settings page integration.

**Files:** 1 new + 2 modified, +208 LOC.

**Tests added:** 0 (frontend UI tested via manual QA per Sprint 5b precedent)

**Bundle delta vs Sprint 5c main HEAD baseline (419.53 KB JS / 42.98 KB CSS):**
- JS: 419.53 → 421.94 KB (+2.41 KB) — slight overshoot kickoff envelope (+1-2 KB expected)
- CSS: 42.98 → 43.04 KB (+0.06 KB) — within envelope

Drivers: UsageSection.tsx (~165 LOC), 1 new lucide icon (`TriangleAlert`), 4 tauri.ts additions (interface + interface + const + wrapper).

**Quality signals:**

- **Event payload deliberately ignored — canonical-source-of-truth invariant.** CC weighed trade-off explicitly: +5ms IPC roundtrip vs. drift risk if UPSERT semantics evolve. Chose re-fetch over optimistic add. **Banked:** «when frontend can either optimistically update from event payload OR re-fetch authoritative source, prefer re-fetch when latency cost is sub-100ms. Canonical-source-of-truth invariant compounds over time as feature evolves.»
- **Hand-mirrored Russian month names в frontend** — avoids IPC roundtrip on first paint, hand-sync acceptable для tiny static list. Fallback к «этом месяце» on malformed inputs. **Banked:** «when small static lists need to mirror between backend + frontend (12 month names, fixed enum variants), hand-sync is fine. IPC-fetch only когда list might grow or change.»
- **Russian decimal separator `,` instead of `.`** — `toFixed(1).replace('.', ',')` для proper Russian convention. Sprint 5c bundle had `12 345` formatting already; Phase 4 extends к decimal display. Polish detail caught.
- **Discriminated `LoadState` union pattern banked from Sprint 5b** — CC explicitly references prior Sprint convention. **Pattern continuity validates banked-conventions repository concept** — CC reads prior banked items, applies в next Sprint, paste-back references source.
- **`renderBody` switch over three top-level early-returns** — keeps Card chrome (header, footer) constant across all states. **Banked:** «when discriminated UI state covers loading/error/success, prefer single render path с conditional body over early-return per state. Card/page chrome stays stable, prevents visual jank between state transitions.»
- **Snake_case vs camelCase reconciliation handled correctly** — `UsageInfo` snake_case (default serde) → frontend type snake_case; `SynthesisCompletedEvent` camelCase (struct has rename_all directive) → frontend type camelCase. CC verified through actual backend serde behavior, не assumed.

### Phase 5 — Tier 2 polish batch (commit `a7ea10e`)

**Scope:** Six editorial/polish items batched into single commit so each landing surface stays self-contained.

**Files:** 6 modified.

**Tests added:** 0 (CHANGELOG/CLAUDE.md/error-message work verified by diff review and manual QA)

**Items completed:**

**A. CHANGELOG promotion** — `[Unreleased]` content moved verbatim к new `[v0.1.0-rc.6] — 2026-05-20` section (Sprint 5c — backup/restore + PR #31 hotfix under Fixed). New `[v0.1.0-rc.5] — 2026-05-20` section для Sprint 5b. New `[Unreleased]` summarises Sprint 5d itself. Footer link refs updated через `…/rc.5`, `…/rc.6`, `HEAD`.

**B. Backup error message dedupe** — `commands::backup::read_manifest_from_zip` constructed full sentence «Этот файл не является корректной резервной копией Glagol (отсутствует manifest.json)». Frontend toast wrapper prepended same phrase, producing «...: ...» duplication. Backend now returns reason-only («отсутствует manifest.json»); frontend wrapper owns user-facing prefix.

**C. Russian plural forms helper + apply** — New `src/lib/pluralize.ts` с `pluralRu(n, one, few, many)` implementing Russian three-form rule. Typed wrappers `pluralizeDocuments` + `pluralizeFiles`. Applied в `BackupSection.tsx` confirm-restore dialog + progress modal.

**D + E. CLAUDE.md update** — New «Conventions banked Sprint 5b–5d» subsection с 26 entries grouped under 8 themes: local-toolchain hygiene, concurrency & resource ownership, path & defensive-coding, state-machine design, PR & GitHub workflow, lockfile & dependency hygiene, frontend conventions, error-surface design. Timestamp updated к 2026-05-20.

**F. Issue #16 — friendly Russian SaluteSpeech error toasts** — New `commands::synthesize::to_user_facing_ru(internal: &str) -> String` helper. 8 categories mapped: no credentials, auth failed/expired, rate limited, network, certificate, API 4xx/5xx (status-aware), invalid response, empty text. Unknown strings fall through tagged «Ошибка синтеза: {internal}» so bugs stay searchable. Applied at Tauri command boundary via `.map_err(|e| to_user_facing_ru(&e))`. Tests assert internal English form; user sees Russian.

**Test count:** unchanged at 168.

**Bundle delta vs Phase 4 baseline:**
- JS: 421.94 → 422.23 KB (+0.29 KB)
- CSS: 43.04 → 43.04 KB (no change)

Driver: pluralize.ts (~30 LOC). No new components, icons, or shadcn primitives.

**Quality signals:**

- **CHANGELOG comparison-link maintenance** — CC updated footer refs through `…/rc.5...rc.6...HEAD`. **Sprint 5a established Keep a Changelog 1.1.0 footer convention; Sprint 5d Phase 5 maintains it.** Banked: «when promoting `[Unreleased]` к versioned section, update footer comparison links between versions. Prevents broken `[Unreleased]: ...compare/vN...HEAD` references.»
- **Backup error dedupe scope verification** — CC found dedupe was в `backup/create.rs`, not `backup/error.rs` как kickoff assumed. **Pre-emptive verification pays off** — CC didn't blindly trust kickoff file mapping. Sprint 5b precedent applied: «kickoff phrasing reflects mental model, не codebase reality. Verify via grep/view before changing.»
- **Existing test `validate_backup_rejects_missing_manifest` still passes** because test assertion was `msg.contains("manifest")` — not exact string match. **Defensive test design from Sprint 5c paid off** — substring assertion robust к message format changes. **Banked:** «when testing error messages, prefer substring-contains assertions over exact-string equality. Robust к UX wording iterations without test breakage.»
- **Pluralization applied к both документов AND файлов locations** — confirm dialog «N документов» + progress modal «X/Y файлов». CC applied helper к both без needing prompt. **Modular extensibility** — future Sprint adding `pluralizeBackups` trivial.
- **Thematic grouping в CLAUDE.md > flat list.** 26 items в flat list = wall of bullets; grouped → discoverable. Future Sprint maintainer hunting «as related к state machines» finds section immediately. **Banked architectural principle для CLAUDE.md:** «conventions list should be thematically grouped when count exceeds ~10 items.»
- **Timestamp updated с context** — «Last updated: 2026-05-20 (Sprint 5d — character counter + Sprint 5b/5c conventions landed)» — one-liner с descriptive scope. **Banked:** «timestamps gain value when accompanied by brief scope description. «Last updated: DATE (what changed)» > bare date.»
- **`to_user_facing_ru` unknown fallback** — tagged «Ошибка синтеза: {internal}» so bugs remain searchable через grep on logs. **Defensive design:** never produces silent unknown errors к user. **Banked:** «error-mapping helpers must always have unknown-input fallback that preserves underlying error text. Never silently swallow uncategorized errors.»
- **Issue #16 scope assessment honest** — ~40 min work, well within 45-min envelope. Single helper + one-line wiring. **Sprint 5d scope discipline maintained** — Issue #16 didn't balloon into restructuring entire error model.

### Phase 6 — pnpm 11.1.1 → 11.1.3 (no commit, local-only)

**Scope:** Developer-local pnpm version upgrade. No repository commit needed (pnpm version not pinned в Glagol project files).

**Steps executed:**

```powershell
npm install -g pnpm@11.1.3
pnpm install     # «Already up to date» — no lockfile regen
pnpm tsc --noEmit  # clean
pnpm build       # clean, 1900 modules transformed
```

**Verification:** `pnpm install` showed «Already up to date» — no lockfile regeneration needed. Lockfile commit not required per Sprint 5c Cargo.lock lesson (only commit when actual changes happen).

**Test count:** unchanged at 168.

---

## PR #32 creation + CI green via subscribe convention

**Process:** CC composed PR body using kickoff skeleton, posted directly к `mcp__github__create_pull_request` (Sprint 5d trust convention 5th validation — preview step skipped per established trust). Subscribed via `mcp__github__subscribe_pr_activity` immediately after creation.

**Web fetch sanity check found body clean:**

- 3 `#16` references (Issue body title + RU/EN tier 2 sections) — autolink correctly к Issue #16
- 2 `#31` references (Sprint 5c hotfix PR в CHANGELOG promotion context) — autolink correctly к PR #31
- No other `#N` references
- `Created by Dmitriy + Claude` footer present (no AI auto-injection)
- 5 phase commits clearly tagged (`78582c2`, `41efa5a`, `1eddd5b`, `7e50720`, `a7ea10e`) — squash-merge consolidates
- No `<...>` Rust generics в body (escape lesson not applicable этой PR)

**Sidebar issue-link triggered GitHub's auto-close mechanism** — body's «(closes Issue #16)» wording detected by GitHub, sidebar explicitly states «Successfully merging this pull request may close these issues». **Issue #16 auto-closed on merge.**

**Banked:** «PR body с `closes #N` (or `fixes #N`, `resolves #N`) auto-closes referenced issue on merge. No need для manual issue close afterwards. Sidebar confirms auto-close detection before merge.»

**Subscribe convention worked end-to-end** — CC monitored CI events automatically, surfaced result в chat without user manual relay step. **Sprint 5c trust convention validated на multiple dimensions** — PR body composition + CI monitoring + closing-keyword usage.

**CI green, PR merged.**

---

## Post-merge runtime QA — 14 scenarios PASSED

User executed runtime verification protocol на Windows machine. All gates clean:

- `cargo test`: 168 passed
- `cargo clippy`: clean
- `pnpm tsc --noEmit`: clean
- `pnpm tauri build`: produces `Glagol_0.1.0_x64-setup.exe`

Uninstall current Glagol с «оставить данные» preserved Sprint 5c base data. Install fresh installer applied migration v2 к existing `glagol.db` без crash.

**All 14 scenarios PASSED:**

1. ✅ Settings UI «Использование SaluteSpeech» section renders
2. ✅ Zero state «Использовано в мае: 0 / 200 000 символов»
3. ✅ Single synthesis increment (58 chars exactly counted via `chars().count()`)
4. ✅ Empty/whitespace rejection via disabled button (UX prevention superior к toast error)
5. ✅ Russian month name «в мае» correct prepositional form
6. ✅ Progress bar visual matches percent display «0,1 %» Russian comma decimal separator
7. ✅ Multi-document accumulation (58 + 64 + 64 + 64 = 250 chars exactly)
8. ✅ Persistence across full app restart (DB-backed, migration v2 wrote к disk)
9. ✅ Tier 2 verifications (6 sub-checks):
   - 9a Backup error toast dedupe — single prefix «Этот файл не является корректной резервной копией Glagol: отсутствует manifest.json»
   - 9b Russian plural forms — «6 документов» (many) + «2 документа» (few) — both grammar rules applied correctly
   - 9c CHANGELOG promoted к `[v0.1.0-rc.5]` + `[v0.1.0-rc.6]` versioned sections visible on GitHub
   - 9d CLAUDE.md timestamp 2026-05-20 + «Conventions banked Sprint 5b–5d» subsection с 8 thematic groups
   - 9e Issue #16 friendly Russian toast appeared on auth error («Ключ SaluteSpeech отклонён...»), credentials restored after test
10. ✅ End-to-end multi-doc accumulation + restart persistence + GitHub CHANGELOG visible

**Quality signals from QA:**

- **Counter increment exactness validated к single-character precision** — 58 chars text → counter shows exactly 58. UTF-8 grapheme counting working through pipeline (frontend display → backend command → DB write → frontend re-fetch).
- **Empty/whitespace UX architecture observed** — frontend disables button, не fires backend toast. CC's implementation chose UX prevention over post-action error. **Banked:** «for client-side validatable conditions, prefer disabled-button affordance over post-action error toast. Backend handling is defense-in-depth, не primary UX path.»
- **Russian plural correctness validated** — 6 documents shows «6 документов» (many form: numbers ending in 5-20), 2 documents shows «2 документа» (few form: numbers 2-4). Three-form rule encoded correctly в `pluralRu(n, one, few, many)`.
- **Migration v2 applied successfully on existing user database** — first-time test of incremental migration in production-like scenario. `glagol.db` opened post-uninstall, migration v2 detected v1 already applied, applied v2 (CREATE TABLE api_usage), no data loss.

---

## Closure tag pushed

```powershell
git tag -a v0.1.0-rc.7 -m "Sprint 5d closure: SaluteSpeech character counter + Sprint 5b/5c polish batch"
git push origin v0.1.0-rc.7
```

Clean push, single command sequence (no Sprint 5c reverse-order accident). Tag `v0.1.0-rc.7` landed on GitHub. Sprint 5d officially closed.

---

## Sprint 5d lessons-learned (consolidated)

Anti-patterns + conventions banked from this session, building on Sprint 5b's 10 + Sprint 5c's 13 (total now ~36 actively-applied conventions):

24. **Cloud env supports full Rust + Tauri compile chain after GTK install.** `apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`. CI matrix becomes verification step, not discovery step. Sprint 5c PR #30 CI failure cycle impossible after this. **Critical infrastructure improvement.**

25. **Defensive clamp at signed→unsigned API boundaries.** SQLite stores INTEGER (signed i64); public Rust API may expose `u64`. Clamp explicitly с `.max(0) as u64` для honest public contract. Banked Sprint 5d Phase 2.

26. **`should_panic` defensive guard for `unreachable!()` arms.** When function has `unreachable!()` arm, test that arm explicitly. Future refactor accidentally substituting fallback value would be caught. Banked Sprint 5d Phase 2.

27. **Order alignment with existing atomicity over literal kickoff spec.** When feature spec dictates ordering that conflicts с existing transaction boundaries, prefer alignment. Under-count semantic strictly better than over-count для advisory counters. Banked Sprint 5d Phase 3.

28. **Extract testable unit instead of adding mocking framework.** When test would require infrastructure that doesn't exist в project, extract narrower testable function and test that directly. Don't add framework just для single test. Banked Sprint 5d Phase 3.

29. **Widen impl return type, keep IPC return narrow.** When refactor exposes new internal data, expose it к internal callers (event emission), keep Tauri command IPC minimal. Banked Sprint 5d Phase 3.

30. **Count post-preprocessor text for usage tracking.** Aligns counter с `DocumentRecord.char_count` stored value. UI consistency between counter display + document row display. Banked Sprint 5d Phase 3.

31. **Advisory writes return `()` not `Result<()>`.** Type signature encodes «don't bother handling error — it's already logged via eprintln». Pure functions take `&Connection` для testability. Banked Sprint 5d Phase 3.

32. **Canonical-source-of-truth invariant beats optimistic UI updates** for sub-100ms IPC roundtrips. Future schema/semantic changes won't drift the display. Banked Sprint 5d Phase 4.

33. **Hand-mirrored small static lists between backend + frontend.** Avoid IPC roundtrip on first paint для tiny lists (12 month names, fixed enums). Hand-sync acceptable; fallback for malformed inputs. Banked Sprint 5d Phase 4.

34. **Russian decimal separator `,` not `.`** — use `toFixed(1).replace('.', ',')` для proper convention. Sprint 5c thousands separator + Sprint 5d decimal complete the Russian number formatting suite. Banked Sprint 5d Phase 4.

35. **`renderBody` switch over per-state early-returns** для discriminated UI states. Keeps page chrome (Card/header/footer) constant across loading/error/success transitions. Banked Sprint 5d Phase 4.

36. **Substring-contains assertions over exact-string equality** в error message tests. Robust к UX wording iterations without test breakage. Banked Sprint 5d Phase 5.

37. **Thematic grouping для conventions lists when count > 10.** Wall-of-bullets becomes discoverable taxonomy. CLAUDE.md «Conventions banked» section uses 8 themes для 26 entries. Banked methodologically Sprint 5d Phase 5.

38. **Timestamps gain value with descriptive scope.** «Last updated: DATE (what changed)» > bare date. Future maintainer reading timestamp gets context without grep'ing history. Banked Sprint 5d Phase 5.

39. **Error-mapping helpers must have unknown-input fallback preserving underlying text.** Tagged fallback («Ошибка синтеза: {internal}») keeps bugs searchable через log grep. Never silently swallow uncategorized errors. Banked Sprint 5d Phase 5 Issue #16.

40. **Disable invalid action > post-action error toast** for client-side validatable conditions. Empty text → disabled «Озвучить» button rather than fire toast on click. Backend toast is defense-in-depth, не primary UX. Banked Sprint 5d QA Scenario 4.

41. **Single-PR Sprint feasible с phase structure preserving review boundaries.** Sprint 5d shipped Tier 1 + Tier 2 + Tier 3 в one PR (#32) с 5 phase commits squashed. Zero CI failure cycles, zero hotfix cycles. **Trust convention matured к peak form here.**

42. **`closes #N` syntax auto-closes issues on merge.** Sidebar «Successfully merging this pull request may close these issues» confirms auto-close before merge. No manual issue close required afterwards. Banked Sprint 5d PR #32.

---

## Cumulative project status

**Closed sprints:** 0, 1, 2, 3, 3a, 4, 5a, 5b, 5c, 5d (10 closures)
**Test baseline:** 168 on Windows main HEAD `b7c1e81`
**Open issues:** 0 (Issue #16 auto-closed via PR #32 merge)
**Closure tags accumulated:** `v0.1.0-rc.4` (Sprint 5a) → `v0.1.0-rc.5` (Sprint 5b reduced) → `v0.1.0-rc.6` (Sprint 5c + hotfix) → `v0.1.0-rc.7` (Sprint 5d)

**Quality progression Sprint-over-Sprint:**

| Sprint | PRs merged | Wall clock | CI failures | Hotfix cycles |
|---|---|---|---|---|
| 5b | 3 (ship + hotfix + scope reduction) | 10-12 h | 0 | 1 |
| 5c | 2 (ship + hotfix) | 6-8 h | 1 | 1 |
| 5d | **1** (ship only) | **5-6 h** | **0** | **0** |

**Decreasing PR count + decreasing wall clock per Sprint despite increasing feature surface** (Sprint 5d: Tier 1 feature + 6 Tier 2 items + Tier 3 toolchain). Compounding efficiency through banked conventions.

**Health check:** **peak form**. Sprint 5d proved CC + Working Agreements protocol + user QA collaboration shipping multi-tier scope cleanly. Single-PR Sprint feasible с phase structure. Trust conventions (PR body composition, subscribe CI monitoring, closing-keyword syntax) validated end-to-end в production conditions.

---

## Sprint 5e backlog refined for next session

Items remaining after Sprint 5d:

**Tier 1 (significant scope, likely own Sprint):**

1. **ESLint + Vitest frontend testing setup** — adds `eslint.config.js`, `vitest.config.ts`, dev deps, CI workflow update, initial verification tests. Significant tooling investment. Sprint 5e dedicated focus likely required.

2. **Pluralization formal test** — `pluralize.ts` currently tested only via manual QA (Sprint 5d Scenario 9b). Sprint 5e Vitest setup enables formal regression test для three-form rule с table-driven assertions across (1, 2, 3, 4, 5, 11, 12, 13, 14, 15, 21, 22, ...) edge cases.

**Tier 2 (modest scope, batch-able):**

3. **CLAUDE.md SmartScreen screenshot publication** — environment-blocked since Sprint 5a (domain reputation needs accumulation through download volume). Conditional на ability to capture good screenshot showing accumulation progress.

4. **DOCX table narration tuning** — Sprint 5 backlog item. Current behavior may не handle multi-column tables gracefully. Conditional на user signal.

5. **Tier 2/3 abbreviations expansion** — preprocessor enhancement. Currently handles common Russian abbreviations; expansion would cover more edge cases. Conditional на user reports.

6. **shadcn CLI vs hand-written component policy decision** — backlog from Sprint 5b. Project has used both patterns; policy needed для consistency.

7. **Drag-and-drop file input** — Sprint 4 deferred. Currently user uses file picker dialog; drag-drop quality-of-life enhancement.

**Tier 3 (forward-looking, conditional on signal):**

8. **STT (speech-to-text) feature exploration** — Sprint 6+ if real user signal. SaluteSpeech recognition endpoint + UI integration. Foundation laid в Sprint 5d (recognitions_seconds field reserved в `api_usage` table).

9. **Premium tier monetization roadmap** — Sprint 6+ если decision made к pursue. Requires payment integration + server backend + STT feature + customer support workflow. Significant new complexity class beyond MVP.

---

## Session timing

Start of session: ~12:00 local time (Day 7 Session 2 resume after щи break)
End of session: ~17:30 local time (Sprint 5d closure tag pushed)
Total wall clock: ~5.5 hours

**Comparison к prior Sprints:**
- Sprint 5b: ~10-12 hours (configurable library + scope reduction)
- Sprint 5c: ~6-8 hours (backup/restore + hotfix)
- **Sprint 5d: ~5.5 hours** (character counter + Tier 2 batch + Tier 3 toolchain)

**Sprint 5d most efficient Sprint к date** despite largest accumulated feature scope (Tier 1 + 6 Tier 2 items + Tier 3 toolchain). Banked conventions actively reduce session duration.

Sprint 5e next session pickup: ESLint + Vitest setup OR feature work depending on user priority. Estimated 4-6 hours для tooling Sprint, 2-4 hours для smaller feature work.

---

*Created by Dmitriy + Claude*
