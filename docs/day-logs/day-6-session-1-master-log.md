# Glagol — Day 6 Session 1 Master Log

**Period:** May 18, 2026 23:59 local → May 19, 2026 ~04:00 local (Sprint 4 entry through closure, single session crossing midnight wall clock but contained within Day 6 logical session)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 4 (File input + parsing) — **CLOSURE**
**Status at end of Session 1:** Sprint 4 100% complete. File parsing (TXT/MD/DOCX/PDF) shipped + runtime-verified across 8 manual QA scenarios. Tag `v0.1.0-rc.3` pushed. CLAUDE.md global instructions updated with Sprint-PR pattern validation.

> Day 5 Session 5 (`day-5-session-5-master-log.md`) covers Sprint 3a closure preceding this session.
> This file (Day 6 Session 1) covers Sprint 4 entry-through-closure within single working session that crossed midnight wall clock but represents one continuous focused effort.

---

## TL;DR

Day 5 closed с three Sprint milestones in single calendar day (record velocity). User declared «совсем не устал и очень воодушевлен» at Day 5 Session 5 closure 23:59 — chose to continue без pause directly into Sprint 4.

Session 1 delivered:

1. **CLAUDE.md global instruction addition** в Claude Settings → Instructions for Claude. Codifies Sprint-PR pattern preference based on Glagol empirical evidence. Cross-chat persistent guidance for future structured software projects.

2. **Sprint 4 architectural Q&A** — 7 questions resolved + 4 procedural sub-decisions. User exercised override autonomy on multiple decisions: drag-drop scope cut (Q6), 10 MB file size limit (Q6, vs my 50 MB suggestion), no OCR service URL recommendations в scanned PDF disclaimer (Q7).

3. **Phase 1 blocker discovered + resolved** before any coding. CC verified `pdfium-bind` 0.1.0 API surface and found NO text extraction — only rendering. Surfaced to chat via Working Agreements pre-implementation Q&A protocol. User approved Option A pivot: `pdfium-render` 0.9.1 + `build.rs` auto-download from `bblanchon/pdfium-binaries`.

4. **PR #24** — full Sprint 4 in single comprehensive PR. **Largest in project history:** +1736 / −13 across 16 files, 4 new dependencies, 4 phases sequential. Executed without regressions, without rework, without bug discovery. Test count 123 → **147** (+24).

