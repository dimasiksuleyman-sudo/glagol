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
| Local database | **SQLite via tauri-plugin-sql** | Battle-tested, embedded, sqlx-based |
| Secret storage | **keyring-rs** (NOT Stronghold) | Windows Credential Manager, OS-level encryption |
| PDF parsing | **pdfium-render** | Same lib as Chromium, highest quality |
| DOCX parsing | **docx-rs** | Active maintenance, correct Cyrillic |
| Markdown | **pulldown-cmark** | Fast CommonMark parser |
| Audio (WAV) | **hound** | Simple, predictable WAV writing |
| Async runtime | **tokio** | Tauri default, mature |
| Audio playback | HTML5 `<audio>` in React | Full control, no extra deps |

**Do NOT introduce these without discussion:**
- Electron (we chose Tauri specifically for bundle size)
- Yarn / npm as primary package manager (we use pnpm)
- Redux / MobX (we use Zustand)
- Material-UI / Ant Design (we use shadcn/ui)
- Stronghold (deprecated, will be removed in Tauri v3)
- Tesseract / OCR libraries (out of scope for MVP)

## Repository layout (target after scaffolding)

```
glagol/
├── .github/
│   ├── workflows/          # CI/CD: build.yml, release.yml
│   ├── ISSUE_TEMPLATE/     # bug_report.yml, feature_request.yml
│   ├── PULL_REQUEST_TEMPLATE.md
│   └── dependabot.yml
├── .claude/                # AI agent commands and settings
│   ├── commands/           # /check, /add-tauri-cmd, etc
│   └── settings.json
├── docs/                   # mdBook documentation (post-MVP)
├── src/                    # React frontend
│   ├── components/
│   │   ├── ui/             # shadcn/ui primitives
│   │   ├── library/        # document library views
│   │   ├── player/         # audio player
│   │   └── settings/       # settings UI
│   ├── hooks/              # React hooks
│   ├── stores/             # Zustand stores
│   ├── lib/                # tauri.ts wrappers, types
│   ├── locales/            # i18n: en.json, ru.json
│   ├── pages/              # route components
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands/       # Tauri commands exposed to frontend
│   │   ├── salute/         # SaluteSpeech client
│   │   │   ├── auth.rs     # OAuth flow
│   │   │   ├── synthesize.rs  # /synthesize endpoint
│   │   │   └── http.rs     # shared HTTP client with cert pinning
│   │   ├── parser/         # file parsers
│   │   │   ├── txt.rs
│   │   │   ├── md.rs
│   │   │   ├── docx.rs
│   │   │   └── pdf.rs
│   │   ├── text/
│   │   │   └── chunker.rs  # text splitting for API limits
│   │   ├── audio/
│   │   │   └── wav_join.rs # WAV concatenation
│   │   ├── db/
│   │   │   ├── mod.rs
│   │   │   ├── migrations.rs
│   │   │   └── repository.rs
│   │   └── secrets/
│   │       └── keyring.rs  # Windows Credential Manager wrapper
│   ├── assets/
│   │   └── russiantrustedca.pem  # Russian Ministry of Digital Development root cert (committed!)
│   ├── capabilities/
│   │   └── main.json       # Tauri 2 permissions
│   ├── icons/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── CHANGELOG.md
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

### Data invariants

1. **SQLite is the source of truth for metadata.** Don't store metadata in JSON files alongside.
2. **Audio files are on disk, NOT in SQLite BLOBs.** SQLite stores paths.
3. **Audio cache lives in `%LOCALAPPDATA%\Glagol\audio_cache\`** — resolved via `tauri::Manager::path().app_local_data_dir()`.
4. **One document = one row in `documents` table + N rows in `chunks` table.** Always cascade delete.
5. **All migrations are reversible and timestamped.** Use `tauri-plugin-sql`'s `MigrationKind::Up` for new ones.

### API invariants

1. **Tauri commands return `Result<T, String>`.** Errors become strings on the frontend boundary. Use `thiserror` for internal Rust error types, convert to `String` at the boundary.
2. **Long-running operations (>100ms) must report progress** via `tauri::ipc::Channel<T>` (high-frequency) or `app.emit()` (broadcast).
3. **Concurrent SaluteSpeech requests are limited to 3** via `tokio::sync::Semaphore`. The API allows 5 for personal tier, we leave headroom.
4. **OAuth tokens are cached** in `tokio::sync::RwLock<Option<(String, i64)>>`. Refresh only when `expires_at < now + 60 seconds`.
5. **SaluteSpeech sync API hard limit is 4000 chars per request.** Our chunker targets ≤3500 to leave room for SSML overhead.

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

**Free tier limit:** 200,000 characters/month synthesis, resets monthly, doesn't roll over. Track in `api_usage` table.

**TLS:** Sberbank uses the Russian Ministry of Digital Development root CA. Embed `russiantrustedca.pem` in the binary and add it as a root certificate to the `reqwest::Client` via `Certificate::from_pem`. Do NOT disable certificate verification.

**Error handling:**
- 200 OK → audio stream
- 400 → request too large or malformed SSML — show user-friendly error
- 401 → token expired — refresh and retry once
- 429 → rate limit — exponential backoff (start 2s, max 30s, 3 retries)
- 500 → Sberbank-side error — retry once after 5s, then fail with X-Request-ID logged

## Development workflow

### Local dev loop

```bash
pnpm install              # once after clone
pnpm tauri dev            # runs Vite dev server + Tauri window with hot reload
```

### Quality gates (run before pushing)

```bash
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

