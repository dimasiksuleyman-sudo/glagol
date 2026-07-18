# Glagol — Day 11 Master Log

**Period:** July 18, 2026 (Sprint 6 Dictation marathon — PR4 kickoff Q&A through PR4 runtime-QA closure)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 6 — Dictation — **PR4 of 5**
**Status at end of Day 11:** PR1 + PR2 + PR3 + PR3.1 + **PR4** merged. **307 tests passing, 1 ignored, 0 regressions, 0 hotfixes.** Dictation now **auto-inserts into the active window** on a packaged release build. Marathon tag still deferred to PR5.

> Day 11 was a single continuous unit: the PR4 kickoff Q&A, implementation across four phases, sanity, merge, and the full first pass of the Windows runtime-QA checklist. Unlike Day 10, PR4 landed **without a hotfix** — the instrument (the tracing subscriber) was already installed and verified from #40, so the calibration rider produced data on the first run instead of the third.

---

## TL;DR

Day 11 shipped the second half of the dictation feature — synthetic paste into foreign windows — and then let real Windows hardware correct two contract descriptions that were reasonable on paper and wrong in fact.

1. **The kickoff Q&A found a composition mine before any code — the same class as U8 and the D7 hole.** The plan (line 42) requires "paste only after the hotkey is released"; Day 10's own Phase 0 showed `Released` polls `HIWORD(lparam)` — **Space only**, never Ctrl/Shift. "After the hotkey" ≠ "after the modifiers." A user still holding Ctrl+Shift turns our `Ctrl+V` into `Ctrl+Shift+V`. Fix (D5): synthetic release of Shift/Alt/Meta before paste — which turned out to be **documented Microsoft practice** (MSDN `SendInput` Remarks). Neither document showed the gap alone; only reading them side by side did.

2. **The round was symmetric for the first time in the marathon.** 15 questions → 4 maintainer edits to the kickoff author's decisions, 3 kickoff-author edits to the maintainer's answers, **1 claim the maintainer retracted after checking his own history** (the `Type` mode was never approved — he had stated his own proposal as a settled decision). On Day 9 the score was 0 of 4 in the author's favour. The instrument this time was mutual review, and it worked both ways.

3. **PR #41 — auto-paste, 277 → 307 tests (+30), zero regressions, zero hotfixes.** Both negative-regression cycles (VerifyOwn, ReleaseModifiers) run with real failing traces, not promises. Phase 0 closed D1 by graph (`windows 0.61.3` already unified across Tauri+cpal, enigo adds 4 packages, no split) and closed D16 **forever** by measurement (worst tail deficit 68 frames, 1 flush round covers it, budget of 16 ≈ 15× margin → tail-loss branch unreachable).

4. **Runtime QA closed the four load-bearing points unit tests cannot reach** — UIPI, a physically-held Shift, a live image in the clipboard, and the word on the pill. All four passed. The fakes tested logic; the hardware tested contact with reality.

5. **Hardware corrected two contract descriptions.** The elevated-window failure is **not** UIPI-on-paste as the kickoff described — it's the hotkey never reaching the unelevated process, so dictation never *starts* (zero-length clip, `TooShort`). And the "restore skipped: non-text snapshot" branch fired at `WARN`, landing in the release log under the base directive — the payoff of yesterday's Q3 side-effect.

6. **The marathon's first dictated sentence landed in a foreign window** — Notepad, Chrome, Word, Telegram, Terminal — with no manual `Ctrl+V`.

---

## Sprint 6 — where the marathon stands

```
PR1  feat/stt-client               ✅ #35 + #36
PR2  feat/dictation-recorder       ✅ #37 + #38
PR3  feat/dictation-hotkey-overlay ✅ #39 + #40
PR4  feat/dictation-paste          ✅ #41            ← merged today
PR5  feat/dictation-page              ← next
                                      tag after PR5
```

---

## Kickoff Q&A — 15 questions, closed symmetrically

Full decision set D1–D16 locked before Phase 0. The round's own tally, banked because it is the round's main result, not decoration:

