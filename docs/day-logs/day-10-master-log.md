# Glagol — Day 10 Master Log

**Period:** July 16–17, 2026 (Sprint 6 Dictation marathon — PR3 Phase 0 through PR3.1 runtime-QA closure)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 6 — Dictation — **PR3 of 5**
**Status at end of Day 10:** PR1 + PR2 + PR3 + PR3.1 merged. **277 tests passing, 1 ignored, 0 regressions.** Dictation-to-clipboard works on a packaged release build. Marathon tag still deferred to PR5.

> The calendar day split: PR3 landed July 16, its runtime QA slipped to July 17 and pulled a hotfix (#40) with it. Both are logged here as one unit, because PR3 was not honestly closed until the QA ran.

---

## TL;DR

Day 10 turned three invisible layers into the marathon's first user-facing feature — and then discovered that the instrument meant to measure it had never been switched on.

1. **Phase 0 research gate (no code until cleared) answered the question that mattered most — and it wasn't about code.** The marathon plan defends against antivirus heuristics by claiming "RegisterHotKey, not a keyboard hook." Verified independently in `global-hotkey 0.8.0` source: zero occurrences of `SetWindowsHookEx` / `WH_KEYBOARD_LL`. `Released` is synthesised by a thread spawned **per press** inside `global_hotkey_proc`, polling `GetAsyncKeyState` every 50 ms and terminating on release. The claim holds. Push-to-talk is robust; the pre-agreed toggle fallback (D2-A) was not needed.

2. **PR #39 — hotkey, overlay, tray, pipeline.** 254 → **273 tests** (+19). One deviation: CC added a `discarded` disposition, closing a hole in D7 that the kickoff author never saw.

3. **PR #40 — the day's real finding.** 11 `tracing::*!` calls across the crate, **zero subscribers**, no `tracing-subscriber` in `Cargo.toml`. Everything dispatched to nowhere; `RUST_LOG` inert. D8's calibration rider — "log each clip's RMS so that after a week there's data instead of guesswork" — was implemented, compiled, green, **marked done in #39's body, and produced nothing.** A public app with an installer had no diagnostics at all. 273 → **277 tests**.

4. **Runtime QA closed honestly**, including the one thing unit tests cannot check: the packaged release build writes its log, with RMS lines, and without transcripts or keys.

5. **The first real calibration numbers arrived** — and they exonerated the threshold while refuting two decisions built around it. Both were the maintainer's; the threshold was too.

6. **The marathon's first dictated sentence** landed in the clipboard via `Ctrl+V`.

---

## Sprint 6 — where the marathon stands

```
PR1  feat/stt-client              ✅ #35 + #36
PR2  feat/dictation-recorder      ✅ #37 + #38
PR3  feat/dictation-hotkey-overlay ✅ #39 + #40
PR4  feat/dictation-paste            ← next
PR5  feat/dictation-page
                                     tag after PR5
```

**Working today, on a release build:** hold `Ctrl+Shift+Space` → pill appears on screen with live levels → speak → release → "Распознаю…" → text in clipboard → `Ctrl+V` anywhere. Tray icon switches idle ⇄ recording. Closing the main window minimises to tray.

---

## Phase 0 — research gate

Three checks, delivered before a line of code. All three paid.

**1. How `Released` exists at all.** `RegisterHotKey` natively delivers only `WM_HOTKEY` on press — there is no key-up. The whole push-to-talk model rests on `Released`, so *how* it is synthesised is not a detail; it decides whether the product is shippable.

```rust
// global_hotkey 0.8.0, windows impl — global_hotkey_proc, on WM_HOTKEY:
GlobalHotKeyEvent::send(... Pressed);
std::thread::spawn(move || loop {
    let state = GetAsyncKeyState(HIWORD(lparam as u32) as i32);
    if state == 0 { ...send(Released); break; }
    std::thread::sleep(Duration::from_millis(50));
});
```

Verified independently: `grep -c "SetWindowsHookEx|WH_KEYBOARD_LL|WH_KEYBOARD"` over the entire Windows impl → **0**.

Consequences, all good:
- **No low-level keyboard hook** → the plan's AV argument holds. This never touched PR3's code; it touches installer signing, SmartScreen and product trust.
- The polling thread **lives only during the hold** — spawned on `WM_HOTKEY`, broken on release. No permanently running poller, nothing to check for idle CPU.
- Upstream's own source comments name push-to-talk and 50 ms as an imperceptible release latency. Our exact use case is a first-class upstream consideration, not an exploited side effect.
- Release latency ≤ 50 ms is harmless: samples are already buffered; at most 50 ms of trailing silence is lost.

**2. `arboard` default features.** `default = ["image-data"]` pulls the heavy `image` crate; `default-features = false` drops it with the text path unaffected. (`image` still compiles — for Tauri's `image-png` tray feature, as designed.)

**3. Plugin ↔ Tauri.** `tauri-plugin-global-shortcut 2.3.2` requires `tauri >= 2.10`; ours is 2.11.1.

> **Correction banked from Phase 0.** `MOD_NOREPEAT` is set at registration — so `Pressed` **does not repeat** while held. The Q&A-7 amendment ("keyboard autorepeat ~30 Hz is the norm, hence trace-level at most") rested on a false premise. The D4 guard is still correct, for a different reason: it catches a second *deliberate* press during `Processing`, not autorepeat. Right decision, wrong rationale.

---

## PR #39 — hotkey, overlay, tray, clipboard pipeline

**Branch:** `claude/dictation-hotkey-overlay-tray-t4zem9` · **merged** · **squash SHA** `c105aed` · 15 files, +1946 / −16

- `dictation/pipeline.rs` — `run_dictation` behind four seams (recorder / provider / sink / emitter), silence filter, wall-clock watchdog (D10), clipboard delivery via `arboard` in `spawn_blocking`, `dictation-state` events with `disposition` + `truncated`
- `dictation/session.rs` — hotkey handler, tray idle ⇄ recording, Показать/Выход, close→tray with one-time dialog (`tray_notice_shown`), missing-key guard, plus a **session-generation guard** defusing a teardown-vs-new-press race CC found on its own
- `OverlayPill.tsx` + `main.tsx` label branch — opaque pill inside a transparent window, Rust-driven show/position, frontend-driven content and hide timing

**Tests: 254 + 19 = 273.** Bundle JS +17.1 kB (+4.3 kB gzip). Schema impact: none.

### Deviation — `discarded`

CC added a `discarded` value to the `disposition` enum so that a tap, a silent clip, or an empty transcript hides the pill without flashing "Скопировано".

**This closed a hole in the kickoff, not in the code.** D8 mandated "tap < 300 ms → quietly do nothing", while D7 offered exactly two outcomes: `done { disposition }` or `error`. There was no way to express *nothing happened*. The contract therefore required the pill to announce "Copied" after a clip that was never sent. Two decisions written apart, not composed — the same class as the U8 defect on Day 9 and the cap-vs-watchdog mine caught in the PR3 kickoff.

Forward-compatible: event name and shape unchanged, only the value set grew; the `done.disposition == "clipboard"` snapshot test for PR4 stays.

---

## PR #40 — observability (the day's real finding)

**Branch:** `fix/observability-logging` · **merged** · **squash SHA** `c057a93`

### How it surfaced

Runtime QA step: "check the computed RMS in the debug log — the first real number for calibrating D8." There was no log. Not in devtools (that's the webview; `tracing` writes from Rust to stdout). Not in the terminal either — with `RUST_LOG` set.

```
pipeline.rs  → tracing::debug! ×4 (line 320 = the RMS line)
lib.rs       → only plugin_opener::init(), plugin_dialog::init()
Cargo.toml   → tracing = "0.1"        ← the facade
             → tracing-subscriber      ← ABSENT
```

`tracing` is a facade. Without a `Subscriber`, events dispatch into the void: the macro compiles, runs, and reaches nothing. `RUST_LOG` is read by `EnvFilter`, which did not exist. A crate-wide count found **11 such calls**.

**What this meant.** The D8 rider — the maintainer's own Q&A amendment, "log each clip's RMS; after a week of real use there will be statistics instead of guesswork" — was implemented, compiled, tested, and **ticked as done in #39's body**. After a week there would have been nothing. The threshold 0.005 would have stayed a guess forever, because the data meant to replace it was written nowhere.

Wider: Glagol is public, has an installer, is at v0.1.0-rc.7. A user reporting a bug could be asked for nothing. There was no diagnostics for the user and none for the maintainer.

**The perfect silent failure:** compiles, tests green, PR body says done, output zero. Nothing turns red. Fourth of this class in three days.

### The fix

```toml
tracing-subscriber = { version = "=0.3.23", features = ["env-filter"] }  # 13.03.2026, 79M dl
tracing-appender   = "=0.2.5"                                            # 17.04.2026, 9.1M dl
```
Pure Rust, no `windows` graph — the D1-B class is structurally impossible here.

- **dev**: stdout + `EnvFilter::from_default_env()`, default `glagol_lib=debug`
- **release**: daily-rolling file in `app_log_dir`, `max_log_files(7)`, directive `info,glagol_lib::dictation=debug`
- Init inside `setup()` (needs `AppHandle` for `app_log_dir`); events before setup are lost — accepted.

**Pre-answered trap — `WorkerGuard`.** `tracing_appender::non_blocking` returns `(NonBlocking, WorkerGuard)`. Drop the guard and the writer **silently stops flushing**. The observability fix would itself have become a silent failure — fifth of the class in three days, this time inside the cure. Guard parked in `AppState` for process lifetime.

**Phase 0 for the hotfix:** confirmed `main.rs` carries `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` → **no console in a release build** → the stdout layer alone would have worked for the maintainer and written nowhere for users. This is what justifies the file layer, and it is why the naive fix ("add a subscriber") was wrong.

### Redaction rules — made written, and guarded

1. **Keys (STT / SaluteSpeech) never reach the log.** Project convention; a file on disk living seven days is the worst place to break it.
2. **Transcript text never reaches the log.** It is the user's speech. A file of dictated speech on disk is a 152-ФЗ-class problem in a product positioned as "your data doesn't leave."
   Logged instead: RMS, durations, error codes, dispositions, flags — never content.

Enforced by a **source-scan test** over the whole crate: no `tracing::` call may capture `transcript` / `api_key` / whole-word `key`. String literals are stripped, so `"empty transcript"` stays legal while `?transcript` does not.

**Two review gaps found before merge, both mine:**

- **Rule #1 was unguarded where it matters.** My spec scoped the scan to `dictation/*` — where the transcript lives. Keys live in `stt/`, `secrets/`, `commands/credentials.rs`, `salute/`. CC first established ground truth (those modules have **zero** tracing calls today — so rule #1 wasn't violated, merely unguarded), then widened the scan crate-wide, then **proved it by injecting a key capture into `salute/auth.rs`** and watching the scan catch it. The negative cycle for rule #1 had to run where keys live; a cycle in `pipeline.rs` would have proved the wrong rule.
- **A self-contradiction in the plan.** Release default `info` would filter out the `debug` RMS lines — so the release-log QA step was wrong by construction. My proposed resolution was to strand calibration in dev. CC resolved the other way: release directive `info,glagol_lib::dictation=debug`. **The better answer** — D8's rider now collects data during *real use*, no log-call levels changed (D-L7 intact), and the QA plan works as written. My version would have quietly buried the rider's purpose.

**CC's own initiative, banked as convention:** clippy run on **both profiles**. The file-logging branch only compiles with `debug_assertions` off — without `--release` it would have shipped un-linted. Not in my spec.

**Tests: 273 + 4 = 277.** Both fixes proven by injection cycles.

**Honest limit, stated in the PR body:** unit tests guard the rule and the event shape. That the subscriber is live and writing in the packaged app is verifiable **only** by Windows runtime QA.

---

## Runtime QA — closed

| Check | Result |
|---|---|
| Gates on Windows (authoritative) | ✅ 273 → 277 passed, clippy both profiles, fmt, tsc |
| Hotkey → pill → live levels | ✅ closes the PR2 carry-over: `dictation-level` observed for the first time |
| Happy path → `Ctrl+V` | ✅ **"Этот текст начитан с помощью глагола."** |
| Russian error with no key | ✅ |
| Tap < 300 ms → `discarded` | ✅ no "Скопировано" flash |
| Repeat hotkey during processing | ✅ ignored, no spam |
| 60+ s → `truncated` → pill | ✅ **D9 (recorder cap) + D10 (pipeline watchdog) compose** — the mine defused in the kickoff, confirmed live |
| Word open → no non-breaking space | ✅ **honest cost of D3 confirmed**, exactly as written in #39's body |
| Tray idle ⇄ recording, menu | ✅ |
| Transparency (D6) | ✅ rounded pill, desktop visible behind — degradation path not needed |
| Multi-monitor (D5) | ⬜ **not verified** — needs a second monitor; with one, both branches (cursor-monitor pick and primary fallback) are indistinguishable |
| Release build → file log | ✅ see below |

### The release log — the thing unit tests cannot check

```
D:\Glagol\glagol.exe                                             ← installed release, not target\debug
C:\Users\kiss2\AppData\Local\app.glagol.desktop\logs\glagol.2026-07-17.log
```

```
INFO  rusqlite_migration: Database migrated to version 3
INFO  glagol_lib::salute::auth: OAuth token refreshed successfully rquid=… expires_at_ms=…
DEBUG glagol_lib::dictation::pipeline: dictation clip finalized duration_ms=8830 rms=0.01248 truncated=false
INFO  glagol_lib::stt::openai_compat: stt transcribe success chars=20
```

- **RMS lines present in the release log** → the directive works; calibration accrues in real use. CC's resolution of Q2 validated on live.
- **No transcript** (`chars=20`, a length, not content) → rule #2 holds on disk.
- **No keys** → rule #1 holds. The scan guards the *source*; only this file knows what actually landed.

**Two navigation errors worth recording, both mine.** I first sent the search to `%APPDATA%` (Roaming); Tauri 2.11.1 resolves `app_log_dir` on Windows to **`local_data_dir`**`/${bundle_identifier}/logs`. Absence proved nothing — the instrument was pointed at the wrong directory. Then the file showed **0 bytes**, and I framed two defect hypotheses; both were wrong. `tracing_appender::non_blocking` buffers and flushes on a background thread — the listing had simply caught the window before the first flush.

> That last one is a real user-facing consequence, not a footnote: `WorkerGuard` flushes **on drop**, i.e. on clean exit. A user grabbing the log mid-session gets an unflushed tail — exactly the interesting part. USER_GUIDE must say: **close Glagol before sending logs.** → carry-over.

---

## Calibration — the first real numbers

The whole point of D8's rider. Threshold was set at 0.005 with a stated ceiling of 0.01.

| | RMS | dBFS | vs floor | outcome |
|---|---|---|---|---|
| noise floor (**fan running**) | 0.00396 | −48.1 | — | `reason=Silent` ✅ |
| **threshold** | 0.005 | −46.0 | +2.0 dB | |
| whisper | 0.01048 | −39.6 | +8.5 dB | passed, 49 chars |
| normal speech | 0.01642 | −35.7 | +12.4 dB | passed, 99 chars |

Release-build clips: `0.01249 / 8.83 s`, `0.02594 / 2.42 s`, `0.02723 / 2.04 s`.

**The threshold is vindicated on 3 of 3 classes — and it was the maintainer's number**, from the Q&A-12 answer ("start at 0.005, only obvious silence"), not the kickoff author's invention. It rejects the floor, passes whisper, passes speech.

**One hypothesis died, correctly.** I had argued the whisper/floor window might be near-zero, making RMS useless as a discriminator for whispered speech. It is **8.5 dB**. Measured, not decided.

**Byte-exactness of D7, twice:** 16000 × 2 × 9.96 + 44 = **318 764** ✓ · 16000 × 2 × 4.86 + 44 = **155 564** ✓. The invariant holds on real hardware, not only in fakes.

### What the data refuted

All three were the maintainer's, from the same Q&A answer that got the threshold right:

1. **Ceiling 0.01 — refuted.** Real speech measured 0.0164 and whisper 0.0105. The ceiling would sit at **95% of whisper's RMS** and 61% of normal speech. It is not a fuse, it is a cliff. By the data it should be **~0.007**.
2. **"Quiet speech 0.02–0.05" — not confirmed.** The microphone runs about **half** those estimates: normal speech landed *below* the stated floor for *quiet* speech. The ceiling was built on a premise inflated 2×.
3. **"If whisper is rejected in QA, halve the threshold" — dangerous.** 0.005 ÷ 2 = **0.0025**, below the measured noise floor of **0.00396**. Silence would pass **always**, and every tap over 300 ms would return a Whisper hallucination — grammatical Russian nobody spoke — straight into the clipboard. **The criterion needs a floor: never below the measured room floor plus margin.** As written, it is a self-destruct instruction.

Why hallucination matters more than it sounds: Whisper on noise does not return emptiness, it returns plausible text. The second guard — `discards_empty_transcript_without_writing_clipboard` — does not fire, because a hallucination isn't empty. The silence filter is the **only** defence. Same class as the resampler's silent degradation: plausible output, complete fiction, nothing turns red.

**Threshold not moved.** Correct on 3/3, but poorly centred: 2.0 dB above the floor, 6.4 dB below whisper, where the geometric mid-point would be 0.0064 (±4.2 dB). Moving it on n=1 per class is exactly what cost us three times. The logger now records every clip; in a week there will be a sample.

### Two structural findings the numbers exposed

**Short clips read ~2× "louder" than long ones.** 8.8–10 s clips → 0.0125–0.0164; 2.0–2.4 s clips → 0.0259–0.0272. RMS averages across the whole clip, and long speech carries pauses that drag the mean down. **The discriminator weakens with length** — a 60-second dictation with pauses drifts toward 0.006–0.008 at the same voice level. Reliable on short clips, degrading exactly where losing text hurts most. Not a number to tune: a property of the metric. Fix is percentile or peak instead of mean. → post-MVP.

**The threshold is absolute; the floor is a function of the room.** A fan was already running during the "silence" measurement — the floor tolerated it. But a café or a car easily puts ambient above normal speech, and then "silence" passes the filter, Whisper receives background noise and hallucinates into the clipboard. The failure is **silent**. Fix is an adaptive threshold (ambient measured in the first 200 ms) or SNR instead of absolute RMS. → **known limitation, must be written in USER_GUIDE**, not discovered in a review six months out.

### The logger's first find, before it even merged

```
18:16:19.333073Z  requesting OAuth token rquid=d644a634…
18:16:19.333067Z  requesting OAuth token rquid=f62fb58f…
```

**Two simultaneous OAuth requests to Sber at startup** — 6 µs apart, two rquids, both successful. This is TTS auth, not dictation, and it fires on launch before any synthesis. Looks like a thundering herd: two callers saw an empty cache and both went for a token; the second overwrites the first. Damage today is zero. But it is the same class we keep finding — silent, unnoticed, visible only once the lights came on. → carry-over.

---

## Carry-over registry

| → | Item | Origin |
|---|---|---|
| **PR4** | `clip_rms` allocates a full `Vec<f32>` duplicate (~3.84 MB on a 60 s clip) just for RMS; `rms_iter(n, impl Iterator)` keeps D6's `rms(&[f32])` signature and gives the pipeline a zero-alloc path | maintainer, Day 10 |
| **PR4** | `UnsupportedConfig` carries three meanings; the tail-loss case is *our* bug but the Russian text blames the device | #38 sanity |
| **PR4** | `live_mic` should print the chosen config (rate/channels/format) — still unknown which ladder step wins on real hardware | Day 9 QA |
| **PR4** | Question for CC: `MAX_FLUSH_ROUNDS` value and margin at 44.1k (2.75625) | #38 sanity |
| **PR4/5** | Release directive is narrower than the pipeline: `stt` stays at `info`, so a user's log has no model/lang/bytes. Half the diagnostics dark. | Day 10 release log |
| **PR4/5** | Global `info` admits foreign crates (`rusqlite_migration` appeared); the scan only guards `glagol_lib`. Standard fix: `warn,glagol_lib=info,glagol_lib::dictation=debug` | Day 10 release log |
| **PR5** | **D8 amendments by data:** ceiling 0.01 → **0.007**; relaxation criterion needs a floor ("never below room floor + margin") | Day 10 calibration |
| **PR5** | USER_GUIDE: **close Glagol before sending logs** (WorkerGuard flushes on drop) | Day 10 |
| **PR5** | USER_GUIDE: known limitation — absolute threshold degrades in loud ambient | Day 10 calibration |
| **PR5** | Whisper `prompt` with a vocabulary hint ("Глагол", "Привезём") — transcribed "глагола" as a common noun; also normalises numbers inconsistently ("раз два три 4 5") | Day 10 QA |
| **PR5** | Two OAuth requests at startup + cache race in `salute::auth` | Day 10 logger |
| **PR5** | **Revisit D10** — device names are generic (`Capture Input terminal`, `Microphone`): silent first-match collision risk, unusable picker | Day 9 QA |
| **post-MVP** | RMS mean weakens with clip length → percentile/peak | Day 10 calibration |
| **post-MVP** | Adaptive threshold / SNR instead of absolute RMS | Day 10 calibration |
| **unresolved** | **D5 (multi-monitor) not verified** — needs a second monitor | Day 10 QA |
| **note** | `logging.rs` is excluded from its own scan (it holds the forbidden identifiers as literals) — a documented hole; a future `tracing::` call there goes unchecked | Day 10 review |
| **note** | The scan will one day false-positive on `db/repository.rs` (`get_setting(key)` — a settings name, harmless). Right class of failure: loud, rare, one-minute fix. Comment needed: use `setting_name` for `app_settings` keys. | Day 10 review |

---

## Stats — Day 10

| Metric | Value |
|---|---|
| PRs merged | 2 (#39 PR3, #40 PR3.1) |
| Tests | 254 → 273 (#39) → **277** (#40), +1 ignored |
| Regressions | **0** |
| New dependencies | `tauri-plugin-global-shortcut 2.3.2`, `arboard 3.6.1`, `tracing-subscriber 0.3.23`, `tracing-appender 0.2.5` — all `=`-pinned |
| Migrations | **none** (`tray_notice_shown` is a row in existing `app_settings`) |
| New `unsafe` | none |
| Injection / negative cycles | 3 (transcript scan, key scan in `salute/auth.rs`, plus #39's) |
| Dead code found | **11 tracing calls**, live since before the marathon |
| Kickoff holes found by the author | **0 of 2** (`discarded`, scan scope) |
| Calibration data points | 7 clips, 3 classes |

---

## Lessons — Day 10

1. **A facade without a subscriber is the perfect silent failure.** It compiles, tests pass, the PR body ticks the box, and the output is zero. There is no unit test for "does the log actually reach a disk" — that check exists only at runtime, on the target platform, in a release build.

2. **The cure nearly caught the disease.** Dropping `WorkerGuard` would have silently stopped the writer — the observability fix reproducing, inside itself, exactly the class of bug it was written to expose.

3. **A rule is guarded only where its guard is scoped.** The spec put the scan where the transcript lives; the keys live elsewhere, and rule #1 went unenforced while the PR body implied otherwise. Scope the guard to the rule, not to the example.

4. **When code lives behind `cfg`, the gate must run on both profiles.** Otherwise the branch ships un-linted and unseen. Banked as convention.

5. **Data beat both parties, asymmetrically.** The maintainer's threshold was right on 3/3; the maintainer's ceiling was a cliff, the maintainer's relaxation criterion was a self-destruct instruction, and the kickoff author's fear about whisper was unfounded. Nobody's intuition survived intact — which is the argument for the rider that almost never shipped.

6. **Absence proves nothing until the instrument is verified.** Two hypotheses about a "broken" logger dissolved: the search was in Roaming instead of Local, and the 0-byte file was a flush window. Both times the defect was in the measurement, and both times it was mine.

---

## What's next

**PR4 — `feat/dictation-paste`:** auto-insertion via `enigo`, clipboard snapshot/restore, `disposition: "pasted"` (the contract laid in D7 for exactly this). Windows-authoritative territory again. Six carry-over items ride along.

Then PR5 (Dictation page, history, device picker, configurable hotkey, D8 amendments, USER_GUIDE limitations), the 18-point manual QA checklist, and the marathon tag.

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #39 (Dictation PR3):** `feat: global hotkey, overlay and dictation-to-clipboard pipeline` · merged · `c105aed` · 15 files, +1946/−16 · 254 → 273
- **PR #40 (Dictation PR3.1):** `fix: install tracing subscriber, dev stdout + prod rolling file` · merged · `c057a93` · 273 → 277
- **Phase 0 audit point:** `global-hotkey 0.8.0`, windows impl — `global_hotkey_proc`, `MOD_NOREPEAT` at registration, `GetAsyncKeyState` poll thread per press
- **Release log path:** `%LOCALAPPDATA%\app.glagol.desktop\logs\glagol.<date>.log` (Tauri: `local_data_dir/${bundle_identifier}/logs`)
- **PR3 kickoff (D1–D15, D2-A):** `docs/day-logs/kickoff-dictation-pr3-hotkey-overlay.md`
- **Marathon plan:** `PlanofRUWhisper.md`
- **`main` HEAD at Day 10 close:** `c057a93`, working tree clean

---

*Day 10 captures: PR3 Phase 0 research gate (Released mechanism verified in source — no keyboard hook, the plan's AV argument holds) + PR #39 delivering the marathon's first user-facing feature (+19 tests) + the `discarded` deviation closing a hole in D7 + the discovery that 11 `tracing` calls had never had a subscriber, making D8's calibration rider a documented no-op + PR #40 installing observability with both redaction rules made written and proven by injection + runtime QA closed on a packaged release build + the first seven calibration clips, which vindicated the threshold and refuted the ceiling, the estimates, and the relaxation criterion built around it.*
*277 tests, 1 ignored, 0 regressions. The marathon's first dictated sentence: "Этот текст начитан с помощью глагола." — its last word intact, which is the resampler flush fix (D8-A) visible in production on live speech.*
*Last updated: July 17, 2026*

---

*Created by Dmitriy + Claude*
