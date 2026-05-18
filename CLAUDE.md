# CLAUDE.md

> This file is the operating manual for AI coding assistants (Claude Code, Cursor, etc.) working in the Glagol repository.
> If you are a human contributor, see [CONTRIBUTING.md](CONTRIBUTING.md) instead.

## Project overview

**Glagol** is an open source Windows desktop application for synthesizing speech from long Russian texts and documents using the SaluteSpeech API by Sberbank. It is built with Tauri 2.x (Rust backend) + React 19 + TypeScript (frontend). The MIT-licensed project is independent and NOT affiliated with Sberbank.

**Core value proposition:** local library of synthesized documents with resume-playback, free for most users (200,000 chars/month SaluteSpeech free tier).

**Primary target:** Windows 10/11 x64. macOS/Linux are stretch goals after v1.0.

## Tech stack — non-negotiable choices

| Layer | Choice | Reason |
|---|---|---|
| Desktop framework | **Tauri 2.x** | Small bundle, native performance, Rust security |
| Backend language | **Rust stable** (≥1.77) | Memory safety, performance, ecosystem |
| Frontend framework | **React 19 + TypeScript** | Tauri 2 templates, broad knowledge |
| Styling | **Tailwind CSS + shadcn/ui** | Copy-paste components, no vendor lock-in |
| State (frontend) | **Zustand** | Light, no boilerplate, works with Tauri |
| Build / package manager | **pnpm** | Fast, disk-efficient, lockfile committed |
| HTTP client (Rust) | **reqwest + rustls** | Pure-Rust TLS, works with embedded cert |
| Local database | **SQLite via rusqlite + rusqlite_migration** | Battle-tested, embedded, sync (chose over `tauri-plugin-sql` for security/test reasons) |
| Secret storage | **keyring-rs** (NOT Stronghold) | Windows Credential Manager, OS-level encryption |
| PDF parsing | **pdfium-render** | Same lib as Chromium, highest quality. Pdfium shared library downloaded by `build.rs` from `bblanchon/pdfium-binaries` and cached in `OUT_DIR/pdfium/`; path baked in via `PDFIUM_LIBRARY_PATH`. |
| DOCX parsing | **docx-rust** | Parsing-focused fork (the original `docx-rs` is writer-first); correct Cyrillic |
| Markdown | **pulldown-cmark** | Fast CommonMark parser |
| Audio (WAV) | **hound** (synthesis-side) + manual streaming (concat-side) | Simple, predictable; streaming WAV header normalization established in Sprint 1 PR #13 |
| Async runtime | **tokio** | Tauri default, mature |
| Audio playback | HTML5 `<audio>` via Tauri asset protocol | Streaming, range requests, no full-file IPC roundtrip |