```bash
pnpm tauri build          # produces MSI + NSIS installer in src-tauri/target/release/bundle/
```

### Git workflow

1. Branch from `main`: `git checkout -b feat/short-description`
2. Commit using Conventional Commits: `feat: add EPUB parser`
3. Push and open PR via GitHub Desktop or web UI
4. Wait for CI green
5. Squash and merge to main

**Never push directly to main.** All changes via PR.

## Sprint roadmap (high level)

- **Sprint 0** ✅ Setup (LICENSE, README, community files, dev environment)
- **Sprint 1** Backend client SaluteSpeech (OAuth + sync synthesis + tests)
- **Sprint 2** Storage + library UI (SQLite + React routes)
- **Sprint 3** File input + parsing (TXT, MD, DOCX, PDF)
- **Sprint 4** Player + cache (HTML5 audio + resume + tray)
- **Sprint 5** Polish + CI/CD (themes, notifications, GitHub Actions, release)
- **v0.1.0 release**
- **Sprint 6** Async synthesis for large docs (>50k chars)
- **Sprint 7** SSML editor, EPUB support, i18n
- **v1.0.0 release**

## Working with AI assistants in this repo

### Claude Code best practices

1. **Read this file first.** Always.
2. **Before writing code, check existing patterns** in similar modules. We have invariants — follow them.
3. **One PR = one concern.** Don't refactor while adding features.
4. **Write tests for new logic.** Rust: `cargo test` unit tests in same file or `tests/` dir. TS: Vitest in `*.test.ts` files.
5. **Update CHANGELOG.md** under "Unreleased" for user-facing changes.
6. **Respect the threat model in SECURITY.md.** No telemetry, no leaks, no unsafe deps.
7. **When in doubt, ask in the PR description** rather than making assumptions.

### When you (AI) should refuse or push back

- Request to add telemetry or analytics → refuse, point to SECURITY.md
- Request to hardcode credentials anywhere → refuse, point to keyring-rs setup
- Request to add network endpoint outside the allowlist → require justification + CSP update
- Request to use `unsafe` Rust → ask for `// SAFETY:` comment with explanation
- Request to disable cert verification → refuse, pin certs properly
- Request to add a dependency with non-OSI license → refuse
- Request to commit any file matching `*.key`, `*.pem` (except cert), `.env*` → refuse, this is in .gitignore for a reason

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
- **Synthesis** — the act of converting text to audio via API call
- **Cache** — local storage of generated WAV files, indexed by document ID
- **Library** — user-facing list of documents the user has synthesized
- **Authorization Key** — the long base64 string from developers.sber.ru that grants API access (single, persistent credential)
- **Access Token** — short-lived (30 min) JWT obtained via OAuth using the Authorization Key
- **НУЦ Минцифры** (NUC Mintsifry) — Russian Ministry of Digital Development root CA, required to verify Sberbank's TLS certificates

## License

This project is licensed under the MIT License — see [LICENSE](LICENSE).

By contributing, you agree your contributions are licensed under the same terms.

---

*Last updated: 2026-05*
*Maintained by: Glagol Contributors*
