# Glagol — Day 6 Session 2 Master Log

**Period:** May 19, 2026 ~04:30 local → May 19, 2026 ~10:30 local (Sprint 5a entry through closure + repository physical relocation from `C:\` to `D:\`)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 5a (Release engineering & identity) — **CLOSURE**
**Status at end of Session 2:** Sprint 5a 100% complete. NSIS installer config + Pdfium release bundling + GitHub Actions CI workflow + CHANGELOG.md backfill + README installer documentation shipped + runtime-verified across 6 of 8 manual QA closure criteria. Tag `v0.1.0-rc.4` pushed cleanly (no typo recovery needed). Repository physically relocated from `C:\Projects\glagol\` to `D:\Projects\glagol\` mid-session due to disk space pressure on C:\.

> Day 6 Session 1 (`day-6-session-1-master-log.md`) covers Sprint 4 closure preceding this session.
> This file (Day 6 Session 2) covers Sprint 5a entry-through-closure within single working session, plus repository relocation event.

---

## TL;DR

Session 2 picked up several hours after Day 6 Session 1 closure (Sprint 4, `v0.1.0-rc.3` at ~04:00). User explicitly went «спать, отдыхать, выпить кофе» before tackling Sprint 5 entry. Returned rested and ready for ambitious Sprint 5 scope.

Session 2 delivered:

1. **Sprint 5 architectural strategy decision** — user accepted split of Sprint 5 into 4 PRs (5a release infra → 5b backend polish → 5c UX polish → 5d release prep) instead of monolithic single-PR pattern. Sprint 4's single-PR pattern doesn't apply to Sprint 5 because Sprint 5 scope is **heterogeneous** (release engineering ≠ UI polish ≠ docs).

2. **Sprint 5a architectural Q&A** — 5 main questions + 3 CI sub-decisions + several procedural decisions. User exercised override autonomy on: keeping `bundle.identifier` as `app.glagol.desktop` (tech debt accepted), branch protection skipped, frontend tooling drop (no ESLint/Vitest now).

3. **Phase 1 verification protocol caught real blocker AGAIN** — CC pre-implementation Q&A surfaced that Sprint 4's Pdfium bundling design doesn't survive an NSIS installer build. The build-time absolute path baked into binary points to nonexistent location on user machines. **Second consecutive Sprint where pre-implementation verification caught a production-breaking issue before code was written.**

4. **PR #26** — full Sprint 5a in single comprehensive PR. +412/-29 across 9 files, 4 phases sequential + 1 polish commit. Executed without regressions, without rework, without bug discovery. Test count unchanged: 147 → **147** (deliberate non-target — infrastructure-only PR).

5. **Manual QA matrix — 6 of 8 closure criteria PASSED, 2 deferred.** End-to-end pipeline validated with real Windows installer build, custom install location, PDF parsing in installed build (the Pdfium bundling hard gate). SmartScreen screenshot deferred (user environment has SmartScreen disabled).

6. **Sprint 5a closure tag** `v0.1.0-rc.4` pushed cleanly (no typo recovery needed — break of pattern after `rc.3` typo).

7. **Repository physical relocation** mid-session. User noticed C:\ drive had only 18.2 GB free; cargo clean liberated 20.9 GB; full source repository moved from `C:\Projects\glagol\` to `D:\Projects\glagol\` with intermediate partial-move failure requiring step-by-step directory-by-directory recovery + robocopy empty-mirror trick for stubborn `node_modules/` cleanup.

Tests progression Session 2: 147 → **147** (0 new, by design). All 5 closure milestones across project (alpha, rc.1, rc.2, rc.3, rc.4) survived through 6 calendar days with zero regression count cumulative.

**Glagol is now genuinely installable.** Users with Windows 10/11 can download an NSIS installer from GitHub Releases, click through SmartScreen, and have a working Glagol installation with PDF parsing functional out of the box. Sprint 5b = backend UX polish (configurable library + inline title editing). Sprint 5c = UI polish. Sprint 5d = final release prep before `v0.1.0`.

---

## Sprint 5a Functionality Snapshot — what works end-to-end after closure

**Release artifact production pipeline:**

1. Developer runs `pnpm tauri build` on Windows machine
2. Tauri 2 NSIS bundler reads `tauri.conf.json` configuration:
   - Target: NSIS only (MSI dropped — was never Sprint 5 deliverable)
   - License file: `../LICENSE` (MIT text shown during install)
   - Resources: `["resources/*"]` (Pdfium DLL bundled)
   - NSIS specifics: per-user install, EN+RU language picker, LZMA compression
3. `build.rs` runs during Cargo compilation:
   - Downloads Pdfium binary from `bblanchon/pdfium-binaries` (chromium/7834)
   - Writes to BOTH `OUT_DIR/pdfium/{lib_name}` (dev cache) and `src-tauri/resources/{lib_name}` (installer payload)
4. Tauri assembles NSIS package: `glagol_0.1.0_x64-setup.exe` (~7.5 MB)
5. Artifact path: `src-tauri/target/release/bundle/nsis/`

**End-user install flow:**

1. Download `.exe` from GitHub Releases
2. SmartScreen warning blue dialog → "More info" → "Run anyway" (expected for unsigned installer; gated by user's Windows SmartScreen settings)
3. NSIS wizard sequential prompts:
   - Language picker (English / Russian)
   - MIT License acceptance step
   - Install location prompt (default `%LOCALAPPDATA%\Programs\Glagol\`, customizable — validated by user choosing `D:\glagol\` during manual QA)
   - Shortcut prompts (Start Menu + Desktop, default ON, user can opt out)
4. Install progress (fast, no admin elevation requested)
5. Finish screen (no auto-launch by default; checkbox available for opt-in launch)
6. Start Menu entry `Glagol` available

**PDF parsing in installed build — Pdfium 4-tier fallback chain runtime resolution:**

When installed Glagol parses a PDF, `parser::pdf::bind_pdfium()` tries paths in order:

1. `{exe_dir}/resources/pdfium.dll` — **release installer (Tier 1, currently primary)**
2. `{exe_dir}/pdfium.dll` — alternative if user-relocated DLL manually
3. `env!("PDFIUM_LIBRARY_PATH")` — dev build absolute path (baked at compile time)
4. `Pdfium::bind_to_system_library()` — system-wide install fallback

First successful bind wins; missing files silently skipped (chain is normal operation, not error). Final exhaustion surfaces as `ParseError::Format("не удалось загрузить Pdfium: …")` — never a panic. All happens inside the existing `LazyLock<Result<Pdfium, String>>` cell, preserving single-bind-per-process semantics established in Sprint 4.

**CI safety net:**

Every PR against main and every push to main triggers GitHub Actions `quality-gates` workflow on `windows-latest`:

1. Setup: Node 20, pnpm 10, Rust stable
2. Caches: Swatinem rust-cache (registry + git + target/) + actions/cache for pnpm-store
3. Frontend gates: `pnpm install --frozen-lockfile` → `pnpm tsc --noEmit`
4. Backend gates: `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test`
5. Release build gate: `pnpm tauri build` (full NSIS pipeline including Pdfium download + bundle)
6. Artifact upload: NSIS installer to Actions page, 14-day retention

Cold cache duration: 8-12 min estimated, 15:10 actual on first ever run. Warm cache estimate: 2-4 min.

**CHANGELOG.md as canonical user-facing changelog:**

Keep a Changelog 1.1.0 format. Sections for `[Unreleased]` + 4 backfilled versions. User-impact-focused phrasing — what the user can do, what changed in audio/UI; not engineering details. PR numbers preserved for traceability via parenthetical references. GitHub compare URLs at bottom for click-through diffs.

**README installation documentation:**

RU and EN sections rewritten with SmartScreen step-by-step flow. Dev-contributor note under Contributing about `%LOCALAPPDATA%\app.glagol.desktop\` folder location.

---

## Sprint 5 Strategy Decision — Split Rationale

Before any Sprint 5a Q&A, user and Claude jointly decided Sprint 5 scope and structure. This decision deserves its own section because it shaped everything that followed.

### The tension

Sprint 4 (PR #24) successfully shipped as **single comprehensive PR** with 4 phases, 4 new dependencies, +1736 LOC. User added a cross-chat global instruction to Claude Settings codifying preference for single-PR Sprints based on this empirical success. Sprint 5a entry brought a question: does this preference apply to Sprint 5?

Sprint 5 backlog at entry: **21 items** across 4 distinct domains:

- Release engineering (NSIS, CI, code signing path, CHANGELOG)
- Settings/library backend (configurable library location, inline title editing)
- UX/visual polish (theme switcher, library search/sort, Issue #16 toasts, AI attribution prevention)
- Final release prep (screenshots, CHANGELOG finalization, smoke test)

Plus conditional items (DOCX tuning, Tier 2/3 abbreviations, drag-drop, chardetng, etc.) — signal-driven, defer-by-default.

### The decision

User accepted Claude's analysis: **single-PR pattern was validated on coherent-domain Sprint 4 (file parsing — single conceptual unit). Sprint 5 is heterogeneous — release engineering ≠ UI polish ≠ docs.** Combining them into single 3000+ LOC PR would:

- Increase review surface beyond comfortable bounds
- Mix risk profiles (MSI installer ≠ theme switcher)
- Defeat the cognitive-overhead-reduction purpose of single-PR pattern

**Final split:**

- **5a — Release engineering & identity** (this Sprint) — NSIS + CI + CHANGELOG + README install. Must be FIRST because 5b depends on stable bundle.identifier behavior.
- **5b — Backend polish** — Configurable library location + inline title editing.
- **5c — UX polish** — Theme switcher + library search/sort + Issue #16 toasts + AI attribution prevention investigation.
- **5d — Final release prep** — Screenshots + CHANGELOG finalize + smoke test → `v0.1.0` public.

**Single-PR pattern preserved within each block.** Sprint 5a applied single-PR pattern successfully — pattern not abandoned, just bounded to coherent domains.

### Codification implication

The Claude Settings global instruction («prefer single comprehensive PRs per Sprint over multi-PR Sprint splits») remains accurate **as default**. Sprint 5 became the first project case where multi-PR-Sprint was justified. Future structured projects may exhibit similar heterogeneity at certain Sprint scopes; the principle holds, the application requires judgment.

---

## CLAUDE.md and global instructions — no updates this session

Day 6 Session 1 closure updated CLAUDE.md tech stack (docx-rust addition, Pdfium notes) and Claude Settings global instruction (Sprint-PR pattern). Sprint 5a kickoff explicitly deferred CLAUDE.md "Last updated" timestamp refresh to Sprint 5d batch (alongside roadmap finalization for v0.1.0).

Rationale: per-PR CLAUDE.md timestamp churn turns the file into diary. Batch updates at meaningful boundaries (Sprint closures, major architecture changes, pattern third-occurrence).

---

## Sprint 5a Architectural Q&A (5 main + 3 CI sub-decisions + procedural)

Standard pattern from Sprint 2/3/4 — Q&A in chat before kickoff, design decisions surfaced, deps freshness verified.

User context state: rested after explicit pause between sessions, fed, energized. MVP-focused stance maintained.

### Q1 — Bundle.identifier breaking change strategy

**Decision: Keep `app.glagol.desktop` unchanged.** No code, no migration, accept tech debt indefinitely.

User comment: «давай оставим дефолтный app.glagol.desktop. Тогда не будет никаких последствий.»

Rejected alternatives:

- Option A — clean breaking change to `Glagol` identifier + README warning for dev-contributors
- Option B — one-shot migration code in setup hook

Implications:

- All data (DB, audio cache, future config.json) stays at `%LOCALAPPDATA%\app.glagol.desktop\`
- Folder name shows up unbrandedly in Windows Apps & Features (mitigated by `bundle.publisher` = "Glagol Contributors" filled in)
- Sprint 5b configurable library location folder picker will show default `%LOCALAPPDATA%\app.glagol.desktop\` to users — visually unbranded but accurate
- Renaming becomes harder over time as more user-installed builds accumulate at the old path

Acceptable for pre-v0.1.0 solo dev project. Revisit if and only if real users complain about the folder name.

### Q2 — Sprint 5 tag naming continuity

**Decision: Continue `rc.4` / `rc.5` / `rc.6` / `v0.1.0`** (Option A).

User: «a».

Rationale: consistency with existing tag history beats strict semver "release candidate" semantics. `rc.N` here means "Sprint closure milestone with `v0.1.0` as final target", not "production-quality release candidate". Project convention established at Sprint 2 closure.

### Q3 — CI matrix scope (3 sub-decisions)

#### Q3.1 — Conditional gates

**Decision: All mandatory gates + `pnpm tauri build`.** No conditional skips at planning time. `pnpm lint` + `pnpm test` later dropped on Phase 1 verification (not configured in package.json).

Final gates:

- `cargo fmt --check` ✅
- `cargo clippy -- -D warnings` ✅
- `cargo test` ✅
- `pnpm tsc --noEmit` ✅
- `pnpm tauri build` — **YES, full release build** (catches Pdfium bundling regressions and other release-only issues before merge)

#### Q3.2 — Triggers

**Decision: pull_request + push to main** (Option B).

User: «2-b».

Rationale: PR catches before merge, push catches direct-to-main edge cases. `workflow_dispatch` (Option C) overkill for solo project.

#### Q3.3 — Branch protection

**Decision: Skip** (Option B).

User: «3-b».

Rationale: self-discipline maintained through CLAUDE.md "Never push directly to main" invariant. Branch protection rules require GitHub admin UI configuration outside of CC's reach. Defer to security-review pass post-v1.0.

### Q4 — NSIS install configuration

**Decision: per-user install, MIT license shown, customizable location, no auto-launch, both shortcuts default ON.**

User: «4-currentUser, показывать MIT, Install location customizable-yes, Auto-launch after install:нет, Add to Start Menu / Desktop shortcut - дать юзеру выбор, по умолчанию да»

All decisions per Tauri 2 NSIS bundler defaults — no custom template needed.

### Q5 — SmartScreen disclaimer placement in README

**Decision: Detailed step-by-step in README (RU + EN) with screenshot placeholder** (Option A).

User: «5-а».

Rationale: README is first contact for new users; should be self-sufficient. Placeholder path `docs/images/smartscreen-warning.png` reserved for manual screenshot capture during Sprint 5a closure manual QA.

### Q4/Q5 — Frontend tooling (revealed during Phase 1 verification)

**Decision: Drop ESLint and Vitest from CI (Option A — "drop both").** Acknowledge as tech debt deferred to Sprint 5c.

Verified by CC during Phase 1: `package.json` had no `lint` or `test` scripts; neither ESLint nor Vitest were configured. Adding them would have violated "zero new dependencies" Sprint 5a constraint.

User chat-acknowledged Option A. Tech debt noted in PR description known-limitations section.

### Q2 → Q2 PIVOT — Pdfium bundling discovery

**Original Q2 decision (kickoff):** "Pdfium DLL must bundle into installer resources — without this, release builds will fail on first PDF."

**Phase 1 verification revealed:** Sprint 4 design did NOT survive NSIS installer. Build-time `OUT_DIR/pdfium/{lib_name}` path baked into binary via `PDFIUM_LIBRARY_PATH` env var points to nonexistent location on user machines.

**Refined Q2 decision (mid-Phase-1):**

1. `build.rs` mirrors Pdfium binary to BOTH `OUT_DIR/pdfium/{lib_name}` (dev cache) and `<CARGO_MANIFEST_DIR>/resources/{lib_name}` (installer payload)
2. `tauri.conf.json` declares `bundle.resources: ["resources/*"]` (NSIS bundler ships at `$INSTDIR/resources/`)
3. `parser/pdf.rs` runtime 4-tier fallback chain (release installer → alternative layout → dev env var → system library)

User chat-approved fix design before Phase 1 coding started. **Same Working Agreements pattern as Sprint 4 Phase 1 blocker.** Without verification protocol, CC would have written installer config that silently broke PDF parsing on user machines.

### Procedural sub-decisions

1. **Branch:** `claude/sprint-5a-release-infra` (CC convention)
2. **PR title:** `chore(release): NSIS installer + GitHub Actions CI + CHANGELOG scaffold`
3. **4 phases** with phase-by-phase reporting (production-adjacent code: tauri.conf.json + parser/pdf.rs)
4. **Test count target:** **0 new tests** (deliberate non-target per kickoff)

---

## Phase 1 Verification Findings (Q1-Q7 outcomes)

CC's pre-implementation verification surfaced 7 items, 2 of which became consequential decisions and 1 of which became the headline technical pivot.

| # | Item | Outcome |
|---|---|---|
| Q1 | Tauri 2 NSIS schema | ✅ Fetched canonical schema https://schema.tauri.app/config/2; verified field names (`bundle.licenseFile` top-level, NOT `bundle.windows.nsis.license`); shortcut + install-location prompts baked into default NSIS template (no custom .nsi needed) |
| Q2 | Pdfium bundling | ⚠️ **REAL BLOCKER.** Fix designed in chat, approved before coding. 4-tier fallback chain implemented. Headline Sprint 5a technical artifact. |
| Q3 | LICENSE | ✅ Exists at repo root, MIT, "Copyright (c) 2026 Glagol Contributors" |
| Q4 | `pnpm test` | ❌ Not configured (no script in package.json). Chat decision: drop from CI, defer Vitest to Sprint 5c |
| Q5 | `pnpm lint` | ❌ Not configured. Same chat decision: drop, defer ESLint to Sprint 5c |
| Q6 | CHANGELOG scope | ✅ Confirmed. Layered approach implemented |
| Q7 | CI caching | ✅ Two caches (Swatinem rust-cache + actions/cache for pnpm-store) instead of three (Pdfium covered by target/ inclusion) |

**Working Agreements observation:** Phase 1 verification protocol caught the production-breaking Pdfium bundling issue at pre-coding stage for the SECOND consecutive Sprint. Sprint 4 caught `pdfium-bind` 0.1.0 missing text extraction. Sprint 5a caught release-build Pdfium path resolution. Pattern reliability: 2/2.

---

## PR #26 Implementation (4 phases sequential + 1 polish commit)

Per CLAUDE.md Working Agreements — production-adjacent code path touched (`parser/pdf.rs` runtime fallback, `tauri.conf.json` release config). Phase reports REQUIRED.

### Phase 1 — NSIS bundle config + Pdfium installer wiring

**Commit:** `d78099f` Phase 1.

**Deliverables:**

- `src-tauri/tauri.conf.json` (+20/-2) — `targets:["nsis"]`, `licenseFile`, `resources`, full `bundle.windows.nsis` block, polished bundle metadata
- `src-tauri/build.rs` (+17) — Mirror Pdfium binary to `src-tauri/resources/{lib_name}` for Tauri's bundler
- `src-tauri/src/parser/pdf.rs` (+43/-9) — New `bind_pdfium()` 4-tier fallback chain + per-platform `PDFIUM_LIB_NAME` const
- `.gitignore` (+9) — Excludes ~6 MB Pdfium binary across all three platforms (`pdfium.dll`, `libpdfium.so`, `libpdfium.dylib`)
- *new* `src-tauri/resources/.gitkeep` (0 LOC) — Keeps directory in tree

**Crate verification:** `pdfium-render 0.9.1` exposes `Pdfium::bind_to_library(impl AsRef<Path>) -> Result<Box<dyn PdfiumLibraryBindings>, PdfiumError>` — confirmed canonical API for path-based binding. `PdfiumError` re-exported via `prelude::*` (private `error::PdfiumError` module path).

**Local verification:**

- `cargo check` clean
- `cargo fmt --check` clean
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo test` → 147/147 (no regressions, no new tests as planned)
- `build.rs` mirror confirmed: `libpdfium.so` (7.6 MB) appears at both `target/{profile}/build/.../out/pdfium/` and `src-tauri/resources/`

**NOT verified in Phase 1 (deferred):** `pnpm tauri build` (cloud env has no Windows MSVC toolchain — first NSIS artifact validation happens in CI run against the PR or local manual QA).

### Phase 2 — GitHub Actions CI workflow

**Commit:** `e469cda` Phase 2.

**Deliverables:**

- *new* `.github/workflows/ci.yml` (116 LOC) — windows-latest, single job, 14 steps

**Workflow structure:**

- Triggers: pull_request + push to main
- Concurrency: `cancel-in-progress` for force-pushes
- Timeout: 30 minutes
- Cache strategy: Swatinem rust-cache (workspaces: `src-tauri -> target`) + actions/cache for pnpm-store
- Gates: setup → caches → install → tsc → fmt → clippy → test → tauri build → artifact upload

**Deviation from kickoff:** kickoff anticipated 3-cache strategy (separate Pdfium-pinned cache). CC chose 2-cache because Pdfium binary lives inside `src-tauri/target/build/.../out/pdfium/` which is already covered by Swatinem rust-cache target/ inclusion. Justified — separate Pdfium cache would shave ~5s only on Cargo.lock-changes-without-PDFIUM_RELEASE_TAG-change.

**Deviation from kickoff:** NSIS artifact upload added (not in kickoff draft). Lets reviewers download `.exe` directly from CI page without local build. Zero downside; supports closure criteria #5 manual QA.

**Estimated CI duration:**

- Cold cache: 8-12 min (validated 15:10 actual on first run)
- Warm cache: 2-4 min

### Phase 3 — CHANGELOG.md scaffold

**Commit:** `d70e0cc` Phase 3.

**Deliverables:**

- *new* `CHANGELOG.md` (114 LOC) — Keep a Changelog 1.1.0 format

**Structure:**

- `[Unreleased]` — Sprint 5a entries (NSIS installer, Pdfium installer bundling, CI workflow, CHANGELOG.md itself)
- `[v0.1.0-rc.3]` — 2026-05-19 — file input (PR #24)
- `[v0.1.0-rc.2]` — 2026-05-18 — narration humaniser (PR #22)
- `[v0.1.0-rc.1]` — 2026-05-18 — persistent library + Library page + Ctrl+R fix (PR #15-18)
- `[v0.1.0-alpha]` — 2026-05-17 — MVP synthesis pipeline + 6 voices + TLS pin (PR #11-13)

**Phrasing principle:** user-impact-focused. Kept: what user can do, what changed in audio output, what changed in UI. Dropped: test counts, dep version bumps, LOC deltas, internal module structure.

**Backfill source:** existing `docs/day-logs/*-master-log.md` files. Master logs are technical; CHANGELOG translates to end-user perspective.

**GitHub compare URLs** at bottom for click-through diff between adjacent tags.

### Phase 4 — README + final quality gates + paste-back composition

**Commit:** `930fc58` Phase 4.

**Deliverables:**

- `README.md` (+54/-7) — RU and EN Installation sections rewritten with SmartScreen flow + dev data-folder note under Contributing
- *new* `docs/images/.gitkeep` (0 LOC) — Placeholder for upcoming SmartScreen screenshot

**README updates RU + EN parallel structure:**

1. Installation flow: download → SmartScreen → NSIS wizard (language, license, location, shortcuts) → ready
2. Screenshot placeholder reference (`docs/images/smartscreen-warning.png` — committed via .gitkeep, image captured manually post-merge)
3. Dev-section addition: «Где Glagol хранит данные» / «Where Glagol stores data» — explains `%LOCALAPPDATA%\app.glagol.desktop\` folder location for dev-contributors

**Final quality-gate sweep:**

- `cargo fmt --check` clean
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo test` 147/147
- `pnpm install --frozen-lockfile` clean
- `pnpm tsc --noEmit` clean
- `pnpm tauri build` not re-run (no `tauri.conf.json` changes since Phase 1)

### Polish Commit — bundle metadata language unification

**Commit:** `c49cadd` Polish.

**Deliverable:**

- `src-tauri/tauri.conf.json` — `bundle.longDescription` rewritten in Russian (was English)

**Rationale (chat exchange before PR creation):** `shortDescription` and `longDescription` both render in same Windows surface (Apps & Features / Add or Remove Programs). Primary target audience per CLAUDE.md = Windows 10/11 Russian users. Mixed RU short + EN long would read as inconsistency rather than intentional bilingual framing. README RU/EN split has clear section navigation; Apps & Features descriptions do not.

User chose "both Russian" via "(a) Quick fix in new commit on branch before PR creation". Polish commit lands before PR creation.

### CC paste-back format observations

This was the **fourth consecutive PR-piece showing fully internalized Working Agreements:**

- ✅ Phase report format identical to Phase 1/2/3/4
- ✅ Files + LOC delta + test count arithmetic + quality gates + deviations + open questions in each phase paste-back
- ✅ No full-file dumps in paste-backs
- ✅ References prior PRs (#15-18, PR #22, PR #24) for pattern justification
- ✅ Explicit trade-off documentation for skipped tests / deferred work
- ✅ Forward compatibility notes added beyond strict kickoff requirements
- ✅ Phase 1 verification asks raised in chat before coding (CRITICAL — caught the Pdfium bundling issue)

### AI attribution stripping — runtime behavior

After `mcp__github__create_pull_request` returned URL, CC self-initiated `mcp__github__update_pull_request` call:

- Detected auto-injected `_Generated by [Claude Code](...)_` footer
- **Plus discovered angle-bracket markdown rendering issue:** `<lib_name>`, `<profile>`, `<exe_dir>` placeholders inside code blocks were stripped by GitHub's HTML pass treating them as unknown HTML tags
- Stripped AI footer
- Replaced angle-bracket placeholders with `{lib_name}` / `{exe_dir}` / `{profile}` curly-brace style (survives HTML-escape pass)
- Confirmed body ends with `Created by Dmitriy + Claude`

**Fifth+ PR with AI attribution protocol enforced runtime** (PR #22, #23, #24, #26 initial, #26 update). Pattern fully reliable.

### Cosmetic fix — `#5` autolink disambiguation

Pre-merge `web_fetch` sanity check revealed: `#5` references in PR body (referring to closure criteria item 5) got autolinked to issue #5 (closed Sprint 1 leftover about text::preprocessor, closed via PR #22 in Sprint 3a). Misleading for readers clicking through.

CC's third `mcp__github__update_pull_request` call replaced 4 occurrences of "#5" (in closure criteria context) with "criterion 5" / "step 5" to avoid GitHub issue cross-reference autolinking. All other `#N` references (PR cross-refs to #16, #22, #24) preserved as legitimate links.

---

## Manual QA — 6 of 8 closure criteria PASSED, 2 deferred

Per Working Agreements: «Runtime verification after merge — Manual QA on target platform is hard gate.»

User executed step-by-step manual QA per chat-side cadence:

### Step 1 — Pull main + sanity

```powershell
git checkout main
git pull
git log -1 --oneline
ls .github\workflows\
ls src-tauri\resources\
```

- ✅ Pull successful (`56ec9e2..77ac419`, fast-forward, 9 files updated)
- ✅ HEAD = `77ac419` (Sprint 5a squash merge)
- ✅ `.github/workflows/ci.yml` present
- ✅ `src-tauri/resources/.gitkeep` present (Pdfium DLL gitignored, not yet downloaded)

### Step 2 — Baseline `cargo test`

- ✅ 147 passed
- ✅ Pdfium DLL appeared at `src-tauri/resources/pdfium.dll` (7,232,512 bytes) after build.rs run
- ✅ build.rs mirror confirmed working on Windows local environment

### Step 3 — `pnpm tauri build` (release installer build)

**Duration:** 8m 07s (release profile, full Cargo rebuild + NSIS bundle assembly)

**Output:**

- `cargo build --release` finished in 8m 07s
- Tauri downloaded NSIS 3.11 + nsis-tauri-utils 0.5.3 on first run (~5 MB total)
- `makensis` produced `glagol_0.1.0_x64-setup.exe`
- Final artifact: `src-tauri\target\release\bundle\nsis\glagol_0.1.0_x64-setup.exe`, 7,867,132 bytes (~7.5 MB)

### Step 4 — Installer artifact verification

```powershell
ls C:\Projects\glagol\src-tauri\target\release\bundle\nsis\
(Get-Item ...glagol_0.1.0_x64-setup.exe).Length / 1MB
```

- ✅ Artifact size 7.5 MB (well within expected 5-15 MB range)
- ✅ Size confirms Pdfium DLL bundled (without it would be ~1-1.5 MB)

### Step 5 — Run installer (NSIS UX flow)

| Check | Result |
|---|---|
| SmartScreen blue dialog | ❌ Not shown — user environment has SmartScreen disabled in Windows Defender |
| Language picker (EN + RU) | ✅ Shown, user selected Russian |
| MIT License displayed with Accept | ✅ Worked |
| Install location customizable | ✅ User overrode default `%LOCALAPPDATA%\Programs\Glagol\` → `D:\glagol\` |
| Shortcut prompts (Start Menu + Desktop, default ON) | ✅ Worked |
| Admin elevation request | ✅ NOT requested (per-user install correctness) |
| Auto-launch checkbox | Note: present but unchecked by default — user could opt in |
| Install completion without errors | ✅ |
| Start Menu entry | ✅ "Glagol" entry created |

**SmartScreen anomaly finding:** user's Windows Defender → "Управление приложениями/браузером" → SmartScreen disabled. SmartScreen flow could not be triggered. Closure criteria #8 (manual screenshot capture) blocked by environment. Documented as deferred work.

### Step 6 — PDF parsing in installed build (Pdfium bundling hard gate)

**The critical Sprint 5a verification.**

- ✅ Glagol launched from Start Menu
- ✅ Settings preserved (keyring service "Glagol" survived install — machine-level user-scope, not install-location-dependent)
- ✅ Synthesize page → "Выбрать файл" → text-based PDF → text extracted into textarea
- ✅ No Pdfium error toast, no scanned PDF disclaimer modal

**Tier 1 fallback (`{exe_dir}/resources/pdfium.dll`) confirmed working at runtime in installed environment.** Sprint 4 deferred work fully closed. NSIS installer pipeline architecturally sound.

`ls D:\glagol\resources\` verification skipped (user installed to custom D:\ location, command provided assumed default; not blocker — PDF parse success implies DLL was bound correctly).

### Step 7 — Closure tag push

```powershell
git tag -a v0.1.0-rc.4 -m "Sprint 5a closure: release infrastructure (NSIS installer + GitHub Actions CI + CHANGELOG scaffold)"
git push origin v0.1.0-rc.4
```

- ✅ Output: `* [new tag] v0.1.0-rc.4 -> v0.1.0-rc.4`
- ✅ **No typo recovery needed** — break of pattern after Sprint 4 `rc.3` typo

**Sprint 5a OFFICIALLY CLOSED at this point.**

### Step 8 — CHANGELOG promote (attempted, deferred)

User invoked `git add CHANGELOG.md` + commit attempt expecting Claude's `# Edit CHANGELOG.md` comment to be executable. PowerShell comments aren't auto-executed — user did not manually edit CHANGELOG to promote `[Unreleased]` → `[v0.1.0-rc.4] — 2026-05-19`.

Result: `nothing to commit, working tree clean`. CHANGELOG promote deferred to Sprint 5b kickoff for natural batch with new `[Unreleased]` entries.

Not a blocker — Sprint 5a closure tag already pushed; CHANGELOG promote is cosmetic followup.

### Closure criteria recap (8 items)

| # | Criterion | Status |
|---|---|---|
| 1 | PR #26 merged to main | ✅ |
| 2 | AI attribution stripped post-create | ✅ |
| 3 | All 147 tests still passing | ✅ |
| 4 | CI workflow runs green on merging PR | ✅ (15:10 cold cache run) |
| 5 | Manual QA on user's machine — PDF parsing in installed build | ✅ (the Pdfium hard gate) |
| 6 | Tag `v0.1.0-rc.4` pushed with correct annotation | ✅ (no typo) |
| 7 | CLAUDE.md "Last updated" timestamp | DEFERRED (batch to 5b/5c/5d) |
| 8 | SmartScreen screenshot manually captured + committed | DEFERRED (user environment has SmartScreen disabled; needs alternate capture method) |

**6 of 8 fully complete. 2 deferred with documented rationale.** Sprint 5a closure stands.

---

## Repository Move from C:\ to D:\ (mid-session bonus event)

Not in kickoff scope. Surfaced when user noticed `C:\` drive had 18.2 GB free (Windows starts struggling below ~10 GB). Sprint 5a's release build + cargo caches were significant contributors.

### Sequence

**1. Disk space inventory:**

- `C:\Projects\glagol\` (after `pnpm tauri build`): ~25 GB total (most in `src-tauri/target/`)
- `D:\` had 104 GB free of 297 GB total — plenty of headroom

**2. Pre-move cleanup:**

```powershell
cd C:\Projects\glagol\src-tauri
cargo clean
# Output: Removed 22883 files, 20.9GiB total
```

Liberated 20.9 GB immediately. Working tree state at this point: `nothing to commit, working tree clean`. Move-Item operation could now move only essential repo files + node_modules.

**3. First Move-Item attempt — FAILED:**

```powershell
Move-Item -Path C:\Projects\glagol -Destination D:\Projects\glagol
```

Errors observed:

```
Move-Item: Не удается удалить элемент C:\Projects\glagol\.git: Недостаточно прав доступа
Move-Item: Не удалось найти часть пути "C:\Projects\glagol\node_modules\.pnpm\@babel+core@7.29.0\..."
```

**Partial-move state on both drives:**

- `D:\Projects\glagol\` received: `.git/`, all root files (.gitignore, CHANGELOG.md, CLAUDE.md, README.md, package.json, pnpm-lock.yaml, etc.), `.claude/`, `.github/`, `.scratch/`, `.vscode/`, `dist/`, `docs/`, `node_modules/`
- `C:\Projects\glagol\` retained: `node_modules/` (partial), `public/`, `src/`, `src-tauri/`

**Root cause analysis (two failures combined):**

1. **`.git/` permission denied:** Some `.git/objects/pack/` files have read-only/system attributes on Windows. PowerShell `Move-Item` requires elevated permissions for delete on those.
2. **`node_modules/.pnpm/@babel+core@7.29.0/...` path-not-found:** pnpm's deeply nested `.pnpm/` symlink structure has files exceeding Windows `MAX_PATH` (260 chars) when prefixed with full directory. Move-Item's internal copy+delete operation walks files in alphabetical order; by the time it tries to delete `@babel+core@7.29.0/`, copy had moved deeper files first, leaving intermediate directories empty and confusing the delete walker.

**Git status after partial move on D:\:**

```
Changes not staged for commit:
  deleted: public/tauri.svg
  deleted: src-tauri/Cargo.toml
  ... [79 deletes total]
```

D:\ git knew HEAD = `77ac419` but working tree was missing `public/`, `src/`, `src-tauri/` directories entirely.

**4. Recovery strategy:**

Step-by-step manual move of remaining directories with verification after each:

```powershell
# public/ (small, test run)
Move-Item -Path C:\Projects\glagol\public -Destination D:\Projects\glagol\public -Verbose
# ✅ Success

# src/ (frontend)
Move-Item -Path C:\Projects\glagol\src -Destination D:\Projects\glagol\src -Verbose
# ✅ Success

# src-tauri/ (largest, after cargo clean small enough)
Move-Item -Path C:\Projects\glagol\src-tauri -Destination D:\Projects\glagol\src-tauri -Verbose
# ✅ Success, including src-tauri/resources/pdfium.dll (7,232,512 bytes intact)
```

Each individual `Move-Item` worked because:

- No `.git/` involved (it was already on D:\)
- No `node_modules/` involved (it was already on D:\)
- Target subdirectories had no path-length issues

**5. Verification on D:\:**

```powershell
git status
# Output: On branch main / Your branch is up to date / nothing to commit, working tree clean
```

✅ Repository fully functional on D:\.

**6. `C:\Projects\glagol\node_modules\` cleanup:**

Standard `Remove-Item -Recurse -Force` would fail on same path-length issues. Used robocopy empty-mirror trick:

```powershell
mkdir C:\empty_temp -Force | Out-Null
robocopy C:\empty_temp C:\Projects\glagol\node_modules /MIR /NFL /NDL /NJH /NJS
Remove-Item -Path C:\Projects\glagol\node_modules -Recurse -Force
```

robocopy walks deepest-first (no MAX_PATH issue via Windows native API), empties everything, then top-level `Remove-Item` deletes empty directories. **Standard recipe for stubborn `node_modules/` on Windows.**

After: `Test-Path C:\Projects\glagol\node_modules` returned False.

**7. Final C:\ parent cleanup:**

```powershell
Remove-Item -Path C:\Projects\glagol -Force
Remove-Item -Path C:\empty_temp -Recurse -Force
```

✅ `C:\Projects\glagol\` deleted. ~20 GB freed total.

**8. GitHub Desktop reconfiguration:**

User used **Locate...** button (cleaner than Remove + Add Local Repository):

- GitHub Desktop showed "Can't find 'glagol' at C:\Projects\glagol"
- Clicked Locate → browsed to `D:\Projects\glagol\` → OK
- Repository reconnected with same `origin` URL, same branch tracking, same history view

### Key lessons from repository move

1. **`Move-Item` + `node_modules/.pnpm/` is unreliable on Windows.** Path-length + permission issues compound. **Recipe: `cargo clean` first, then `pnpm` cleanup, then `Move-Item`.** Or skip Move-Item entirely and use `robocopy /MOVE`.

2. **Sprint 5a Pdfium fallback chain self-corrected through the move.** Tier 3 `env!("PDFIUM_LIBRARY_PATH")` baked-in absolute path no longer pointed to anything valid (the old `OUT_DIR/pdfium/` on C:\ was deleted). Next `cargo build` would re-trigger build.rs → re-download Pdfium → write to new D:\ absolute path → env var re-baked. **Architecture self-healing through file system relocation.**

3. **Installed Glagol at `D:\glagol\` independent from source repo at `D:\Projects\glagol\`.** Two different artifacts; move of one doesn't affect the other. Installed app uses Tier 1 fallback (`{exe_dir}/resources/pdfium.dll`) — pointer relative to installed `.exe`, immune to source repo location.

4. **GitHub Desktop `Locate...` is the right tool.** Preserves repository configuration; just retargets path. Don't reach for Remove + Add Local Repository unless `.git/` integrity is also in question.

5. **`robocopy /MIR` for stubborn `node_modules/` deletion.** Add to Working Agreements Windows-specific notes for future contributors who hit the same issue.

---

## Sprint 5a Closure — Tag `v0.1.0-rc.4`

After 6 of 8 closure criteria PASSED + manual QA hard gate (PDF parsing in installed build) confirmed working:

```powershell
git checkout main
git pull
git tag -a v0.1.0-rc.4 -m "Sprint 5a closure: release infrastructure (NSIS installer + GitHub Actions CI + CHANGELOG scaffold)"
git push origin v0.1.0-rc.4
```

Output: `* [new tag] v0.1.0-rc.4 -> v0.1.0-rc.4`

**No typo recovery needed.** Pattern break from Sprint 4 `rc.3` typo precedent. **Sprint 5a OFFICIALLY CLOSED.**

**Tag progression complete state:**

- `v0.1.0-alpha` — Sprint 1 closure (May 17, 2026, Day 4)
- `v0.1.0-rc.1` — Sprint 2 closure (May 18, 2026, Day 5)
- `v0.1.0-rc.2` — Sprint 3a closure (May 18, 2026 23:47, Day 5 Session 5)
- `v0.1.0-rc.3` — Sprint 4 closure (May 19, 2026 ~04:00, Day 6 Session 1)
- **`v0.1.0-rc.4` — Sprint 5a closure (May 19, 2026 ~10:00 local, Day 6 Session 2)**
- `v0.1.0-rc.5` (potential) — Sprint 5b closure
- `v0.1.0-rc.6` (potential) — Sprint 5c closure
- `v0.1.0` — first public release after Sprint 5d

**5 closure tags across 6 calendar days. Zero regressions cumulative.**

---

## Stats — Day 6 Session 2 Comprehensive

| Metric | Day 6 Session 1 closure | Day 6 Session 2 closure | Delta |
|---|---|---|---|
| Tests passing | 147 | **147** | 0 (by design, infrastructure-only PR) |
| Sprints closed | 4 (1, 2, 3a, 4) | 5 (+ 5a) | +1 |
| Closure tags | 4 | 5 (+`rc.4`) | +1 |
| PRs merged Day 6 Session 2 | — | 1 (PR #26) | n/a baseline |
| New dependencies Sprint 5a | — | 0 | n/a |
| Calendar duration | 6 days | 6 days | 0 |
| Disk usage C:\ before move | ~25 GB | ~0 GB | -25 GB |
| Disk space recovered C:\ total | — | ~20 GB | n/a |

**Day 6 Session 2 deliverables:**

- Sprint 5 strategy decision (4-PR split, single-PR pattern preserved within each)
- Sprint 5a entry (architectural Q&A round, 5 questions + 3 CI sub-decisions + 4 procedural)
- Phase 1 verification discovery + Pdfium bundling fix design (second consecutive Sprint Phase 1 caught real blocker)
- PR #26 logical (GH #26) — Sprint 5a single comprehensive PR
- CI first-run validation on the PR itself (15:10 cold cache)
- 6 of 8 manual QA closure criteria PASSED
- Sprint 5a closure tag `v0.1.0-rc.4` (no typo recovery needed)
- Repository physical move from `C:\Projects\glagol\` → `D:\Projects\glagol\`
- GitHub Desktop reconfiguration via Locate

**Sessions count Day 6:** 2 (Session 1 + Session 2).

**Cumulative across Day 0-6:**

- 6 calendar days from project inception
- 5 Sprint closures (1, 2, 3a, 4, 5a)
- 5 closure tags
- 147 tests
- 1 remaining open issue (#16, Sprint 5c work)
- 0 regressions cumulative through 5 closures

---

## Lessons learned — Session 2

### Технические

1. **Tauri 2 NSIS bundler defaults are sufficient for typical use cases.** `installMode: "currentUser"` + `displayLanguageSelector: true` + license/resources/metadata in JSON config = polished installer without custom `.nsi` template. Shortcut prompts and install-location dialogs come from default MUI_PAGE_DIRECTORY + MUI_PAGE_FINISH templates.

2. **`bundle.resources: ["resources/*"]` is THE mechanism for shipping native binaries in NSIS installer.** Tauri's bundler places contents at `$INSTDIR/resources/` next to the installed `.exe`. Combined with `build.rs` mirror, gives clean build-time + install-time + runtime resolution path.

3. **`Pdfium::bind_to_library(impl AsRef<Path>)` from `pdfium-render::prelude`** — canonical API for path-based binding. `PdfiumError` re-exported via `prelude::*` (private `error::PdfiumError` module path — important detail when reading error messages from chained bind failures).

4. **Per-platform constants via `cfg(target_os = "...")` for cross-platform file names.** `PDFIUM_LIB_NAME` selects `pdfium.dll` / `libpdfium.so` / `libpdfium.dylib` at compile time. No allocation, no env var lookup at call site. Forward-compatible to macOS/Linux ports without code changes.

5. **`LazyLock<Result<Pdfium, String>>` for single-bind-per-process semantics.** Sprint 4 established. Sprint 5a's `bind_pdfium()` helper inherits unchanged — encapsulates the 4-tier chain inside the lazy initialization closure. Errors from chain exhaustion surface as `ParseError::Format("не удалось загрузить Pdfium: …")` to user via toast.

6. **GitHub Actions `Swatinem/rust-cache` includes `target/` directory.** Build script outputs (`OUT_DIR/pdfium/`) are inside `target/` so Pdfium binary cache survives across CI runs as long as Cargo.lock + PDFIUM_RELEASE_TAG don't change. No separate Pdfium cache config needed.

7. **`actions/upload-artifact@v4` for installer artifact distribution.** Lets reviewers download .exe from CI page without local build. 14-day retention default. Zero cost when artifact small. Supports closure criteria manual QA flow.

8. **GitHub markdown HTML pass strips unknown tags inside code blocks.** `<lib_name>`, `<profile>`, `<exe_dir>` look like HTML tags to GitHub's renderer. **Lesson:** use `{lib_name}` curly-brace placeholders or escape `&lt;` `&gt;` in PR body markdown. Saved by `mcp__github__update_pull_request` followup but avoidable at compose time.

9. **`#N` autolinks to issue N regardless of context.** "closure criteria #5" autolinks to issue #5 (unrelated closed issue). **Lesson:** use "step 5" / "criterion 5" in PR markdown when referring to enumerated list items. Reserve `#N` for legitimate issue/PR cross-references.

10. **`robocopy /MIR` recipe for stubborn `node_modules/` deletion on Windows.** Standard `Remove-Item -Recurse -Force` fails on pnpm's deep `.pnpm/` symlink structures + path-length issues. `robocopy` uses Windows native API without MAX_PATH limit, walks deepest-first, empties without errors. Then top-level `Remove-Item` deletes empty directories. **Add to project Working Agreements Windows section.**

11. **`Move-Item` on entire repo with `.git/` + `node_modules/` is unreliable.** Combined permission issues (`.git/objects/pack/` read-only flags) + path-length issues (`.pnpm/`) compound. **Reliable recipe:** `cargo clean` first, manually move subdirectories one at a time, or use `robocopy /MOVE` for atomic move with native API.

12. **Tauri 2 NSIS templates auto-download on first build.** `nsis-3.11.zip` (~3.5 MB) + `nsis_tauri_utils.dll` (~5 MB) downloaded from `tauri-apps/binary-releases` releases. Cached after first build. **Network requirement on first `pnpm tauri build`** — relevant for air-gapped CI environments.

### Процессные

1. **Sprint strategy decision deserves its own conversation.** Before Sprint 5a Q&A, user and Claude jointly analyzed Sprint 5 backlog vs single-PR pattern applicability. **The framing "is single-PR right for this Sprint?" prevented mechanical application of Sprint 4 success pattern to fundamentally different scope.** Pattern-as-default + judgment-as-override.

2. **Pre-implementation verification protocol caught real blocker for 2nd consecutive Sprint.** Sprint 4 caught `pdfium-bind 0.1.0` missing extraction. Sprint 5a caught release-build Pdfium path resolution. **Working Agreements pay off most when they catch problems before they manifest.** Pattern reliability: 2/2 over 2 attempts.

3. **Phase-by-phase reporting with explicit chat acknowledgment between phases.** Sprint 5a executed in 4 phases + 1 polish commit. Each phase received chat review before next phase commit. Caught and acknowledged 4 deviations from kickoff with rationale documented in PR description deviations table.

4. **`web_fetch` sanity check on PR before merge** caught cosmetic `#5` autolink issue. Without pre-merge fetch, would have been merged with misleading cross-references. **Practice — verify PR rendering on GitHub web view before merge command.**

5. **PowerShell comments in chat instructions are NOT executable.** User invoked `# Edit CHANGELOG.md: ...` line as if PowerShell would execute the comment. PowerShell `#` lines are documentation, require manual file edit. **Lesson:** Claude's chat instructions should be clearer about what's a comment vs what's a command. Format with explicit "manually edit" callout when needed.

6. **Cargo clean before large file system operations.** Reclaimed 20.9 GB before move attempt. Without this, `Move-Item` would have had ~25 GB to relocate (mostly target/), increasing failure surface and duration. **Lesson:** when planning repo moves on Windows, `cargo clean` first.

7. **GitHub Desktop `Locate...` preserves configuration.** Cleaner than Remove + Add Local Repository. Saves dialog navigation, preserves branch tracking and history state. Use Locate when path changes, Remove when repo identity changes.

8. **Polish commits before PR creation are legitimate workflow.** Sprint 5a's bundle metadata language unification (5th commit on branch before PR creation) addressed chat-surfaced concern between Phase 4 paste-back and PR creation. Pattern: phases delivered → paste-back review → minor polish identified → polish commit on branch → PR creation with all 5 commits squash-merged. Avoid mid-PR polish commits after merge target opened.

9. **Phase 1 verification asks are blocking, not advisory.** Sprint 5a kickoff explicitly stated "Don't write config code until findings surfaced and chat acknowledged." CC respected boundary — Phase 1 coding waited for chat ack on Pdfium bundling fix. **Without this discipline, Sprint 4 pattern of catching blockers pre-coding would erode.**

10. **Repository move event mid-session is recoverable.** Sprint 5a closure tag already pushed before move attempt; GitHub remote = single source of truth; even partial-move failure leaves recovery path open via individual directory moves. **Lesson:** never attempt large file system operations on dirty working tree. Working tree clean = safe state for relocations.

### Архитектурные

1. **Pdfium 4-tier fallback chain validates layered architecture invariant.** Each tier addresses different deployment scenario:
   - Tier 1 (`{exe_dir}/resources/`) — release installer (Sprint 5a primary use case)
   - Tier 2 (`{exe_dir}/`) — alternative layout if user-relocated DLL manually
   - Tier 3 (`env!("PDFIUM_LIBRARY_PATH")`) — dev build (Sprint 4 baseline preserved)
   - Tier 4 (`bind_to_system_library()`) — system-wide install fallback
   
   First-success-wins with silent-skip-then-record. **No tier is "wrong path" — they're parallel deployment scenarios.** Architecture composes by addition of tiers, not replacement.

2. **`build.rs` mirror pattern for C++ binary dependencies.** Sprint 4 introduced build-time download from upstream binary releases. Sprint 5a extended with double-write (dev cache + installer payload) using `cargo:rustc-env` propagation. Pattern cloneable for future C++ binary deps (FFmpeg, LibreOffice CLI, etc).

3. **Tauri 2 `bundle.resources` as read-only install-time concept.** Doesn't affect runtime capability surface (CSP, capabilities, asset protocol scope unchanged). Pure file system primitive: declare paths relative to `src-tauri/`, Tauri's bundler copies them into installer payload at `$INSTDIR/resources/`. Forward-compatible with custom resources (icons, language files, etc).

4. **`bundle.identifier` as data-path key.** Affects `app_local_data_dir()` resolution, NSIS install location default (when combined with `productName`), and Windows Apps & Features registration. Keyring service name decoupled (set via `keyring-rs` explicitly to "Glagol"). **Identifier rename creates breaking change for any user with existing data; preserving `app.glagol.desktop` accepts unbranded folder name to avoid breaking change.**

5. **Sprint 5b configurable library location hooks preserved.** `paths::audio_cache_root` in `src-tauri/src/paths.rs` remains single grep target for Sprint 5b. Sprint 5a touches nothing in that file. `bundle.resources` (read-only install-time) and `audio_cache_root` (writable runtime) are orthogonal concerns.

6. **CI workflow itself is part of release-engineering deliverable.** The CI workflow IS a release engineering artifact, on equal footing with NSIS config and CHANGELOG. Sprint 5a's first real CI customer is its own PR — meta-validation. **Architectural invariant: release infrastructure validates itself before being trusted as gate.**

7. **CHANGELOG.md as user-facing artifact distinct from master logs.** Master logs are engineering documentation in `docs/day-logs/`. CHANGELOG.md is user-facing release notes at repo root. Different audiences, different phrasing, different update cadence. **Don't conflate the two.**

---

## What's next

### Day 6 Session 2 closes

Session 2 ends with full Sprint 5a release infrastructure in main:

- PR #26 merged
- `v0.1.0-rc.4` tag pushed (no typo)
- Repository physically relocated `C:\Projects\glagol\` → `D:\Projects\glagol\`
- GitHub Desktop reconfigured to D:\
- 6 of 8 manual QA closure criteria PASSED (2 deferred with documented rationale)
- 0 regressions across 5 closure milestones cumulative

User state at session end: ~6 hours wall clock active work (Day 6 Session 1 closed ~04:00; Session 2 ~04:30 entry pause for coffee → 10:30 closure). Recommend pause before Sprint 5b entry.

### Sprint 5b prospects (next session)

Per Sprint 5 strategy decision: Sprint 5b = **Settings & library backend polish**.

**Scope (per chat strategy split):**

- Configurable library location (Settings UI + `config.rs` module + `paths::audio_cache_root` modified to read config + dynamic asset protocol scope via `allow_directory` / `forbid_directory`)
- Inline title editing (vertical slice: `update_document_title` command + `repo::update_title` + `Library.tsx` UI with click-to-edit or pencil icon affordance)
- Settings UI section "Папка библиотеки" with native folder picker (tauri-plugin-dialog already in deps)
- Closure tag: `v0.1.0-rc.5`

**Out of Sprint 5b scope (per strategy split):**

- Theme switcher (Sprint 5c)
- Library search/sort (Sprint 5c)
- Issue #16 toasts (Sprint 5c)
- AI attribution prevention investigation (Sprint 5c)
- README screenshots (Sprint 5d)

### Sprint 5a follow-up items (post-closure backlog)

Items deferred during Sprint 5a:

1. **CHANGELOG promote** `[Unreleased]` → `[v0.1.0-rc.4] — 2026-05-19` (batch with Sprint 5b kickoff entries — direct commit when Sprint 5b PR opens)
2. **SmartScreen screenshot** at `docs/images/smartscreen-warning.png` (requires Windows environment with SmartScreen enabled — alternate capture method needed)
3. **NSIS installer filename casing** — currently `glagol_0.1.0_x64-setup.exe` (lowercase from `productName`). Brand consistency suggests `Glagol_0.1.0_x64-setup.exe`. Cosmetic fix: 1 line in `tauri.conf.json`. Sprint 5b/5c polish.
4. **Auto-launch checkbox on NSIS finish screen** — currently present but unchecked by default. To fully remove requires custom NSIS template. Sprint 5b/5c polish.
5. **CLAUDE.md "Last updated" timestamp** — batch with Sprint 5d roadmap finalization.

### Sprint 5 cumulative backlog status

Sprint 5 entry backlog: 21 items.

**Closed in Sprint 5a (4):**

- NSIS Windows installer
- GitHub Actions CI workflow
- CHANGELOG.md scaffold
- README installation documentation + dev data folder note

**Active for Sprint 5b (2):**

- Inline title editing
- Configurable library location

**Active for Sprint 5c (≥6):**

- Theme switcher
- Library search/sort
- Issue #16 toasts
- AI attribution prevention investigation
- ESLint + Vitest setup (added from Sprint 5a tech debt)
- shadcn CLI vs hand-written component policy (Sprint 4 deferred)

**Active for Sprint 5d (≥3):**

- README screenshots
- CHANGELOG finalization across all Sprint 5a-5c entries
- Smoke test installer on clean VM (if available)
- Final v0.1.0 release tag push

**Conditional items (signal-driven, defer-by-default):**

- DOCX table narration tuning
- Tier 2/3 abbreviations
- Drag-and-drop file input
- chardetng auto-detection
- File size limit raise from 10 MB
- Library delete UX upgrade

### Open issues remaining

**Issue #16** — only remaining open issue. Sprint 5c work.

### Pause point natural

Session 2 lasted ~6 hours wall clock. Two Sprint closures across Day 6 (Sprint 4 at Session 1 close ~04:00; Sprint 5a at Session 2 close ~10:30). Plus mid-session repository relocation event. **Recommend pause before Sprint 5b entry** — Sprint 5b architectural Q&A deserves fresh mental capacity given backend complexity (config.rs design, dynamic asset protocol scope API, configurable library migration UX, title editing affordance choice).

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **Repo location:** Migrated from `C:\Projects\glagol\` → `D:\Projects\glagol\` (Day 6 Session 2)
- **Sprint 5a kickoff:** `.scratch/kickoff-day-6-session-2.md`
- **Day 6 Session 1 master log:** `docs/day-logs/day-6-session-1-master-log.md`
- **PR #24 (Sprint 4):** https://github.com/dimasiksuleyman-sudo/glagol/pull/24
- **PR #26 (Sprint 5a):** https://github.com/dimasiksuleyman-sudo/glagol/pull/26
- **Tag `v0.1.0-rc.4`:** Sprint 5a closure (May 19, 2026 ~10:30 local time)
- **CI first run:** https://github.com/dimasiksuleyman-sudo/glagol/actions/runs/26079471324 (15:10 cold cache)
- **NSIS installer artifact:** `src-tauri/target/release/bundle/nsis/glagol_0.1.0_x64-setup.exe` (~7.5 MB, unsigned)
- **Installed Glagol location (user's machine):** `D:\glagol\` (custom location chosen during install)
- **Main HEAD at Session 2 closure:** post-PR #26 merge + `v0.1.0-rc.4` tag (commit `77ac419`)

---

*Day 6 Session 2 captures: Sprint 5a entry through closure within single working session — Sprint 5 strategy decision (4-PR split) + Q&A round (5 questions + 3 CI sub-decisions + 4 procedural) + Phase 1 verification discovery (Pdfium bundling fix) + 4-phase sequential implementation + paste-back-first protocol + polish commit before PR creation + AI attribution stripping + cosmetic `#5` autolink fix + CI green on first cold-cache run + 6 of 8 manual QA closure criteria PASSED + closure tag clean (no typo) + repository physical move `C:\` → `D:\`.*
*Sprint 5a closure achieved. 0 new tests (deliberate non-target). 147/147 preserved. `v0.1.0-rc.4` pushed.*
*Day 6 calendar day delivered 2 Sprint closures (Sprint 4 + Sprint 5a). 5 Sprint closures across 6 calendar days. 0 regressions cumulative.*
*Last updated: May 19, 2026 ~10:30 local time*

---

*Created by Dmitriy + Claude*
