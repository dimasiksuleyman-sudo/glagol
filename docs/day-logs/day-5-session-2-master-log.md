# Glagol — Day 5 Session 2 Master Log

**Period:** May 18, 2026 (Sprint 2 sessions 2-4, single calendar day continuation from Session 1)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 2 (Storage + Library) — **CLOSURE**
**Status at end of Session 4:** Sprint 2 **100% complete**. Persistent library + full lifecycle verified runtime. Tag `v0.1.0-rc.1` pushed.

> Session 1 (`day-5-session-1-master-log.md`) covers Sprint 2 entry + PR #15 logical (GH #17 — DB foundation). This file picks up at Session 2 and runs through closure.

---

## TL;DR

За один calendar day (May 18) после Session 1 закрытия (PR #15 logical, DB foundation) — закрыли **весь оставшийся Sprint 2 functional scope**: persistent synthesis flow (PR #16 logical → GH #18), real Library page с playback/delete/export (PR #17 logical → GH #19), и Sprint 1 leftover bug fix через cache-first credentials testing (PR #18 logical → GH #20). К концу Session 4 — tag `v0.1.0-rc.1` pushed, Sprint 2 OFFICIALLY CLOSED.

**Day 5 spans 4 CC sessions total** (Session 1 covered separately). Sessions 2-4 = **single sustained day**, ~10 chat hours, four merged PRs + Sprint 2 closure tag.

Test count progression Sessions 2-4: 89 → 95 (PR #16) → 100 (PR #17) → 103 (PR #18). Sprint 1 closure was 76 tests; Sprint 2 added **+27 tests** with **0 regressions**.

After Session 4 — Glagol transitioned from «one-shot text-to-WAV tool» to «personal audiobook library с автоматическим preservation». User has full lifecycle: synthesize → automatic library save → listen anytime via asset protocol → manage (delete/export) → all entirely in-app.

---

## Sprint 2 Functionality Snapshot — what works end-to-end after closure

**Cold boot:**
1. Launch Glagol — opens at last route (or Settings if first time)
2. `glagol.db` exists в `%LOCALAPPDATA%\app.glagol.desktop\` (auto-created via setup hook + migration runner from PR #15)
3. `audio_cache/` directory exists в same parent (auto-created в setup hook from PR #16)

**Configure once:**
4. Settings → paste SaluteSpeech Authorization Key → click «Сохранить» → keyring stores
5. Click «Проверить» → live OAuth handshake against Sberbank → status «подтверждён Сбером»
6. Status survives Ctrl+R refresh (PR #18 logical fix)

**Synthesize:**
7. Onto Synthesize page → paste text → select voice → click «Озвучить и сохранить в библиотеку»
8. Progress events fire (chunker → sequential synthesis per chunk → wav_join)
9. After success: toast «Сохранено в библиотеку» with action button «Открыть библиотеку»
10. Atomic persistence: rusqlite transaction (INSERT row → fs::write WAV → commit)
11. Secondary «Сохранить на диск» button appears below textarea для optional disk export

**Listen and manage:**
12. Open Library page (via toast action or nav menu)
13. See stacked list of all rows, newest first (ORDER BY created_at DESC via idx_docs_created)
14. Each row: title (first 60 chars heuristic) · voice name · char count · relative time
15. Native HTML5 `<audio>` player с asset protocol streaming (no full-file IPC roundtrip)
16. Native controls: play/pause, timeline, volume, playback rate (через 3-dot menu)
17. Download icon → file copy via `export_audio` command
18. Trash icon → instant delete (row + cached file gone atomically)
19. Empty state с illustration + CTA when last row deleted

This is **the full Sprint 2 deliverable.** Real product, не demonstration.

---

## Session 2 — PR #16 logical / GitHub #18 (Persistence refactor)

### Pre-implementation architectural Q&A (4 questions + 1 sub-decision)

Sprint 1 pattern preserved: Q&A first, kickoff after, no design pivots mid-implementation.

| Q | Topic | Decision | Why |
|---|---|---|---|
| Q1 | Response contract | **Split commands.** `synthesize_document → document_id` (String), audio bytes never leave Rust. New `get_audio_path` + `export_audio`. **`write_wav_file` удаляется.** | IPC overhead reduction (50K-char doc = 21 MB через IPC → 0 bytes); Sprint 2 mission alignment (cache = canonical, disk = derived); cancel safety; PR #17 reusability |
| Q2 | Partial failure semantics | **Transaction-wrapped persistence.** rusqlite Transaction opens → INSERT → fs::write → commit. Drop semantics auto-rollback on fs::write failure | Atomicity primitive; orphan rows >> orphan files (rows are user-visible breakage; orphan files invisible); minimal LOC; Sprint 4 status='synthesizing' foundation |
| Q3 | UUID + created_at ownership | **Command layer generates.** `uuid::Uuid::new_v4()` + `chrono::Utc::now().timestamp_millis()` в command | Sprint 1 paradigm fit (commands = integration points); single source of truth; frontend simplicity; repository stays pure CRUD without time/UUID mocking burden |
| Q4 | Test coverage for filesystem failures | **Trust SQLite primitive.** Unit tests cover orchestration, skip fs::write failure mocking. Manual QA matrix covers integration | Don't test third-party primitives; cost/benefit (trait abstraction = +40 LOC for rare edge case); Sprint 1 precedent (write_wav_file не мокался) |

**Pre-answered Q&A** для CC (8 likely questions) explicit в kickoff: Mutex<Connection> + transaction shape + AppHandle injection + temp dir testing + SynthesisError enum extension + duration_ms decision + audio_cache dir timing + Link from toast.

**Web search verification** для `tauri = features = ["protocol-asset"]` deferred — это discovered runtime by CC (build script requires Cargo feature parity to runtime config). Forward learning для Session 3 kickoff.

### Implementation — 3 phases (mandated phase-by-phase reporting)

Sprint 2 pattern shift from Session 1: **phase-by-phase reports required** because PR #16 trogает Sprint 1 production code path (existing `commands::synthesize` + Synthesize page).

**Phase 1 — Backend additive:** new `commands/storage.rs` skeleton (just `get_audio_path` + `export_audio` shells), setup hook creates `audio_cache_root`, invoke_handler registration. Wait for chat ack before breaking changes в Phase 2.

**Phase 2 — Backend breaking:** `commands::synthesize` full refactor. CC сделал substantial implementation choices:

1. **Extracted `persist_synthesis_result` helper** instead of testing through mockito. Rationale (CC-side): "the auth/synthesis HTTP path is already covered by salute::auth and salute::synthesize module tests using mockito с with_base_url; injecting mocks would require adding URL parameters to the impl signature — that adds noise to production code purely for test reachability." **Chat endorsement:** this was actually **better than spec**. My kickoff prescribed mockito-based testing at command level, but Sprint 1's existing 6 commands::synthesize tests были all early-validation paths (empty text rejected, whitespace rejected, etc.) — none actually reached HTTP layer. So spec'овский "6 existing mockito tests adapt" framing был incorrect. CC's extraction correctly identified the cleaner pattern: test what's our risk surface (persist_synthesis_result — naked new logic), trust what's third-party primitive (transaction rollback), mock what's HTTP в его dedicated module.

2. **Critical section discipline.** Synchronous block: `db.lock() → tx.begin → repo::insert → fs::write → tx.commit → drop guard`. `!Send` MutexGuard makes any future async escape impossible at compile time. Documented в commit message as load-bearing invariant.

3. **No `SynthesisError::Persistence` enum variant** — kept inline `.map_err(|e| e.to_string())?` pattern consistent с existing module. Spec called both approaches acceptable.

4. **`audio_duration_ms` stays NULL** — defer to Sprint 4 or PR #17 if simple. CC didn't add (kept persistence diff focused).

**Phase 3 — Frontend update:** `tauri.ts` signature changes (`Promise<string>` instead of `Promise<Uint8Array>`); `Synthesize.tsx` refactor для library-first flow + option β (persistent secondary «Сохранить на диск» button below textarea, lastDocumentId state cleared at start of each new synthesis но не on textarea/voice change).

### Quality gates Session 2 PR

| Check | Result |
|---|---|
| `cargo check` | clean |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | **95 passed; 0 failed; 0 ignored** (89 baseline + 5 storage + 3 persistence + −2 write_wav_file = +6 net, target was «~96», 1 below) |
| `pnpm tsc --noEmit` | clean |
| `pnpm build` | succeeded; **393.12 KB JS** — identical to Sprint 1 baseline (no transitive bloat from 2 added wrappers − 1 deleted) |

### Test count delta breakdown

```
Sprint 1 baseline (post PR #15):         89
+ commands::storage (Phase 1):           +5
+ commands::synthesize (Phase 2):        +3 persistence orchestration
- commands::synthesize (Phase 2):        -2 write_wav_file removed
                                         ===
Final:                                   95
```

95 vs spec target «~96»: within tolerance, explained in paste-back. No 4th persistence test (`title truncation invariant`) added — optional value-add, не блокер. Reserved для future если bug на кириллице surfaces.

### Merge + runtime verification

PR title intentionally без literal `#N`: `feat: persist synthesis results to library; split disk export from in-memory return`. GitHub assigned **#18** (logical PR #16 in roadmap). Three phase-commits squash-merged в `17de979` on main.

Runtime verification (5 steps protocol from Session 1, adapted):

| # | Step | Result |
|---|---|---|
| 1 | `git pull` | Fast-forward, 6 files changed, +503 insertions |
| 2 | `cargo test` | **95 / 95**, 0.19s — `:memory:` test discipline preserved |
| 3 | `pnpm tsc --noEmit` | clean |
| 4 | `pnpm tauri dev` | 52 sec compile (new deps + bundled SQLite через libsqlite3-sys + protocol-asset feature); app окно открылось без panics; Settings/Synthesize work как Sprint 1; end-to-end synthesis flow passed (новый toast с action button «Открыть библиотеку») |
| 5 | Disk + DB verification | `glagol.db` row inserted (16 KB → grew с metadata for synthesis); `audio_cache/a8f6f39d-20b4-4196-82e3-38594b8a6ce4.wav` created (2,168,556 bytes ≈ 45 sec audio для 653-char text @ ~14 chars/sec); row schema correct (status='ready', source_type='paste', voice='Nec_24000', char_count=653, audio_path='a8f6f39d-...wav', created_at=1779095678397 ms) |

DB Browser for SQLite installed (recommendation pattern для future Sprints). Schema verified visually: 10 columns в правильном порядке, 1 index (`idx_docs_created` on `created_at DESC`), no views, no triggers. Matches PR #15 migration v1 exactly.

**Important observation re-confirmed:** Tauri 2 `app_local_data_dir()` resolves через `bundle.identifier` (`app.glagol.desktop`) on dev builds, не через `productName` (`Glagol`). Известный Session 1 finding remains true — Sprint 5 MSI installer signed release будет productName-based. Workaround = none needed; defer to Sprint 5.

---

## Session 3 — PR #17 logical / GitHub #19 (Library page real content)

### Pre-implementation architectural Q&A (6 questions + sub-decisions)

Major architectural session — Library page UI/UX surface most significant since Sprint 1's Synthesize page.

Verified deps freshness first через web search:
- **Tauri 2 asset protocol Windows scheme:** `http://asset.localhost`, NOT `https://asset.localhost`. This was breaking change в Tauri 2 (verified via GitHub commit ref) — my mental cache from Tauri 1 era was incorrect. Stale memory recoverable через verification step.
- `convertFileSrc` location: `@tauri-apps/api/core` (not `@tauri-apps/api/tauri` from v1).
- `assetProtocol` config shape: `app.security.assetProtocol = { enable, scope, requireLiteralLeadingDot? }`.
- `requireLiteralLeadingDot` Windows default = `false` — our `{uuid}.wav` paths не имеют leading dots, glob `**` matches normally.

| Q | Topic | Decision | Why |
|---|---|---|---|
| Q1 | Layout | **Stacked list** (full-width rows, not Card grid, not Table) | Podcast/audiobook playback queue mental model; density balance; long Russian titles naturally wrap; native audio player inline без UX gymnastics; Implementation simplicity through shadcn Card-as-list-item |
| Q1.5 | Inline title editing | **Deferred Sprint 5 polish.** User feedback: "это даст возможность обозвать файл используя концепцию всего файла. Например — генеральная доверенность" | Sprint 5 = polish + first public release; user-facing UX improvement worth dedicated PR with editing semantics (Enter save, Esc cancel, validation, etc.); separates concerns между functional baseline (Sprint 2) и daily-use polish (Sprint 5) |
| Q2 | Audio player | **Native HTML5 `<audio controls>`** with `controlsList="nodownload"` + `preload="none"` | Sprint 2 scope discipline (PR #17 already имеет 4 functional surfaces); native works correctly out of box (streaming, range requests, keyboard shortcuts, playback rate); Sprint 4 custom controls becomes natural additive upgrade for resume/queue features; user agrees "MVP самое то, B на данный момент избыточно" |
| Q3 | Asset protocol scope | **Sprint 2: static config `["$APPLOCALDATA/audio_cache/**"]`.** Sprint 5 (when configurable library location lands): **dynamic Rust scope API** через `asset_resolver().scope().allow_directory()`, NOT wildcard `["**"]` | Defense in depth even for trivial threat (XSS pathway extremely narrow in our strict CSP context, но scoping costs zero); Tauri's recommendation pattern; consistent с Sprint 5 plan for configurable paths |
| Q4 | Delete UX | **A: instant delete** (no confirm, no undo) для MVP. **Sprint 5 conditional:** if real users report misclicks → upgrade to undo-toast pattern (frontend-only soft delete, ~60 LOC, no schema changes) | User feedback: "я никогда бездумно не тыкаю на корзину. Если юзеры будут жаловаться потом переделаем." — defer based on actual usage signal, not speculation; signal-driven approach to polish |
| Q5 | Refresh strategy | **Mount-time fetch only.** React Router unmount/remount on navigation = built-in refresh. No focus listener, no event subscription | Glagol = single-window desktop app, не multi-tab browser; no external mutation source; Sprint 4 will add event-based updates когда background synthesis lands |
| Q6 | Edge states | Discriminated union pattern: loading → 3 Skeleton rows; empty → Card с icon + text + CTA Link to /synthesize; fetch error → Card + retry button; per-row playback error → trust native HTML5 audio; per-row delete error → toast + row stays | Each state has visual identity без flickering; CTA gives direction in pristine empty state; retry button gives agency on transient errors; Trust native primitive over custom error interception |

Web search verification consistent с Sprint 1 paradigm — paid off (Tauri 2 http:// scheme caught early before kickoff prescribed wrong CSP).

### Implementation — 2 phases (CC required to report after each)

**Phase 1 — Backend + config:**
- `commands/storage.rs` extended с `list_documents` + `delete_document` (and their `*_impl` helpers)
- `lib.rs` `invoke_handler!` registers both
- `tauri.conf.json` updated с CSP `media-src 'self' asset: http://asset.localhost` + `assetProtocol.scope = ["$APPLOCALDATA/audio_cache/**"]`
- **Deviation discovered runtime:** `tauri = { features = ["protocol-asset"] }` required в `Cargo.toml`. Tauri's build script enforces consistency between runtime config (assetProtocol enabled) и compile features (protocol-asset). My kickoff missed this — gap in spec, CC caught it.

**Phase 2 — Frontend:**
CC made multiple implementation choices сверх spec, all defensible:

1. **`<Header />` preserved across all states** instead of returning different layouts per state in render. Prevents page-identity flicker between loading/empty/ready/error transitions. UX improvement.

2. **Aria-labels on icon-only Download + Trash buttons** alongside `title` attributes. Accessibility hygiene above spec ask.

3. **Cancellation flag in `DocumentRow` useEffect cleanup** для unmount-mid-IPC scenarios (e.g. row deleted while `getAudioPath` still resolving). Matches Sprint 1's `CredentialsContext` pattern.

4. **`audio_path === null` forward compat:** `DocumentRow` skips both IPC call и `<audio>` render when `audio_path === null`. Not triggered today (Sprint 2 rows always have audio), но free Sprint 4 readiness for `status='error'` rows.

5. **Default export filename sanitisation:** `doc.title.replace(/[\\/:*?"<>|]/g, "_").trim().slice(0, 80) || "glagol"`. Strips Windows-reserved chars, caps length, falls back to "glagol" if empty after sanitisation.

6. **`<audio key={document.id}>`** — defensive remount on row identity change so previous playback can't bleed into different src after delete shifts list.

7. **Local Skeleton component** at `src/components/ui/skeleton.tsx` instead of `pnpm dlx shadcn@latest add skeleton`. Identical class set к upstream shadcn template; avoids network-dependent install step at build time. Pragmatic.

### New files и dependencies

```
src/lib/format.ts                 — formatRelativeTime + pluralizeRu (RU)
                                    ~50 LOC, no external dep (no date-fns,
                                    no dayjs — bundle stays lean)
src/components/ui/skeleton.tsx    — minimal shadcn-style Skeleton
src-tauri/src/commands/storage.rs — list_documents + delete_document
                                    (extension of existing PR #16 file)
```

No new direct deps. **Bundle delta: +4.63 KB JS** (3 new lucide icons + Skeleton + format.ts + expanded Library.tsx). Well within tolerance.

### Test count delta breakdown

```
Sprint 2 baseline (post PR #16):       95
+ commands::storage (Phase 1):         +5  (1 list_documents + 4 delete_document)
                                       ===
Final:                                100
```

5 tests targeting `*_impl` functions:

- `list_documents_impl_returns_rows_ordered_by_created_at_desc` — insert 3 rows in random order, verify newest-first ordering, full 10-field round-trip preserved
- `delete_document_impl_removes_row_and_file` — happy path
- `delete_document_impl_returns_error_for_unknown_id` — empty DB, expects "document not found"
- `delete_document_impl_succeeds_when_file_already_missing` — row exists but no file on disk; verifies best-effort fs::remove_file semantics
- `delete_document_impl_releases_lock_before_returning` — uses `db.try_lock().is_ok()` after impl returns to prove guard dropped

Last test pattern — elegant двойной invariant catch (lock released + no early-return-while-holding-lock) в single assertion. Worth noting for future Mutex discipline testing.

### Quality gates Session 3 PR

| Check | Result |
|---|---|
| `cargo check` | clean |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | **100 passed; 0 failed** (95 baseline + 5 new, zero deletions, zero adaptations) |
| `pnpm tsc --noEmit` | clean |
| `pnpm build` | succeeded; **397.75 KB JS** (+4.63 KB / +1.29 KB gzip от Sprint 1 baseline) |

### Lock discipline implementation pattern (worth recording)

CC chose **block-scoped Mutex guard** instead of explicit `drop(conn)` in `delete_document_impl`:

```rust
let relative_audio = {
    let conn = db.lock().expect("db mutex poisoned");
    let record = db::repository::get(&conn, document_id)?...;
    db::repository::delete(&conn, document_id)?;
    record.audio_path
    // Guard Drop fires here, before any fs op below.
};
if let Some(rel) = relative_audio { let _ = fs::remove_file(audio_root.join(rel)); }
```

**Why better than `drop(conn);`:**
1. Block scope = compiler-enforced ordering. Future refactor cannot accidentally inline `fs::remove_file` before lock release без visible structural change.
2. Single expression returns only the data needed после lock release (audio_path Option). Cleaner data flow.
3. Reads naturally as business logic: "inside the lock, figure out what to delete — outside the lock, delete it." Mirror's mental model.

Same pattern was also applied in PR #16 logical `persist_synthesis_result` (CC consistent across both modules). Worth establishing as module-wide discipline reference.

### Merge + extensive runtime verification

PR title: `feat: real library page with playback, delete, and export`. GitHub assigned **#19** (logical PR #17).

Two phase-commits squash-merged в new main HEAD. Manual QA was the **most extensive** of Sprint 2 — 10 шагов covering all PR features:

| # | Step | Result |
|---|---|---|
| 1 | Library page показывает 1 row from Session 2 testing | ✅ Card renders с title (truncated), subtitle (voice · char count · relative time), native audio bar, Download + Trash icons top-right |
| 2 | Native audio player с asset protocol playback | ✅ **First runtime confirmation** — WAV streams from `%LOCALAPPDATA%\app.glagol.desktop\audio_cache\` через `http://asset.localhost/...` без CSP violations, без 403, без range request errors. Asset protocol scope correctly limited |
| 3 | Native playback rate menu accessible | ✅ Через 3-dot menu — `Обычный`, `1.25`, `1.5`, `1.75`, `2` доступны. Russian-localized values (WebView2 default behavior) |
| 4 | Native download button hidden | ✅ `controlsList="nodownload"` works — Edge download menu item не появляется |
| 5 | Export to disk via export_audio command | ✅ Click Download → native Save dialog с pre-filled `отово. Оба файла можно скачать выш.wav` (sanitised) → file copied; source cached file untouched (idempotent export через `fs::copy`) |
| 6 | Instant delete (no confirm) | ✅ Single click Trash → row vanishes immediately, no confirm dialog, no undo toast (Q4 decision A confirmed runtime) |
| 7 | Atomic delete (UI + disk + DB sync) | ✅ All three layers synced: row gone from Library list (optimistic state update), file gone from `audio_cache/`, row gone from `documents` table (verified via DB Browser refresh) |
| 8 | Empty state Card after delete-all | ✅ AudioLines icon + "Здесь будут ваши озвученные документы" + Button «Озвучить первый документ» renders correctly |
| 9 | Empty state CTA → /synthesize navigation | ✅ Click button → React Router navigate to /synthesize; first synthesis after empty state → row appears на top |
| 10 | Multi-row newest-first ordering | ✅ Synthesize two short texts in sequence; both rows appear с correct chronological order (newest top); relative times: «только что» для нового, «1 минуту назад» для предыдущего |

**Bonus:** SaluteSpeech smart prosody observation — Сбер's API произносит "1.1." (с trailing dot) как "1.1" without the second dot, suggesting some server-side AI prosody analysis. URL/email pronunciation incorrect (known Sprint 3 work — `text::preprocessor` module).

**Multi-chunk single-row invariant verified runtime** (5000+ char text → multiple chunks → joined → single row в Library) — Sprint 2's most important data integrity invariant from PR #16's transaction-wrapped persistence.

---

## Session 4 — PR #18 logical / GitHub #20 (Ctrl+R credentials fix + Sprint 2 closure)

### Investigation-driven, not feature-driven

Sprint 1 closure (Day 4) flagged Ctrl+R bug в Manual QA edge case section. Original hypotheses:
- **H1:** Frontend re-mounts с `"unknown"` initial state
- **H2:** Mount-time probe `testCredentials()` fails despite keyring valid
- **H3:** WebView2 reload resets Rust-side state (tagged unlikely)

Workaround documented: don't Ctrl+R after Settings save. Acceptable for MVP.

### Reproduction in Sprint 2 state

Session 4 chat reproduced bug runtime после PR #19 merge:
- **Before Ctrl+R:** Settings page «Текущий статус: подтверждён Сбером»
- **After Ctrl+R:** «Текущий статус: не настроен или не работает»
- DevTools Console: clean (no errors)
- Keyring: still intact

**Hypothesis triangulation:**
- ❌ H1 partially wrong — после Ctrl+R state = `"invalid"`, not `"unknown"`. Mount probe completes, just returns error.
- ✅ H2 confirmed — probe fails despite keyring valid.
- ❌ H3 wrong — Rust state not reset on WebView reload (process unchanged).

User question — "зачем кому-то в настольном приложении нажимать Ctrl плюс R?":

Valid и важный вопрос. Steelmanned both sides. Arguments against fixing: Tauri 2 disables Ctrl+R in release builds by default; workaround trivial; low impact. Arguments for fixing: investigation valuable regardless (10 min effort); bug surface broader than Ctrl+R (any code path that re-mounts CredentialsContext); dev experience improvement (hot reload friction); Sprint 2 closure tag (`v0.1.0-rc.1`) implies polish.

**User chose A:** investigate now, decide based on findings. "Если нам это понадобится то конечно выбираю а. плюс это будет опытом для нас что вызывало эту ошибку."

### Root cause analysis на actual source files

User dumped `commands/credentials.rs` + `CredentialsContext.tsx` в chat. Analysis exposed:

**Sprint 1's `test_credentials_impl` conflated two operations:**

```rust
pub(crate) async fn test_credentials_impl(state: &AppState) -> Result<(), String> {
    let auth_key = keyring::get_auth_key()?;        // (A) cheap: read keyring
    let auth = Arc::new(SaluteAuth::new(...));
    auth.get_token().await?;                        // (B) expensive: real OAuth HTTP
    let mut guard = state.salute_auth.lock().await;
    *guard = Some(auth);
    Ok(())
}
```

**Mount-time probe в CredentialsContext** fires `testCredentials()` on every mount (cold boot OR Ctrl+R). Command always hits Sberbank. Any transient error → `.catch` → `setState("invalid")`.

**Why "valid" preserves on cold boot but not Ctrl+R:** timing. On cold boot, first mount happens after Vite/HTML/JS load (~100-200ms after Vite serves) — Sberbank ready. On Ctrl+R, **WebView reload faster** because cached code paths. React mount happens almost immediately после refresh — possibly while previous SaluteAuth's HTTP connection still being torn down. Concurrent OAuth call competes for socket resources, somebody errors out.

**The deep bug:** "test_credentials does expensive Sberbank OAuth on every call, including auto-mount probe, and any failure maps to invalid." The design itself is fragile independent of Ctrl+R: any network blip на mount produces user-visible "invalid" state.

### Fix design — cache-first short-circuit с `force` parameter

```rust
pub(crate) async fn test_credentials_impl(state: &AppState, force: bool) -> Result<(), String> {
    if !force {
        let guard = state.salute_auth.lock().await;
        if guard.is_some() {
            return Ok(());  // ← cached auth from this process lifetime
        }
        // Guard drops at end of `if` block
    }

    let auth_key = keyring::get_auth_key()?;
    let auth = Arc::new(SaluteAuth::new(...));
    auth.get_token().await?;
    let mut guard = state.salute_auth.lock().await;
    *guard = Some(auth);
    Ok(())
}
```

**Two-mode contract:**
- `force=false` (mount-time probe / `CredentialsContext`) — trust cache, no Sberbank call
- `force=true` (Settings → Test button) — bypass cache, do fresh OAuth

**Why cache-first instead of splitting commands:** chat investigation considered `has_credentials` (cheap) + `test_credentials` (expensive) split. Rejected — single command с boolean flag = smaller surface, same intent clarity, frontend default (`force=false`) encodes safe behavior for probe path без requiring two API names.

**Why not detect token expiry in cache hit:** `SaluteAuth::get_token()` already handles refresh on demand next time synthesize actually needs token. Mount probe just needs to know "we're in usable state right now," not "key is still authoritative at Sberbank." Anything more re-introduces the very flakiness this PR removes.

### Implementation — single phase (small scope)

Justified single-phase because: ~30 LOC backend + ~5 LOC frontend + 3 new tests + 1 adapted. Smaller surface than PR #15-17.

**Files modified:** 4
- `src-tauri/src/commands/credentials.rs` — fix + tests
- `src/lib/tauri.ts` — `testCredentials(force = false)` signature
- `src/contexts/CredentialsContext.tsx` — `testCredentials(false)` explicit
- `src/pages/Settings.tsx` — Test handler `testCredentials(true)`

**Test count delta:**
```
Sprint 2 baseline (post PR #17):   100
+ new orchestration tests:         +3
+ adapted (no count change):       (0)
                                   ===
Final:                             103
```

New tests:
- `test_credentials_uses_cache_when_force_false_and_auth_cached` — seed cache with placeholder, call с force=false, assert Ok (cache short-circuit fires, placeholder key never reaches real OAuth)
- `test_credentials_skips_cache_when_force_true` — seed cache, call с force=true, assert error «no credentials configured» (proves cache bypassed)
- `test_credentials_full_oauth_path_when_no_cache_and_force_false` — empty cache, call с force=false, assert error «no credentials configured» (proves cache-first only short-circuits when populated)

Adapted: `test_test_credentials_no_keys_returns_error` — now passes `force=true` explicitly.

### Negative regression sanity cycle (Sprint 1 hotfix discipline)

Per Sprint 1 PR #13 streaming WAV pattern — proving new tests catch the bug, not coincidentally pass.

**Step 1.** Cache-first block commented out:
```
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 102 filtered out
thread '…test_credentials_uses_cache_when_force_false_and_auth_cached' panicked
    at src/commands/credentials.rs:262:14:
    force=false with cached auth must succeed via cache: "no credentials configured"
```

Test fails with **exact** error message from assertion. Without cache-first, fallthrough hits keyring path which is empty в mock backend → maps to "no credentials configured" → assertion catches это.

**Step 2.** Block restored:
```
test result: ok. 103 passed; 0 failed
```

Fix is load-bearing. This pattern is the **first** time в Sprint 2 where negative regression sanity preserved в PR body как explicit documentation. Establishes pattern for future bugfix PRs — show, don't tell.

### Quality gates Session 4 PR

| Check | Result |
|---|---|
| `cargo check` | clean |
| `cargo fmt --check` | clean |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cargo test` | **103 passed; 0 failed** (100 baseline + 3 new) |
| `pnpm tsc --noEmit` | clean |
| `pnpm build` | succeeded; **397.77 KB JS** (+0.02 KB rounding noise vs PR #19 baseline) |

### Merge + runtime verification

PR title: `fix(credentials): cache-first test_credentials to survive Ctrl+R refresh`. GitHub assigned **#20** (logical PR #18). Closes issue #15 auto-recognized in body — GitHub sidebar showed «Development → Successfully merging this pull request may close these issues → Ctrl+R refresh in Tauri WebView2 resets CredentialsContext» linkified ref.

Single commit `4c0db0c` squash-merged.

**Ctrl+R bug verification — the moment of truth:**

| # | Step | Result |
|---|---|---|
| 1 | `git pull` + `cargo test` | 103 / 103 |
| 2 | `pnpm tauri dev` | clean start |
| 3 | Settings page baseline | «Текущий статус: подтверждён Сбером» |
| 4 | **Press Ctrl+R** | WebView reloads |
| 5 | **Settings status after reload** | **«Текущий статус: подтверждён Сбером»** (NOT switched to «не настроен») |

✅ **Bug FIXED.** Sprint 1 baseline (or PR #19 state) would show "invalid". Cache-first preserves valid state through asset protocol-style short-circuit.

**Sprint 2 regression check** (post-fix sanity):
- Test button («Проверить») triggers fresh OAuth call — force=true path не сломан
- Library page intact (asset protocol playback works)
- Synthesize page intact (synthesis → toast → row appears)
- Delete works

No regressions от bug fix.

### Issue #15 auto-closed

GitHub `Closes #15` syntax в PR body recognized correctly. After squash-merge — issue #15 transitioned к Closed status automatically. One less open issue в repo.

---

## Sprint 2 Closure — Tag `v0.1.0-rc.1`

After PR #20 merge + runtime verification + regression check passed:

```powershell
cd C:\Projects\glagol
git checkout main
git pull
git tag -a v0.1.0-rc.1 -m "Sprint 2 closure: persistent library, asset protocol playback, end-to-end lifecycle"
git push origin v0.1.0-rc.1
```

Output:
```
* [new tag]         v0.1.0-rc.1 -> v0.1.0-rc.1
```

Tag pushed to origin. Visible на https://github.com/dimasiksuleyman-sudo/glagol/tags. Sprint 2 OFFICIALLY CLOSED.

**Tag semantics:** annotated tag (`-a`) включает timestamp + message, visible via `git show v0.1.0-rc.1`. Permanent commit marker. Naming follows semver pre-release identifiers (`v<major>.<minor>.<patch>-rc.<n>`).

**Roadmap progression of tags:**
- `v0.1.0-alpha` — Sprint 1 closure (May 17, 2026) — MVP code complete, runtime verified
- `v0.1.0-rc.1` — Sprint 2 closure (May 18, 2026 — **today**) — persistent library, full lifecycle
- `v0.1.0-rc.2` (potential) — Sprint 3 closure (preprocessor: URL/email)
- `v0.1.0` (stable) — Sprint 5 closure (public release с MSI installer)

These are **internal milestones**, not public releases. GitHub Releases page used как UI primitive — но no installers, no changelog publishing yet.

---

## Stats — Sprint 2 final

| Metric | Sprint 1 closure | Sprint 2 closure | Delta |
|---|---|---|---|
| Tests passing | 76 | **103** | +27 |
| New Rust LOC (Sprint 2) | — | ~770 | (paths.rs + db/* + commands/storage.rs + persistence in synthesize.rs) |
| New TS LOC (Sprint 2) | — | ~440 | (Library.tsx full rewrite + format.ts + tauri.ts wrappers + Synthesize.tsx refactor) |
| New Rust deps direct | 0 | 2 | rusqlite, rusqlite_migration |
| New Rust deps transitive | 0 | ~3 | libsqlite3-sys (bundled SQLite), uuid (already present), http-range (через protocol-asset feature) |
| Cargo Tauri features added | 0 | 1 | protocol-asset |
| Bundle size JS | 393.12 KB | 397.77 KB | +4.65 KB (+1.31 gzip) |
| Schema versions deployed | 0 | 1 | migration v1 — documents table |
| PRs merged | 4 (PR #11-14) | 4 (logical #15-18 → GH #17-20) | 4 each Sprint |
| Issues closed | 0 (during Sprint 1) | 1 | #15 Ctrl+R refresh |
| Open issues remaining | 3 (#5, #15, #16) | 2 (#5, #16) | -1 (#15 closed) |
| Calendar duration | 5 days (Day 0-4) | 1 day (Day 5, 4 sessions) | -4 days |

**Sprint 2 was significantly faster than Sprint 1.** Reasons:
1. Established patterns — Q&A → kickoff → phase-by-phase CC reporting → paste-back review → web_fetch sanity → merge → runtime verification. No paradigm decisions needed.
2. Smaller architectural surface — all PRs built on Sprint 1's foundation (HTTP client, OAuth, command structure, AppState, frontend routing/UI primitives).
3. Single contributor + CC partner — no coordination overhead, fast review cycles.
4. DB foundation laid early (PR #15) made everything downstream cleaner.

---

## Lessons learned — Sprint 2

### Технические

1. **Tauri 2 asset protocol Cargo feature consistency.** `tauri.conf.json` enabling `assetProtocol` requires matching `Cargo.toml` feature `tauri = { features = ["protocol-asset"] }`. Build script enforces parity — CC caught it runtime в Session 3 Phase 1. Lesson: when enabling any Tauri runtime feature через config, mentally check Cargo features parity too. Same applies для plugin permissions, IPC patterns.

2. **Tauri 2 custom protocols use `http://` scheme on Windows.** Not `https://` (Tauri 1 era). Asset URLs look like `http://asset.localhost/C/Users/.../audio_cache/uuid.wav`. CSP `media-src` directive must use `http://asset.localhost`. Stale memory from Tauri 1 → recoverable through web search verification step.

3. **`convertFileSrc` lives в `@tauri-apps/api/core` (not `@tauri-apps/api/tauri` from v1).** Naming shift in v2.

4. **rusqlite Transaction Drop semantics для atomic-ish persistence.** Transaction rollbacks automatically on Drop if not committed. `INSERT row → fs::write → tx.commit()` pattern means: file write failure → tx not committed → row rolls back via Drop. No explicit `tx.rollback()` needed. Clean primitive composition.

5. **Block-scoped Mutex guard pattern.** Better than `drop(conn);` для locked-then-fs-op flows. Compiler-enforced ordering through structure. Future refactor cannot accidentally inline fs op into lock scope без visible code change. Established as discipline pattern в PR #16/#17 across two modules.

6. **`Connection::open_in_memory()` тест speed scaling.** 103 tests run в 0.19s on local machine. `:memory:` strategy validated at scale — no Windows SQLite file locking flakiness, no temp dir cleanup overhead. Q3 PR #15 decision pays dividends compound through Sprint 2.

7. **Lock release before fs ops critical для AV scanner Windows behavior.** AV scanner can stall `fs::remove_file` для hundreds of milliseconds. Holding the Mutex across это blocks every other command. Verified through `try_lock` test pattern в PR #17.

8. **Native HTML5 audio surprises (positive).** WebView2's default audio controls expose playback rate menu through 3-dot menu. Q2 decision A "native is enough для MVP" partially confirmed runtime — feature юзер хотел (playback rate) actually accessible, just one tap away vs first-class button.

9. **Sber smart prosody hint.** SaluteSpeech ignores decorative trailing punctuation in numbers ("1.1." → "1.1"). Server-side prosody analysis. Useful baseline для Sprint 3 `text::preprocessor` design — don't double-process what Sber already handles.

### Процессные

1. **Phase-by-phase reporting matters для production code paths.** Single-pass acceptable when scope is purely additive (Session 1 PR #15). When PR touches existing production code path (PR #16 refactor synthesize), phase-by-phase keeps regression surface visible — chat ack between phases gives chance to course-correct. Re-established discipline в Sessions 2-4.

2. **CC's spec deviations often improvements.** Pattern emerged: CC encounters spec wrinkle, makes pragmatic choice, documents в paste-back, chat reviews and approves. Examples Session 2-4:
   - `persist_synthesis_result` helper extraction (better than mockito at command level)
   - Block-scoped Mutex guards (cleaner than `drop()`)
   - Header preserved across states (UX improvement)
   - Cancellation flag in DocumentRow useEffect
   - audio_path === null forward compat
   - Local Skeleton component instead of network install
   
   Spec is **starting point**, not contract. CC's domain expertise during implementation is valuable signal — don't reject deviations reflexively.

3. **Negative regression sanity cycle establishes load-bearing-ness.** PR #13 (Sprint 1) pattern reused в PR #20 (Sprint 2). Comment out fix → new test fails с expected error → restore → test passes. Document в paste-back AND в PR body. Reviewer confidence — proves fix is the actual cause of test passage, not coincidence.

4. **AI attribution footer keeps appearing despite explicit kickoff prohibition.** Third PR в Sprint 2 (PR #20) still got "Generated by Claude Code" footer despite four kickoff prohibitions. Tool-level injection через `mcp__github__create_pull_request`. Not blocker (factually true, project openly AI-assisted), but pattern signals: fixing requires either (a) post-create `mcp__github__update_pull_request` call to strip footer, or (b) different MCP tool, or (c) live with it. Deferred to Sprint 5 polish backlog.

5. **GitHub PR numbering drift.** Sprint 2 logical PRs #15-18 became GH PRs #17-20. Three-position offset стабильная — preserved через все 4 PRs. Logical-to-GH mapping documented в Session 1 log + Session 2 master log тут.

6. **Web search verification of deps freshness paid off.** Three search points в Session 3 (asset protocol scheme, convertFileSrc location, assetProtocol config shape) каждый caught stale memory from Tauri 1 era. Kickoff would have prescribed wrong CSP otherwise. Practice — verify before commit для evolving framework conventions.

7. **DB Browser for SQLite installation pattern.** Established Session 2 — visual DB inspection is faster than CLI queries для verification steps. Recommend install once, reuse через Sprint 3-5. Bonus tool.

### Архитектурные

1. **Transaction-wrapped persistence pattern для multi-step writes.** rusqlite Transaction + std::fs::write coordinated via Drop semantics gives quasi-atomicity. Lock primitive из standard library cooperates с filesystem primitive из std::fs through careful ordering. No locking framework needed.

2. **Cache-first short-circuit для idempotent operations.** PR #20 fix established pattern: if process-lifetime cache holds proof of past success, trust it on subsequent calls. Force parameter gives explicit bypass. Same pattern applicable для future commands где operation result стабилен в process lifetime.

3. **Discriminated union state machines для async UI.** PR #17 Library page state: `loading | empty | ready | error`. Cleaner than `data && !loading && !error` boolean stew. Same pattern PR #12 (Sprint 1) `CredentialsContext` tri-state. Scales к Sprint 4 status='synthesizing' addition без architectural rewrite.

4. **paths::audio_cache_root как single grep target.** Established Session 1. Validated Session 3 — when Sprint 5 configurable library location lands, single function modification + dynamic asset protocol scope addition будет enough. Grep marker pattern beats TODO comments.

5. **Static config + dynamic API для scope evolution.** Sprint 2 uses static `assetProtocol.scope = ["$APPLOCALDATA/audio_cache/**"]`. Sprint 5 will keep static config as safety net but add dynamic `allow_directory()` calls на startup для user's configured path. Defense in depth without giving up Sprint 5 flexibility.

6. **Repository functions stay pure CRUD.** UUID + created_at generated в command layer (PR #16 Q3). Repository takes pre-formed DocumentRecord. Pure functions easy to test без time/randomness mocking burden.

7. **Native HTML5 first, custom controls deferred.** Q2 PR #17 decision A confirms philosophy: trust the platform primitive, defer custom UI to когда genuine product value demands. Sprint 4 будет swap к custom controls когда resume playback + queue management become first-class — by then we'll know what we actually need.

---

## What's next — Sprint 3 prep

Sprint 2 closed. Натуральная pause point.

### Sprint 3 — Polish + accuracy

Compass artifact Sprint 3 goal: **`text::preprocessor` module** для humanization of mechanical pronunciation.

**Likely scope (TBD via Q&A at Sprint 3 entry):**
- URL detection (regex или crate-based) + replacement strategy ("смотрите URL в документе" vs spell-out vs preserve)
- Email detection + reading control (spell out chars or substitute "адрес электронной почты")
- Number formatting heuristics (override Sber defaults where they misfire? — careful, Sber already smart per Sprint 2 observation)
- Abbreviation expansion (т.е. → "то есть", etc. → "и так далее")
- Possibly date/time formatting (DD.MM.YYYY → "двадцатое мая две тысячи двадцать шестого года")
- Closure tag `v0.1.0-rc.2`

### Sprint 4 — Player + cache + parallel

- Custom audio controls (playback rate slider, position scrubber, resume playback)
- Status transitions (`'synthesizing'`, `'error'` rows displayed in Library)
- Parallel chunk synthesis (Semaphore-bounded concurrency)
- Event-based Library updates (`document-saved`, `document-status-changed` events)
- Background synthesis (UI free while synthesis runs)
- Chunks table introduction (for resume + per-chunk progress)

### Sprint 5 — Public release prep

Накопленный Sprint 5 backlog (через все sessions):
1. **CI setup** (GitHub Actions: cargo test + pnpm test + lint matrix)
2. **Configurable library location UI** (Settings + dynamic asset protocol scope)
3. **Signed MSI installer** (Wix или WiX-alternative)
4. **CHANGELOG.md** (batch all Sprint 2 + Sprint 3 entries here)
5. **Documentation polish** (README, SECURITY.md updates, dev vs release path notes)
6. **Inline title editing on Library rows** (для smysловых titles)
7. **Library delete UX upgrade** (conditional на real users feedback)
8. **AI attribution footer prevention** (research через `mcp__github__update_pull_request` post-create)
9. **Smart title boundary cut** (don't break слова в середине)
10. **README documentation о Tauri 2 path resolution** (dev `app.glagol.desktop\` vs release `Glagol\`)
11. **Accessibility audit** (aria-labels on all icon-only buttons - some already exist from PR #17)
12. **Sprint 4 backward-compat checks** before Sprint 5 release tag

### Sprint 2 deferred items resolved within Sprint 2

- ✅ Library page real content — PR #17
- ✅ asset protocol enablement — PR #17
- ✅ persistence refactor — PR #16
- ✅ Ctrl+R credentials bug fix (issue #15) — PR #18
- ✅ Sprint 2 closure tag — `v0.1.0-rc.1`

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **Sprint 2 entry doc:** `.scratch/kickoff-day-5.md`
- **Session 1 kickoff:** `.scratch/kickoff-day-5-session-1.md`
- **Session 2 kickoff:** `.scratch/kickoff-day-5-session-2.md`
- **Session 3 kickoff:** `.scratch/kickoff-day-5-session-3.md`
- **Session 4 kickoff:** `.scratch/kickoff-day-5-session-4.md`
- **Session 1 master log:** `.scratch/day-5-session-1-master-log.md`
- **PR #18 (logical #16):** https://github.com/dimasiksuleyman-sudo/glagol/pull/18
- **PR #19 (logical #17):** https://github.com/dimasiksuleyman-sudo/glagol/pull/19
- **PR #20 (logical #18):** https://github.com/dimasiksuleyman-sudo/glagol/pull/20
- **Issue #15 (closed by PR #20):** https://github.com/dimasiksuleyman-sudo/glagol/issues/15
- **Tag `v0.1.0-rc.1`:** Sprint 2 closure
- **Main HEAD at closure:** TBD (post PR #20 squash-merge — assigned by GitHub)

---

*Day 5 Sessions 2-4 captures: PR #16 logical (persistence refactor + command split) + PR #17 logical (real Library page) + PR #18 logical (Ctrl+R fix + Sprint 2 closure tag).*
*Sprint 2 closure achieved. Three new tests + 1 adapted from Session 4 brought total to 103.*
*`v0.1.0-rc.1` pushed to origin. Ready for solid pause before Sprint 3.*
*Last updated: May 18, 2026*