**Do NOT introduce these without discussion:**
- Electron (we chose Tauri specifically for bundle size)
- Yarn / npm as primary package manager (we use pnpm)
- Redux / MobX (we use Zustand)
- Material-UI / Ant Design (we use shadcn/ui)
- Stronghold (deprecated, will be removed in Tauri v3)
- Tesseract / OCR libraries (out of scope for MVP)
- `tauri-plugin-sql` (was original plan, replaced with rusqlite in Sprint 2 — see PR #15 logical for rationale)

## Repository layout (as of Sprint 2 closure)

```
glagol/
├── .github/
│   ├── workflows/          # CI/CD: build.yml, release.yml (Sprint 5)
│   ├── ISSUE_TEMPLATE/     # bug_report.yml, feature_request.yml
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── dependabot.yml
├── .claude/                # AI agent commands and settings (when added)
│   ├── commands/           # /check, /add-tauri-cmd, etc
│   └── settings.json
├── .scratch/               # Gitignored — kickoffs, master logs in progress, personal notes
├── docs/                   # Documentation
│   └── day-logs/           # Per-day/per-session master logs (published via docs PR after Sprint closure)
├── src/                    # React frontend
│   ├── components/
│   │   ├── ui/             # shadcn/ui primitives (Card, Button, Skeleton, ...)
│   │   ├── library/        # Library page components (when split)
│   │   ├── player/         # audio player (Sprint 5)
│   │   └── settings/       # settings UI components
│   ├── contexts/           # CredentialsContext (tri-state)
│   ├── hooks/              # React hooks
│   ├── stores/             # Zustand stores (when needed)
│   ├── lib/                # tauri.ts wrappers + format.ts + voices.ts + types
│   ├── locales/            # i18n: en.json, ru.json (Sprint 7)
│   ├── pages/              # route components: Settings, Synthesize, Library
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── paths.rs        # Single source of truth for filesystem locations
│   │   ├── state.rs        # AppState: Mutex<Connection> + tokio::sync::Mutex<Option<Arc<SaluteAuth>>>
│   │   ├── commands/       # Tauri commands exposed to frontend
│   │   │   ├── credentials.rs   # set/test/delete with force-bypass cache-first
│   │   │   ├── synthesize.rs    # synthesize_document returns document_id
│   │   │   ├── storage.rs       # list_documents/get_audio_path/delete_document/export_audio
│   │   │   └── file.rs          # read_and_parse_file (size/content caps + extension dispatch)
│   │   ├── salute/         # SaluteSpeech client
│   │   │   ├── auth.rs     # OAuth flow with embedded cert
│   │   │   ├── synthesize.rs    # /synthesize endpoint
│   │   │   ├── errors.rs   # SaluteError enum
│   │   │   └── http.rs     # shared HTTP client with cert pinning + RqUID
│   │   ├── parser/         # File parsers (Sprint 4)
│   │   │   ├── mod.rs           # ParsedDocument + ParseError + try_all dispatcher
│   │   │   ├── txt.rs           # BOM → UTF-8 strict → Windows-1251 fallback
│   │   │   ├── md.rs            # pulldown-cmark event filter; code blocks → «фрагмент кода»
│   │   │   ├── docx.rs          # docx-rust paragraph + table (row-by-row) extraction
│   │   │   └── pdf.rs           # pdfium-render dynamic bind; scanned PDFs flagged
│   │   ├── text/
│   │   │   ├── chunker.rs       # text splitting for API limits
│   │   │   └── preprocessor.rs  # URL/email/abbreviation humanization (Sprint 3)
│   │   ├── audio/
│   │   │   └── wav_join.rs # WAV concatenation with streaming header normalization
│   │   ├── db/             # SQLite layer
│   │   │   ├── mod.rs           # init_database + test_connection() helper
│   │   │   ├── migrations.rs    # rusqlite_migration runner + schema
│   │   │   └── repository.rs    # DocumentRecord CRUD free functions
│   │   └── secrets/
│   │       └── keyring.rs  # Windows Credential Manager wrapper
│   ├── assets/
│   │   └── russiantrustedca.pem  # Russian Ministry of Digital Development root cert (committed!)
│   ├── capabilities/
│   │   └── main.json       # Tauri 2 permissions
│   ├── icons/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── CHANGELOG.md            # Maintained from Sprint 5 onward (batched)
├── CLAUDE.md               # this file
├── CODE_OF_CONDUCT.md
├── CONTRIBUTING.md
├── LICENSE
├── README.md
├── SECURITY.md
├── package.json
├── pnpm-lock.yaml          # committed!
├── tailwind.config.ts
├── tsconfig.json
└── vite.config.ts
```

## Architecture invariants

These rules MUST hold across all PRs. If a change requires breaking one, escalate in the PR description.

### Security invariants

1. **No secrets in code, config files, or environment variables.** Authorization Keys live in Windows Credential Manager (`keyring-rs`). Access tokens live only in RAM.
2. **All HTTP to Sberbank goes through `src-tauri/src/salute/http.rs`** — that module handles cert pinning, RqUID generation, retries, and auth. No other module makes raw `reqwest` calls to Sberbank.
3. **No network requests outside the allowlist:** `ngw.devices.sberbank.ru:9443`, `smartspeech.sber.ru`, `api.github.com` (updater only). Enforced via Tauri CSP and `capabilities/main.json`.
4. **No telemetry by default.** Sentry is opt-in, off by default. No analytics. No tracking pixels. No fingerprinting.
5. **No `unsafe` Rust without a `// SAFETY:` comment** explaining the invariants.
6. **No `dangerouslySetInnerHTML` in React.** Ever.
7. **No eval, no dynamic script loading.** CSP `script-src 'self'`.
8. **All user-supplied paths must be validated** against `app_local_data_dir()` or explicit dialog selection.
9. **Asset protocol scope is scoped, not wildcard.** Sprint 2 uses static `["$APPLOCALDATA/audio_cache/**"]`. Sprint 5 (configurable library location) extends via dynamic Rust API (`asset_resolver().scope().allow_directory()`), NOT by switching to `["**"]`.

### Data invariants

1. **SQLite is the source of truth for metadata.** Don't store metadata in JSON files alongside.
2. **Audio files are on disk, NOT in SQLite BLOBs.** SQLite stores **relative paths** (`{uuid}.wav`), resolved through `paths::resolve_audio_path()`.
3. **Audio cache lives under `paths::audio_cache_root()`** — currently `%LOCALAPPDATA%\<bundle>\audio_cache\` (dev: `app.glagol.desktop`, release: `Glagol`). Sprint 5 makes this configurable via Settings; the function is the single grep target for that change.
4. **One document = one row in `documents` table.** `chunks` table reserved for Sprint 4 (parallel synthesis + resume playback).
5. **All migrations are versioned via `user_version` pragma** (managed by `rusqlite_migration`). Migrations are append-only; never edit a shipped migration.
6. **Persistence is transaction-wrapped.** Multi-step writes (INSERT row + fs::write file) use rusqlite `Transaction` — drop semantics auto-rollback on early return. Orphan files acceptable (invisible to user); orphan rows never (user-visible breakage).

### API invariants

1. **Tauri commands return `Result<T, String>`.** Errors become strings on the frontend boundary. Use `thiserror` for internal Rust error types, convert to `String` at the boundary.
2. **Long-running operations (>100ms) must report progress** via `tauri::ipc::Channel<T>` (high-frequency) or `app.emit()` (broadcast).
3. **Concurrent SaluteSpeech requests are limited to 3** via `tokio::sync::Semaphore`. The API allows 5 for personal tier, we leave headroom. (Implementation: Sprint 4 parallel synthesis.)
4. **OAuth tokens are cached** in `tokio::sync::RwLock<Option<(String, i64)>>`. Refresh only when `expires_at < now + 60 seconds`.
5. **`test_credentials` supports cache-first short-circuit with `force: bool` parameter.** Mount-time probes use `force=false` (trust process-lifetime cache); user-initiated Test button uses `force=true` (fresh OAuth call). Established Sprint 2 PR #18.
6. **SaluteSpeech sync API hard limit is 4000 chars per request.** Our chunker targets ≤3500 to leave room for SSML overhead.
7. **Audio bytes never leave Rust over IPC.** `synthesize_document` returns `document_id` (UUID string). Frontend uses `get_audio_path` + asset protocol for playback, `export_audio` (server-side `fs::copy`) for disk export. Established Sprint 2 PR #16.

### Code style invariants

1. **Rust: `cargo fmt` + `cargo clippy -- -D warnings`** — enforced in CI, no exceptions.
2. **TS/React: ESLint + Prettier** — config in repo, enforced in CI.
3. **Functional React components only.** No class components. Hooks > HOCs.
4. **Tailwind utility classes preferred over custom CSS.**
5. **Public Rust APIs documented with `///` doc comments.**
6. **No `console.log` in production code.** Use proper logging (`tracing` on Rust side, dev-only `console.*` in TS).

## SaluteSpeech API — critical reference

**Auth endpoint:** `POST https://ngw.devices.sberbank.ru:9443/api/v2/oauth`
- Headers: `Authorization: Basic <base64(client_id:client_secret)>`, `RqUID: <new-uuid-v4-each-time>`, `Content-Type: application/x-www-form-urlencoded`
- Body: `scope=SALUTE_SPEECH_PERS`
- Response: `{access_token, expires_at}` — `expires_at` is Unix milliseconds, token lives 30 minutes

**Synthesize endpoint:** `POST https://smartspeech.sber.ru/rest/v1/text:synthesize?format=wav16&voice=Nec_24000`
- Headers: `Authorization: Bearer <access_token>`, `Content-Type: application/text`
- Body: raw UTF-8 text, ≤4000 chars, optional SSML wrapped in `<speak>...</speak>`
- Response: binary WAV/PCM/OPUS stream

**Voices (use the `_24000` suffix for quality):**
- `Nec_24000` — Natalia (female, default, supports stress marks `+`)
- `Bys_24000` — Boris (male)
- `May_24000` — Marfa (female)
- `Tur_24000` — Taras (male)
- `Ost_24000` — Alexandra (female)
- `Pon_24000` — Sergey (male, supports stress marks `+`)
- `Kin_24000` — Kira (en-US only)

**Free tier limit:** 200,000 characters/month synthesis, resets monthly, doesn't roll over. Track in `api_usage` table (Sprint 5+).

**TLS:** Sberbank uses the Russian Ministry of Digital Development root CA. Embed `russiantrustedca.pem` in the binary and add it as a root certificate to the `reqwest::Client` via `Certificate::from_pem`. Do NOT disable certificate verification.

**Error handling:**
- 200 OK → audio stream
- 400 → request too large or malformed SSML — show user-friendly error
- 401 → token expired — refresh and retry once
- 429 → rate limit — exponential backoff (start 2s, max 30s, 3 retries)
- 500 → Sberbank-side error — retry once after 5s, then fail with X-Request-ID logged

**Observed smart prosody (Sprint 2 finding):** SaluteSpeech ignores decorative trailing punctuation in numbers (e.g. "1.1." → spoken as "1.1"). Preprocessing should not duplicate Sber's smart behavior — only fix things Sber objectively mishandles (URLs, emails, technical abbreviations).

## Development workflow

### Local dev loop

```powershell
pnpm install              # once after clone
pnpm tauri dev            # runs Vite dev server + Tauri window with hot reload
```

### Quality gates (run before pushing)

```powershell
pnpm lint                 # ESLint
pnpm typecheck            # tsc --noEmit
pnpm test                 # Vitest
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

There will be a `/check` command in `.claude/commands/` that runs all of the above.

### Building a release locally

```powershell
pnpm tauri build          # produces MSI + NSIS installer in src-tauri/target/release/bundle/
```

### Git workflow

1. Branch from `main`: `git checkout -b feat/short-description` (or `fix/`, `docs/`, `claude/` per branch naming convention)
2. Commit using Conventional Commits: `feat: add EPUB parser`
3. Push and open PR via GitHub Desktop or web UI
4. Wait for CI green (when CI is added in Sprint 5)
5. Squash and merge to main

**Never push directly to main.** All changes via PR. Even hotfixes, even one-line fixes, even docs.

## Sprint roadmap (high level)

- **Sprint 0** ✅ Setup (LICENSE, README, community files, dev environment)
- **Sprint 1** ✅ Backend client SaluteSpeech (OAuth + sync synthesis + chunker + WAV join + keyring + Tauri commands + minimal UI) — `v0.1.0-alpha` tag
- **Sprint 2** ✅ Storage + library UI (SQLite persistence + Library page + asset protocol playback + full lifecycle) — `v0.1.0-rc.1` tag
- **Sprint 3** ⏳ Text preprocessing (URL/email humanization, abbreviation expansion, number formatting policy) — `v0.1.0-rc.2` tag
- **Sprint 4** ⏸️ File input + parsing (TXT, MD, DOCX, PDF)
- **Sprint 5** ⏸️ Player polish + CI + first public release (custom audio controls, status badges, parallel synthesis, signed MSI installer, CHANGELOG)
- **v0.1.0 release**
- **Sprint 6** Async synthesis for large docs (>50k chars)
- **Sprint 7** SSML editor, EPUB support, i18n
- **v1.0.0 release**

Each Sprint closes with an annotated git tag (`v0.1.0-alpha`, `v0.1.0-rc.1`, etc.) and a docs PR publishing accumulated master logs to `docs/day-logs/`.

## Working agreements (accumulated through Sprint 1-2)

These conventions emerged during Sprint 1-2 sessions and are now project-wide standards. They supplement (do not override) the architecture invariants above.

### Sprint workflow protocol

Each Sprint follows the same pipeline. Deviating from any step requires explicit chat discussion.

1. **Sprint entry document** — written before any Sprint work begins. Lives in `.scratch/kickoff-day-N.md`. Defines Sprint scope, expected PRs, architectural Q&A topics.

2. **Pre-implementation architectural Q&A** — happens in chat between user and chat-Claude before any code is written. Surfaces design decisions, deps freshness checks (via web search), trade-offs. No CC involvement during Q&A.

3. **Kickoff document for each PR** — written by chat-Claude after Q&A, saved to `.scratch/kickoff-day-N-session-M.md`. Contains: locked architectural decisions (do-not-relitigate), scope IN/OUT lists, file-by-file breakdown, pre-answered questions, quality gates, workflow phases, PR creation parameters.

4. **CC implements per kickoff** — follows phase structure if specified. Reports after each phase OR at completion if scope is purely additive (small surface).

5. **Phase-by-phase reporting** — REQUIRED when PR touches existing production code paths. Optional but encouraged for purely additive scope. Each phase ends with sanity checks (`cargo check`, `cargo test`, `pnpm tsc`) and chat acknowledgment before next phase begins.

6. **Paste-back-first protocol** — CC composes consolidated paste-back summary, posts to chat, **does NOT call `mcp__github__create_pull_request`** until chat review approves. Paste-back format below.

7. **Web fetch sanity check** — after CC creates PR, chat-Claude fetches PR URL via `web_fetch`, reviews PR body content, observes auto-injection issues, approves or requests corrections.

8. **Explicit `merge it` from user** — CC does NOT auto-merge. User reviews chat-Claude's sanity check, decides, gives explicit "merge it" command. User performs squash-merge via GitHub web UI.

9. **Runtime verification post-merge** — `git pull` + `cargo test` + `pnpm tsc --noEmit` + `pnpm tauri dev` runtime QA on Windows. 5-step protocol minimum, expanded for PRs touching production code paths.

10. **Closure tag for Sprint milestones** — `git tag -a vX.Y.Z-channel.N -m "Sprint N closure: ..."` + `git push origin vX.Y.Z-channel.N`. Manual command, not part of any PR.

11. **Master log per session/day** — chat-Claude writes `day-N-session-M-master-log.md` after Sprint closure. Saved initially to `.scratch/`, then published via dedicated docs PR to `docs/day-logs/`.

### CC paste-back format

CC's paste-back to chat after PR work complete (before PR creation) must include:

- **Branch + final commit hash**
- **Files touched** grouped by phase (filenames + LOC delta, NOT full file content dumps — chat has project knowledge, project files load on demand for debugging)
- **Test count arithmetic** explicit: `baseline + new − deleted = final`
- **New tests** listed by name (one per line)
- **Adapted tests** listed by name (if signature changes etc.)
- **Deleted tests** listed by name with rationale
- **Quality gate outputs** — last ~5 lines of each command (`cargo test`, `pnpm tsc`, `pnpm build`)
- **Bundle size delta** if frontend touched (baseline + delta numbers)
- **Deviations from spec** consolidated — CC's improvements over kickoff are welcome, document rationale
- **Open questions** if any (otherwise: "none")

**Do NOT include** in paste-back: full file contents, code snippets longer than ~10 LOC, mechanical re-statements of what the kickoff already specified. Chat-Claude has project knowledge and reads files on demand during debugging or review. Paste-back is a status report, not a code dump.

### Code conventions established Sprint 1-2

These patterns emerged from specific implementations and apply to similar future work:

- **`*_impl` helper pattern for Tauri commands.** Tauri commands taking `tauri::State<'_, AppState>` are not directly unit-testable (no Tauri runtime in test env). Extract a pure `*_impl(state: &AppState, ...)` function for tests; thin command wrapper delegates to impl.

- **Block-scoped Mutex guards** when followed by FS or network operations. Use:
  ```rust
  let extracted_value = {
      let conn = db.lock().expect("...");
      // ... DB ops ...
      record.audio_path  // return the value needed outside the lock
  };  // ← guard drops here
  // FS / network ops below use extracted_value
  ```
  This is compiler-enforced ordering. Avoid `drop(conn);` explicit pattern — future refactors can accidentally inline operations into the lock scope without visible structural change.

- **`Connection::open_in_memory()` for repository tests.** Per-test in-memory SQLite. No tempfile, no Windows file locking flakiness. Established in `db/mod.rs` as `test_connection()` helper.

- **Per-test temp dir with UUID** for filesystem tests. `std::env::temp_dir().join(format!("glagol_test_{}", Uuid::new_v4()))`. Cleanup in test body via `std::fs::remove_dir_all`. No `tempfile` crate needed.

- **Pure-CRUD repository functions.** UUID + timestamps generated at command layer, not in repository. Repository takes pre-formed records, returns deterministic results without time/randomness mocking.

- **Transaction-wrapped multi-step writes.** When DB INSERT + filesystem write must be atomic-ish:
  ```rust
  let tx = conn.transaction()?;
  repo::insert(&tx, &record)?;
  fs::write(&path, &bytes)?;
  tx.commit()?;
  ```
  Drop semantics auto-rollback on early return (file write failure → row not committed).

- **Cache-first short-circuit with `force: bool` parameter** for repeatedly-invoked validation commands. Mount-time probes trust process-lifetime cache; user-initiated actions explicitly bypass via `force: true`. See `commands::credentials::test_credentials_impl` for canonical example.

- **Discriminated union state machines** for async UI states. Pattern: `{ kind: 'loading' } | { kind: 'empty' } | { kind: 'ready', data } | { kind: 'error', message }`. Cleaner than boolean stews (`loading && !error && data`). Scales to status additions without rewrite.

- **Negative regression sanity cycle** for all bugfix PRs (PR #13 streaming WAV pattern established, PR #20 credentials cache-first reinforced):
  1. Apply fix
  2. Write new test catching the bug
  3. Comment out the fix
  4. Run new test — must fail with the expected error message
  5. Restore fix
  6. Run new test — must pass
  7. Document the cycle in paste-back and PR body
  
  This proves the fix is load-bearing, not a hopeful patch.

### Attribution and authorship

This is a human-architected project. AI tools (Claude in chat, Claude Code) are collaborative partners under human direction, not autonomous authors.

**Required PR footer (instead of any AI-tool default):**

```
Created by Dmitriy + Claude
```

This acknowledges both contributors honestly: Dmitriy as architect, designer, reviewer, decision-maker; Claude as pair-programming partner (chat) and implementation tool (Claude Code).

**Prohibited attribution patterns:**

- ❌ `Generated by Claude Code` (implies autonomous AI authorship — incorrect)
- ❌ `Co-authored-by: Claude <noreply@...>` trailer in commit messages
- ❌ Auto-injected AI tool links or session URLs in PR bodies
- ❌ Any framing that omits human architecture/review work

**If your tooling auto-injects unwanted attribution:**

After PR creation via `mcp__github__create_pull_request`, immediately follow up with `mcp__github__update_pull_request` to strip the auto-footer and replace with the correct "Created by Dmitriy + Claude" line. This is required, not optional — incorrect attribution is a defect to be corrected, not a cosmetic preference.

### Documentation conventions

- **PR descriptions use bilingual format:** RU summary at top + EN technical body below. RU summary covers "what + why + scope NOT included"; EN body has standard sections (Description, Type of change, Schema impact, How has this been tested, Test breakdown, Manual testing steps, Security checklist, Breaking changes, Additional context, Checklist).

- **PR titles avoid literal `#N` references.** GitHub assigns numbers; titles describe content. Example: `feat: real library page with playback, delete, and export` (good) vs `feat: Library page (PR #17)` (bad — collides with GitHub auto-linkification).

- **Conventional Commits for commit messages.** `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`. Scope optional in parentheses: `feat(library): ...`.

- **Single squash-merge commit per PR** with composed long-form message. Phase-commits on branch are squashed at merge — they're working memory, not git archaeology.

- **Branch naming:**
  - `feat/<descriptor>` — human-written feature branches
  - `fix/<descriptor>` — human-written bugfix branches
  - `docs/<descriptor>` — documentation-only changes
  - `claude/<descriptor>` — Claude Code session branches (auto-deleted after squash-merge)

- **Sprint-deferred items** tracked in user's local notebook with explicit code-side grep markers. Example: `paths::audio_cache_root` is the single point of resolution for audio cache path; Sprint 5 work targeting configurable library location starts by grepping this symbol.

- **Master log writing schedule:**
  - During Sprint: each session/day gets a master log written to `.scratch/`
  - At Sprint closure: master logs published via dedicated docs PR to `docs/day-logs/`
  - Never push directly to `main` for docs either — always via PR

### Windows-specific notes

These quirks repeatedly affect dev sessions:

- **PowerShell 5.x does NOT support `&&` chaining.** Use `;` separator or run commands separately. PowerShell 7+ supports `&&` but is not default on Windows 10.

- **Tauri 2 dev path resolution differs from release.** `app_local_data_dir()` resolves to `%LOCALAPPDATA%\<bundle.identifier>\` (e.g. `app.glagol.desktop\`) for unsigned dev builds, and `%LOCALAPPDATA%\<productName>\` (e.g. `Glagol\`) for signed release builds (MSI installer). Document the actual dev path in user-facing docs at release time.

- **DB Browser for SQLite** recommended for runtime verification. Visual schema inspection during manual QA is faster than CLI queries. Free, OSS, ~10 MB installer.

- **Asset protocol Windows scheme is `http://`** (not `https://`). CSP `media-src` must include `http://asset.localhost`. Verified Tauri 2.x convention.

- **Tauri runtime features require Cargo feature parity.** Enabling `assetProtocol` in `tauri.conf.json` requires `tauri = { features = ["protocol-asset"] }` in `Cargo.toml`. Build script enforces this — discovered runtime in Sprint 2 PR #17.

- **Pdfium DLL distribution.** `src-tauri/build.rs` downloads `chromium/7834` from `bblanchon/pdfium-binaries` on the first build (uses `curl` + `tar` already on every supported host) and caches the unpacked `pdfium.dll` / `libpdfium.so` / `libpdfium.dylib` in `OUT_DIR/pdfium/`. The absolute path is propagated to the compiled binary via the `PDFIUM_LIBRARY_PATH` env var and read in `parser::pdf` via `env!()`. For the Sprint 5 MSI installer, the matching `pdfium.dll` must be bundled alongside the `.exe` (Tauri's NSIS / WiX bundle config picks it up from a known location next to the binary; falls back to `Pdfium::bind_to_system_library()` at runtime if the cached path is missing).

### Things NOT to repeat

Anti-patterns encountered Sprint 1-2 that should be avoided:

- **Don't paste full file contents in CC paste-back.** Chat has project knowledge; files load on demand. Paste-back is a status report, not a code dump.

- **Don't claim "Sprint X = 100%" before runtime verification.** Code complete ≠ runtime verified. Manual QA on target platform is a hard gate.

- **Don't write `&&` in PowerShell command examples** for Windows users. Use `;` or separate commands.

- **Don't merge bugfixes without negative regression sanity cycle.** The cycle proves the fix is causal, not coincidental.

- **Don't direct-commit to `main`.** Even hotfixes, even one-line fixes, even docs. PR + squash-merge.

- **Don't assume CC auto-fixes auto-injected AI attribution.** Tool-level injection requires explicit `mcp__github__update_pull_request` follow-up. This must happen for every PR.

- **Don't over-engineer based on speculation.** Sprint 5 polish items get added when real usage signals demand. Pre-emptive UX upgrades (undo toasts before users complain about misclicks) are friction, not features.

## Working with AI assistants in this repo

### Claude Code best practices

1. **Read this file first.** Always. Including the Working Agreements section above.
2. **Before writing code, check existing patterns** in similar modules. We have invariants — follow them.
3. **One PR = one concern.** Don't refactor while adding features.
4. **Write tests for new logic.** Rust: `cargo test` unit tests in same file or `tests/` dir. TS: Vitest in `*.test.ts` files.
5. **Update CHANGELOG.md** under "Unreleased" for user-facing changes (from Sprint 5 onward).
6. **Respect the threat model in SECURITY.md.** No telemetry, no leaks, no unsafe deps.
7. **When in doubt, ask in the PR description** rather than making assumptions.
8. **Follow the Sprint workflow protocol above** — kickoff → phases → paste-back → PR creation → wait for merge.

### When you (AI) should refuse or push back

- Request to add telemetry or analytics → refuse, point to SECURITY.md
- Request to hardcode credentials anywhere → refuse, point to keyring-rs setup
- Request to add network endpoint outside the allowlist → require justification + CSP update
- Request to use `unsafe` Rust → ask for `// SAFETY:` comment with explanation
- Request to disable cert verification → refuse, pin certs properly
- Request to add a dependency with non-OSI license → refuse
- Request to commit any file matching `*.key`, `*.pem` (except cert), `.env*` → refuse, this is in .gitignore for a reason
- Request to skip Working Agreements protocol (e.g. "just go ahead and write the code without Q&A") → push back, ask for confirmation that protocol is being intentionally bypassed for this specific case

### Useful slash commands (in `.claude/commands/`)

These will be added as the project progresses:

- `/check` — runs all linters and tests
- `/add-tauri-cmd <name>` — scaffolds a new Tauri command (Rust + TS wrapper)
- `/add-migration <name>` — creates timestamped SQL migration
- `/add-parser <format>` — scaffolds a new file parser module
- `/release <version>` — bumps version in 3 places + creates git tag

## Glossary

- **TTS** — text-to-speech (text → audio)
- **SaluteSpeech** — Sberbank's speech synthesis API (third-party)
- **SSML** — Speech Synthesis Markup Language (XML-like, for controlling pronunciation)
- **Chunker** — module that splits long text into ≤3500-char pieces respecting sentence boundaries
- **Preprocessor** — module that humanizes mechanical pronunciation issues (URLs, emails, abbreviations) before chunking
- **Synthesis** — the act of converting text to audio via API call
- **Cache** — local storage of generated WAV files, indexed by document ID
- **Library** — user-facing list of documents the user has synthesized
- **Authorization Key** — the long base64 string from developers.sber.ru that grants API access (single, persistent credential)
- **Access Token** — short-lived (30 min) JWT obtained via OAuth using the Authorization Key
- **НУЦ Минцифры** (NUC Mintsifry) — Russian Ministry of Digital Development root CA, required to verify Sberbank's TLS certificates
- **CC** — Claude Code, the implementation-focused AI tool used for writing code per kickoff specs
- **chat-Claude** — the conversational Claude (e.g. Claude in this chat or claude.ai) used for architectural discussion, Q&A, code review, master log writing
- **Kickoff** — pre-implementation spec document written by chat-Claude before each PR, capturing locked architectural decisions and step-by-step implementation guidance for CC

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE).

By contributing, you agree your contributions are licensed under the same terms.

---

*Last updated: 2026-05-19 (Sprint 4 entry — file parsers landed)*
*Maintained by: Glagol Contributors*
