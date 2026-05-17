# Glagol — Day 4 Master Log (Part 2)

**Period:** May 17, 2026 (continuation — Sessions 3 through 5)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 1 (SaluteSpeech client) — **CLOSURE**
**Status at end of Day 4:** Sprint 1 **100% complete**. End-to-end pipeline verified in production environment.

> Part 1 (`day-4-master-log.md`) covers Sessions 1-2 (wav_join, keyring, hotfix #9). This file picks up at Session 3.

---

## TL;DR Part 2

За один день закрыли весь Sprint 1: backend commands (PR #11), frontend UI (PR #12), и критический hotfix (PR #13) после Manual QA discovery. К концу дня — **MVP работает end-to-end через GUI**: пользователь вводит SaluteSpeech AK, пишет русский текст, выбирает один из 6 голосов, нажимает «Озвучить и сохранить», получает WAV файл на диске который проигрывается в Windows Media Player.

**Day 4 spans 5 CC sessions** — самый длинный день проекта по содержательности. Включает 3 merged PRs + manual QA discovery + critical bugfix + runtime verification.

Test count progression Day 4: 38 → 48 (Session 1) → 55 (Session 2) → 71 (Session 3) → 71 (Session 4) → **76 (Session 5)** на main.

---

## Session 3 — PR #6b-1 / GitHub #11 (Tauri commands backend)

### Цель

Реализовать backend для пользовательского flow: 5 Tauri commands (4 заявленных в спеке + `write_wav_file` добавилось по ходу анализа). После этого frontend PR #6b-2 строится против работающего API, не моков.

### Pre-implementation discussion — 7 архитектурных вопросов

Перед kickoff'ом прошли 7 Q&A для CC. Critical decisions:

1. **AppState через `Mutex<Option<Arc<SaluteAuth>>>`** в tokio::sync — async-safe locking, lazy initialization, Arc для shared instance между concurrent commands
2. **Sequential chunk synthesis** для MVP — параллелизм через Semaphore deferred до Sprint 4
3. **`Result<Vec<u8>, String>` everywhere** — flat string errors на frontend boundary
4. **One retry on 401** через `SaluteAuth.invalidate()` — без exponential backoff
5. **Strict production CSP + dev override** — `csp` (только Sber hosts) + `devCsp` (+ ws://localhost:1421 для Vite HMR)
6. **5-я команда `write_wav_file`** добавлена после анализа capabilities — без неё frontend не может писать на диск
7. **Mock keyring + mock SaluteAuth** через mockito для тестов — без живого Tauri runtime

### Implementation через CC cloud

CC задал **3 хороших вопроса** к спеке (voice serialization, token retry semantics, mutex lock pattern). Все 5 решений согласованы до старта кода.

**Backend additions over spec:**
- `VoiceId::from_api_id()` + `impl FromStr for VoiceId` + `UnknownVoiceId` error type — для парсинга `"Nec_24000"` из frontend
- `SaluteAuth::invalidate()` — public method для retry на 401
- `ProgressEvent` enum с `#[serde(tag = "kind", rename_all = "camelCase")]` для tagged TS discriminated union

**Files created:**
- `src-tauri/src/state.rs` (47 LOC) — AppState struct
- `src-tauri/src/commands/mod.rs` (10 LOC) — module decl
- `src-tauri/src/commands/credentials.rs` (~210 LOC) — 3 commands + 6 tests
- `src-tauri/src/commands/synthesize.rs` (~270 LOC) — 2 commands + ProgressEvent + 6 tests

**Files modified:**
- `src-tauri/Cargo.toml` (+1 dep: `tauri-plugin-dialog = "2"`)
- `src-tauri/src/lib.rs` (+18/-7 lines) — manage state, register plugin, swap greet → 5 commands
- `src-tauri/tauri.conf.json` — CSP + devCsp
- `src-tauri/capabilities/default.json` — +dialog permissions
- `src-tauri/src/salute/auth.rs` (+invalidate method + 1 test)
- `src-tauri/src/salute/synthesize.rs` (+VoiceId parsing + 3 tests)

### Quality gates Session 3

| Check | Result |
|---|---|
| `cargo check` | clean |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | **71 passed; 0 failed; 0 ignored** (55 prior + 16 new) |

### Merge

PR #11 squash-merge как `68b6381`. Бран `claude/pr-6b1-tauri-commands` auto-deleted GitHub'ом.

Local verification — `cargo test` после `git pull` показал **71 passing**.

---

## Session 4 — PR #6b-2 / GitHub #12 (Frontend + binary IPC)

### Цель — финальный PR Sprint 1

Полная замена дефолтного Tauri Greet shell на 3-страничное React приложение wired против 5 commands из PR #11.

### Pre-implementation — 7+2 архитектурных вопросов

| # | Решение |
|---|---|
| Q1 | react-router-dom 7.x (не TanStack, не conditional rendering) |
| Q2 | React Context для credentials (не Zustand — overkill для MVP) |
| Q3 | shadcn/ui `init --yes` non-interactive setup |
| Q4 | sonner для toasts (не legacy useToast) |
| Q5 | Select dropdown с **только** именами голосов (после fact-check — все голоса Сбера поддерживают ударения автоматически и через `'`, не только Наталья/Сергей как утверждал compass artifact) |
| Q6 | Single button "Озвучить и сохранить" — chain synthesis → save dialog |
| Q7 | Coming Soon banner для Library |
| Q-extra-1 | No dark mode toggle, только `prefers-color-scheme` |
| Q-extra-2 | Hardcoded RU strings, без i18n |

### CC discoveries — adapt-on-fail decisions

После Phase 1 (Tailwind + shadcn setup) CC reported 4 отклонения от спеки — все принятые:

| Что | Reason |
|---|---|
| **Tailwind v4** вместо v3 | shadcn в 2026-05 ставит v4 (4.3.0). CSS-first config (`@theme inline`) вместо `tailwind.config.ts`. Работает. |
| **Base color "neutral"** (Nova preset) вместо запрошенного slate | Spec'овский `--base-color slate` flag depricated в новом CLI; используется `--preset` system. Neutral для MVP даже лучше — universal без blue undertone. |
| **shadcn как production dep** | Новое поведение CLI 2025+. Tree-shakeable. Acceptable. |
| **`radix-ui` monolithic** вместо `@radix-ui/react-*` | Новый style import path. Equivalent functionality. |

### Q1 from CC — IPC performance discovery

**CC спросил Q1 (performance):** `Vec<u8>` через Tauri IPC сериализуется как JSON array of numbers. 21 MB WAV → 84 MB JSON over the wire. Для 50K-char документа — секунды latency.

**Решение:** минимальный backend tweak — `synthesize_document` возвращает `Result<tauri::ipc::Response, String>` вместо `Vec<u8>`. `Response` едет как raw ArrayBuffer без serde overhead. Frontend получает через `invoke<ArrayBuffer>` → `Uint8Array`.

`synthesize_document_impl` неприкосновенен (продолжает возвращать `Vec<u8>`) — только thin command wrapper меняется (7 строк). **0 test changes.**

`write_wav_file` остаётся на JSON-array input — Tauri 2 raw-body API для input требует HTTP header trick, не стоит того.

PR title пришлось менять на `feat(frontend): user-facing UI + binary IPC for synthesis` — отражает both frontend + small backend tweak.

### Implementation phases

CC отчитывался по Phase 1-5:
- **Phase 1:** Tailwind v4 + shadcn init + 9 components — committed
- **Phase 2:** Skeleton + routing + page stubs — committed (sanity checkpoint: `pnpm build` 393 KB, `pnpm dev` стартует за 327ms)
- **Phase 3:** Real implementation (5 wrappers + Context + 3 pages + backend tweak) — committed
- **Phase 4-5:** Quality gates + paste-back + PR creation

### Frontend code highlights

**`src/lib/tauri.ts`** (90 LOC):
- `ProgressEvent` как proper discriminated union (`{kind: "chunked", total: N} | {kind: "synthesizingChunk", current, total} | {kind: "joining"}`)
- 5 typed wrappers с правильным IPC bridge (ArrayBuffer → Uint8Array)
- JSDoc на каждой функции с context

**`src/contexts/CredentialsContext.tsx`** — tri-state signal с StrictMode-safe useEffect:
- States: `"unknown" | "valid" | "invalid"`
- Mount-time probe `testCredentials()` через cancellation flag (React 19 dev double-mount race protection)
- Source of truth — keyring + cached `SaluteAuth` на Rust side; context мирорит latest known answer

**`src/pages/Synthesize.tsx`** (180 LOC):
- 3 ветви render по `state`: `"unknown"` → "Загружаем…", `"invalid"` → Gating Card с Link to Settings, `"valid"` → full UI
- Single button handler: synthesizeDocument → dialog.save() → null check → writeWavFile → toast
- `try/catch/finally` с reset progress
- ProgressIndicator helper с 5%/90%/5% split percentage logic
- Path basename для toast через `path.split(/[\\/]/).pop() ?? path` (Windows + POSIX safe)

### Quality gates Session 4

| Check | Result |
|---|---|
| `pnpm tsc --noEmit` | exit 0 |
| `pnpm build` | 1894 modules, 393 KB / 126 KB gzipped |
| `pnpm dev` | Vite started in 327ms |
| `cargo test` (backend regression) | 71 passed |
| `cargo clippy + fmt` | clean |
| Manual QA | **deferred** to local Windows runtime (no WebView2 in CC cloud) |

### Merge

PR #12 squash-merge как `69f2e2b`. Auto-delete сработало.

`pnpm install` локально подтянул 397 deps. Один minor environment hurdle — pnpm 11 заблокировал `msw` build script (transitive dep через shadcn). Решено через `pnpm approve-builds` → отказать msw (мы не browser-mocking project).

### Premature "Sprint 1 = 100%" claim

После merge PR #12 я (Claude в чате) заявил **«Sprint 1 = 100%»** в master log preview. **Был неправ.** Sprint 1 в этот момент был **«code complete, runtime unverified»**. Manual QA как раз и существует, чтобы такие claims проверять до фиксации.

Lesson reinforced: **never claim completion before runtime verification on target platform.** Это правило zapisanaв lessons learned ниже.

---

## Manual QA — critical bug discovery

После merge PR #12 — `pnpm tauri dev` локально на Windows 11.

### Сценарий 1 (Shell + Navigation) — PASS

Окно открывается. Sidebar с тремя пунктами «Озвучить» / «Библиотека» / «Настройки» работает. Default route `/synthesize`. Lucide icons загружены. Geist Cyrillic шрифт работает. DevTools console чистый — никаких CSP violations.

Brand добавил CC: «Glagol — Озвучка длинных русских текстов» в AppShell tagline. Не из спеки, инициатива.

### Сценарий 2 (Settings flow) — PASS

| Action | Result |
|---|---|
| Невалидный AK "fake123" → Save | Toast «Ключ сохранён» |
| Test (с fake123) | Toast error: `API returned status 400: {"code":4,"message":"Can't decode 'Authorization' header"}` |
| Delete | Toast «Ключ удалён» |
| Real AK → Save → Test | Toast «Ключ работает», status → «подтверждён Сбером» |

**Critical milestone:** real OAuth round-trip к `ngw.devices.sberbank.ru:9443` через embedded НУЦ Минцифры сертификат **сработал**. TLS pinning из PR #2 (Day 2) доказан в production env впервые с момента написания. **Главный технический риск проекта закрыт окончательно.**

Observation про error UX: Невалидный AK дал status 400 (не 401) потому что Sber не дошёл до проверки credentials — упал на Base64 decode. Mapping в `salute/auth.rs` показывает raw `Api(status, body)` ошибку. Acceptable для MVP; UX-friendly toast mapping → Sprint 5 polish.

### Сценарий 3 (Synthesize end-to-end) — **FAIL** ❌

Это **golden moment** проекта. Здесь мы поймали критический bug.

```
User input:
  text: "Привет, это тестовая озвучка для проверки приложения Глагол."
  voice: Наталья
  click: «Озвучить и сохранить»

Result:
  Toast error: "invalid WAV data: chunk 0: Ill-formed WAVE file:
                data chunk length is not a multiple of sample size"
```

Bug **воспроизводится** на разных текстах (62 chars, 463 chars) и голосах (Наталья, Борис). Систематическая, не glitch.

### Diagnostic — PowerShell forensics

User вспомнил, что Sber **может** возвращать WAV-shaped responses с error codes внутри (видели на Day 0 на 401). Hypothesis: «возможно мы получаем WAV-shaped error payload, а не реальное audio».

**Изоляция через PowerShell test** — повторили Day 0 OAuth + synthesize flow без приложения:

**Attempt 1:** `Invoke-RestMethod` дал 1 KB файл с mojibake content. Это **false positive** — `Invoke-RestMethod` неправильно стримит binary, пытается text decode и обрезается на первом не-UTF8 байте. Bug в test, не в Sber.

**Attempt 2:** Raw HTTP через `[System.Net.HttpWebRequest]` с `GetResponseStream().CopyTo()` — корректный binary read. Файл 85,036 bytes, проигрывается в Windows Media Player как нормальный WAV.

**Hex dump first 80 bytes:**

```
Offset 0-3:    52 49 46 46              "RIFF"
Offset 4-7:    F7 FF FF FF              RIFF size = 0xFFFFFFF7 ← STREAMING MARKER
Offset 8-11:   57 41 56 45              "WAVE"
Offset 12-15:  66 6D 74 20              "fmt "
Offset 16-19:  10 00 00 00              fmt size = 16 ✓
Offset 20-21:  01 00                    format = 1 (PCM) ✓
Offset 22-23:  01 00                    channels = 1 ✓
Offset 24-27:  C0 5D 00 00              sample rate = 24000 ✓
Offset 28-31:  80 BB 00 00              byte rate = 48000 ✓
Offset 32-33:  02 00                    block align = 2 ✓
Offset 34-35:  10 00                    bits per sample = 16 ✓
Offset 36-39:  64 61 74 61              "data"
Offset 40-43:  D3 FF FF FF              data size = 0xFFFFFFD3 ← STREAMING MARKER
Offset 44+:    actual samples
```

**Root cause identified:** SaluteSpeech streams WAV responses with max-`u32` markers (`0xFFFFFFxx`) in RIFF size and data size fields. Server doesn't know final size when sending headers (streaming TTS generation). The actual audio payload is intact — Windows Media Player accepts it because WMP ignores declared sizes and reads to EOF.

**`hound::WavReader` strictly validates declared sizes.** `0xFFFFFFD3` is odd, fails `% sample_size (2) == 0` check, rejects buffer entirely. Streaming WAVs — known gap in hound's spec coverage.

**Bug is in PR #5 `audio::wav_join`**, не в Sber. Sber returns functionally valid WAV; our reader is too strict.

---

## Session 5 — Hotfix PR #13 (Streaming WAV normalization)

### Goal

Fix `audio::wav_join` to handle SaluteSpeech streaming WAVs without breaking existing valid WAV handling.

### Pre-implementation — 3 architectural Q&A

| Q | Decision |
|---|---|
| Q1 — Implementation strategy | **Pre-process bytes before hound** (rewrite size fields), not replace hound. Minimal change. |
| Q2 — Test fixture | **Synthetic WAV** in test code, not commit real 85 KB binary fixture. |
| Q3 — Detection criteria | **Always normalize** (no detection branching). No-op for valid WAVs, single code path. |
| Coverage scope | RIFF size + data size + skip unknown chunks (LIST/bext/fact/INFO) walking |

### CC pre-implementation — 2 questions

**Q1 from CC (proper chunk walking vs windows(4) scan):**
- Option (a) — `bytes.windows(4)` scan for `b"data"`. Simple, but risk false-positive если в WAV есть LIST chunk с ASCII string содержащей подстроку "data".
- Option (b) — proper RIFF chunk walking from offset 12, reading `(id, size)` pairs, skipping chunks. +15 LOC, correct under all reasonable inputs.

**Решение:** (b) + defensive `chunk_size > bytes.len()` overflow guard.

**Q2 from CC (regression sanity check):**
Предложил comment out normalize call → cargo test → verify exact same error что в Manual QA → restore. Подтверждено как **best practice для всех future bugfix PRs**.

### Implementation + sanity cycle

CC реализовал:
- `pub fn normalize_streaming_wav(&[u8]) -> Result<Vec<u8>, WavJoinError>` (public для testability)
- `fn find_data_chunk_offset(&[u8]) -> Option<usize>` (private RIFF walker)
- `join_wav_chunks` minor diff: pre-pass through normalize + replace `chunks` → `normalized` в 3 местах
- Helper `make_streaming_wav(...)` для DRY fixtures
- 5 new tests

**Regression sanity result:**

```
With normalize commented out:
  test_join_handles_sber_streaming_pattern → FAIL
  panic message: Invalid("chunk 0: Ill-formed WAVE file: data chunk
                 length is not a multiple of sample size")

Manual QA error in chat:
  invalid WAV data: chunk 0: Ill-formed WAVE file: data chunk
  length is not a multiple of sample size

Difference: only Display prefix `invalid WAV data:` from WavJoinError
            #[error("invalid WAV data: {0}")] annotation.
Internal message: BYTE-FOR-BYTE IDENTICAL.

Conclusion: synthetic fixture exactly reproduces Sber pattern.

With normalize restored: all 15 audio::wav_join tests pass.
```

Это **first regression test pattern в проекте** — proper bugfix discipline. Установлен стандарт для future hotfix PRs.

### Quality gates Session 5

| Check | Result |
|---|---|
| `cargo check` | clean |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | **76 passed; 0 failed; 0 ignored** (71 prior + 5 new) |
| Regression sanity cycle | exact error match confirmed |

### Merge + final Manual QA

PR #13 squash-merge как `b372430`. Auto-delete сработало.

Local verification:
```
git pull → 76 passed на main
pnpm tauri dev → окно открывается
Settings → AK всё ещё в keyring → Test → "Ключ работает"
Synthesize "Привет тест" Натальей → Save → glagol.wav
Windows Media Player → проигрывается ✓
```

**Repeat с разными голосами** — Борис, Марфа, Тарас, Александра, Сергей — все проигрываются нормально.

**Sprint 1 = TRULY 100% — runtime verified в production env.**

---

## Manual QA edge cases (post-fix)

После PR #13 merge — дополнительные scenarios:

| Scenario | Result |
|---|---|
| Cancel в Save As dialog | ✅ Silent no-op, progress bar исчезает, никаких toast errors |
| Empty text + click Synthesize | ✅ Button disabled (frontend trim check) |
| 1-character text "А" | ✅ Сохраняет, проигрывается без ошибок |
| 3000+ character text | ✅ Режет на chunks, sequential synthesis, progress bar реально движется по фрагментам, сохраняет, проигрывается полностью |
| **Ctrl+R refresh окна** | ⚠️ **Credentials reset to "invalid", надо вводить AK заново** |

Последний — **regression, не expected behavior**. Hypothesis для investigation в Sprint 2:
- H1: Ctrl+R reload в Tauri не сохраняет CredentialsContext state (frontend re-mounts с `"unknown"` initial)
- H2: Mount-time probe `testCredentials()` после reload по какой-то причине fails despite AK в keyring
- H3: Что-то в Tauri WebView2 reload triggers Rust-side state reset (unlikely — Mutex<Option<Arc<SaluteAuth>>> на main process side)

**Action:** GitHub Issue opened (separate from Sprint 1 closure scope). Workaround for users: не делать Ctrl+R после Settings save (acceptable for MVP).

---

## Sprint 1 — final state на main

### Module structure

```
src-tauri/
├── assets/
│   └── russiantrustedca.pem            (Day 2 — embedded НУЦ root CA)
├── src/
│   ├── lib.rs                          (run() with state mgmt + 5 commands)
│   ├── main.rs                         (unchanged from scaffold)
│   ├── state.rs                        (AppState — Session 3)
│   ├── salute/                         (Days 2-3, extended Day 4 Session 3)
│   │   ├── mod.rs
│   │   ├── errors.rs                   (SaluteError — 8 variants)
│   │   ├── http.rs                     (TLS-pinned client + RqUID)
│   │   ├── auth.rs                     (OAuth + caching + invalidate)
│   │   └── synthesize.rs               (sync API + VoiceId + FromStr)
│   ├── text/                           (Day 3 Session 2)
│   │   ├── mod.rs
│   │   └── chunker.rs                  (504 LOC, all edge cases)
│   ├── audio/                          (Day 4 Sessions 1 + 5)
│   │   ├── mod.rs
│   │   └── wav_join.rs                 (with normalize_streaming_wav)
│   ├── secrets/                        (Day 4 Session 2)
│   │   ├── mod.rs
│   │   └── keyring.rs                  (Wincred via keyring-rs 3.x)
│   └── commands/                       (Day 4 Session 3)
│       ├── mod.rs
│       ├── credentials.rs              (3 commands + 6 tests)
│       └── synthesize.rs               (2 commands + ProgressEvent + 6 tests)
├── capabilities/
│   └── default.json                    (extended with dialog permissions)
├── tauri.conf.json                     (CSP + devCsp, Sber-only connect-src)
└── Cargo.toml                          (+hound, +keyring, +tauri-plugin-dialog)

src/
├── App.tsx                             (Router + Routes)
├── main.tsx                            (CredentialsProvider outside BrowserRouter)
├── index.css                           (Tailwind v4 + shadcn theme tokens)
├── components/
│   ├── layout/
│   │   └── AppShell.tsx                (sidebar + NavLink + Outlet + Toaster)
│   └── ui/                             (9 shadcn primitives)
│       ├── button.tsx, input.tsx, label.tsx
│       ├── textarea.tsx, select.tsx, progress.tsx
│       ├── card.tsx, separator.tsx, sonner.tsx
├── contexts/
│   └── CredentialsContext.tsx          (tri-state + mount probe)
├── lib/
│   ├── tauri.ts                        (5 typed wrappers + ProgressEvent type)
│   ├── voices.ts                       (VOICES const, 6 голосов)
│   └── utils.ts                        (shadcn cn() helper)
└── pages/
    ├── Settings.tsx                    (AK form + 3 handlers + status label)
    ├── Synthesize.tsx                  (3 render branches + full pipeline)
    └── Library.tsx                     (Coming Soon Card)
```

### Test coverage final

| Module | Tests |
|---|---|
| `audio::wav_join` | 15 (+5 streaming WAV) |
| `salute::http` | 3 |
| `salute::auth` | 9 (+1 invalidate) |
| `salute::synthesize` | 12 (+3 VoiceId parsing) |
| `text::chunker` | 18 |
| `secrets::keyring` | 7 |
| `commands::credentials` | 6 |
| `commands::synthesize` | 6 |
| **Total** | **76 passed, 0 failed, 0 ignored** |

### Frontend bundle

- 393 KB JS / 126 KB gzipped (1894 modules)
- 38 KB CSS (shadcn theme + Tailwind utilities)
- 76 KB woff2 fonts (Geist Variable, 5 weights with Cyrillic)
- Total install: ~2.3 MB

---

## Lessons learned — Day 4 (cumulative)

### Технические

1. **Streaming WAVs — known gap в hound.** Producers like SaluteSpeech emit `0xFFFFFFxx` size markers в RIFF и data chunks потому что не знают final size when streaming. Strict readers reject. Pre-processing с rewrite real sizes — clean fix.

2. **Tauri 2 binary IPC — `tauri::ipc::Response`.** Для `Vec<u8>` returns it's mandatory. JSON array of numbers serialization кроет 4x overhead. `Response::new(bytes)` skips serde entirely, frontend получает ArrayBuffer.

3. **`tokio::sync::Mutex` vs `std::sync::Mutex` для async commands.** Stdlib mutex blocks async runtime. Use tokio mutex everywhere в `#[tauri::command] async fn`.

4. **React StrictMode double-mount race.** React 19 dev re-mounts components dvazhdy. Without cancellation flag в useEffect, setState calls race. Pattern: `let cancelled = false; ...; return () => { cancelled = true; };`.

5. **Tailwind v4 — CSS-first config.** No `tailwind.config.ts`. Use `@import "tailwindcss"` + `@tailwindcss/vite` plugin. Theme tokens в CSS variables. Different paradigm от v3 but works.

6. **shadcn `init` non-interactive flags evolved.** `--base-color slate` → `--preset nova` etc. Always check `--help` before scripting.

7. **keyring 3.x mock state per-Entry, не global.** Tests должны держать single Entry per test. Different paradigm от 2.x where mock was shared HashMap.

8. **`.gitignore` patterns без leading slash matter широко.** `secrets/` (без `/`) matches anywhere — case-study from hotfix #9. Explicit `!path/` + `!path/**` patterns для un-ignore.

9. **PowerShell 5.x не поддерживает `&&`.** Use `;` separator или PS 7+. Common Windows surprise for bash habits.

10. **PowerShell `Invoke-RestMethod` нельзя trust для binary.** Text-mode decoding обрезает binary streams. Use `[System.Net.HttpWebRequest]` + `GetResponseStream().CopyTo()` для raw bytes.

### Процессные

1. **Never claim "Sprint X = 100%" before runtime verification.** Code complete ≠ runtime verified. Manual QA на target platform = hard gate. Я нарушил это правило после PR #12 merge — correction came через bug discovery в Сценарии 3.

2. **Negative regression tests для bugfixes.** Comment out fix → verify test fails с **exact same error** что в production → restore → tests pass. Proves fix targets real bug, не hopeful patch. **Установлен standard для всех future hotfix PRs.**

3. **Forensic data в bug investigations.** Hex dumps, byte counts, comparison к external tools (WMP) — concrete evidence beats hypothesis. PR #13 PR body содержит full hex table — будущие contributors могут понять issue без перечитывания всего chat history.

4. **Pre-implementation Q&A pays off cumulatively.** Sessions 3, 4, 5 каждая имела Q&A round до старта CC. Total saved time на debugging — multiple hours. Pattern становится reflexive — не нужно напоминать.

5. **CC's questions often improve specs.** Day 4 had 3 cases where CC questions led to better decisions (binary IPC tweak, tri-state Context, RIFF chunk walking vs windows(4)). AI agent с good judgment > pure code monkey.

6. **Split monolithic PRs для security-critical paths.** PR #6 → 6a (keyring security в чате) + 6b (UI через CC). Pattern continued для review surface management.

7. **Cloud CC self-destructs containers, auto-deletes branches.** Settings → Pull Requests → «Automatically delete head branches» = zero manual cleanup. Должно быть default для open source projects with AI contributors.

8. **`pnpm approve-builds` per-machine state.** msw: false как personal preference — discard local change, не commit. Real "project-level" approval policy надо decide separately в Sprint 5 CI setup.

9. **Direct commits на main banned.** PR #9 hotfix через PR, не direct commit. Pattern reinforced — даже single-line emergencies через PR + squash-merge.

10. **Master log splits OK для long days.** Day 4 master log Part 1 (Sessions 1-2) + Part 2 (this file, Sessions 3-5) — keeps each file readable. Standard precedent для future long days.

### Про итеративную работу с CC cloud

11. **Phase-based progress reports work.** CC reports после Phases 1/2/3/4 вместо одного финального dump. Catches issues early (Tailwind v4 vs v3 noticed at Phase 1, not at PR creation).

12. **Stop before PR creation rule.** Established pattern: paste-back перед `mcp__github__create_pull_request`. Окно для code review. Two cases где review caught issues before PR was created.

13. **Cloud container survives multiple sessions если active.** CC сегодня survived from Session 3 through Session 5 (~6 часов wall clock) с warm context. **Single warmup pays off many times.**

14. **Cloud CC vs local CC tradeoffs.** Cloud — no WebView2 → no Tauri dev smoke tests possible. Trade-off: Rust + frontend type-safety + cargo test as gates, runtime verification deferred to user's local Windows. Acceptable since Manual QA matrix exists.

---

## Sprint 1 retrospective stats

| Metric | Value |
|---|---|
| Calendar days | 5 (Day 0 → Day 4) |
| Actual work sessions | 9 CC sessions across 4 days |
| PRs merged | 10 (PR #2-#4, #6-#13, excluding #5 abandoned) |
| Hotfixes | 2 (PR #9 gitignore + PR #13 streaming WAV) |
| Total LOC added | ~2200 Rust + ~600 TypeScript |
| Unit tests | 76 (from 0) |
| Backend modules | 7 (`salute`, `text`, `audio`, `secrets`, `state`, `commands::credentials`, `commands::synthesize`) |
| Frontend pages | 3 (Settings, Synthesize, Library mock) |
| Dependencies added | 18 Rust + 13 TS production + 3 TS dev |
| Final bundle size | ~2.3 MB frontend assets, Rust binary ~12 MB debug (release pending Sprint 5) |
| Original roadmap estimate | Sprint 1 = 5-6 calendar days |
| Actual time | 5 calendar days **including** discovery + bug + fix + verification |

### Velocity analysis

Sprint 1 finished **on schedule** despite catching a critical regression mid-closure. Without the bug discovery, we'd have called Sprint 1 done at end of Session 4 — but that would have been **false completion**. Manual QA caught the real state.

**The hotfix PR #13 itself took 60 minutes** end-to-end (kickoff → spec → implementation → review → merge) — testament to:
- Established workflow (kickoff → Q&A → phases → paste-back → review → merge)
- Forensic diagnostic before coding (hex dump confirmed root cause before any patches)
- CC's improving pattern recognition (Q&A questions caught early, sanity cycle proposed without prompting)

---

## What we built — capability summary

A user can now:

1. Install Glagol (build locally — release artifacts deferred to Sprint 5)
2. Open application, see clean Russian UI
3. Configure SaluteSpeech Authorization Key once
4. Paste any Russian text (1 char to ~100K chars practical, 200K monthly free tier limit)
5. Choose one of 6 native voices (Наталья / Борис / Марфа / Тарас / Александра / Сергей)
6. Click one button, watch progress bar
7. Save the resulting WAV file anywhere
8. Play it in any media player

**This is the MVP from the original compass artifact.** Glagol is now a working product, не proof of concept.

---

## Next — Sprint 2 (Storage + Library UI)

After Day 4 closure + solid pause:

| Component | Estimated |
|---|---|
| `tauri-plugin-sql` + SQLite migrations | 1 session |
| `db/repository.rs` (documents + chunks CRUD) | 1 session |
| `%LOCALAPPDATA%\Glagol\audio_cache\` infrastructure | shared with above |
| React Library page (real content, replay, delete) | 1 session |
| Refactor Synthesize page для saving metadata + WAV в cache | 0.5 session |
| Ctrl+R credentials reset bug investigation | 0.5 session (carry from Day 4 finding) |

**Estimated:** 3-4 CC sessions, ~Day 5-8 на твоём календаре.

Дальше — Sprint 3 (Parsers: TXT/MD/DOCX/PDF), Sprint 4 (Player + Cache + parallel synthesis), Sprint 5 (CI + Polish + first public release).

---

## Reference links

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #11 (Tauri commands backend, merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/11
- **PR #12 (Frontend + binary IPC, merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/12
- **PR #13 (Streaming WAV hotfix, merged):** https://github.com/dimasiksuleyman-sudo/glagol/pull/13
- **Day 4 master log Part 1:** `docs/day-logs/day-4-master-log.md`
- **Sprint 1 = TRULY 100% closure date:** May 17, 2026
- **Main HEAD at closure:** `b372430`

---

*Day 4 Part 2 captures Sessions 3-5 + Manual QA discovery + critical hotfix + final runtime verification.*
*Sprint 1 closure achieved. Ready for solid pause before Sprint 2.*
*Last updated: 2026-05-17*
