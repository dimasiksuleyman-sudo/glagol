# Glagol — Day 12 Master Log

**Period:** July 18, 2026 (Sprint 6 Dictation marathon — PR5a kickoff Q&A through PR5a merge)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 6 — Dictation — **PR5a of PR5 (a/b split)**
**Status at end of Day 12:** PR1–PR4 + **PR5a** merged. **340 tests passing, 1 ignored, 0 regressions, 0 hotfixes.** The dictation backend — migration v4, the six-command contract, history/privacy plumbing — is in `main` and frozen. UI (PR5b) is the last piece before the marathon tag.

> Day 12 was the shortest PR-day of the marathon: one backend PR, no manual QA (nothing to QA — no UI). But it produced three lessons that matter more than the code, all from the same habit — **refusing the convenient answer in favour of the exact one.** Twice the maintainer corrected himself; once he corrected me; once we were both wrong in different directions and the real toolchain settled it.

---

## TL;DR

1. **PR5 was split a/b along the axis of verification (Q1-B).** Backend (migration, commands) is machine-tested and platform-agnostic; UI needs the second 26-point manual pass on Windows. PR5a merges and **freezes the command contract** so PR5b's kickoff references frozen commands, not a moving target. This is the first multi-PR split in the marathon, and the reason is not diff size — it's that the two halves are proved differently.

