# Day 6 Session 3 — Master Log

**Date:** Tuesday, May 19, 2026
**Sprint:** Sprint 5b (configurable library location + inline title editing → scope-reduced MVP)
**Outcome:** PR #27 merged, PR #28 hotfix merged, scope-reduction PR #29 kickoff drafted (implementation pending Day 7+)
**Closure tag status:** `v0.1.0-rc.5` deferred to Sprint 5b scope-reduction PR closure

---

## Session arc summary

Single session covering full Sprint 5b lifecycle from kickoff through merged shipping code, **plus** mid-session product-level realization that triggered scope reduction. Six distinct phases sequentially:

1. Sprint 5b kickoff + 6-question architectural Q&A round
2. Pre-Phase-1 verification (V1-V6 reality checks + 2 chat-approved deviations)
3. Phases 1-5 sequential implementation (PR #27, 5 commits squashed)
4. PR #27 autolink fix + merge
5. Post-merge runtime QA discovery: `\\?\` Windows extended-length path prefix bug
6. Hotfix PR #28 (`dunce` crate integration) + Windows CI test-assertion catch + merge
7. Product-level reframe: dual-root fallback doesn't solve user's actual disk-space pain
8. Scope reduction kickoff drafted (revert Phases 1-3 + hotfix; preserve Phase 4 + Phase 5)

Session length significant — natural pause point at end with kickoff ready for Day 7 implementation.

---

## Sprint 5b Q&A round outcomes (6 locked decisions)

Pre-implementation Q&A established architectural locks. All decisions chat-approved with «1+ / 2+ / 3+ / 4+ / 5+ / 6+» compact ack.

- **D1 (config.rs module design):** `%LOCALAPPDATA%\app.glagol.desktop\config.json`, JSON via serde_json, atomic temp-rename persistence, `Mutex<Config>` (not tokio), eager load on startup, malformed JSON → defaults with warning (preserve file)
- **D2 (migration UX):** Option C1 — dual-root resolution + scope additive. No migration, no schema change, transparent legacy file fallback. **This decision became the misalignment that triggered scope reduction at session end.**
- **D3 (dynamic scope):** Option α — custom registration via `app.asset_protocol_scope().allow_directory()`, no `tauri-plugin-persisted-scope` dep. Static scope `["$APPLOCALDATA/audio_cache/**"]` stays as permanent default allowance
- **D4 (folder picker validation chain):** 5 steps — is_absolute → create_dir_all → test-write probe → canonicalize comparison vs default → store with `null` for default-collapse semantics
- **D5 (inline title editing UX):** Pencil icon (lucide-react, 16px) always visible + Enter/Blur/Esc commit paths + silent revert on empty
- **D6 (backend command shape):** `update_document_title(id, title)` + `repo::update_title` returning `rows_affected` (rusqlite::Error, no DbError enum needed)

---

## Pre-Phase-1 verification — exemplary practice

CC ran verification pass before Phase 1 coding, surfacing 6 findings + 2 deviation requests:

**Quick confirmations (V1-V6):**
- V1: `productName` actually lowercase `"glagol"` — Phase 5 change is real work, not no-op
- V2: `serde_json` already in `Cargo.toml` — no add needed
- V3: `setup()` lives in `lib.rs::run` line 38, not `main.rs`
- V5: `audio_cache_root` signature is `(app: &AppHandle) -> Result<PathBuf, String>` — keep as-is, read AppState internally
- V6: `src/lib/types.ts` doesn't exist — inline `LibraryPathInfo` in `tauri.ts` following existing pattern

**Deviation A (chat-approved):** `DbError::NotFound` variant doesn't exist anywhere. Existing `delete` function returns `Result<usize, rusqlite::Error>` with command layer translating 0-rows к user-facing error. CC proposed `update_title` follow same pattern. **Rationale ack:** preserves D6 «not found» contract through mechanism adaptation; no new enum infrastructure (YAGNI).

**Deviation B1 (chat-approved):** `setup()` ordering needs to change. Current Sprint 5a ordering creates `audio_cache_root` BEFORE `app.manage(AppState)`, but Sprint 5b needs config in AppState before paths.rs reads it. CC proposed B1 (reorder setup()) over B2 (pass Config to audio_cache_root). **Rationale ack:** localizes change to single function; preserves «no call-site churn» locked goal from V5.

**Process value of this verification step:** Third consecutive instance (Sprint 4 Phase 1 Pdfium pivot → Sprint 5a Phase 1 Pdfium bundling fix → Sprint 5b pre-Phase-1 reality check) where pre-implementation Q&A catches problems before manifesting in code. Working Agreements protocol vindicated.

---

## Phase implementation log

### Phase 1 — Config foundation (commit `d15ecc1`)

**Scope:** New `config.rs` module + `AppState` integration + setup-hook reorder + dynamic scope registration

**Files:** 1 new + 4 modified, +275/-10 net

**Tests added (4):** `load_returns_defaults_when_file_missing`, `load_returns_defaults_when_file_malformed_and_preserves_file`, `save_then_load_round_trip`, `save_uses_atomic_temp_rename_leaving_no_stray_tmp`

**Test count:** 147 → 151

**Quality signals worth banking:**

- **Test thoroughness exceeded kickoff spec.** Malformed-JSON test additionally asserts file remains on disk (D1 «preserve for user inspection» contract). Atomic-write test verifies `.tmp` doesn't linger post-save.
- **Clippy + fmt gates added self-imposed beyond kickoff requirement.** `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` from Phase 1 forward. Established as Sprint 5b convention.
- **`config.clone()` once at setup** rather than re-locking mutex for `allow_directory` call after `manage()`. Cheap clone (small struct), avoids re-locking pattern.

### Phase 2 — paths.rs dual-root resolution (commit `09cf3eb`)

**Scope:** `paths.rs` only — strict scope discipline

**Files:** 1 modified, +195/-26

**Tests added (3):** `audio_cache_root_impl_returns_configured_when_present`, `audio_cache_root_impl_returns_default_when_no_configured`, `resolve_audio_path_impl_dual_root_fallback_and_collision` (3 inline scenarios)

**Test count:** 151 → 154

**Quality signals:**

- **Mutex discipline:** `read_configured_library_path` holds guard ONLY for `Option<PathBuf>` clone, never across fs operations. Convention worth locking: «hold mutex only for the data extraction, never across IO».
- **No-config fast path:** `None` branch literal `default.join(relative)` with zero `.exists()` syscalls. Existing users (pre-Sprint-5b, all `library_path: None`) get exactly zero behavioral or performance impact. Backward-compat provably regression-free at syscall level.
- **Panic policy distinction:** `try_state::<AppState>()` None → panic (programming error: setup-ordering bug); `app_local_data_dir()` OS error → `Result<_, String>` propagation (runtime). Correct error-class taxonomy.
- **Clippy `unnecessary_literal_unwrap` caught in test composition** (second occurrence in Sprint 5b). Pattern: don't `Some(x).unwrap()` immediately after construction; capture inner value in local first. Banked as anti-pattern.

### Phase 3 — Settings UI + commands (commit `ada922f`)

**Scope:** First cross-boundary phase — backend + frontend simultaneously

**Files:** 2 new + 5 modified. Rust ~370 LOC across 4 files; TypeScript ~165 LOC across 3 files

**Tests added (2):** `get_library_path_info_pure_logic_for_both_branches`, `set_library_path_with_paths_full_validation_chain` (5 inline sub-scenarios: A happy + canonicalize, B idempotent re-save, C empty-string reset, D non-absolute rejection, E default-collapse to None)

**Test count:** 154 → 156

**Bundle delta (vs Phase 2 baseline 411.87 KB JS / 41.31 KB CSS):** +2.26 KB JS / +1.34 KB CSS. Drivers: 2 new lucide icons (FolderOpen, RotateCcw), LibraryPathSection component, getLibraryPath/setLibraryPath wrappers.

**Quality signals:**

- **`commit_library_path` rollback semantics:** snapshot → mutate → persist → revert-on-failure. Same primitive composition as rusqlite Transaction-on-Drop. Convention locked: «AppState mutations that persist to disk: snapshot → mutate → persist → revert-on-failure».
- **`fs::canonicalize` requires path to exist** — pre-create `default_root` before canonicalization in validation step 5. Silent-failure-in-production category catch.
- **`paths::default_audio_cache_root_under(&Path)` pub(crate) exposure** — clean architectural decision avoiding re-routing through AppHandle (which would re-acquire same mutex we're already inside).

### Phase 4 — Inline title editing (commit `de143f5`)

**Scope:** Vertical slice — backend command + repo function + Library.tsx pencil affordance + edit mode

**Files:** 5 modified. Rust ~118 LOC (31 module work + 87 inline tests); TypeScript ~130 LOC

**Tests added (3):** `repo::update_title_returns_one_row_affected_and_persists_change`, `repo::update_title_returns_zero_rows_affected_for_unknown_id`, `commands::storage::update_document_title_impl_full_validation_chain` (4 inline sub-scenarios)

**Test count:** 156 → 159

**Bundle delta (vs Phase 3 baseline 414.13 KB JS / 42.65 KB CSS):** +1.40 KB JS / +0.22 KB CSS. Pencil icon + edit-mode logic + commit/cancel handlers + wrapper.

**Quality signals:**

- **`inFlight` ref guard for Enter-then-Blur race** — sync ref, not async state. Enter `preventDefault() + commit()` triggers blur from input collapse via `setDraft(null)`; without ref guard, blur handler re-enters `commit()` before first call's state propagates. Anti-pattern banked: «React triple-commit paths (Enter/Blur/Esc) need sync flag guard against double-fire — useRef, not useState».
- **Triple commit path discipline:** Enter / Blur / Esc all thin wrappers around one `commit()` function. Single point of change, single point of failure. Same pattern Sprint 1 established for synthesize.
- **Optimistic update with error revert:** `handleRename` never throws, so row leaves edit mode unconditionally. UI state machine doesn't depend on async success/failure. Toast handles errors orthogonally.
- **Mid-test re-read pattern** in storage validation test (B/C don't disturb A's persisted title) — catches partial-fail regressions if Sprint 5c+ adds side effects.

### Phase 5 — Polish (commit `6c79093`)

**Scope:** Pure polish — zero new tests, zero functional change

**Files:** 2 modified. `CHANGELOG.md` `[Unreleased]` → `[v0.1.0-rc.4]` promotion + new `[Unreleased]` with Sprint 5b entries. `src-tauri/tauri.conf.json` `productName: "glagol"` → `"Glagol"`.

**Quality signals:**

- **CHANGELOG `v` prefix preserved** despite kickoff text saying `[0.1.0-rc.4]` without prefix — CC chose consistency with existing file shape over verbatim transcription. Documented in commit message. Solid judgment call.
- **`pnpm tauri build` not re-run in cloud env** (no Windows MSVC toolchain). Installer filename verification deferred to post-merge manual QA + CI workflow. Zero compilation risk for productName string change.

---

## PR #27 creation + autolink fix

**Process:** Paste-back-first protocol per CLAUDE.md Working Agreements. CC composed PR body, posted preview to chat, received approval, then called `mcp__github__create_pull_request`. Followed immediately with `mcp__github__update_pull_request` to strip auto-injected `_Generated by [Claude Code]_` footer.

**Web fetch sanity check caught autolink misfire:**

Body contained `(matches the rusqlite Transaction-on-Drop posture from PR #16)` — GitHub autolinked `#16` to **Issue #16** (UX toasts for SaluteSpeech, single remaining open issue), not to logical PR #16 (refactor synthesize for persistence, actually GitHub PR #18 per Day 5 Session 1 master log).

**Identical pattern to Sprint 5a's `#5` autolink misfire.** Third Sprint with same anti-pattern → triggers codification rule.

**Fix:** Single `mcp__github__update_pull_request` call replacing parenthetical with «same Transaction-on-Drop posture used for DB persistence». No version reference at all — engineering trivia about which prior PR pioneered a pattern is autolink-risky and not load-bearing for review.

**Lesson banked:** «`#N` references in PR bodies are autolink-risky unless the integer IS the actual PR/issue target. When tempted to cite engineering history via PR number, inline-prose it or omit entirely.»

**PR #27 merge:** User performed squash-merge via GitHub web UI. Main HEAD `3c5bdae..c5281ed` fast-forward.

---

## Post-merge runtime QA — bug discovery

CC's automated quality gates all passed pre-merge:
- `cargo test`: 159/159
- `cargo clippy`: 0 warnings
- `cargo fmt --check`: clean
- `pnpm tsc --noEmit`: clean

Local runtime verification on user's Windows machine surfaced **two distinct issues** mid-QA:

### Issue 1: `pnpm-workspace.yaml` `msw` build approval

`pnpm tsc --noEmit` first run triggered node_modules recreation prompt (state inconsistent after repo move C:\ → D:\). After approval, install completed but pnpm 10+ default-secure behavior blocked exit code due to `msw@2.14.6` build script not approved.

Resolution: `pnpm approve-builds` → space → enter → permanent decision committed in `pnpm-workspace.yaml` (`msw: true` line added). Direct main commit (infrastructure/tooling, not code change).

### Issue 2 (critical): `\\?\` Windows extended-length path prefix

User selected `D:\GlagolLib` via folder picker. UI displayed `\\?\D:\GlagolLib` (verbatim canonical form). Diagnostic checks revealed:

- ✅ `config.json` persistence working — `"library_path": "\\\\?\\D:\\GlagolLib"` on disk
- ❌ Default audio_cache empty — new synthesis not going there
- ❌ `D:\GlagolLib` empty — new synthesis not going there either
- ❌ Old documents stop playing after path change
- ✅ Reset → default → old documents play again

User's instinct was sharp: «возможно ли такое поведение если в системе уже установлен был глагол с помощью инсталлятора?» — caught hypothetical complication that wasn't actually the bug, but demonstrated proper debugging instinct (verify environment state before blaming code). After user uninstalled prior release + installed fresh `pnpm tauri build` artifact, bug persisted → confirmed code-side issue not environment.

**Root cause identified:** `std::fs::canonicalize` on Windows unconditionally returns extended-length path form (UNC verbatim prefix `\\?\`). Sprint 5b D4 validation step 5 stored canonical form verbatim. The `\\?\` prefix contaminated downstream:

- Asset protocol scope glob comparison: prefixed scope vs unprefixed URL paths → 403 Forbidden
- `fs::write` to prefixed path: inconsistent landing across Windows API layers
- `paths::resolve_audio_path` dual-root: comparison against prefixed PathBuf returning wrong existence results

**Why Sprint 5b tests didn't catch this:** `set_library_path_with_paths_full_validation_chain` used `tempfile::TempDir` paths short enough that `fs::canonicalize` didn't consistently return `\\?\` form. Test asserted round-trip equivalence (storage round-trips) but not path form constraints (no extended-length prefix). Bug surfaced only at runtime with user-chosen paths that consistently triggered prefix.

---

## Hotfix PR #28 — `dunce` crate integration

**Single-phase focused hotfix.** Kickoff drafted with 4 locked decisions:

- D1: Add `dunce = "1"` crate (industry standard for this Windows quirk; used by cargo, rustup)
- D2: `dunce::canonicalize` at save time replaces `fs::canonicalize` in `set_library_path_with_paths`
- D3: `dunce::simplified` at load time normalizes any pre-hotfix `\\?\`-prefixed values in existing `config.json` files (backward-compat)
- D4: No proactive disk rewrite — heal naturally on next legitimate save, keep load as pure read

**Implementation (commit `80e6e13`):** 3 files modified, ~55 LOC including tests. New tests:
- `config_load_normalizes_extended_length_prefix_from_legacy_disk_format` (cross-platform)
- `set_library_path_with_paths_does_not_emit_extended_length_prefix_on_windows` (cfg(windows))

Test count: 159 → 161 on Windows; 160 on Linux (cfg-gated test filtered).

**PR #28 creation:** Clean autolinks (CC explicitly verified: «zero `#N` references except #27 which is the actual prior PR being referenced»). Autolink discipline internalized — pattern from Sprint 5a + Sprint 5b now actively prevented at composition time.

**CI test-assertion catch:**

Windows CI failed on `set_library_path_with_paths_full_validation_chain` (existing Sprint 5b Phase 3 test):

```
left:  "C:\\Users\\runneradmin\\AppData\\Local\\Temp\\glagol_cmd_config_custom_..."
right: "\\\\?\\C:\\Users\\runneradmin\\AppData\\Local\\Temp\\glagol_cmd_config_custom_..."
```

The test computed expected value via `fs::canonicalize` directly. **Test passed pre-hotfix because production code also used `fs::canonicalize` (both sides equally buggy).** After dunce migration, production stored form is clean (`C:\Users\...` without prefix) but test expected still had prefix → assertion fails on Windows CI.

**This is second-order manifestation of exact pattern CC banked for master log:** «path correctness tests should explicitly assert on path form constraints, not just round-trip equivalence». Production code now correct, but test code retained pre-fix expectation. Single-line audit miss.

**Resolution (commit 2 on same branch):** Replace `fs::canonicalize` → `dunce::canonicalize` in test expected-value computation. Apples-to-apples comparison with production.

**Lesson banked (escalated):** «When migrating path-normalization primitives in production, audit ALL test assertions that compute expected paths — not just write new regression tests. CI on multiple OSes catches mismatches that single-OS dev env misses.»

**PR #28 merge:** Main HEAD `28b58c8..8f10ab0` fast-forward. Squash-merge consolidated 2 commits.

---

## Product-level realization — scope reduction trigger

After hotfix merge, user proceeded with manual QA. Critical observation:

> «файл при изменении папки не прослушивается, а при сбросе снова прослушивается. Это доказывает, что он никуда не уходит, а просто остается в старой папке. Процесс переноса не проходит.»

User framing exposed Sprint 5b D2 design misalignment:

**User's actual pain:** «суть изменения места библиотеки была в том, чтобы не занимать место на моем диске C — потому что если мне нужен будет какой-то файл, я могу его скачать через интерфейс программы»

**What Sprint 5b D2 actually did:** Dual-root fallback **leaves files on C:** + adds new location → user gets **two places taking space**, not one.

**The misalignment:** Engineering elegance (avoid migration complexity) ≠ user pain solution (disk space management).

User proposed two paths:
- **Path A:** Physical migration with blocking modal (move files on path change)
- **Path B:** Revert configurable library entirely (MVP discipline — feature not ready, ship without it, user can delete unwanted files manually)

**User chose Path B:** «выбираю откат. Для MVP вполне достаточно функций.»

This is **textbook product discipline.** Pre-public-release Glagol has zero real users. Better to ship reduced-scope MVP than ship known-broken or known-misaligned feature. Configurable library defers to Sprint 5c with proper user-pain-driven design (Path A: physical migration).

**Notable:** PR #28 hotfix work was technically correct — it solved the `\\?\` prefix bug. But the bug existed in a feature that shouldn't ship in current form. Both PRs (#27 + #28) work was foundationally sound engineering applied to a misaligned feature. This is not waste — it's the cost of learning.

---

## Sprint 5b scope-reduction kickoff (drafted, implementation pending)

**6 locked decisions (D1-D6):** Forward-only `feat:` PR (no `git revert`), CHANGELOG silently deletes unshipped entries, dep tree cleanup (`dunce` + `tauri-plugin-dialog` removed), closure tag `v0.1.0-rc.5` preserved, 11 tests removed (Phase 4 preserved), setup() reverts to Sprint 5a shape.

**Net delta: approximately -940 LOC across 14 files.** Largest deletion in project history.

**Scope preserved (ships in `v0.1.0-rc.5`):**
- Inline title editing (Phase 4, ~150 LOC + 3 tests)
- NSIS installer brand casing `Glagol_0.1.0_x64-setup.exe` (Phase 5)
- CHANGELOG `[v0.1.0-rc.4]` promotion (Phase 5)

**Test count target:** 161 → 150 (-11; pre-Sprint-5b baseline 147, net Sprint 5b addition = +3 tests Phase 4 only)

**File location:** `.scratch/kickoff-day-6-session-3-revert.md`

**Implementation deferred:** Day 7+ fresh session after natural pause point at session end.

---

## Sprint 5b lessons-learned (consolidated)

Anti-patterns banked from this session for future Sprint reference:

1. **Engineering elegance ≠ user pain solution.** Dual-root fallback was technically beautiful; failed to solve disk-space pain. **Mitigation:** When designing features, first articulate user pain explicitly, then check that proposed solution actually solves THAT pain (not adjacent technical problem).

2. **Path correctness tests must assert on form, not just round-trip equivalence.** Both Sprint 5b Phase 3 test and hotfix Phase 1 test had this gap; surfaced via Windows CI. **Mitigation:** When writing tests for OS-specific path handling, explicitly assert on form constraints (no `\\?\` prefix, canonical equivalence) in addition to round-trip checks.

3. **When migrating path-normalization primitives, audit ALL test assertions that compute expected paths.** Don't just write new regression tests. **Mitigation:** Sprint 5b migration of `fs::canonicalize` → `dunce::canonicalize` required test-assertion audit; the failing test was a known existing test, not new code.

4. **`#N` references in PR bodies are autolink-risky.** Third Sprint with same pattern (Sprint 5a `#5`, Sprint 5b `#16`). **Mitigation locked:** «When tempted to cite engineering history via PR number, inline-prose it or omit entirely. Only `#N` permitted is the actual PR/issue target.»

5. **React triple-commit paths (Enter/Blur/Esc) need sync flag guards.** `useRef` not `useState` — state updates are async through render. **Mitigation:** Pattern documented for future inline-edit work.

6. **Mutex discipline:** hold guard only for data extraction, never across IO. **Mitigation:** Convention locked for all future `AppState` consumers.

7. **AppState mutations that persist to disk need rollback semantics:** snapshot → mutate → persist → revert-on-failure. **Mitigation:** Convention locked for future `Config` field additions or new persistent state.

8. **`clippy::unnecessary_literal_unwrap` in test composition** caught twice in Sprint 5b. **Mitigation:** Don't `Some(x).unwrap()` immediately after construction; capture inner value in local first.

9. **CHANGELOG promote convention deviation from kickoff text is acceptable** when it preserves existing file shape consistency. CC's choice of `[v0.1.0-rc.4]` with `v` prefix over kickoff's `[0.1.0-rc.4]` was correct.

10. **Pre-implementation verification step pays off.** Third consecutive Sprint where verification round caught problems before code (Sprint 4 Pdfium pivot, Sprint 5a Pdfium bundling, Sprint 5b V1-V6 + 2 deviations). **Mitigation:** Working Agreements protocol continues to require verification for all Sprints touching existing production code paths.

---

## Sprint 5c backlog updates from this session

Items added or refined for Sprint 5c kickoff Q&A:

1. **Configurable library location with physical migration** (Path A). Blocking modal with progress UI. Partial-failure semantics: rollback or retry on rename failure mid-loop. Atomic guarantee per-file (rename succeeds or original preserved). Estimated 1-2 hours of fresh-energy implementation including Q&A round.

2. **Backup + restore via .zip** (new this session, user-proposed). Two buttons in Settings:
   - «Создать резервную копию» → folder picker → `glagol-backup-2026-MM-DD.zip` contains `glagol.db` + all `audio_cache/*.wav`
   - «Восстановить из резервной копии» → file picker (.zip) → confirm dialog «overwrites current data» → unzip + replace + restart app
   Atomic unit: DB + audio together (DB without WAV is metadata orphan, WAV without DB is unnamed file). Decision: 2 buttons, not split into 4 (user mental model doesn't separate them). Likely Sprint 5d or 5e timing — logically follows configurable library Sprint 5c work.

3. **pnpm 10+ approve-builds decisions persistence** — already addressed mid-session (msw entry in `pnpm-workspace.yaml`). Sprint 5c should batch this with potential ESLint + Vitest setup in single tooling-cleanup PR.

4. **`pnpm 11.1.1 → 11.1.3` toolchain bump** — flagged during session, deferred. Sprint 5c tooling-cleanup batch.

Existing Sprint 5c backlog items unchanged:
- NSIS auto-launch checkbox removal (requires custom NSIS template)
- SmartScreen screenshot (environment blocker — needs domain reputation accumulation)
- CLAUDE.md "Last updated" timestamp (deferred to Sprint 5d)
- shadcn CLI vs hand-written component policy
- Library row drag-to-reorder
- Tier 2/3 abbreviations expansion
- DOCX table narration tuning
- Drag-and-drop file input (Sprint 4 deferred)
- Issue #16 (SaluteSpeech error toasts)

---

## Cumulative project status

**Closed sprints:** 0, 1, 2, 3, 3a, 4, 5a (7 closures)
**In-flight:** Sprint 5b (PR #27 merged, hotfix PR #28 merged, scope-reduction PR pending Day 7+)
**Test baseline:** 161 on Windows main HEAD `8f10ab0` (will become 150 after scope-reduction merge)
**Open issues:** 1 (Issue #16, Sprint 5c work)
**Sprint 5b shipping value (post-revert):** Inline title editing + NSIS brand casing + CHANGELOG promote
**Sprint 5b discarded work:** Configurable library location feature (Phases 1-3 + hotfix PR #28 dunce integration) — ~940 LOC deletion in scope-reduction PR

**Cost of Sprint 5b configurable library exploration:** ~5-6 hours engineering time across one full session. Output: deep lesson banking about engineering-elegance-vs-user-pain misalignment, plus solid foundation for Sprint 5c proper redesign with Path A (physical migration).

**Net Sprint 5b outcome after scope reduction:** ~150 LOC shipped value (Phase 4 + Phase 5). Original Sprint 5b kickoff estimated «~700+ LOC PR with 4 new dependencies». Actual ships: ~150 LOC + 0 new dependencies (dunce + plugin-dialog both reverted). Closer to Sprint 5a's polish-PR profile than originally scoped Sprint 4-style feature PR.

This is **healthy MVP discipline**, not failure. Shipping less than originally planned to avoid known-misaligned feature is correct prioritization.

---

## Session timing

Start of session: approximately 11:00 local time (Day 6 May 19, 2026, post Session 2 pause)
End of session: late evening, user-initiated natural pause
Total wall clock: approximately 8-10 hours across full session
Cumulative session count for Day 6: 3 sessions (Sprint 4 closure + Sprint 5a closure + this Sprint 5b arc)

Pause point: kickoff drafted for Day 7 implementation. Resume protocol: CC reads `.scratch/kickoff-day-6-session-3-revert.md` and implements per locked decisions. Post-merge: 9-scenario manual QA, then `v0.1.0-rc.5` closure tag.

---

*Created by Dmitriy + Claude*