- **4 maintainer edits** to the kickoff author's decisions: `VerifyOwn` added to D7's restore conditions; "do not re-press modifiers after paste" added to D5; the trace-order canon `Snapshot → SetText → ReleaseModifiers → Paste → Settle → VerifyOwn → Restore` (author's draft had neither `ReleaseModifiers` nor `VerifyOwn`); three distinct no-restore log reasons.
- **3 kickoff-author edits** to the maintainer's answers: `libxdo-dev` not needed (enigo ≥0.5 uses pure-Rust `x11rb`; `libxkbcommon` already present) — a factual correction to the maintainer's install plan; the D14 logging rationale was **right decision, stale reasoning** (the subscriber already shipped in #40 — re-installing would `Err`); the `MAX_FLUSH_ROUNDS` "question" promoted to a mandatory Phase 0 computation.
- **1 retraction:** the maintainer proposed a three-value `InsertionMode` enum citing prior approval of a `Type` mode. On checking his own history he found his own sentence ended "if you want, we add it" — with no yes following. He retracted. The enum shipped two-valued (`Paste | ClipboardOnly`); `Type` is an OUT item pending an explicit PR5 decision + a line-42 plan amendment.

**Composition mine (D5) — found at kickoff-assembly time.** Documented in the TL;DR above; the class is U8 / the D7 hole: two decisions written apart, never composed. Found by neither original author — by reading the plan and Day 10's Phase 0 together.

**Process rule banked (D15).** PR3's kickoff carried a "Carry-over (baked in)" section with four checkboxes; **three of four evaporated** — precisely the three with no phase to live in. The one that survived was the only one tied to a QA step. Rule: **a carry-over without a phase name is a wish, not a task.** PR4's carry-overs therefore rode as **Phase 4 with their own gate**, not checkboxes. All four shipped.

---

## PR #41 — auto-paste into the active window

**277 → 307 tests (+30, −0).** Two commits: the feature (277→303) and a follow-up pinning the three no-restore reasons (303→307).

### Phase 0 — research gate (no code until cleared)

| Check | Result |
|---|---|
| `windows` graph | `windows 0.61.3` already unified across the Tauri + cpal graph; enigo declares `windows = "0.61"` → same `0.61.3`, adds only 4 packages (`enigo`, `memmap2`, `nom`, `xkbcommon`), **no split** — unlike `cpal 0.18` |
| Toolchain vs MSRV | 1.85 / edition 2024 — pinned in CI |
| `Key::Other(0x56)` | maps to raw `VIRTUAL_KEY(0x56)` in 0.6.1 source, no layout-dependent mapping (that path is `Key::Unicode` only) → Ctrl+V survives a Russian layout (D4) |
| `SendInput` Err | enigo surfaces the detectable short-write as `Err`; UIPI stays undetectable per MSDN → D11 unchanged |
| `MAX_FLUSH_ROUNDS` (D16) | worst post-pad tail deficit **68 frames**, one flush round emits ≥43, budget of 16 ≈ **15× margin** → tail-loss branch **unreachable for any real device** → D16 **closes forever**, no `UnsupportedConfig` split |

`enigo =0.6.1` pinned exact, **unconditional** (D2, not `cfg(windows)` — so the insertion path stays on the Linux lint gates), `Cargo.lock` in the same commit.

### Deviations from spec (documented, all accepted)

1. **TS mapping "test" = a compile-time `never`-guard, not a vitest case** — no JS runner in the repo; adding one is out of PR4 scope. The exhaustiveness half is compiler-enforced; the **string** half (`pasted` → «Вставлено») is invisible to the compiler → **QA #23 became load-bearing** (the only place that wiring is verified). Banked in the PR body.
2. **Seams are unit structs constructing the real Enigo/Clipboard per method call** — strictly D8-compatible ("per insertion, no state, drop releases held"), both `Clone+Send+'static`. Sanity confirmed via source: `paste()` uses **one** `Enigo::new` for the whole chord, drop after `Control Release` — so "per method call" = "per chord," no premature Ctrl release.
3. **`live_mic` config as a production `tracing::info!`** in `CpalSource::start` (logged on every real capture, real diagnostic value) — an improvement over the `#[ignore]`-test-only plan.
4. **Local Linux gate needed a placeholder `PDFIUM_LIBRARY_PATH`** (org egress blocks the GitHub-release download, 403) — local accommodation only, no committed `build.rs` change; Windows CI downloads pdfium normally.

### Negative regression cycles (2, both performed with real traces)

1. **`VerifyOwn`** — replaced the ownership check with `owns_clipboard = true` → `d9_buffer_changed_before_restore_cancels_restore` failed (`writes` became `["привет мир","старый буфер"]`, clobbering the intruder) → restored → green.
2. **`ReleaseModifiers`** — dropped the step from `plan_insertion` → 6 tests failed incl. the trace-order asserts (`["paste"]` vs `["release_modifiers","paste"]`) → restored → green.

### The Q3 follow-up commit — better than the requirement

The three no-restore reasons route through a single `NoRestoreReason::message()` (source of truth) so a refactor cannot silently reword one; pinned + capture-tested per branch. **Side-effect the kickoff did not ask for:** "рестор отменён: буфер изменился" raised from `debug` to `warn`, so all three reasons are uniform and land in the release log under the base `warn` directive — not only via `dictation=debug`. This is what made QA #21 observable on a stock release build (see below).

### Gates (all green, both profiles)

`fmt` 0 · `clippy` dev 0 · `clippy` release 0 · `test` 307 passed, 1 ignored · `tsc` 0 · `build` 0. Bundle `index.js` 446.0 kB / 140.31 kB gzip — effectively unchanged.

**Windows CI: green before merge.** For PR4 this was not a formality — `enigo 0.6.1` + `windows 0.61.3` were compiled and run on Linux, and a green Linux is exactly the configuration that lied on `cpal 0.18.1`. The graph held; Phase 0 predicted it, Windows CI confirmed it. Local Windows release build: `Compiling enigo v0.6.1` → `Finished` with zero warnings.

**Merged:** `16a9eaf` — `feat: auto-paste into the active window (Dictation PR4) (#41)`. 12 files, +1643/−190.

---

## Runtime QA — first full pass (Windows 10)

The hard gate. Of 26 points, **21 executed and passed**, 2 deferred to PR5 (need UI), 2 skipped by environment, and **2 corrected the kickoff's description of what would happen**.

### The four load-bearing points (all ✅)

| # | What it verifies | Result |
|---|---|---|
| **23** | `pasted` → «Вставлено» (string half, compiler-blind) | ✅ pill read «Распознаю → Вставлено»; log `outcome=Pasted` |
| **19** | D5 defensive modifier release — paste with Shift physically held | ✅ text inserted cleanly, no `Ctrl+Shift+V` corruption. The composition mine has no presence on hardware |
| **8** | Elevated window (UIPI) | ✅ **with a corrected diagnosis** — see below |
| **21** | Non-text clipboard → restore skipped | ✅ `WARN … рестор пропущен: снапшот не-текст`, image lost deliberately |

### Correction #1 — the elevated-window barrier is on the hotkey, not the paste

The kickoff framed #8 as UIPI silently dropping the synthetic paste (`pasted` reported, no text). Hardware showed a **different, earlier barrier**. With an elevated window active, the log reads:

```
duration_ms=0 rms=0.0 ... discarded before transcription reason=TooShort
```

Dictation **starts** but records **zero audio** — the hotkey's `Released` fires instantly. `WM_HOTKEY` is not fully delivered to the unelevated process while a higher-integrity window is focused. So dictation never reaches the `SendInput` stage; there is nothing to paste. The transcript is not lost because it never existed. Confirmed from the opposite side: an elevated PowerShell **behind** an active Notepad dictates fine — it is the **active** elevated window that blocks, and it blocks at input, not output.

**For USER_GUIDE (PR5): "cannot dictate into an admin window because the hotkey doesn't reach us," not "the paste is dropped by UIPI."** The user-visible result is the same; the cause and the wording differ.

### Correction #2 — Win10 overlay transparency

QA #17: on Win10 the overlay has no Acrylic/Mica compositor path; transparency rides Tauri's layered window, which is what was observed all pass — pill readable, no black box. Nothing further to verify.

### Passed conventionally

| # | Point | # | Point |
|---|---|---|---|
| 1 | STT key check | 12 | Works from tray |
| 2 | Focus not stolen | 13 | No key/net/mic → clean Russian toast |
| 4 | Chrome (browser field) | 16 | Win+V captures the transcript |
| 5 | Word | 18 | End-to-end with real AITunnel key |
| 6 | Telegram | 20 | Text clipboard → prior content restored (`ЯКОРЬ`) |
| 7 | Terminal (non-elevated) | 24 | Release log clean: `stt` lines present, no foreign crates, no transcripts/keys |
| 9 | Sub-300 ms tap → nothing, no API spend | 25 | `live_mic` prints the winning config |
| 10 | >60 s → truncated, result visible | | |
| 11 | Re-press during "Распознаю…" → ignored | | |

### #26 (TTS regression) — passed in the available half, with an explicit boundary

PR4 touched shared code (`pipeline.rs`, `logging.rs`, `mod.rs`/`rms_iter`), so #26 exists to prove dictation's PR did not break the core TTS product. Full audio output could **not** be produced — Sber removed the SaluteSpeech free tier; the log shows the request reaching Sber and being refused there:

```
salute::auth: using cached SaluteSpeech token
salute::synthesize: synthesize request voice=Boris text_len=138
salute::synthesize: non-success ... status=402 Payment Required
```

This **disentangles two facts**: "TTS doesn't speak" = "Sber turned off the balance," not "PR4 regressed." The code runs the full path (cached token → synth request → provider). `rms_iter` is intact (dictation RMS is live in the logs). And the `402` is caught loudly — logged with `status` and `body_len`, `WARN` — the same "loud failure, not silent" principle as #13. **#26 counted as passed in the provable half; full audio blocked by a cause outside PR4 and outside our code.**

### Skipped / deferred

- **#22** (clipboard-manager overwrite) — no manager installed; the only live proof of race #2 (`VerifyOwn`) stays with the hostile unit test `d9_buffer_changed_before_restore_cancels_restore` and its negative cycle. Acceptable.
- **#14, #15** (mode radio, history opt-in) — genuinely impossible before PR5's UI. The plan anticipated two QA passes.

### D7 timing-rider — first real numbers

Six `outcome=Pasted` insertions across the pass: **313, 328, 308, 319, 312, 313 ms** — all in a 308–328 band. `Settle=300 ms` dominates; the chord + release + restore around it cost ~10–30 ms. This is the data the rider was built to collect, and it directly informs the post-merge observation #2 (whether to move `Settle` out of the non-text branch). Trend is flat and tight; formal decision still waits a week of real use, per the rider's own logic.

**Hardware footnote:** the mic reports native 16 kHz (`native_sample_rate=16000`), the STT target rate — so on this machine the resampler never engages and the flush ladder is never loaded. D16 was closed by calculation; the hardware confirms its worst case isn't even reachable here.

---

## Post-merge observations (4) + the Settle question — addressed to PR5, not "next time"

Per D15, "at the next touch" is a condition, not an address — and PR5 (Dictation page, history, radio, device picker) has no reason to open `insert.rs`. So these get a **named phase with a gate**, not a wish.

| # | Item | Address |
|---|---|---|
| 1 | `release_modifiers` omits Control **intentionally** (the paste chord presses it itself) — needs a one-line comment `// Control absent by design — the paste chord presses and releases it itself (D4)`. Risk is reader confusion, not breakage (an extra Ctrl release is harmless). | PR5 carry-over phase |
| 2 | `Settle(300 ms)` runs even in the non-text branch, where no restore follows → ~300 ms of latency for nothing. CC's fix — move `Settle` inside `if let Ok(prior)` in the planner — is precise: the non-text case is knowable **at planning time** (unlike buffer-changed, known only post-`VerifyOwn`), so the one branch that can save is the one expressible in the pure planner. **The 300 ms number itself is NOT touched — the rider is collecting data on it.** | PR5 carry-over phase |
| 2b | Open question: does the executor also `Settle` when `paste` returns `Err`? If so, that's 300 ms of latency on the failure path. **Needs CC answer first.** | PR5 kickoff Phase 0 |
| 3 | `VerifyOwn` uses exact string compare → a CRLF-normalising clipboard manager gives a false "buffer changed" → restore cancelled. Biases safe (never clobber a third party). **Not a code change** — a known-limitation line in USER_GUIDE, in the **same paragraph as the image-loss limitation** (to the user both read as "the prior clipboard sometimes isn't restored"). | PR5 USER_GUIDE |

---

## Off-PR: TTS provider migration (Sber balance) — pricing scouted, decision pre-made

Sber removed the SaluteSpeech free tier (~1500 ₽ to top up). Migration to another TTS provider is its own mini-task, not PR4/PR5. Scouted today so it isn't re-researched:

| Provider / model | $/1M chars | ≈ ₽/1M chars | Russian? |
|---|---|---|---|
| **aitunnel GPT-4o mini TTS** | — | **120 ₽** | ✅ |
| Grok TTS (xAI) | $15 | ~1350 ₽ | ✅ (20+ langs) |
| Groq Orpheus English | $22 | ~1980 ₽ | ❌ English only |
| Groq Orpheus Saudi Arabic | $40 | ~3600 ₽ | ❌ Arabic |
| OpenAI TTS (direct) | ~$30 | ~2700 ₽ | ✅ |
| ElevenLabs | ~$50 | ~4500 ₽ | ✅ |

**Groq is out — not on price, on language:** GroqCloud's only two TTS models are Orpheus English and Orpheus Saudi Arabic (they replaced PlayAI in Jan 2026). No Russian. A Russian-TTS app can't use it regardless of the ~16× price gap vs aitunnel.

**Decision (pre-made): stay on aitunnel GPT-4o mini TTS at 120 ₽/1M.** Cheapest by an order of magnitude, has Russian, and it's the **same provider already serving STT** (`whisper-large-v3-turbo`) — one provider for STT+TTS means one keyring secret, one auth path, one failure point. Groq would only make sense for STT (Whisper v3 Turbo at $0.04/audio-hour), but aitunnel-whisper already works, so nothing to change.

**Caveat on Groq as a vendor** (for a future STT decision, not TTS): GroqCloud is under a new CEO after the LPU architects moved to NVIDIA; whether the platform keeps developing or fades is currently unclear. Standard advice: don't build a single-vendor dependency.

---

## Icons & tray — spec locked for PR5

Custom app icon and tray icons enter in PR5. Decisions locked today:

- **App logo: indigo plaque.** Ⰳ (Glagolitic U+2C03) on an indigo plaque, light contour. The plaque is the product's face — at large sizes (Start menu, taskbar, About window) the colour separates Glagol from the monochrome mass of other icons.
- **Indigo hex: `#3B3A6A`** — one token, for both the logo plaque and (likely) the Dictation-page accent in the PR5 UI.
- **Tray: coloured sign, no plaque, no theme variants.** The tray already runs an indigo sign today that turns red during recording, and it works. A **coloured** sign carries its own readability on any taskbar (light Win10, dark Win11) without repainting to match the panel — which is why the earlier "monochrome, two theme-dependent versions" idea was dropped. This is simpler and more robust than theme-following.
- **Tray states: idle = indigo, rec = red (whole sign recolours).** No red-dot badge — the recording dot lives in the pill; a dot on a 16 px tray icon would be pixel grime. Rec is the entire glyph going red, exactly as drawn in the tray concept.
- **Glyph is an outline path, NOT a font glyph.** The Glagolitic Ⰳ must be baked as an SVG outline so it renders as designed on a machine with no Glagolitic font installed (a `<text>U+2C03</text>` would fall back to `☐`). Same class as "never `Key::Unicode`" in D4 — do not rely on something the target system may lack. The maintainer's existing contour path is the source; CC does not generate from a font.

**SVG source set (CC generates ICO/PNG from these):** `logo.svg` (Ⰳ + `#3B3A6A` plaque + light contour), `tray-idle.svg` (indigo sign, no plaque), `tray-rec.svg` (red sign). Sizes: logo 16–512 (ICO + PNG), tray 16 + 32.

**Phase 0 check (low probability, not a redesign):** verify `#3B3A6A` reads at 16 px on a very dark Win11 taskbar (dark-on-dark). It works today, so likely fine — but if it dims, add a 1 px light outline (invisible on a light panel, a contour on a dark one), a 5-minute SVG edit, **not** an indigo change.

---

## Carry-over registry

| → | Item | Origin |
|---|---|---|
| **PR5** | `release_modifiers` — one-line comment: Control omitted by design (D4) | Day 11 post-merge |
| **PR5** | Move `Settle` inside the `if let Ok(prior)` branch (non-text path skips it); **300 ms value untouched** — rider collecting | Day 11 post-merge |
| **PR5 Phase 0** | Does the executor `Settle` on `paste` `Err`? (latency on failure path) | Day 11 post-merge |
| **PR5 USER_GUIDE** | Known limitation: CRLF-normalising clipboard manager → false "buffer changed" → prior clipboard not restored. Same paragraph as image-loss. | Day 11 post-merge |
| **PR5 USER_GUIDE** | Elevated-window: cannot dictate into an admin window — **hotkey doesn't reach us** (not "UIPI drops the paste") | Day 11 QA correction |
| **PR5 USER_GUIDE** | Known limitation: non-text clipboard content (image/files) is lost on auto-paste | Day 11 (D7) |
| **PR5** | **D8 amendments by data:** ceiling 0.01 → **0.007**; relaxation criterion needs a floor | Day 10 calibration |
| **PR5** | USER_GUIDE: close Glagol before sending logs (WorkerGuard flushes on drop) | Day 10 |
| **PR5** | USER_GUIDE: absolute threshold degrades in loud ambient | Day 10 calibration |
| **PR5** | Whisper `prompt` vocabulary hint ("Глагол", "Привезём"); number normalisation inconsistent | Day 10 QA |
| **PR5** | Two OAuth requests at startup + cache race in `salute::auth` (seen again Day 11: two refreshes in one minute with no dictation) | Day 10 logger / Day 11 QA |
| **PR5** | **Revisit D10** — device names generic (`Capture Input terminal`); silent first-match collision risk | Day 9 QA |
| **PR5 (decision)** | `Type` (character-by-character) insertion mode — needs explicit product yes + line-42 plan amendment + implementer + radio, one package | Day 11 kickoff |
| **PR5** | **Custom icons + tray** — logo (Ⰳ on `#3B3A6A` indigo plaque), tray idle (indigo sign) / rec (red sign), no plaque, no theme variants. Glyph = outline path, not font glyph. 3 SVG sources → CC generates ICO/PNG. See "Icons & tray" section. | Day 11 |
| **PR5 Phase 0** | Verify `#3B3A6A` reads at 16 px on dark Win11 taskbar; if not, add 1 px light outline (not an indigo change) | Day 11 |
| **off-PR** | TTS provider migration off Sber → **aitunnel GPT-4o mini TTS (120 ₽/1M)**, pricing scouted | Day 11 |
| **post-MVP** | RMS mean weakens with clip length → percentile/peak | Day 10 calibration |
| **post-MVP** | Adaptive threshold / SNR instead of absolute RMS | Day 10 calibration |
| **unresolved** | **D5 (multi-monitor) not verified** — needs a second monitor | Day 10 QA |
| **note** | `logging.rs` excluded from its own scan — documented hole | Day 10 review |
| **note** | Scan will one day false-positive on `get_setting(key)`; use `setting_name` for `app_settings` keys | Day 10 review |
| **note** | Kickoff files (PR2/PR3/PR4) are **internal-only, not committed** to the repo — no history gap to close, logs are self-contained | Day 11 |

---

## Stats — Day 11

| Metric | Value |
|---|---|
| PRs merged | 1 (#41 PR4) |
| Tests | 277 → 303 (#41 feat) → **307** (#41 follow-up), +1 ignored |
| Regressions | **0** |
| Hotfixes | **0** (contrast: PR3 needed #40) |
| New dependencies | `enigo 0.6.1` (`=`-pinned, unconditional), pulling `windows 0.61.3` (already unified), `memmap2`, `nom`, `xkbcommon` |
| Migrations | **none** (`stt_insertion_mode` is a string in existing `app_settings`) |
| New `unsafe` | none |
| Negative cycles | 2 (VerifyOwn, ReleaseModifiers), both with real failing traces |
| Composition mines found at kickoff | 1 (D5 — modifier release) |
| Process failures banked as rule | 1 (D15 — carry-overs need a phase) |
| Kickoff-round edit tally | 4 maintainer → author, 3 author → maintainer, 1 retraction |
| QA points | 21 passed / 2 deferred (PR5 UI) / 2 skipped (env) / 2 kickoff descriptions corrected |
| D7 timing samples | 6 (308–328 ms, Settle-dominated) |

---

## Lessons — Day 11

1. **Two correct documents can hide a defect that neither shows alone.** The plan said "paste after the hotkey"; Phase 0 said the hotkey polls Space only. Each was right; the gap lived in their composition (D5). Read the pair, not the page. Same class as U8 and the D7 hole — and, like both, found by neither original author.

2. **Symmetric review beats one-directional review.** For the first time the edits ran both ways, and one of the most valuable moves was the maintainer catching **his own** over-statement — a proposal he'd remembered as an approval. The instrument that caught it was mutual challenge, not either party's confidence.

3. **A carry-over without a phase is a wish.** PR3 proved it by losing three of four; PR4 fixed it by giving each an addressed phase with a gate, and all four shipped. "At the next touch" is not an address — the next touch may never come.

4. **Hardware corrects reasonable descriptions.** The kickoff's UIPI story for elevated windows was sound and wrong: the real barrier is earlier (the hotkey), not later (the paste). "Founders know what they built; analysts see descriptions" (Day 9) applies to our own kickoffs too. The QA didn't just check boxes — it rewrote two of them.

5. **A side-effect done right pays off two steps later.** Raising "buffer changed" from `debug` to `warn` (Q3, yesterday) was what made the non-text-restore branch visible on a stock release build during QA today. The rider's data (308–328 ms) reached the log for the same reason. Observability decisions compound.

6. **Rule out on the most specific fact first.** Groq TTS was ruled out not on its ~16× price gap but on language — it has no Russian at all. The cheapest wrong tool is still the wrong tool; the deciding fact was the one most specific to the product.

---

## What's next

**PR5 — `feat/dictation-page`:** the Dictation settings page, history (opt-in), device picker, configurable hotkey, the insertion-mode radio, **custom app icon + tray icons** (indigo logo, indigo/red tray, spec locked above), D8 calibration amendments, and USER_GUIDE (with the three known limitations: image-loss, CRLF-restore, elevated-window). Carry-over phase with a gate for the four post-merge observations. Then the remaining QA points (#14, #15, #22 if a manager is installed), the second full QA pass, and **the marathon tag**.

**Off-PR, separately:** TTS migration from Sber to aitunnel GPT-4o mini TTS.

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #41 (Dictation PR4):** `feat: auto-paste into the active window` · merged · `16a9eaf` · 12 files, +1643/−190 · 277 → 307
- **Phase 0 findings:** `enigo 0.6.1` → `windows 0.61.3` (unified, no split); `Key::Other(0x56)` → raw VK; `MAX_FLUSH_ROUNDS` worst tail 68 frames / 15× margin (D16 closed)
- **Release log path:** `%LOCALAPPDATA%\app.glagol.desktop\logs\glagol.<date>.log`
- **Release log directive (D14):** `warn,glagol_lib=info,glagol_lib::dictation=debug,glagol_lib::stt=debug`
- **PR4 kickoff (D1–D16):** internal-only, not committed
- **Marathon plan:** `PlanofRUWhisper.md`
- **`main` HEAD at Day 11 close:** `16a9eaf`, working tree clean
- **STT in use:** aitunnel `whisper-large-v3-turbo`, lang=ru
- **TTS decision:** aitunnel GPT-4o mini TTS (120 ₽/1M) — migration off Sber pending
