# Glagol — Day 12 Session 2 Master Log

**Period:** July 18, 2026 (Sprint 6 Dictation marathon — PR5b kickoff Q&A through merge, second QA pass, and **marathon close**)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 6 — Dictation — **PR5b of 5 — FINAL PR**
**Status at end of session:** PR1–PR5b merged. `main = e397ddb`, CI green on main. **344 tests passing, 1 ignored, 0 regressions, 0 hotfixes across all six PRs.** Second 26+1-point Windows QA passed. Dictation is a complete user feature. **Marathon ready to tag.**

> Day 12 Session 1 merged the backend (PR5a). Session 2 shipped the UI (PR5b), ran the second QA pass, and closes the marathon. This log carries both the session record **and** the Sprint 6 retrospective — see "Marathon close" at the end.

---

## TL;DR

1. **PR5b — the final PR — merged, 340 → 344, zero regressions, zero hotfixes.** The backend built across PR1–PR5a got its face: settings page, history, configurable hotkey, device picker, insertion-mode radio, lifetime counter. Icons deferred to a post-tag micro-PR (sanctioned escape hatch, Q7/D9 — no source PNG or tracing toolchain in CI).

2. **The D5 mine was found and killed in two halves during this PR's own review — a textbook "two decisions written apart."** PR5a's `list_dictations` gated **reads** on the history toggle ("off → empty"); PR5b's D5 decision wanted "off stops writing, accumulated stays visible." The same kickoff made both. Found by code-reading (not clicking): the backend read-gate AND a frontend `if (history_enabled)` around the event-refetch. Both un-gated; privacy now rests entirely on the **write** gate (D4). Resolution was an explicit maintainer decision — frozen contract vs D5 → D5 wins.

3. **The undead-half pattern repeated inside the fix itself.** Un-gating the mount fetch left the `dictation-state` `done` refetch still guarded (plus `history_enabled` in the effect deps). The maintainer caught it with a grep, not a guess, and the `[settings?.history_enabled]` dependency — not just the visible guard — was the real root. Symptom was invisible in tests (no JS unit-runner) and invisible in the UI (off → no new row either way), detectable only via the counter, which increments regardless of the toggle.

4. **The second QA pass closed what the first (post-PR4) couldn't reach.** №14 (change device/hotkey live), №15 (history opt-in in DB), and a new №27 (D5 remount+refetch, load-bearing, invisible to tests) — all now reachable because the UI exists. All passed on hardware.

