# Day 7 Session 1 — Master Log

**Date:** Wednesday, May 20, 2026
**Sprint:** Sprint 5c — Backup/Restore via .zip
**Outcome:** PR #30 merged, hotfix PR #31 merged, closure tag `v0.1.0-rc.6` pushed
**Manual QA:** 8/10 PASSED (Scenarios 9-10 skipped — architecturally equivalent)
**Sprint 5d entry locked:** SaluteSpeech remaining-character counter

---

## Session arc summary

Single session covering full Sprint 5c lifecycle от resume after Day 6 Session 3 pause through closure tag + Sprint 5d planning prep. Eight distinct phases sequentially:

1. Day 6 Session 3 master log publication via direct main commit (carry-over from prior session)
2. Sprint 5c Q&A round — 7 architectural decisions (D1-D7)
3. 4 phases sequential implementation (PR #30, 5 commits squashed)
4. PR #30 CI failure + fixup commit (cargo gates lesson banked)
5. PR #30 merge + post-merge runtime QA — Scenarios 1-4 PASSED, Scenario 5 BLOCKED
6. Hotfix PR #31 — Windows file-locking issue (mem::replace + symmetric recovery)
7. PR #31 merge via subscribe convention + resume QA — Scenarios 5-8 PASSED
8. Sprint 5c closure tag `v0.1.0-rc.6` pushed

Plus product-level conversation about configurable library архитектурный «dead end» and backup/restore being the right replacement feature for actual user pain (cross-machine portability + cloud sync support).

---

## Pre-Sprint-5c product decision (critical context)

Before Sprint 5c Q&A round, user surfaced a product-level question:

> «Что если пользователь перенесется на новый компьютер, делает рестор, а на новом компьютере не будет диска D, где раньше была библиотека? Что произойдет в таком сценарии?»

User initial framing: hardcode data location to «папка с глаголом» (program folder). This raised three Windows-specific architectural concerns:

1. **Permissions:** `C:\Program Files\Glagol\` requires admin для write
2. **Uninstall wipes data:** standard NSIS uninstaller deletes program folder
3. **AV heuristics:** apps writing to program folder trigger antivirus false positives

After discussion, chose **Option C — `%LOCALAPPDATA%` hardcode + backup/restore feature**. Data lives в fixed `%LOCALAPPDATA%\app.glagol.desktop\`, cross-machine portability achieved через backup/restore Sprint 5c work, not through configurable path.

**Configurable library officially closed as not-feature.** User's framing: «и так на C, и так на D, ещё и скачать можно — избыточно». Sprint 5b backlog item permanently removed.

This conversation closed the loop on Sprint 5b's product misalignment — the **disk space pain doesn't exist** (modern disks cheap + per-document download already available), but the **cross-machine pain does exist** (real friction when migrating). Backup/restore solves the actual pain, configurable path solved imaginary pain.

---

## Sprint 5c Q&A round outcomes (7 locked decisions)

Pre-implementation Q&A established architectural locks. Compact ack «1+ / 2+ / 3+ / 4+ / 5+ / 6+ / 7+» on all locks.

- **D1 (zip crate + compression):** `zip = "2"` direct dep, `default-features = false, features = ["deflate"]`. Compression strategy: db + manifest Deflated, audio Stored. User framing «архивация для удобства, не для сжатия» drove the decision — single-file container for portability, не optimization для file size.
- **D2 (file format):** `glagol-backup-YYYY-MM-DD-HHMMSS.zip` filename (local timezone), `manifest.json` (ISO 8601 UTC) at root, `glagol.db` Deflated, `audio_cache/*.wav` Stored. 6-field manifest schema with `backup_version: 1` для future compat.
- **D3 (restore conflict semantics):** Replace-all с two safety nets — confirm dialog с counts + automatic pre-restore backup. User framing «чем стерильнее решение, тем меньше шансов на ошибку» drove the simplicity. Merge semantics rejected.
- **D4 (post-restore behavior):** Auto-restart с 2-second delay after toast. `AppHandle::restart()` Tauri 2 API.
- **D5 (backup UX):** Folder picker → progress modal (non-dismissible) → success toast с filename. No cancel mid-operation per scope discipline.
- **D6 (restore UX):** Two-call architecture — `validate_backup` (~50ms, non-destructive) separate от `restore_backup` (destructive). Allows confirm dialog с counts между phases.
- **D7 (test strategy + closure criteria):** ~8 backup tests target, 10-scenario manual QA matrix.

---

## Phase implementation log

### Phase 1 — Backup module foundation + manifest (commit `d95286b`)

**Scope:** `backup/` module skeleton + `BackupManifest` struct + `BackupError` enum + manifest round-trip tests.

**Files:** 2 new + 2 modified, +164 LOC.

**Tests added (2):**
- `manifest_round_trip_preserves_all_fields`
- `manifest_json_is_pretty_printed_and_field_named` — defensive regression guard против future `#[serde(rename_all = ...)]` accident

**Test count:** 150 → 152

**Quality signals:**

- **`manifest_json_is_pretty_printed_and_field_named` test design** — locks field names against future serde rename accidents. **Banked:** «когда JSON schema публично-видимая, write tests asserting on exact field names, not just round-trip.»
- **`thiserror` direct dep already present in tree** — Sprint 5c used it for `BackupError` without new dep addition.
- **`pub mod backup;` placement** между `parser` и `state` в `lib.rs` — alphabetical convention maintained.

### Phase 2 — Backup creation logic (commit `7fc228b`)

**Scope:** `create_backup_impl` pure function + Tauri command wrapper + 3 tests.

**Files:** 2 new + 3 modified, +491 LOC.

**Tests added (3):**
- `create_backup_writes_valid_zip`
- `create_backup_includes_manifest_db_and_audio`
- `create_backup_manifest_counts_match_inputs`

**Test count:** 152 → 155

**Quality signals:**

- **Deterministic audio ordering** (sorted by path before archiving). CC noticed это enables third-order benefit: byte-identical backups from identical libraries → SHA256 comparison + cloud sync deduplication potential. **Banked:** «archive entries должны быть deterministically ordered when both source state и archiving operation are determined.»
- **`spawn_blocking` pattern** — first project use. Clone AppHandle before boundary, move clone into closure, drive `app.emit()` from inside blocking task. Captured handle `Clone + Send + Sync`, closure satisfies `Fn(u64, u64) + Send`. **Banked:** «when async command needs to call sync IO-heavy code, use `tauri::async_runtime::spawn_blocking` with cloned AppHandle pattern.»
- **`Fn(u64, u64)` not `FnMut`** — most permissive closure constraint, future-proofs against accidental mutation requirements.
- **Empty-library tolerance** — missing `audio_cache/` OR missing `glagol.db` treated as zero-count, не error. Matches Scenario 10 explicitly. Backup of fresh-install state works.

### Phase 3 — Restore + validation logic (commit `3b32f7c`)

**Scope:** `validate_backup_impl` + `restore_backup_impl` + 3 Tauri commands + path-traversal defense + 5 tests.

**Files:** 1 new + 3 modified, ~595 LOC churn.

**Tests added (5):**
- `validate_backup_rejects_missing_manifest`
- `validate_backup_rejects_count_mismatch` (covers external + internal mismatches as sub-scenarios)
- `validate_backup_rejects_unknown_backup_version`
- `validate_backup_rejects_path_traversal` (zip-bomb defense)
- `restore_backup_replaces_existing_data_and_writes_safety_zip` (combined replacement + safety net validation)

**Test count:** 155 → 160 (kickoff target 158, +2 over per CC's defensible additions: path-traversal test was explicitly required в kickoff но не enumerated, safety-zip fold avoided duplicate fixture setup)

**Quality signals:**

- **`is_unsafe_entry_name` defense-in-depth** — validation + extraction both check path traversal. **TOCTOU defense:** archive could be swapped on disk между validate и restore calls. **Banked:** «security-sensitive operations need TOCTOU-aware re-checking — validate pre-check + execution re-check.»
- **`filename_prefix: &str` parameterization** — `create_backup_impl` serves both normal backups (`glagol-backup-`) и pre-restore safety backups (`glagol-pre-restore-`) через single parameter. **Banked:** «when same primitive serves multiple operational use cases, parameterize don't fork.»
- **DB Mutex release pattern** — Phase 3 command implementation acquires + drops AppState.db Mutex guard before `spawn_blocking`. Released **lock**, but underlying SQLite `Connection` stays alive в `Mutex<Connection>`. CC documented risk inline в paste-back: «if 'file in use' surfaces on real Windows, fallback is `mem::replace` and we'll iterate.» **This honest signaling about residual risk was decisive value-add of Working Agreements protocol — the risk DID materialize в runtime QA.**

### Phase 4 — Frontend UI + integration (commit `ea24585`)

**Scope:** `BackupSection.tsx` component + Settings integration + 4 typed wrappers + CHANGELOG entry.

**Files:** 1 new + 3 modified, ~390 LOC.

**Tests added:** 0 (frontend UI tested via manual QA per Sprint 5b precedent)

**Test count:** unchanged at 160

**Bundle delta:** +6.26 KB JS / +0.25 KB CSS (vs Sprint 5b post-merge baseline 413.27 / 42.73). Drivers: `BackupSection.tsx` (281 LOC), AlertDialog (Radix portal + overlay, two distinct dialogs), Progress component, 2 lucide icons (`Archive`, `Upload`), 4 wrappers + 2 interfaces + 2 consts.

**Quality signals:**

- **Bundle delta over kickoff envelope (+3-5 KB expected, actual +6.26 KB)** — CC explicitly disclosed gap с reasoning. **Banked convention:** «when delta exceeds estimate, документировать почему immediately, не explain away.»
- **Radix AlertDialog vs Dialog semantic distinction** — AlertDialog blocks outside-click by design (destructive action UX), only Escape needs explicit `onEscapeKeyDown` block. **Banked:** «Radix `AlertDialog` vs `Dialog` differ on outside-click default; AlertDialog blocks by design.»
- **`buttonVariants` + `cn`** для destructive variant on AlertDialogAction — shadcn idiom для applying variants на default-button-wrapper components.
- **CHANGELOG preservation strategy** — CC prepended Sprint 5c entry without promoting Sprint 5b entries к `[v0.1.0-rc.5]` section. Reasoning: «leaving them в place avoids accidentally rewriting the rc.5 release notes the user may still be drafting.» Defensible editorial caution. **Deferred:** Sprint 5d batch для full CHANGELOG promotion + master log publication.

---

## PR #30 creation + CI failure + fixup commit

**Process:** CC composed PR body, posted directly without preview step (Sprint 5d trust convention applied — first validation). Web fetch sanity check post-creation found body clean: zero `#N` references, AI footer absent, `Mutex<Connection>` rendered correctly с `&lt;...&gt;` escape (Sprint 5c first learning).

**Banked Sprint 5c earlier:** «PR body HTML sanitizer strips `<...>` content даже inside backticks. Escape angle-bracket generics as `&lt;...&gt;` в any future PR bodies that reference Rust generic types.»

**CI failure on Windows matrix:**

```
error: unused import: `PathBuf`
  --> src\backup\restore.rs:23:23

error[E0425]: cannot find value `BACKUP_FILENAME_PREFIX` in this scope
  --> src\backup\create.rs:331:13
(3 occurrences)
```

**Root cause:** CC ran `rustfmt --check` locally but not `cargo check`. Cloud env (Tauri GTK constraint) was thought to require Windows toolchain для full `cargo check` — wrong assumption. `cargo check` works in cloud, only `cargo test` requires GTK runtime for `commands::backup` → `tauri::AppHandle` linkage.

**Fix commit `27b501a`:**
- Removed `PathBuf` from top-level import в `restore.rs`, added к tests submodule (only used там)
- Added `use crate::backup::BACKUP_FILENAME_PREFIX;` к tests submodule в `create.rs` (3 tests reference const after Phase 3 moved it to `backup/mod.rs`)

**Both fixes minor mechanical** — surfaced by clippy's strict `-D unused-imports` + missing imports после module-level refactor.

**Banked lesson (Sprint 5c critical):** «CC must run `cargo check` minimum locally before pushing branch, в addition к `cargo fmt --check`. `cargo check` catches compilation errors (unused imports, missing imports, type errors) that `rustfmt` does not see. Even when full `cargo test` can't run в cloud env (Tauri GTK constraint), `cargo check` works fine — it only needs Rust toolchain, no system libraries.»

**CI re-ran on push, green, PR merged.**

---

## Post-merge runtime QA — first round

User executed runtime verification protocol на Windows machine:

- ✅ `cargo test`: 160 passed
- ✅ `cargo clippy`: clean
- ✅ `pnpm tsc --noEmit`: clean
- ✅ `pnpm tauri build`: produces `Glagol_0.1.0_x64-setup.exe`
- ✅ Uninstall current Glagol с «оставить данные» (data preservation)
- ✅ Install fresh `Glagol_0.1.0_x64-setup.exe`
- ✅ Launch successful, Settings → «Резервное копирование» section visible

**Scenarios 1-4 PASSED end-to-end:**

1. ✅ Settings UI renders с two buttons + description
2. ✅ Create backup happy path — folder picker → progress modal → success toast «Резервная копия создана: glagol-backup-2026-05-20-122136.zip»
3. ✅ Backup archive integrity — manifest + db + audio_cache structure correct, manifest JSON valid с 6 fields, counts match
4. ✅ Validation rejects non-backup zip — toast «Этот файл не является корректной резервной копией Glagol (отсутствует manifest.json)»

**Минор cosmetic observation на Scenario 4:** error toast contains duplicated «Этот файл не является корректной резервной копией Glagol:» prefix — frontend wrapper plus backend `BackupError::ValidationFailed(reason)` Display impl both add the prefix. **Banked для Sprint 5d cosmetic batch:** «Backend error messages должны NOT include user-facing translation prefix. Frontend wrapper provides context, backend provides specific reason.»

**Scenario 5 BLOCKED — Windows file-locking issue:**

User added 1 test document к Library (creating delta между backup state = 2 docs и current state = 3 docs). Clicked «Восстановить» on `glagol-backup-2026-05-20-122136.zip`, confirm dialog showed correct counts («Сейчас: 3 / В копии: 2»), clicked «Восстановить» button.

**Error toast:**
```
Восстановление не удалось: Ошибка ввода-вывода: Процесс не может
получить доступ к файлу, так как этот файл занят другим процессом.
(os error 32)
```

Windows error code 32 = `ERROR_SHARING_VIOLATION`. File in use was `glagol.db` при `fs::remove_file` step.

**Минор cosmetic observation на Scenario 5:** confirm dialog uses «3 документов» grammar — Russian plural rule violates («3 документа», not «3 документов» for numbers 2-4). **Banked для Sprint 5d cosmetic batch:** «Russian three-form plural rule needed: `pluralize_documents(n)` helper для документ/документа/документов.»

---

## Sprint 5c hotfix — PR #31 (mem::replace + symmetric recovery)

**Risk had been pre-emptively documented** в PR #30 architectural notes:

> «Worth confirming в manual QA scenarios 5 + 7 — if 'file in use' surfaces on real Windows, the fallback is taking the `Connection` out via `mem::replace` and we'll iterate.»

**This was the high-value moment of the session.** Working Agreements protocol caught architectural risk pre-emptively, runtime QA validated it materialized, kickoff already had documented escape hatch — system working exactly as designed.

**4 locked decisions (D1-D4):**

- **D1:** `mem::replace` pattern с symmetric error recovery (Option B). Swap real Connection out for in-memory placeholder before `spawn_blocking`, recover real Connection back into AppState on restore failure.
- **D2:** `restore_backup_impl` signature unchanged. Fix lives entirely at command boundary, не in pure logic.
- **D3:** Zero new tests. Runtime-specific Windows behavior hard to mock cleanly. Manual QA Scenario 5 IS the regression test.
- **D4:** Cosmetic items (error message duplication, Russian plurals) deferred к Sprint 5d natural batch. Hotfix scope tight.

**Implementation (commit `aba65a2`):**

- Single file modified — `src-tauri/src/commands/backup.rs` (+65 / −13 LOC)
- `mem::replace` swap of real Connection for in-memory placeholder before `spawn_blocking`
- Explicit `drop(real_conn)` to close SQLite file handle (load-bearing, не stylistic — Rust NLL could extend lifetime otherwise, reproducing exact bug)
- `try_restore_real_connection(app)` helper для symmetric recovery — best-effort, walks `app_local_data_dir`, checks `glagol.db` exists, opens bare Connection, swaps back
- `let Ok(x) = expr else { return; }` (let-else) used in recovery helper — first project use, stable since Rust 1.65

**Quality signals:**

- **Explicit `drop(real_conn)` source comment** flags load-bearing nature для future maintainers. **Banked:** «when explicit `drop()` is load-bearing для behavior (not just style), inline comment must flag why.»
- **`Connection::open(&db_path)` после `db_path.exists()` check** — defensive, default flags include `SQLITE_OPEN_CREATE` which would silently create empty unmigrated DB. Exists-check prevents this masking real failure mode.
- **No `#[cfg(windows)]` gates** — `mem::replace` unconditionally cross-platform. **Banked:** «defensive patterns that compose без conditional logic preferred over `#[cfg]` gates — simpler mental model, fewer code paths to maintain.»

**PR #31 creation + CI green via subscribe convention:**

CC composed PR body, posted directly (Sprint 5d trust convention second validation). Web fetch sanity check found body clean. CC then called `mcp__github__subscribe_pr_activity` per Sprint 5c trust convention.

**Subscribe convention worked end-to-end** — CC monitored CI events автоматически, surfaced result в чат without user manual relay step. **Sprint 5c trust convention validated на обоих направлениях:** PR body composition + CI monitoring.

**CI green, PR merged.**

**Banked Sprint 5c (new):** «`mcp__github__subscribe_pr_activity` reduces ping-pong when CC trusted to react to CI failures с fix commits — Sprint 5c trust validation. Subscribe still does NOT auto-merge — merge equally needs explicit user `merge it` after CI green.»

---

## Post-hotfix runtime QA — resume

User executed runtime verification protocol second time после hotfix merge. All gates clean (160 tests passing on Windows, `pnpm tauri build` produces fresh installer). Uninstall + reinstall cycle с «оставить данные» preserved Sprint 5c base data.

**Scenarios 5-8 PASSED:**

5. ✅ Restore happy path с confirm dialog — clicked «Восстановить» → restore proceeded без os error 32 → progress modal advanced → success toast
6. ✅ Pre-restore safety net `glagol-pre-restore-*.zip` appeared in same folder as source backup, captured pre-restore state (3 docs)
7. ✅ Restore progress + auto-restart — progress modal updated, success toast, ~2-second delay, app restarted automatically
8. ✅ Post-restart Library shows backup contents (2 docs from backup, NOT 3 docs from pre-restore state). Audio playback works.

**Plus bonus validation от user runtime report:**

- ✅ Phase 4 inline title editing (Sprint 5b shipping value) still works post-Sprint 5c restore
- ✅ Audio download (Sprint 2 feature) still works
- ✅ Audio delete (Sprint 2 feature) still works
- ✅ All prior Sprint 1-5b functionality intact, no regressions

**Scenarios 9-10 SKIPPED** by user decision. Reasoning (documented inline в чат):

- Scenario 9 (cross-machine simulation) tests architecturally equivalent code paths к Scenarios 5+8 from empty starting state
- Scenario 10 (backup of empty library) tests backup creation path already validated в Scenarios 2+3
- Real-user QA proven via user's primary scenarios

**Decisive 8/10 PASSED — all blocking scenarios green, Sprint 5c shipping value validated end-to-end.**

---

## Closure tag pushed

```powershell
git tag -a v0.1.0-rc.6 -m "Sprint 5c closure: backup and restore library via .zip archive"
git push origin v0.1.0-rc.6
```

**Minor PowerShell quirk noted:** user pasted command block с reversed order (push first, tag second). PowerShell executed linearly — first `git push` succeeded (something committable was там), then `git tag` failed с «already exists» (tag had been created by some earlier sequence). Harmless error; tag landed on GitHub correctly. **Banked:** «PowerShell paste blocks с multiple git commands need careful ordering verification.»

**Tag `v0.1.0-rc.6` confirmed on GitHub.** Sprint 5c officially closed.

---

## Sprint 5c lessons-learned (consolidated)

Anti-patterns + conventions banked from this session, building on Sprint 5b's foundation of 10:

11. **Working Agreements protocol-documented risk pays off in real time.** PR #30 architectural notes pre-emptively flagged Windows file-locking risk + named the fallback. Risk materialized в Scenario 5 manual QA. CC applied documented fallback в hotfix PR #31. **Net result:** instead of debugging cycle (user reports bug → investigate → research → propose fix → test), straight pipeline (risk documented → materialized → applied known fix). Working Agreements paid off concrete time savings. **Mitigation locked:** «when CC identifies architectural risks pre-implementation, document them inline в PR body OR kickoff D-decision notes with explicit named fallback. Future debugging starts with checking existing notes, not blank investigation.»

12. **CC must run `cargo check` minimum locally.** Sprint 5c CI failure cycle taught the lesson — `rustfmt --check` alone is too narrow. `cargo check` works в cloud env without Windows toolchain или GTK runtime. **Mitigation:** locked в Sprint 5d CLAUDE.md update batch.

13. **PR body HTML sanitizer strips `<...>` content даже inside backticks.** Escape angle-bracket generics как `&lt;...&gt;`. **Discovered Sprint 5c PR #30** — `Mutex<Connection>` stripped first attempt. **Banked.**

14. **`mcp__github__subscribe_pr_activity` reduces ping-pong cycles.** CC trusted к react к CI failures с fix commits without user manual relay step. Sprint 5c trust convention validated. Subscribe does NOT auto-merge — user retains merge control.

15. **CC composes PR descriptions during paste-back-first protocol.** Sprint 4 → 5a → 5b → 5c (4 consecutive validations). Chat reviews via web_fetch, не rewrites. **Trust convention locked.** Removes one round per PR.

16. **`spawn_blocking` pattern for async-with-blocking-work.** Clone AppHandle before boundary, move clone into closure, drive `app.emit()` from inside blocking task. First project use Sprint 5c Phase 2. **Mitigation:** pattern established for future IO-heavy async commands.

17. **`mem::replace` pattern для resource ownership transfer.** Sprint 5c hotfix introduced. Generalizable to other cases где `Mutex<Resource>` needs explicit resource closure before destructive operation. **Banked:** «when destructive operation requires resource release that Mutex guard drop doesn't guarantee, `mem::replace` с placeholder transfers ownership explicitly.»

18. **Explicit `drop()` can be load-bearing for behavior.** Sprint 5c hotfix introduced as documented pattern. Without explicit drop, Rust NLL may extend binding lifetime, defeating intentional resource release timing. **Banked:** «when explicit `drop()` is load-bearing, inline comment must flag why.»

19. **`let-else` (Rust 1.65 stable) preferred для best-effort recovery paths.** Sprint 5c hotfix first project use. Flatter than nested match/if let. **Banked:** preferred idiom for early-bail Result/Option handling в recovery helpers.

20. **TOCTOU defense через validate + extract re-check.** Sprint 5c Phase 3 path-traversal defense. Archive could be swapped on disk между separate calls — defense-in-depth pattern catches this. **Banked.**

21. **Defense-in-depth архитектурный pattern без `#[cfg]` gates.** Sprint 5c hotfix `mem::replace` works unconditionally на all platforms. Linux/macOS behavior unchanged (Connection dropped, swapped back — both succeed silently). **Banked:** preferred over conditional compilation when defensive primitive composes cleanly.

22. **«Архивация для удобства, не для сжатия»** product framing drove Sprint 5c compression strategy. User's framing was right level of abstraction — implementation followed. **Banked methodologically:** ask «what is this for» before choosing tools.

23. **Pre-emptive engineering-elegance-vs-user-pain check.** Sprint 5b lesson applied actively. Configurable library closed as «not feature» — disk space pain didn't exist (cheap disks + download already available), cross-machine pain did exist (real friction). Backup/restore addresses real pain.

---

## Cumulative project status

**Closed sprints:** 0, 1, 2, 3, 3a, 4, 5a, 5b, 5c (9 closures)
**Test baseline:** 160 (was 147 pre-Sprint-5b + 3 Sprint 5b Phase 4 + 10 Sprint 5c backup)
**Open issues:** 1 (Issue #16, Sprint 5d/5e work)
**Sprint 5c shipping value:** backup/restore via .zip с cross-machine portability, safety nets, auto-restart
**Closure tags accumulated:** `v0.1.0-rc.4` (Sprint 5a) → `v0.1.0-rc.5` (Sprint 5b reduced scope) → `v0.1.0-rc.6` (Sprint 5c + hotfix)

**Health check:** **excellent**. Sprint 5c proved CC + Working Agreements protocol + user QA collaboration shipping Tier 1 features cleanly. Risk-documented + risk-materialized + risk-resolved cycle ran like clockwork. Single CI failure cycle, single hotfix cycle — both expected categories caught early.

**Sprint 5c cost:** ~6-8 hours wall clock. **Output:** working backup/restore feature + 10 new tests + 12 new banked conventions + zero regressions on prior Sprint work.

---

## Sprint 5d entry locked + backlog for next session

**Sprint 5d primary feature:** SaluteSpeech remaining-character counter UI.

User stated explicitly: «После этого займемся счетчиком оставшихся символов на Сбер-аккаунте, чтобы пользователь знал.»

**Architecture preview (from compass artifact + existing schema):**

```sql
CREATE TABLE api_usage (
  month         TEXT PRIMARY KEY,    -- '2026-05'
  chars_used    INTEGER DEFAULT 0,
  recognitions_seconds INTEGER DEFAULT 0
);
```

Sprint 5d will add migration v2 для api_usage table, increment `chars_used` on each successful synthesis в `synthesize_document_impl`, Settings section displaying «Использовано в этом месяце: 12,345 / 200,000 символов» с progress bar. Likely Tier 1 feature similar scope к Sprint 5c.

**Sprint 5d secondary batch items** (cosmetic + tooling):

1. **CHANGELOG promotion** — `[Unreleased]` → `[v0.1.0-rc.5]` (Sprint 5b entries) и `[v0.1.0-rc.6]` (Sprint 5c entries). Sprint 5c PR #30 + #31 entries promoted simultaneously. Editorial work batched с master log publication.
2. **Error message duplication cleanup** — `BackupError::ValidationFailed` Display impl removes user-facing prefix, frontend wrapper provides context (Scenario 4 cosmetic observation).
3. **Russian plural forms** — `pluralize_documents(n)` helper для документ/документа/документов rule + audio файлов equivalent. Apply к confirm dialog wording (Scenario 5 cosmetic observation).
4. **CLAUDE.md Working Agreements section update** — Sprint 5b's 10 banked conventions + Sprint 5c's 13 new banked conventions consolidated. Major doc update worth its own commit.
5. **CLAUDE.md «Last updated» timestamp** — deferred since Sprint 5b. Sprint 5d natural batch.
6. **pnpm 11.1.1 → 11.1.3 toolchain bump** — minor version update, low risk.
7. **ESLint + Vitest frontend testing setup** — significant tooling investment, may merit own Sprint depending on scope.
8. **NSIS auto-launch checkbox removal** (Sprint 5b deferred) — requires custom NSIS template.
9. **Library row drag-to-reorder** — backlog item, conditional on signal.

**Decision needed Sprint 5d entry Q&A:** which Tier 2 items batch с character counter (Tier 1) vs defer further. CHANGELOG promotion + CLAUDE.md update are natural batch (editorial work coupled). pnpm bump trivial enough к include. Other items size-dependent.

**Energy + scope discipline reminder:** Sprint 5b cautionary tale — original kickoff envisioned three features per Sprint, runtime QA exposed architectural misalignment, scope reduced. Sprint 5c shipped one feature cleanly. Sprint 5d should follow Sprint 5c pattern: one Tier 1 + cosmetic batch, не two Tier 1's bundled.

---

## Session timing

Start of session: ~07:00 local time (Day 7 fresh start after Day 6 Session 3 pause)
End of session: ~14:30 local time (Sprint 5c closure tag pushed)
Total wall clock: ~7.5 hours

**Comparison к Sprint 5b session:** Sprint 5b ~10-12 hours включая scope reduction discovery + Path B revert kickoff. Sprint 5c ~6-8 hours including hotfix + closure. **Sprint 5c more efficient** — no scope creep, no Path B-style architectural pivot, single feature shipped clean.

Sprint 5d next session pickup: scope discussion + Q&A round + kickoff. Estimated ~1.5-2 hours for character counter feature design (architecturally simpler than backup/restore — single migration + increment hook + Settings display).

---

*Created by Dmitriy + Claude*