2. **`id DESC` in the prune is defensive-SQL, not a removal-provable guard — and the maintainer proved it against himself.** He added `created_at DESC, id DESC` as a tie-breaker (a genuine improvement over the kickoff's `created_at DESC`), then ran the removal cycle — dropped `id DESC` — and the test **still passed**. Cause: SQLite appends rowid to the index, so a reverse scan of `idx_dictations_created_at` returns equal keys in rowid-DESC order, matching `id DESC` by engine accident. He did **not** hide this, and did **not** build a contrived table-scan test to force the cycle red — that would test SQLite, not our code. The functional invariant I asked for (list↔prune agree on equal timestamps) **is** guaranteed by the test; `id DESC` itself is insurance at zero cost, honestly labelled insurance.

3. **The `pnpm tsc` "blocker" was an environment artifact, and both of us were wrong about it in different ways.** The red was `TS5101 baseUrl deprecated` — but only because a stray global tsc (7.0-track) ran with `node_modules` absent. On the project's pinned TS 5.8.3, `baseUrl` isn't deprecated and tsc is green with **no** tsconfig change. The maintainer had dismissed it ("not my regression, flag separately"); I had escalated it ("blocker, edit tsconfig") and approved his `ignoreDeprecations: "6.0"` patch. Both wrong: he for waving it off, I for prescribing a fix for a non-existent bug. He caught it by not executing my approved edit blindly — he verified on the pinned toolchain and reverted his own commit when green appeared without it.

4. **PR #42 merged — backend, 307 → 340 (+33), zero regressions, zero hotfixes.** Both negative cycles (D5 prune, D4 history gating) run to failure and restored. Phase 0 closed D7 with a good result: hotkey conflicts **are** detectable (`global-shortcut 2.3.2` returns `Err` on an occupied combo), so the rollback branch is real, not just format-validation.

5. **History privacy is enforced by test, not by promise (D4).** Opt-in off = the transcript never touches disk; the seconds counter still increments (aggregate, no text). The two are decoupled in code and each has its own test. This is the ФЗ-152 habit from Привезём applied to Glagol.

6. **The command contract is frozen at six.** PR5b's kickoff sits on these; a seventh command would be a contract extension, a separate decision.

---

## Sprint 6 — where the marathon stands

```
PR1  feat/stt-client               ✅ #35 + #36
PR2  feat/dictation-recorder       ✅ #37 + #38
PR3  feat/dictation-hotkey-overlay ✅ #39 + #40
PR4  feat/dictation-paste          ✅ #41
PR5a feat/dictation-page-backend   ✅ #42            ← merged today
PR5b feat/dictation-page              ← next, the final PR
                                      tag after PR5b + 2nd QA pass
```

---

## Kickoff Q&A — 8 questions, closed

Full decision set D1–D8 locked before Phase 0. Notable:

- **Q1-B (a/b split)** — the maintainer initially wrote "1в" (= C, three PRs) but his prose described a two-part backend/frontend division; the discrepancy was surfaced and resolved to B (two PRs). A typo caught before it set the whole PR structure wrong.
- **Q2 — `Type` mode NOT taken**, post-MVP, plan line 42 stands. This was the last chance to add it to the marathon; declined deliberately (a full feature for a rare case — apps that ignore Ctrl+V — is a schedule risk in the final PR).
- **Q6 — threshold 0.005 not moved.** Data had accumulated <24h; centring on that would repeat the n=1 error that cost three rounds. Ceiling → 0.007 (Day 10 measurement, in PR5b); centring → post-MVP.
- **Q7 — observation 2b struck by source, not investigated.** The maintainer had read the PR4 source at review time: the `Paste` step early-returns `ClipboardOnly` on error before `Settle`/`VerifyOwn`/`Restore` ("Every step after Paste is paste-dependent"). No latency on the failure path. Removed from Phase 0 entirely — a question closed by having already read the code.

---

## PR #42 — dictation backend

**307 → 340 tests (+33 total: +32 feat, +1 tie-breaker).** Squash-merged `ddedc56`. DB now `user_version = 4` — the `dictations` table + index, the first new table since PR1.

### Phase 0 findings

- **D7 hotkey conflict is detectable.** `Shortcut::from_str` validates format (pure, testable); `global-shortcut 2.3.2`'s `register` returns `Err` on an occupied combo. So the rollback branch (re-register the old hotkey, return a Russian error) is real — the user is never left with no hotkey. The global `.with_handler` fires for any registered shortcut, so re-registration needs no new handler.
- **Migration v3→v4 is incremental and preserving** — proven by a `to_version(3) → seed → to_latest` test that asserts prior rows survive.

### Negative regression cycles (2, both real traces)

1. **D5 prune** — removed the `DELETE` → `insert_dictation_prunes_to_cap_dropping_oldest` failed `left: 201, right: 200` → restored → green.
2. **D4 gating** — removed the `history_enabled` gate → `record_dictation_history_gated_by_toggle` failed ("history off must not write…") → restored → green.

### Deviations from spec (accepted)

- **History write in `run_session` via an enriched `DictationOutcome`**, not literally inside `run_dictation` — exactly where `record_recognition_usage` lives, keeping `run_dictation` DB-free and unit-testable. Cleaner seam, faithful to D4 intent.
- **`get_recognitions_minutes` = lifetime total** (SUM over all months), not current-month per plan line 84. STT has no monthly free-tier reset (unlike TTS chars), so a monotonic total is the honest figure. Maintainer-confirmed. → **PR5b label must read "Надиктовано всего", not "в этом месяце", or the number lies about its period.**
- **Error rows written** (`status='error'`, empty text, `duration_ms=0`) when history is on — gives `error_message` a purpose, honours the D2 table.
- **Startup registers the saved hotkey** (default fallback) — so a hotkey changed via Settings survives restart. Needed for D7 to be coherent.
- **`set_dictation_setting` whitelists keys and excludes `dictation_hotkey`** — not in the kickoff, but closes a hole: without it PR5b could write the hotkey via the raw setter, landing the value in the DB without calling `register`, so the hotkey would only change after restart. A silent setting-vs-reality desync. Tested (`set_dictation_setting_rejects_hotkey_and_unknown_keys`).

### Gates (all green, real toolchain)

`cargo check` · `cargo test` 340 pass / 1 ignored / 0 fail · `clippy` debug + release · `fmt --check` · `pnpm tsc --noEmit` (pinned TS 5.8.3, exit 0). No new dependencies, no new `unsafe`, first new table since PR1.

**Merged:** `ddedc56`.

---

## The three lessons — each from refusing the convenient answer

### 1. `id DESC` — insurance, not a guard, and the maintainer said so unprompted

The tie-breaker `created_at DESC, id DESC` is a real improvement (equal `created_at` in ms is reachable during fast dictation; without a secondary key, prune could drop the wrong row). But its **status** was tested honestly:

- **What the test guarantees:** list↔prune agree on which row is newest at equal timestamps. `equal_created_at_uses_id_desc_tiebreaker_consistently_in_list_and_prune` holds this. This is the functional invariant that was asked for. ✅
- **What `id DESC` is NOT:** removal-provable on this engine+schema. Dropping it from both `ORDER BY` left the test green — SQLite's index carries rowid, so equal keys already come back in rowid-DESC. The order matches `id DESC` by engine accident.
- **The honest classification:** `id DESC` is defensive-SQL — it specifies the order by standard rather than relying on what the planner happens to emit today. Drop the index or force a full-scan+sort, and without `id DESC` the tie-break order would be undefined. Kept as zero-cost insurance, **labelled insurance, not a defender that a removal cycle proves.**
- **What was deliberately NOT done:** a contrived `PRAGMA`/table-scan test to force the removal cycle red. That would test SQLite, not our code — guard theatre. Declined.

**My part of this:** my original request assumed `id DESC` was removal-provable and asked for a test "proving the tie-breaker." That model was wrong for this engine. The maintainer fulfilled the **intent** (consistency) and corrected the **model** — third instance of "he has the reality (a run on the engine), I have the description, the reality corrected the description."

### 2. Red gate → check the environment before the code

The `TS5101` red was not a real deprecation. A stray global tsc (7.0-track) ran because `node_modules` wasn't installed; on the pinned TS 5.8.3 `baseUrl` is fine and tsc is green with no change. **Both of us were wrong:** the maintainer dismissed it as someone else's problem; I escalated it to a blocker and approved an `ignoreDeprecations: "6.0"` patch that would have masked a non-problem on the real toolchain. The catch came from **not executing an approved edit blindly** — verifying on the pinned toolchain and reverting the commit when green appeared without it. Had the edit shipped, the repo would carry a config flag for a bug that doesn't exist, and the next developer on a normal environment would wonder why. Same class as Day 11's "right decision, stale reasoning," now for a false alarm: **an approved recommendation does not excuse verification on the real toolchain.**

### 3. Guard theatre is worse than an honest coverage gap

Both lessons above share a spine: the convenient move was available and refused. A "negative cycle executed ✓" that's green-when-it-should-be-red, reported without investigation, is a lie by checkbox. A contrived test that forces red by testing the framework is a lie by ceremony. The marathon's whole test discipline — negative cycles that prove a guard is load-bearing — depends on telling those two apart from a real guard. Day 12 is the case where the tooling said "guard" and the truth was "insurance," and the difference was named rather than papered over.

---

## Carry-over registry (Day 12 additions + live PR5b queue)

| → | Item | Origin |
|---|---|---|
| **PR5b** | Label the minutes counter **"Надиктовано всего"** (lifetime), not "в этом месяце" | Day 12 (deviation) |
| **PR5b** | `id DESC` tie-breaker is insurance, not removal-provable — keep, but don't reclassify as a proven guard | Day 12 lesson |
| **PR5b / infra** | **CC environment cannot pull pdfium from GitHub Releases** (egress 403). Twice now: PR4 placeholder `PDFIUM_LIBRARY_PATH`, PR5a sourced `libpdfium.so` from pypdfium2 on PyPI (allowed host). A permanent local-env accommodation — document once so it isn't re-solved a third time. | Day 11 + Day 12 |
| **PR5b** | Custom icons: **trace prototype B** (maintainer's approved PNG) into clean vector paths — NOT draw from scratch (Claude's hand-built Bézier paths were too rough). Then logo (`#3B3A6A` plaque) / tray idle (indigo) / tray rec (red). Glyph = outline path. | Day 11 + Day 12 |
| **PR5b Phase 0** | Verify `#3B3A6A` reads at 16px on dark Win11 taskbar; if not, 1px light outline (not an indigo change) | Day 11 |
| **PR5b** | UI carry-overs from Day 11: `Control` comment in `release_modifiers` (D4/PR4); move `Settle` inside `if let Ok(prior)` — non-text branch only, 300ms untouched | Day 11 |
| **PR5b USER_GUIDE** | **All 7 known limitations** (Q8): non-text clipboard loss; CRLF-manager restore; elevated-window (hotkey doesn't reach us); absolute threshold in loud ambient; device-name collision; Win+V captures transcript; Ctrl+Shift+Space vs Office non-breaking space | marathon |
| **PR5b** | **D8 calibration:** ceiling 0.01 → **0.007**; relaxation criterion needs a floor (never below room noise + margin), not "halve" | Day 10 |
| **PR5b** | USER_GUIDE: close Glagol before sending logs (WorkerGuard flush); D5 multi-monitor still unverified (needs 2nd monitor) | Day 10 |
| **post-MVP** | `Type` (char-by-char) insertion mode — needs plan line-42 amendment + implementer + radio, one package. NOT in the marathon. | Day 11 |
| **post-MVP** | Threshold centring by distribution once ≥1 week of RMS data (было <24h at Day 12) | Day 12 |
| **post-MVP** | RMS mean weakens with clip length → percentile/peak; adaptive SNR threshold | Day 10 |
| **note** | Command contract **frozen at 6**: `get_dictation_settings`, `set_dictation_setting`, `set_dictation_hotkey`, `list_dictations`, `clear_dictation_history`, `get_recognitions_minutes`. A 7th = contract extension, separate decision. | Day 12 |
| **note** | Kickoffs (PR2–PR5a) internal-only, not committed | Day 11 |

---

## Stats — Day 12

| Metric | Value |
|---|---|
| PRs merged | 1 (#42 PR5a) |
| Tests | 307 → 339 (feat) → **340** (tie-breaker), +1 ignored |
| Regressions | **0** |
| Hotfixes | **0** |
| New dependencies | **none** |
| New `unsafe` | none |
| Migrations | **1** (v3→v4, `dictations` + index — first new table since PR1) |
| Negative cycles | 2 (D5 prune, D4 gating), both with real failing traces |
| Command contract | **frozen at 6** |
| Self-corrections | 2 (maintainer: "1в"→B typo; `id DESC` reclassified as insurance) |
| Cross-corrections | 1 (maintainer caught the tsc false-blocker after my wrong escalation) |
| Kickoff round | 8 questions, D1–D8 |

---

## Lessons — Day 12

1. **Refuse the convenient answer.** "Test added ✓" when the removal cycle is silently green is the convenient answer; "green where I expected red, here's why, and I won't fake it red" is the exact one. The whole day turned on choosing the second three times.

2. **An approved recommendation is not a licence to skip verification.** I approved a tsconfig patch for a bug that didn't exist. Executing it blindly would have shipped a mask for a phantom. It was caught by running the real toolchain, not by trusting the approval.

3. **Name insurance as insurance.** `id DESC` is worth keeping and is not what a removal cycle proves. Conflating "kept for safety" with "proven load-bearing" would quietly weaken the meaning of every real negative cycle in the repo.

4. **Freeze the contract before the consumer is written.** PR5b sits on six commands. Writing the UI kickoff before PR5a merged would have referenced a moving target; writing it after means the UI references frozen behaviour. The a/b split exists to make this freeze possible.

---

## What's next

**PR5b — `feat/dictation-page`** (the final PR): the Dictation settings page + route + menu, history list with the opt-in toggle, device picker, configurable hotkey, the minutes counter ("Надиктовано всего"), custom icons (trace prototype B), USER_GUIDE with all 7 known limitations, D8 threshold calibration (0.007 ceiling + floored relaxation criterion), and the UI carry-overs. Then the second full 26-point QA pass on Windows, and **the marathon tag.**

**Off-PR:** TTS migration Sber → aitunnel GPT-4o mini TTS (120 ₽/1M, scouted Day 11).

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #42 (Dictation PR5a):** `feat: dictation settings, history and device backend` · merged · `ddedc56` · 307 → 340
- **Phase 0:** D7 hotkey conflict detectable (`global-shortcut 2.3.2` `register` → `Err` on occupied); migration v3→v4 incremental + preserving
- **DB:** `user_version = 4`; new table `dictations` + `idx_dictations_created_at`
- **Frozen command contract (6):** `get_dictation_settings`, `set_dictation_setting` (hotkey excluded, whitelisted), `set_dictation_hotkey` (rollback on conflict), `list_dictations` (empty when history off), `clear_dictation_history`, `get_recognitions_minutes` (lifetime)
- **`main` HEAD at Day 12 close:** `ddedc56`, working tree clean, 340 tests
- **PR5b input:** 340 passed, 1 ignored, DB v4, `main = ddedc56`
- **STT:** aitunnel `whisper-large-v3-turbo`, lang=ru, now with proper-noun prompt hint
- **TTS decision:** aitunnel GPT-4o mini TTS (120 ₽/1M) — migration pending