5. **QA found an 8th known limitation the kickoff never anticipated — a single-key hotkey.** The maintainer assigned `=` as a one-key push-to-talk hotkey; capture accepted it, `Shortcut::from_str` validated it. But a single key registered globally is swallowed everywhere — `=` can no longer be typed in other apps while bound. **Decision: don't fix, document.** For this maintainer it's a deliberate trade (his keyboard can't macro three keys into one; one-key is physically simpler). Becomes limitation #8 in USER_GUIDE — a warning with a choice, not a bug.

6. **Marathon complete.** Six PRs, 344 tests, zero regressions, zero hotfixes, two Windows QA passes. Push-to-talk Russian dictation with no system VPN, from empty repo to shipped feature.

---

## Sprint 6 — final state

```
PR1  feat/stt-client               ✅ #35 + #36
PR2  feat/dictation-recorder       ✅ #37 + #38
PR3  feat/dictation-hotkey-overlay ✅ #39 + #40
PR4  feat/dictation-paste          ✅ #41
PR5a feat/dictation-page-backend   ✅ #42
PR5b feat/dictation-page           ✅ #43            ← merged this session
                                      TAG next (after this log)
```

---

## Kickoff Q&A — 10 questions, closed

Full decision set D1–D12 locked before Phase 0. Notable:

- **Q1 — monolith** (not split further); all UI/docs proved by one QA pass, splitting doubles manual work.
- **Q4 — history cap 200 → 10**, rewriting the merged PR5a D5. "Re-paste the last few," not an archive; 200 transcripts on disk contradict the privacy stance. Negative cycle preserved on the new `11→10` numbers.
- **Q5/Q6 → Phase 0:** hotkey capture in the webview (answer: Ctrl/Shift/Alt+key delivered, Meta/Win swallowed → capture + manual fallback); icon-source PNG resolution (answer: PNG not in repo, no tracing toolchain → deferred).
- **Q8 — threshold 0.005 NOT moved** (data still <24h+); ceiling → 0.007, relaxation criterion gets a floor. Centring is post-MVP.
- **Q10 — tag after the second QA pass**, not immediately after merge. Confirmed.

---

## PR #43 — the final PR

**340 → 344 tests (+4: 2 calibration, 1 settle carry-over, 1 cap-value guard; 1 test rewritten for the D5 un-gate).** 3 commits, 17 files, +1263/−33. Squash-merged `e397ddb`.

### Phase 0 findings

- **Hotkey capture (D7):** the Tauri webview reliably delivers Ctrl/Shift/Alt+key to `keydown`; the OS swallows Meta/Win and reserved chords → shipped capture **with** a manual-text fallback (auto-revealed after 4 s).
- **Icons (D9):** `icon_concepts.png` is **not in the repo** and no tracing toolchain is installable in CI (GitHub Releases egress-blocked). Per Q7/D9 the sanctioned escape hatch fires → **deferred to a post-tag micro-PR**. Tray keeps working on existing icons; nothing functional depends on branded icons.

### What landed

Page + route + Mic menu · 6 command bindings (lock-step, no new npm) · insertion-mode radio · history (cap 200→10, preview/expand/copy, toggle, clear) · hotkey capture editor + fallback · device picker with rollback display · D8 calibration (`SILENCE_RMS_CEILING = 0.007`, `SILENCE_RMS_FLOOR = 0.00396 + margin`, `relaxed_silence_threshold` replacing "halve it") · USER_GUIDE (7 limitations + WorkerGuard note, bilingual) · UI carry-overs (Control comment, Settle in non-text branch) · the D5 fix.

### The D5 mine — found and killed in review, both halves

**Root:** PR5a's `list_dictations_impl` returned empty when the history toggle was off — even for rows already on disk. This flatly contradicted the PR5b D5 decision ("toggle governs the future, «Очистить» the past"), which the *same kickoff* also made. The read-side gate meant accumulated history vanished on remount/restart; "visible until Clear" held only within one live session.

**Half 1 (`c9babde`):** dropped the backend early-return; `list_dictations_impl` delegates straight to the repo. Test rewritten `..._gated_by_history_toggle` → `..._reads_are_not_gated_by_history_toggle`. Mount fetch un-gated. Write gate untouched — off still writes nothing.

**Half 2 (`d35551d`):** the `dictation-state` `done` refetch **still** guarded `listDictations` on `settings?.history_enabled`, plus `[settings?.history_enabled]` in effect deps — "the same read-side gate in miniature." Un-gated; effect subscribes once (deps `[]`). Found by grep, not guess; the dependency was the real root, not just the visible guard.

**Why it was nearly invisible:** no JS unit-runner → no automated test possible; and with the toggle off, "list didn't refresh" and "refreshed but empty" look identical. The only visible indicator is the **counter**, which increments regardless of the toggle (D4, from `api_usage`). → became load-bearing QA step №27.

### Gates (all green, CI green on main)

`cargo test` 344 / 1 ignored · `clippy` debug + release · `fmt` · `pnpm tsc` (pinned 5.8.3) · `pnpm build` (no new npm). Bundle delta recorded not gated (D2): JS 446.00 → 469.64 kB (gzip 140.31 → 146.57). Windows CI green on `main` `e397ddb` — the authoritative 344-test Rust run the maintainer can't do locally.

**Negative cycle (D4):** removing the prune `DELETE` fails `insert_dictation_prunes_to_cap_dropping_oldest` at `11 vs 10`; restored → green.

---

## Second QA pass — 26 + №27 (Windows 10, release build)

The hard gate. The first pass (post-PR4) closed 21/26; the backend behaviour didn't change in PR5b, so this pass focused on **new UI + newly-reachable points**, not a full re-run.

### Newly reachable (UI now exists)

| # | Point | Result |
|---|---|---|
| №14 | Change device / hotkey **live** via the page | ✅ (+ finding — see #8 below) |
| №15 | History opt-in — off writes nothing, on writes, cap 10 | ✅ UI shows "2 записи (максимум 10)" |
| **№27** | **D5 remount + refetch (load-bearing, invisible to tests)** | ✅ both halves — see below |

**№27 in detail (the mine's on-hardware proof):** toggle on → dictate → row appears, counter grows. Toggle **off** → dictate → **no new row (D4), but counter grows in real time** (event-refetch un-gated, half 2). Leave page and return → prior rows still visible (half 1). «Очистить» → empty. Counter growing at both toggle positions is the single observable that proves half 2 — confirmed on hardware.

### New UI

Page opens from menu · insertion-mode radio flips `pasted`↔`clipboard` · device picker selects a non-default mic (works) · «Очистить» clears history, counter NOT reset (different stores — `dictations` vs `api_usage`) · counter "Надиктовано всего: 9 минут" (lifetime label, correct) · privacy copy renders verbatim ("тексты не касаются диска без необходимости…").

### Regression (PR5b touched shared code)

- Notepad «привет» → inserts. **D8 calibration (0.007 ceiling) did not break normal speech** — a short phrase isn't dropped as silence. ✅
- TTS — **deliberately not re-run.** The tract was proven intact Day 11 (`402 Payment Required` reaching Sber). Paying ~1000 ₽ to Sber for a regression checkmark is a bad trade when migration to aitunnel (pay-per-use) is the next quest. If a user reports it, fast-fix. ✅ (by prior proof)

### QA finding → 8th known limitation (don't fix, document)

The maintainer set `=` as a **single-key** hotkey. Capture accepted it; `Shortcut::from_str` validated it. But a single key registered globally is swallowed everywhere — `=` can't be typed in other apps while bound. **The kickoff's D7 assumed a modifier chord** (default `CmdOrCtrl+Shift+Space`) and never anticipated a bare key — a kickoff gap hardware found (the Day-9 pattern again).

**Decision: don't fix, document.** For this maintainer it's a deliberate trade — his keyboard can't macro three keys into one, so a single key is the only true one-key push-to-talk. USER_GUIDE limitation #8:

> **8. Одиночная клавиша как хоткей.** Хоткею можно назначить одну клавишу без модификаторов (например `=` или F-клавишу) — удобно для диктовки одним нажатием. Но такая клавиша перехватывается глобально: пока она назначена хоткеем, ввести её в других приложениях нельзя. Не назначайте буквы и цифры, которые печатаете часто; выбирайте редко используемую клавишу.

USER_GUIDE now carries **8** known limitations (was 7).

---

## Marathon close — Sprint 6 retrospective

Six PRs, empty dictation surface → shipped push-to-talk Russian dictation with no system VPN. The RU alternative to Wispr Flow.

### By the numbers

| Metric | Value |
|---|---|
| PRs | 6 (PR1, PR2, PR3, PR4, PR5a, PR5b) + 2 follow-ups (#36, #40) |
| Tests | baseline → **344** passing, 1 ignored |
| Regressions | **0** across all six PRs |
| Hotfixes | **0** (only PR3 needed a follow-up #40, pre-merge) |
| New `unsafe` | **0** — every temptation (WASAPI endpoint ID, UIPI detection, delayed clipboard rendering) declined |
| Windows QA passes | 2 (post-PR4, post-PR5b) |
| Composition mines found before/in review | D5/PR4 (modifier release), D7-hole/PR3, D5-history/PR5b — all "two decisions written apart," none found by the original author |
| Self-corrections banked | `Type` retracted, `id DESC` reclassified as insurance, tsc false-blocker, single-key finding, backtick-mangle |

### What the marathon proved about the method

1. **Pre-implementation Q&A catches production breakers before code exists** — Pdfium path baking, WAV header bug, the D5 modifier-release mine, the D7 hotkey hole. The kickoff protocol paid for itself repeatedly.

2. **Founders know what they built; analysts see descriptions** (Day 9) — proven in both directions this marathon. Hardware corrected the kickoff's UIPI story (barrier is on the hotkey), the `id DESC` model (insurance, not provable), and the single-key assumption. The maintainer corrected his own over-statements (`Type`) and mine (tsc). Reality beat description every time it was consulted.

3. **A carry-over without a phase is a wish** (D15) — PR3 lost 3 of 4 checkbox carry-overs; every PR after gave them addressed phases with gates, and they shipped.

4. **Name insurance as insurance; refuse guard theatre** (Day 12) — the `id DESC` episode. A negative cycle that's green-when-it-should-be-red, reported without investigation, would quietly weaken every real cycle in the repo.

5. **Loud failure over silent** — the through-line from Day 10. `402` logged with status, `UnsupportedConfig`, hotkey rollback, history-off writing nothing observably. The user always sees a reason.

### Post-tag queue (not blocking the tag)

| Item | Note |
|---|---|
| **Icons** | Trace prototype B → logo/idle/rec. Deferred micro-PR (Q7/D9). Needs `icon_concepts.png` committed + tracing toolchain (or Claude-in-chat SVG on a better source). |
| **TTS migration** | Sber → aitunnel GPT-4o mini TTS (120 ₽/1M, pay-per-use), scouted Day 11. The "next small quest." |
| **USER_GUIDE #8** | Single-key hotkey limitation — already drafted above, land with the icon PR or standalone. |
| **Threshold centring** | Post-MVP, once ≥1 week RMS data. |
| **`Type` mode** | Post-MVP, needs plan line-42 amendment. |
| **pdfium egress** | CC env can't pull pdfium from GitHub Releases (3× now: PR4, PR5a, PR5b — sourced from pypdfium2). Permanent local accommodation, documented. |
| **CC-asset gap (kickoff lesson)** | Assets for CC phases (like `icon_concepts.png`) must be committed to git or verified present in Phase 0 — "sent in chat ≠ available to CC." A kickoff-author miss this session. |

---

## Carry-over registry (session additions)

| → | Item | Origin |
|---|---|---|
| **post-tag** | USER_GUIDE limitation **#8** — single-key hotkey global capture | Day 12 S2 QA |
| **post-tag** | Icons: trace prototype B (needs PNG in repo + toolchain) | Day 11/12 |
| **next quest** | TTS → aitunnel GPT-4o mini TTS (120 ₽/1M) | Day 11 |
| **kickoff rule** | CC-phase assets must be in git / Phase-0-verified; "sent in chat ≠ in repo" | Day 12 S2 |
| **post-MVP** | Threshold centring (≥1wk data); `Type` mode (line-42 amend); adaptive SNR / percentile RMS | Day 10/11 |
| **note** | Command contract frozen at 6; D5 un-gate changed `list_dictations` behaviour by explicit decision (frozen vs D5 → D5) | Day 12 S1/S2 |
| **note** | Kickoffs (PR2–PR5b) internal-only, not committed | Day 11 |

---

## Lessons — Day 12 Session 2

1. **Two decisions in one kickoff can still contradict each other.** D5 (history visible when off) and the PR5a read-gate (empty when off) were both decided by the same process, and collided. Making both decisions correctly is not the same as composing them. Found in review, not by the author.

2. **Grep, don't guess, and read the dependency array.** The undead half of the D5 fix wasn't the visible `if` — it was `[settings?.history_enabled]` in the effect deps. A visual scan of the guard would have missed it; grepping every reference found it.

3. **A QA finding can be a feature, a bug, or a deliberate trade — name which.** The single-key hotkey is all three depending on the user. Documenting it as a warning-with-a-choice respects the maintainer's trade without hiding the cost from the next user.

4. **Know when NOT to spend the test.** TTS regression was skipped by paying attention to prior proof (Day 11's `402`) instead of ~1000 ₽ to Sber. The discipline that avoids guard theatre also avoids ceremony theatre.

---

## What's next

**Immediate:** tag the marathon on `e397ddb` (after this log). Suggested `v0.2.0-dictation` or per the maintainer's scheme (last tag was `v0.1.0-rc.7`).

**Post-tag quests:** icons micro-PR · TTS migration to aitunnel · USER_GUIDE #8 · (later) threshold centring, `Type` mode.

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #43 (Dictation PR5b):** `feat: dictation settings page, history, and guide` · merged · `e397ddb` · 340 → 344
- **Phase 0:** hotkey capture (Ctrl/Shift/Alt delivered, Meta/Win swallowed → capture+fallback); icons deferred (no PNG/toolchain in CI)
- **D5 fix:** `c9babde` (backend + mount un-gate) + `d35551d` (event-refetch un-gate)
- **D8 calibration:** `SILENCE_RMS_CEILING = 0.007`, `SILENCE_RMS_FLOOR = 0.00396 + margin`
- **`main` HEAD:** `e397ddb`, CI green, 344 tests, working tree clean
- **DB:** `user_version = 4` (unchanged from PR5a)
- **USER_GUIDE:** 8 known limitations (bilingual RU+EN)
- **STT:** aitunnel `whisper-large-v3-turbo`, lang=ru, proper-noun prompt hint
- **TTS:** aitunnel GPT-4o mini TTS (120 ₽/1M) — migration is the next quest
- **Marathon:** 6 PRs, 344 tests, 0 regressions, 0 hotfixes, 2 Windows QA passes