5. **Manual QA matrix — 8 scenarios all passed.** End-to-end pipeline validated с real PDF and DOCX test files. Preprocessor (PR #22) confirmed working through file-parsed content path. Library page regression-free.

6. **Sprint 4 closure tag** `v0.1.0-rc.3` pushed. Initial push had typo в annotation message; re-created clean.

Tests progression Session 1: 123 → **147** (+24). All 4 closure milestones across project (alpha, rc.1, rc.2, rc.3) survived through 5 calendar days с zero regression count cumulative.

This is **genuine MVP state** functional-wise. Sprint 5 = polish + CI + first public release (signed MSI installer).

---

## Sprint 4 Functionality Snapshot — what works end-to-end after closure

**File parsing pipeline:**
1. User clicks «Выбрать файл» button on Synthesize page
2. Tauri native dialog opens with «Поддерживаемые» + «Все файлы» filters
3. User selects TXT/MD/DOCX/PDF file
4. Backend `read_and_parse_file` dispatches by extension
5. Format-specific parser extracts text (`parser::txt`, `parser::md`, `parser::docx`, `parser::pdf`)
6. File size pre-check (10 MB), content size post-check (500K chars) applied
7. Scanned PDF detection (empty text after trim) sets `is_scanned_pdf` flag
8. Frontend either: replaces textarea content + success toast, OR shows modal OCR disclaimer dialog
9. User reviews parsed content в textarea, can edit
10. Synthesize button → preprocessor (PR #22) → chunker → SaluteSpeech → WAV cache → Library row

**Runtime evidence (verified Session 1 manual QA):**

| Format | Test outcome |
|---|---|
| TXT (Cyrillic, UTF-8) | Content extracted, textarea populated, synthesis OK |
| MD (with code blocks, footnotes, image alt) | Code blocks → «фрагмент кода», alt dropped, footnotes appended at end, synthesis OK |
| DOCX (Cyrillic + tables) | Paragraphs + tables row-by-row Option β, user feedback: «ничего переделывать не надо» |
| Text-based PDF (Cyrillic) | Text extracted via pdfium-render, synthesis OK |
| Scanned PDF | Modal dialog appears с generic OCR disclaimer, textarea preserved on dismiss |
| File >10 MB | Toast «Файл слишком большой» before parse |
| File via «Все файлы» (unknown ext) | `try_all` escape hatch succeeds |
| Cancel file picker | Silent no-op |
| Library after file syntheses | Rows save, load, delete, playback с speed/seek all work |
| End-to-end with preprocessor | URLs «ссылка», emails «email», abbreviations expanded through parsed-file content |

---

## CLAUDE.md Global Instructions Update

Before Sprint 4 functional work, captured cross-chat principle.

### Why this happened

After Sprint 3a (preprocessor) successfully shipped as **single-PR Sprint**, Sprint 4 represented ambitious test — full file parsing scope (4 formats + Tauri command + frontend integration + scanned PDF dialog) in single PR vs traditional multi-PR Sprint split. User explicitly chose «one PR to test how CC handles full Sprint в одном пиаре».

After Sprint 4 successful close (зеро regressions, zero rework, all 4 phases clean), user identified this as **generalizable pattern worth preserving** beyond Glagol project. Global Claude instructions field в Settings persists across all chats — ideal codification location.

### What was added

User entered following text into Claude Settings → General → Instructions for Claude:

> When working on a structured software project with me (e.g. multi-Sprint roadmap with explicit closure tags), prefer single comprehensive PRs per Sprint over multi-PR Sprint splits, provided the project has a kickoff document + phase-by-phase reporting + paste-back-first protocol in place. This pattern has been empirically validated through the Glagol project: Sprint 4 delivered a 700+ LOC PR with 4 new dependencies in 4 phases without regressions or rework. Single-PR Sprints reduce cognitive overhead from PR coordination while preserving review safety through phase boundaries.

### Why this formulation

**Captures principle (single-PR Sprint preference) AND mechanism (Working Agreements protocol requirement).** Brief evidence anchor («empirically validated through Glagol») gives future Claude context для understanding WHY this principle exists. ~80 words fits Settings field comfortably while preserving signal.

**Limits:** Only applies when project has kickoff + phase reporting + paste-back-first protocol. Without these safeguards, multi-PR splits remain safer default. Future Claude reads this и understands when pattern applies vs doesn't.

### Cross-chat implications

Future chat sessions arriving at any structured software project (Glagol-style or others) inherit this guidance automatically. New CC sessions reading user's instructions during onboarding will see preference. Reduces re-derivation of this conclusion across multiple projects.

---

## Sprint 4 Architectural Q&A (7 questions + 4 procedural sub-decisions)

Standard pattern from Sprint 2/3 — Q&A in chat before kickoff, design decisions surfaced, deps freshness verified.

User context state: «совсем не устал и очень воодушевлен», approaching Q&A at full mental capacity despite Day 5 record-velocity workload preceding. MVP-focused stance: explicit overrides on multiple decisions reduced ceremony.

### Q1 — Sprint 4 framing

**Decision: Full Sprint 4 (Option α) in single PR.** 4 formats together, no Sprint 4a/4b split.

User comment: «хочу протестировать как мы справимся именно с full sprint».

Rejected alternatives:
- Option β — split into 4a (TXT/MD foundation) + 4b (DOCX/PDF heavyweights)
- Option γ — minimum viable (TXT + MD only)
- Option δ — skip Sprint 4, jump to Sprint 5 polish

### Q2 — PDF strategy

**Decision: `pdfium-bind` Option A — embed Pdfium binary, zero install friction.**

This was **REVISED mid-session via Phase 1 blocker** — see «Phase 1 Blocker» section below. Final implementation uses `pdfium-render` 0.9.1 with `build.rs` auto-download.

User comment on test material: «DOCX/PDF — Для теста у меня есть».

### Q3 — DOCX strategy + tables policy

**Decision: `docx-rust` parsing-focused fork + Option β for tables.**

`docx-rust` over `docx-rs`: discovered during dep freshness verification:
- `docx-rs` (bokuweb): primarily a writer
- `docx-rust` (PoiScript fork): parsing-focused
- `docx-rust` ~1M downloads inherited, stable

CLAUDE.md tech stack update required as part of PR.

**Table extraction Option β:** row-by-row, cells joined с spaces, rows separated `\n`. User rationale: «Когда я послушаю вживую озвученную таблицу, я сразу пойму, что можно улучшить.» — pragmatic, let runtime audio surface refinement needs for Sprint 5+ polish if needed.

**DOCX conservative defaults:**

| Element | Action |
|---|---|
| Paragraphs/headings/lists | Extract |
| Tables | Option β |
| Headers/footers/comments/footnotes/tracked-changes/embedded-images | Skip |
| Bold/italic/links | Drop markup, keep text |

### Q4 — TXT encoding detection

**Decision: Option D — BOM + smart fallback chain.**

1. BOM detection via `encoding_rs::Encoding::for_bom()` (handles UTF-8 BOM, UTF-16 LE/BE, UTF-32)
2. UTF-8 strict via `std::str::from_utf8()` (catches ~85% modern files)
3. Windows-1251 fallback via `encoding_rs::WINDOWS_1251` (catches ~8% legacy Russian files)

Covers ~99% real Russian .txt files. No external chardet crate.

### Q5 — MD parsing policy

**Decision: Conservative defaults using `pulldown-cmark`.**

Headings, paragraphs, blockquotes, lists: extract. Bold/italic/inline code: drop markup, keep text. Link text: keep, drop URL. **Image alt: drop entirely** (quality varies wildly). **Code blocks: replace с «фрагмент кода»** placeholder. **Footnotes: append at end в «Сноски:» section.** Tables: row-by-row same as DOCX Option β. Horizontal rule + HTML tags: skip.

Rationale: refine via real audio listening в Sprint 5+ if signal surfaces.

### Q6 — File picker + drag-drop UX

**Decision: File picker button only — NO drag-drop в Sprint 4.**

User explicit override of my drag-drop recommendation:
> «хочу для mvp убрать функцию drag and drop только выбрать.»

Drag-drop deferred к Sprint 5 conditional polish based on signal. Significantly simplifies Sprint 4: no Tauri file-drop event handling, no capabilities permission changes, no drop zone styling, no drop conflict resolution.

**Sub-decisions for file picker:**

| Sub-question | Decision | Rationale |
|---|---|---|
| Button placement | B (above textarea, inline с label) | Preserves visual hierarchy |
| Accepted formats | «Поддерживаемые» + «Все файлы» escape hatch | Permissive default + clear error |
| File size limit | **10 MB** (user override of my 50 MB) | Conservative — relax later if signal |
| Content size soft limit | 500K chars | SaluteSpeech monthly quota anchor |
| Parse error UX | Toast + textarea unchanged | Preserves user state |
| File→textarea behavior | Replace (predictable, simplest) | No merge/append complexity для MVP |
| Review step | User clicks «Озвучить» after reviewing | Catches parse errors before quota burn |

### Q7 — Scanned PDF disclaimer

**Decision: Modal alert dialog с generic «найдите OCR сервис в интернете» — NO specific service URLs.**

User rationale: «рекомендаций в дисклеймере не даем — найдите OCR сервис. Так как урлы могут измениться, сервисы закрыться, это приведет к жалобам.»

Linkrot reality — generic recommendation never goes stale. Specific URL recommendations would create maintenance debt + user complaints when services close.

**Detection signal:** PDF text extracted by parser is empty/whitespace-only after `.trim()`.

**Modal disclaimer text (Russian, generic):**

> Похоже, этот PDF — сканированное изображение
> 
> Извлечь текст напрямую не получилось — PDF состоит из изображений страниц, а не текстового содержимого.
> 
> Чтобы озвучить такой документ, его сначала нужно распознать (OCR — оптическое распознавание символов). В интернете есть бесплатные онлайн-сервисы для этого.
> 
> После распознавания сохраните результат как `.txt` или `.docx` и попробуйте снова.

### Procedural sub-decisions (4)

1. **Branch:** `claude/file-parsing` (CC convention)
2. **PR title:** `feat(parser): file input with TXT/MD/DOCX/PDF parsers`
3. **4 phases** with phase-by-phase reporting (production code path touched in Synthesize.tsx)
4. **Test count target:** «not less than 143» (123 baseline + ≥20 new; stretch ~150)

---

## Phase 1 Blocker Discovery + Resolution (pdfium-bind → pdfium-render)

**Critical event Session 1.** CC's pre-implementation Q&A protocol caught spec bug before any code written.

### Discovery

CC verifying crate versions at Phase 1 start. Checked `pdfium-bind` 0.1.0 actual API surface against kickoff Q2 assumption.

**Finding:** `pdfium-bind` 0.1.0 high-level wrapper exposes ONLY:
- `PdfDocument::open()`
- `page_count()`, `get_pdf_version()`, `get_metadata_value()`
- `render_page(page_num, dpi)` → returns RGBA pixel data
- `cleanup_cache()`

**Missing:** ANY `FPDFText_*` text extraction bindings. No `extract_text()` or equivalent helper.

**Conclusion:** Q2 premise — что `pdfium-bind` gives us text extraction with zero install friction — **does not hold at 0.1.0 version.** Blocked Phase 1 for PDF specifically (TXT/MD/DOCX unaffected).

### CC raised 4 options

1. **`pdfium-render` 0.9.1** (original CLAUDE.md choice). Battle-tested text extraction. Ships as .dll separately or via installer bundling.
2. **`pdfium-render` 0.9.1 + `static` feature.** May statically link Pdfium. Adds 6-8 MB binary, removes runtime DLL dep. Needs Windows MSVC verification.
3. **`pdfium-bind` raw FFI for text extraction.** ~30 lines unsafe code. Conflicts с CLAUDE.md security invariant #5 spirit.
4. **Defer PDF к follow-up PR.** Ship Sprint 4 с TXT/MD/DOCX only. PDF гets own session.

CC's recommendation: B if Windows MSVC verification passes, A otherwise. D for de-risking timeline.

### My evaluation + decision

**Chose Option A.** Reasoning captured в chat:

- **Reliability beats aesthetic.** `pdfium-render` battle-tested, mature.
- **Option B unverified.** «Maybe» static linkage не должна быть Sprint 4 test surface.
- **DLL bundle via build.rs is well-understood pattern.** Auto-download from `bblanchon/pdfium-binaries` releases.
- **Sprint 5 installer responsibility clear.** MSI bundle alongside exe.
- **CLAUDE.md correction minimal.** Reverts to original choice (`pdfium-render`).
- **Q2 promise revised honestly.** «Zero install» becomes «zero install in shipped MSI». Dev workflow gets build.rs auto-download (one-time first build).

### Cascading implications

- CLAUDE.md tech stack: **no change for PDF** (preserves original `pdfium-render` choice; build.rs detail added к Windows-specific notes)
- Phase 1 PDF parser implementation uses `pdfium_render::prelude::*` API
- `pdfium-bind` removed from kickoff dep list
- Q7 scanned PDF disclaimer unchanged

### Working Agreements validation

**Phase 1 blocker discovered AT pre-coding stage,** not mid-implementation. This is exactly what «Pre-implementation answers — Open: anything unclear? Raise in chat before coding» protocol designed for. CC caught spec bug because kickoff explicitly invited verification.

Без Working Agreements protocol, CC would have started implementation, discovered API gap mid-coding, written incomplete unsafe FFI wrapper, или completed implementation with rendering instead of extraction (silent failure). Working Agreements prevented all three failure modes.

---

## PR #24 Implementation (4 phases sequential)

Per CLAUDE.md Working Agreements — production code path touched (Synthesize.tsx). Phase reports REQUIRED.

### Phase 1 — Backend parsers (4 modules + tests)

**Commit:** `0e81bb7` Phase 1.

**Deliverables:**
- `src-tauri/src/parser/mod.rs` (124 LOC) — `ParsedDocument` struct + `ParseError` enum + `try_all` dispatcher
- `src-tauri/src/parser/txt.rs` (117 LOC) — encoding chain + 7 tests
- `src-tauri/src/parser/md.rs` (256 LOC) — pulldown-cmark event filter + 7 tests
- `src-tauri/src/parser/docx.rs` (117 LOC) — docx-rust paragraph/table extraction + 2 tests
- `src-tauri/src/parser/pdf.rs` (119 LOC) — pdfium-render dynamic-bound + 3 tests
- `src-tauri/Cargo.toml` (+12 lines) — 4 new deps
- `src-tauri/build.rs` (+130 LOC) — Pdfium download script
- `src-tauri/src/lib.rs` (+3) — `pub mod parser;`

**Crate versions chosen (verified latest stable):**
- `encoding_rs 0.8.35`
- `pulldown-cmark 0.13.3` with `default-features = false` (drops SIMD)
- `docx-rust 0.1.11`
- `pdfium-render 0.9.1`

**Pdfium binary distribution:** `build.rs` downloads `chromium/7834` (May 2026 stable bundle from `bblanchon/pdfium-binaries`), caches в `OUT_DIR/pdfium/`, propagates path via `cargo:rustc-env=PDFIUM_LIBRARY_PATH`. `pdf.rs` reads env var с `env!()`, falls back к `Pdfium::bind_to_system_library()` if cached file missing. **Failure surfaces as `ParseError::Format`, never panic.**

**Test count after Phase 1: 143** (123 + 20 new, hitting ≥143 floor exactly).

**Subtle implementation notes from CC:**
- `Pdfium` wrapped в `std::sync::LazyLock` for single bind per process
- `md.rs` uses `image_depth` counter (not bool) defensively
- `docx.rs` uses `iter_text()` helper to flatten runs ignoring markup

**Quality gates Phase 1:** cargo check / fmt / clippy / test all clean. 143/143 passing.

### Phase 2 — Tauri command + frontend wrapper

**Commit:** `0635722` Phase 2.

**Deliverables:**
- `src-tauri/src/commands/file.rs` (156 LOC) — `read_and_parse_file` command + 4 tests
- `src-tauri/src/commands/mod.rs` (+1) — `pub mod file;`
- `src-tauri/src/lib.rs` (+1) — invoke_handler registration
- `src/lib/tauri.ts` (+33) — `ParsedDocument` interface + `readAndParseFile` wrapper

**Pattern application:** `*_impl` testable inner function pattern (PR #15/#16/#17 precedent). Pre-parse size cap via `fs::metadata` (rejects 200 MB DOCX без unzipping). Post-parse content cap via `chars().count()` (Cyrillic-aware).

**4 new tests:**
- `dispatches_txt_by_extension`
- `dispatches_md_by_extension`
- `rejects_files_above_size_limit` (10 MB + 1 byte file)
- `falls_through_to_try_all_for_unknown_extension`

**Deliberate skip:** content-cap test. CC rationale: «generating 500 001 Cyrillic characters в unit test would add ~1 MB of test data for minimal added confidence. Single-branch logic check. Frontend-runtime via real long file more meaningful.» Endorsed.

**Test count after Phase 2: 147** (143 + 4).

**Quality gates Phase 2:** cargo check / clippy / test clean (147/147). pnpm tsc clean.

### Phase 3 — Synthesize page UI + scanned PDF dialog

**Commit:** `b346f28` Phase 3.

**Deliverables:**
- `src/components/ui/alert-dialog.tsx` (132 LOC, NEW) — shadcn AlertDialog primitives, **hand-written**
- `src/components/ScannedPdfDialog.tsx` (51 LOC, NEW) — generic OCR disclaimer wrapper
- `src/pages/Synthesize.tsx` (+57/-2) — file picker button + handler + dialog render

**Behavioural verification checklist (11 items, all confirmed):**

1. Button placement Q6 = B (`<div flex justify-between>` wrapping Label + Button)
2. Two filters в `open()` — «Поддерживаемые» + «Все файлы»
3. Success path: parsed.text → textarea + toast «Файл загружен (X симв.)»
4. Scanned PDF path: modal opens, textarea preserved
5. Error paths: Russian error strings via `toast.error`, no setText
6. User cancel: `if (picked === null) return;` no-op
7. Dialog dismissal: Radix handles Escape/backdrop/«Понятно» button
8. Existing Synthesize.tsx structure preserved (credential gating, handleSynthesize, ProgressIndicator untouched)
9. Tauri plugin-dialog already в deps from Sprint 1
10. TypeScript strict — no `any`, narrowed `picked: string | string[] | null`
11. No console.log — only toast.success / toast.error

**Judgment call: shadcn alert-dialog hand-written vs CLI install.** CC's reasoning:

> «Sprint 1 components use the `radix-ui` umbrella package (already at `^1.4.3`), not per-primitive `@radix-ui/react-*` packages. Running CLI would have added redundant transitive deps. Hand-written matches `select.tsx` / `sonner.tsx` style.»

**Bundle delta Phase 3:** JS 397.77 → 411.87 KB (+14.10, gzip +3.98). CSS 39.33 → 41.31 KB (+1.98). Above original 5-10 KB estimate, fully accounted for (Radix Dialog primitive bulk + AlertDialog wrapper + ScannedPdfDialog + FileUp icon + Synthesize.tsx additions). Within reasonable envelope.

**Test count after Phase 3: 147** (no new tests — frontend not directly testable без Tauri runtime).

**Quality gates Phase 3:** cargo test 147/147 still passes (no regression). pnpm tsc + pnpm build clean.

### Phase 4 — Quality gates + CLAUDE.md + paste-back composition

**Commit:** `c6e0c7f` Phase 4.

**CLAUDE.md updates:**
- Tech stack DOCX row: `docx-rs` → `docx-rust`
- Tech stack PDF row: `pdfium-render` confirmed (no change) + clarified build.rs binary download
- Repository layout: `commands/` listing `file.rs`; `parser/` listing `mod.rs` + brief per-module description; `preprocessor.rs` marked Sprint 3 (no longer WIP)
- Windows-specific notes: new «Pdfium DLL distribution» section
- Last updated stamp: 2026-05-19

**Final quality gate sweep:**
- `cargo check` → 15.15s
- `cargo fmt --check` → clean
- `cargo clippy --all-targets -- -D warnings` → clean
- `cargo test` → 147 passed; 0 failed; 3 doc-tests ignored
- `pnpm tsc --noEmit` → clean
- `pnpm build` → 411.87 KB JS (gzip 131.48) / 41.31 KB CSS / 3.13s

**Paste-back composition:** consolidated 4-phase summary с file list, test arithmetic, deviations (1: PDF crate revision), judgment calls (3: shadcn hand-write, no content-cap test, DOCX thin coverage), known limitations (4: DOCX behavioural depends на manual QA, PDF detection test two-outcome, system-library fallback, build.rs network access).

**Forward compatibility notes** added unprompted by CC: `ParsedDocument::is_scanned_pdf` extensibility for future format-specific signals, `MAX_FILE_SIZE` / `MAX_CONTENT_CHARS` const findability for Sprint 5+ tuning.

### CC paste-back format observations

This was **3rd consecutive PR-piece showing fully internalized Working Agreements:**

✅ Phase report format identical к Phase 1/2/3
✅ Files + LOC delta + test count arithmetic + quality gates + deviations + open questions
✅ No full-file dumps
✅ References prior PRs (#15/#16/#17, PR #22) for pattern justification
✅ Explicit trade-off documentation for skipped tests
✅ Forward compatibility section added beyond strict kickoff requirements

### AI attribution stripping — runtime behavior

After `mcp__github__create_pull_request` returned URL, CC self-initiated `mcp__github__update_pull_request` call:
- Detected auto-injected `_Generated by [Claude Code](...)_` footer
- Stripped footer line
- Confirmed body ends с `Created by Dmitriy + Claude`

**Third PR with AI attribution protocol enforced runtime** (PR #22, PR #23, PR #24). Pattern fully reliable.

---

## Manual QA — 8 scenarios all passed

Per Working Agreements: «Runtime verification after merge — Manual QA on target platform is hard gate.»

User executed:

```powershell
git pull
cd src-tauri
cargo test  # 147/147
cd ..
pnpm tsc --noEmit
pnpm tauri dev  # First compile: ~1m for Pdfium download
```

App started cleanly.

### Manual QA matrix outcomes

1. **TXT (UTF-8, Cyrillic):** Content extracted, textarea populated, synthesis OK ✅
2. **MD (with formatting):** Code blocks → «фрагмент кода», alt dropped, footnotes appended at end ✅
3. **DOCX (Cyrillic + tables):** Tables row-by-row, user feedback: **«ничего переделывать не надо»** ✅
4. **Text-based PDF (Cyrillic):** Text extracted via pdfium-render, textarea populated ✅
5. **Scanned PDF:** Modal dialog с full disclaimer, textarea preserved ✅
6. **Edge cases (file size, content size, escape hatch, cancel):** All four sub-tests passed ✅
7. **Library regression (load/save/delete/playback/speed/seek):** All work ✅
8. **End-to-end synthesis с preprocessor (PR #22 pipeline through parsed content):** URLs «ссылка», emails «email», abbreviations expanded, numbers safe, filenames safe ✅

**Critical Sprint 4 verifications:**
- DOCX Option β confirmed runtime (no narration tuning needed)
- TLD whitelist works on parsed content (`1.5` not matched, `.pdf` not matched)
- Preprocessor pipeline transparent через file-parsed input
- Scanned PDF detection signal works correctly

**No regressions detected. No bugs filed. No follow-up issues required.**

---

## Sprint 4 Closure — Tag `v0.1.0-rc.3`

After all 8 manual QA scenarios passed:

```powershell
git checkout main
git pull
git tag -a v0.1.0-rc.3 -m "Sprint 4 closure: file input + parsing (TXT/MD/DOCX/PDF) with file picker UI"
git push origin v0.1.0-rc.3
```

Output: `* [new tag] v0.1.0-rc.3 -> v0.1.0-rc.3`

**Initial push had typo** в annotation: «print 4 closure» вместо «Sprint 4 closure», «///» вместо «TXT/MD/DOCX/PDF». PowerShell autocorrect or paste artifact. User noticed mid-conversation, asked for fix.

**Tag re-created clean:**

```powershell
git tag -d v0.1.0-rc.3
git push --delete origin v0.1.0-rc.3
git tag -a v0.1.0-rc.3 -m "Sprint 4 closure: file input + parsing (TXT/MD/DOCX/PDF) with file picker UI"
git push origin v0.1.0-rc.3
```

Final annotation: `Sprint 4 closure: file input + parsing (TXT/MD/DOCX/PDF) with file picker UI`. **Sprint 4 OFFICIALLY CLOSED.**

**Tag progression complete state:**
- `v0.1.0-alpha` — Sprint 1 closure (May 17, 2026, Day 4)
- `v0.1.0-rc.1` — Sprint 2 closure (May 18, 2026, Day 5)
- `v0.1.0-rc.2` — Sprint 3a closure (May 18, 2026 23:47, Day 5 Session 5)
- **`v0.1.0-rc.3` — Sprint 4 closure (May 19, 2026 ~04:00, Day 6 Session 1)**
- `v0.1.0-rc.4` (potential) — Sprint 5 closure (polish + CI)
- `v0.1.0` — first public release с MSI installer

**4 closure tags across 5 calendar days. Zero regressions cumulative.**

---

## Stats — Day 6 Session 1 Comprehensive

| Metric | Day 5 closure | Day 6 Session 1 closure | Delta |
|---|---|---|---|
| Tests passing | 123 | **147** | +24 |
| Sprints closed | 3 (1, 2, 3a) | 4 (1, 2, 3a, 4) | +1 |
| Closure tags | 3 (`alpha`, `rc.1`, `rc.2`) | 4 (+`rc.3`) | +1 |
| PRs merged Day 6 Session 1 | — | 1 (PR #24) | n/a baseline |
| Largest single PR by LOC | ~400 (PR #17 Library) | **+1736 / −13 (PR #24)** | 4x previous |
| New dependencies Day 6 Session 1 | — | 4 (encoding_rs, pulldown-cmark, docx-rust, pdfium-render) | n/a baseline |
| Calendar duration | 5 days | 6 days | +1 (Day 0-6) |

**Day 6 Session 1 deliverables:**
- CLAUDE.md global instructions update (Sprint-PR pattern codification)
- Sprint 4 entry (architectural Q&A round, 7 questions + 4 procedural)
- Phase 1 blocker discovery + resolution (pdfium-bind → pdfium-render)
- PR #24 logical (GH #24) — Sprint 4 single comprehensive PR
- 8-step manual QA all passed
- Sprint 4 closure tag `v0.1.0-rc.3` (with typo recovery)

**Sessions count Day 6:** 1 (this session).

**Cumulative across Day 0-6:**
- 6 calendar days from project inception
- 4 Sprint closures (1, 2, 3a, 4)
- 4 closure tags
- 147 tests
- 1 remaining open issue (#16, Sprint 5 work)
- 0 regressions through 4 closures

---

## Lessons learned — Session 1

### Технические

1. **Pre-implementation API verification mandatory for new crates.** Q2 kickoff specified `pdfium-bind` based on description («embeds prebuilt PDFium binaries») without verifying actual high-level API surface. CC's Phase 1 verification caught gap. **Lesson:** kickoff dep choices should include verification of specific API methods needed (not just crate descriptions).

2. **build.rs auto-download pattern для C++ binary deps.** Pattern сloned для future C++ binary dependency needs (LibreOffice CLI for unusual formats, FFmpeg для audio conversion, etc).
   - Download from upstream binary releases (`bblanchon/pdfium-binaries`)
   - Cache в `OUT_DIR` (gitignored automatically)
   - Propagate path via `cargo:rustc-env`
   - Runtime fallback (`bind_to_system_library`) для offline build / future MSI bundling
   - Failure surfaces as typed error, never panic
   - Network access on first build only

3. **`encoding_rs::Encoding::for_bom()` covers all major BOM variants** (UTF-8, UTF-16 LE/BE, UTF-32). Combined with UTF-8 strict + Windows-1251 fallback handles ~99% real Russian .txt files без chardet auto-detection.

4. **`pulldown-cmark` event-based parsing pattern** для controlled markdown extraction. Filter events, ignore others, build output incrementally. Image alt drop, code block replacement, footnote routing all expressed naturally as event handlers.

5. **`docx-rust` `iter_text()` helper** flattens DOCX runs ignoring bold/italic/hyperlink markup. Avoids manual run iteration. Clean abstraction.

6. **`pdfium-render` text extraction API:** `document.pages().iter().text()?.all()` per page. Straightforward, returns `String` per page. Page concatenation с `\n\n` boundaries preserves paragraph structure для chunker downstream.

7. **shadcn alert-dialog с radix-ui umbrella vs per-primitive package.** Project pattern: hand-written components matching existing imports. shadcn CLI default uses per-primitive packages — incompatible с existing umbrella pattern. **Lesson:** check existing repo pattern before invoking dep installers.

8. **TypeScript narrowing `string | string[] | null`** для Tauri `open()` polymorphic return type. `typeof picked === "string"` narrows correctly. Avoid `as any` casts.

### Процессные

1. **Working Agreements pre-implementation Q&A protocol caught Phase 1 blocker before coding.** Without protocol, CC would have written incomplete/incorrect implementation. **Working Agreements pay off most when they catch problems before they manifest.**

2. **User override autonomy preserved through Q&A format.** Multiple Q decisions had user overrides (Q6 drag-drop removed, Q6 10 MB vs 50 MB, Q7 no OCR URLs). Working Agreements protocol gives user explicit decision points without forcing my recommendations unchecked.

3. **MVP-focused user stance saves chat hours.** «не будем усложнять для MVP» / «не делаем рекомендаций» / «убрать drag and drop» — each clear minimal answer saved 5-10 minutes of discussion. Strong product instincts.

4. **Phase boundaries enable mid-PR pivots safely.** Phase 1 blocker discovery + Option A pivot happened cleanly between architectural Q&A finalization и actual Phase 1 coding. No wasted code, no rework, no PR rewrite.

5. **CC's deliberate test skips warrant explicit documentation.** Content-cap test, DOCX behavioural тests, PDF fixture tests — все 3 skipped с documented rationale. Transparency > coverage theatre. Manual QA covers what unit tests can't reasonably check.

6. **PR description bilingual format с judgment calls section** provides public record of engineering decisions. Future readers (humans + future Claude instances) understand WHY choices were made, не just WHAT was changed.

7. **Tag annotation typos recoverable.** `git tag -d` + `git push --delete origin` + recreate is safe pattern when no consumers depend on annotation content yet (Glagol is solo project, no external consumers).

8. **Global Claude instructions field for cross-project learnings.** Project-specific patterns codified в CLAUDE.md (project root). Cross-project principles codified в Claude Settings → Instructions for Claude (Anthropic platform-level). Two distinct codification layers for different scope of knowledge.

### Архитектурные

1. **Parser layer as separate concern.** `parser::*` modules consume `&Path`, return `ParsedDocument`. Pipeline stage between file input и preprocessor. Each format isolated module — DOCX issue doesn't block TXT. **Composable architecture.**

2. **`ParsedDocument` struct as extensible IPC contract.** `text + is_scanned_pdf + source_format` covers Sprint 4 needs. Future formats can add typed signals (`truncated_at_page`, `password_protected`, etc) without breaking existing callers. Forward compatible.

3. **Limits as `pub(crate) const`.** `MAX_FILE_SIZE`, `MAX_CONTENT_CHARS` findable via grep. Sprint 5+ tuning trivial.

4. **`*_impl` testable inner pattern propagation.** Sprint 1-2 established pattern для Tauri commands. Sprint 4 `commands::file::read_and_parse_file_impl` inherits same pattern unchanged. Consistency across module boundaries.

5. **Russian-language error messages at command layer.** `ParseError::Format` messages compose в Russian directly. Frontend toast displays без translation. Reduces frontend logic, preserves Russian-native UX.

6. **build.rs as compile-time binary procurement.** Pattern для future binary deps. Auto-downloads + caches + binds via env var. Runtime fallback if cache missing. Failure typed, not panic.

7. **Asset protocol playback regression-free через Sprint 4 changes.** Sprint 2 PR #17 introduced asset protocol streaming. Sprint 4 changes (file parsing + new modal) don't touch playback pipeline. Verified runtime via Шаг 7 Library check.

8. **Preprocessor transparent через new pipeline stages.** PR #22 (preprocessor) sits between any input source (paste OR parsed file) и chunker. Sprint 4 added second input source без preprocessor changes. **Layered architecture validates.**

---

## What's next

### Day 6 Session 1 closes

Session 1 ends с full Sprint 4 functionality в main:
- PR #24 merged
- `v0.1.0-rc.3` tag pushed (re-created clean)
- CLAUDE.md global instructions updated (Sprint-PR pattern)
- 8/8 manual QA scenarios passed
- 0 regressions across 4 closure milestones cumulative

### Sprint 5 prospects (next session)

CLAUDE.md roadmap: «Sprint 5 ⏸️ Player polish + CI + first public release (custom audio controls, status badges, parallel synthesis, signed MSI installer, CHANGELOG)»

**Sprint 5 backlog accumulated through Sprints 1-4:**

1. Inline title editing on Library rows (для смысловых titles)
2. Configurable library location UI
3. Library delete UX upgrade (если real users feedback)
4. AI attribution footer prevention (research если `mcp__github__update_pull_request` workflow needed every time)
5. Smart title boundary cut
6. README documentation о Tauri 2 path resolution
7. CHANGELOG.md (batch Sprint 1-5 entries)
8. Accessibility audit extension
9. Tier 2 abbreviations (см., стр.) — if real signal
10. Tier 3 abbreviations (г., с.) — if real signal
11. Number formatting (`№`, `%`, dates) — if real signal
12. Russian grammar-aware URL/email replacement
13. SaluteSpeech quota dashboard
14. Theme switcher (dark/light)
15. Library search and sort
16. Drag-and-drop file input (from Sprint 4 deferred)
17. DOCX table narration tuning (conditional — Option β confirmed runtime, may not need)
18. MD image alt / code block content / footnote inline polish (conditional)
19. chardetng auto-detection (conditional — if files outside UTF-8/CP1251)
20. File size limit raise from 10 MB (conditional)
21. shadcn CLI vs hand-written component policy codification

**Massive Sprint 5 backlog (21 items).** Likely Sprint 5 itself будет split into 5a/5b/5c phases. Major Sprint 5 entry decision: which subset для first public release vs deferred к Sprint 6+ polish.

**Major Sprint 5 work items** (likely required for v0.1.0 release):
- CI/CD via GitHub Actions (build automation)
- Signed MSI installer (WiX или NSIS config + code signing certificate)
- CHANGELOG.md
- Theme switcher (light/dark)
- README screenshots
- Sprint 1 README path documentation (Tauri 2 dev vs release paths)
- AI attribution Cleanup (PR #22-#24 each required manual intervention; investigate prevention)

Sprint 5 will be Glagol's largest Sprint by item count. Single-PR pattern may not apply — likely 3-5 PRs split by theme (installer, CI, polish features, README, CHANGELOG).

### Open issues remaining

**Issue #16** — only remaining issue. Likely Sprint 5 UX polish work.

### Pause point natural

Session 1 lasted Day 5 23:59 → Day 6 ~04:00 — substantial duration. Energy state at session end unknown; recommend pause before Sprint 5 entry. Sprint 5 architectural Q&A deserves fresh mental capacity given backlog scale.

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **Sprint 4 kickoff:** `.scratch/kickoff-day-6-session-1a.md` (renamed с `-a` suffix to avoid conflict с earlier draft)
- **Day 5 Session 5 master log:** `docs/day-logs/day-5-session-5-master-log.md`
- **PR #22 (preprocessor):** https://github.com/dimasiksuleyman-sudo/glagol/pull/22
- **PR #23 (Day 5 Session 5 master log):** https://github.com/dimasiksuleyman-sudo/glagol/pull/23
- **PR #24 (Sprint 4):** https://github.com/dimasiksuleyman-sudo/glagol/pull/24
- **Tag `v0.1.0-rc.3`:** Sprint 4 closure (May 19, 2026 ~04:00 local time)
- **CLAUDE.md updates Session 1:** `commit c6e0c7f` (tech stack revision: docx-rust + pdfium-render notes; layout: parser/ tree; Windows-specific Pdfium DLL section)
- **Claude Settings global instruction:** Added Day 6 Session 1, codifies Sprint-PR pattern based on Glagol empirical evidence
- **Main HEAD at Session 1 closure:** post-PR #24 merge + `v0.1.0-rc.3` tag

---

*Day 6 Session 1 captures: Sprint 4 entry through closure within single working session — Q&A round (7 decisions + 4 procedural) + kickoff drafting + Phase 1 blocker discovery + Option A pivot (`pdfium-bind` → `pdfium-render` + build.rs auto-download) + 4-phase sequential implementation + paste-back-first protocol + AI attribution stripping (third PR enforced runtime) + merge + 8-step manual QA all passed + closure tag re-created clean + CLAUDE.md global instructions update.*
*Sprint 4 closure achieved. 24 new tests bring total to 147. `v0.1.0-rc.3` pushed. PR #24 — largest in project history (+1736 LOC, 4 new deps) — executed without regressions, rework, or bug discovery.*
*Day 6 Session 1 delivered 1 Sprint closure on top of Day 5 record (3 closures). 4 closure tags across 5 calendar days, 0 regressions cumulative.*
*Last updated: May 19, 2026 ~04:00 local time*

---

*Created by Dmitriy + Claude*
