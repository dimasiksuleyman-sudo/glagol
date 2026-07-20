# Glagol — Day 12 Session 3 Master Log

**Period:** July 19, 2026 (post-marathon — v0.2.1 single-instance patch, docs skill, competitive scouting)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Context:** Sprint 6 Dictation marathon closed (tag `v0.2.0`, session 2). This session is **post-marathon maintenance**: the first bug found in production, a docs-editing skill, and competitive/positioning research toward a Habr post.
**Status at end of session:** **v0.2.1 shipped** — `main = 1a5cb4f`, tag `v0.2.1` on the merge commit, release published (Latest, verified signature, `Glagol_0.2.1_x64-setup.exe`). 344 tests, 0 regressions, 0 hotfixes. The single-instance fix is verified on hardware.

> Sessions 1–2 were the marathon's last two PRs (PR5a backend, PR5b UI + tag). Session 3 is what comes *after* shipping: the maintainer used v0.2.0 in real work, found a real bug (multiple instances), and it was fixed and released through the full protocol — no shortcuts on a post-release patch. Notably, the maintainer dictated his bug reports and confirmations **through Glagol itself** — the product testing itself in the author's real usage.

---

## TL;DR

1. **First production bug, found by the author using his own product: multiple instances.** Glagol could launch several copies — each in the tray and Task Manager, each racing to grab the global dictation hotkey. The whole feature silently assumed one process; nothing enforced it. Same class as every composition mine in the marathon — a requirement (one owner of hotkey/DB/log) that no mechanism guaranteed.

2. **Fixed via `tauri-plugin-single-instance`, shipped as v0.2.1 — through the full protocol.** Mini-kickoff → Phase 0 (read code, verify graph, take current version) → PR #45 → sanity → green Windows CI → merge → build → hardware QA → tag → release. A post-release patch got the same discipline as a marathon PR.

3. **CC did three things right where it could have cut corners** — took `=2.4.3` (kickoff's `2.3.6` was stale, verified on crates.io per the freshness convention), ran `cargo tree` (no `windows` split — the cpal-0.18 class check), and **reused the existing `show_main_window` helper** instead of duplicating show/unminimize/focus logic. The last is a composition-mine avoidance: one place shows the window, not two that drift.

4. **The pdfium mine finally bit the maintainer's own machine — and it's now higher-priority.** `build.rs` downloads pdfium from GitHub Releases at compile time; `env!("PDFIUM_LIBRARY_PATH")` is a hard compile-time requirement. v0.2.0 built only because pdfium was cached. This session `curl` failed (code 35, SSL/connect — GitHub Releases unreachable from the maintainer's network), and the release build **died**. Worked around by downloading pdfium via browser + setting the env var. But this is no longer a CI nuisance — **it can block a release build**. Fix (post-v0.2.1): `env!` → `option_env!` (degrade PDF gracefully) or cache pdfium in-repo.

5. **QA confirmed BOTH halves of the fix on hardware.** Not just "one instance" (the lock) but "the running window surfaces from the tray on a second launch" (the D2 callback). The second half is invisible if you only check the process count — it's the reused `show_main_window`. Both verified: one process, one tray icon, window becomes active, hotkey intact.

6. **Competitive scouting: the niche is real and empty in its intersection.** RU market has cloud dictation (Диктуй — same whisper-large-v3-turbo, closed; SpeakFlow; Talkpad), meeting transcription (Таймлист, on-prem), and enterprise APIs (SpeechKit Hybrid). None combines **ready push-to-talk + local/self-hosted + open source**. That intersection is Glagol's.

---

## v0.2.1 — single-instance fix

**Bug:** launching Glagol's shortcut a second time spawned another copy; each copy registered `Ctrl+Shift+Space` on startup and wrote to one SQLite DB + one rolling log. Second copy either failed to grab the hotkey or stole it from the first.

### Mini-kickoff → Phase 0 findings

- **Builder order:** was `opener → dialog → global-shortcut → .setup()`. Single-instance inserted **first** — must run before `global-shortcut` so a second launch is intercepted before it tries to claim the hotkey. (Tauri docs require single-instance first; here it's load-bearing, not ceremony.)
- **Reused helper:** an existing private `show_main_window()` (session.rs) already did `show()` + `unminimize()` + `set_focus()`. Made `pub`, reused — not duplicated. (D3 Q2 → the composition-mine avoidance.)
- **Window label:** `"main"` (Tauri default, confirmed by usage).
- **Graph:** `single-instance 2.4.3` deps unify — `tauri ^2.10→2.11.1`, `windows-sys ^0.60→0.60.2`, `zbus ^5.9→5.15.0`. No `windows` split (cpal-0.18 class). Only new crate is the plugin.
- **Version:** kickoff said `2.3.6`; crates.io latest 2.x is `2.4.3` (2026-07-13). Pinned `=2.4.3` per freshness convention — CC didn't execute the stale number blindly.

### PR #45

- 6 files, +61/−5, one squash commit `1a5cb4f`. Callback surfaces window from tray (not default `set_focus`), second process exits.
- Version bump 0.2.0 → 0.2.1 (`Cargo.toml`, `Cargo.lock`, `tauri.conf.json`). CHANGELOG rollover: stale `[Unreleased]` (already-released v0.2.0 set) dated `[v0.2.0] — 2026-07-18`, new `[v0.2.1]` **Fixed** above.
- Auto-injected `_Generated by Claude Code_` footer stripped via update — ends on `Created by Dmitriy + Claude` per CLAUDE.md.
- Security checklist accurate: no unsafe, no new endpoints (plugin is local IPC — named object on Windows, D-Bus/zbus on Linux, no network), MIT/Apache license.
- 344 tests (0 new — OS-level lock is unit-untestable, D4). All gates green including Windows CI on the actual target platform.

### Release sequence

Merge `1a5cb4f` → tag `v0.2.1` on that commit (version already 0.2.1, no drift) → build → `Glagol_0.2.1_x64-setup.exe` → release published (Latest, verified GPG signature). No `release.yml` — manual release.

### Hardware QA (D4 — the only real check)

Launch the built `.exe`, close to tray, launch again:
- **One** process in Task Manager ✅
- **One** tray icon ✅
- Window **surfaces from tray + becomes active** ✅ (the D2 callback — second half of the fix)
- Hotkey works, no conflict ✅ (confirmed by dictating the confirmation through Glagol)

Both halves verified: the lock (no second copy) AND the callback (window surfaces). The second half is invisible to a process-count-only check.

---

## The pdfium mine — priority raised

**What happened:** `pnpm tauri build` failed on the maintainer's machine:
```
curl exited with exit code: 35 ... pdfium-win-x64.tgz
error: environment variable `PDFIUM_LIBRARY_PATH` not defined at compile time
  --> src\parser\pdf.rs:44   const … = env!("PDFIUM_LIBRARY_PATH");
```

`build.rs` downloads pdfium from GitHub Releases; `pdf.rs:44` uses `env!()` (compile-time, fatal if absent). v0.2.0 built only because pdfium was cached. Here `curl` couldn't reach GitHub Releases (code 35 — SSL/connect, network-level block), and the build died.

**Workaround (this session):** downloaded `pdfium-win-x64.tgz` via browser (browser reached GitHub where curl didn't), `tar -xzf` to `D:\pdfium`, `$env:PDFIUM_LIBRARY_PATH = "D:\pdfium\bin\pdfium.dll"`, rebuilt — `Glagol_0.2.1_x64-setup.exe` produced.

**Why this is now higher-priority (not just a CI nuisance):** this is the **4th time** pdfium egress blocked a build (PR4, PR5a, PR5b placeholders in CI — now a real release build on the maintainer's own machine). It's no longer "CI inconvenience"; it can **block shipping a release**.

**Fix (post-v0.2.1 task):** make pdfium non-fatal — `env!` → `option_env!` (degrade PDF reading gracefully when absent), or cache pdfium in-repo / as a build artifact, or make PDF an optional feature. Then neither CC nor the maintainer dances with manual downloads, and a release build never dies on GitHub reachability.

---

## Docs skill + Documentation invariants (shipped this session)

Built a reusable mechanism so the maintainer can say a fact and CC updates all docs consistently:

- **`glagol-docs` skill** (`.claude/skills/glagol-docs/SKILL.md`, local) — turns one factual statement ("Sber cancelled the free tier") into a complete, RU/EN-synced edit across README + both USER_GUIDEs. Core rules: **grep every occurrence** (never edit from memory), **RU/EN as a pair always**, **app-free vs API-paid kept distinct**, verify facts against code, placeholders for missing screenshots, direct-commit-to-main. Includes a trigger rule: **document when user-observable behaviour changes** (not "when frontend touched") — new setting/screen/default/limitation/external fact.

- **CLAUDE.md `### Documentation invariants`** (committed `cb5005d`) — the same rules as project convention: same-PR doc updates for user-observable changes, RU/EN pairing, app-free/API-paid distinction, factual edits via the skill direct-to-main.

Three-place enforcement (skill = how CC executes, CLAUDE.md = convention, kickoff scope = gate), same structure that rescued carry-overs after D15.

**README updated for v0.2.0** (`38393f8`): dictation section, Groq STT, **Sber free tier removed** (the app is free/MIT, the SaluteSpeech API is now paid — distinction preserved).

---

## Competitive scouting — the niche

Toward a Habr post. What exists in RU (2026):

| Category | Examples | Gap |
|---|---|---|
| Cloud push-to-talk dictation | **Диктуй** (same `whisper-large-v3-turbo`, closed, 30 min/mo free then 299–599 ₽), SpeakFlow (50 min free, AI-formatting), Talkpad | cloud — audio leaves the machine; closed |
| Built-in | Win+H | 88–92% accuracy, no formatting |
| Meeting transcription | Таймлист (on-prem, "behind the firewall") | not push-to-talk, not into active window |
| Enterprise on-prem API | SpeechKit Hybrid, SaluteSpeech | heavy enterprise (govsector), closed, not "install and dictate" |
| "Roll your own" | REXE/NetAngels Whisper guides | raw API, no client |
| **Glagol** | — | **fills the intersection** |

**The empty intersection:** ready **push-to-talk** dictation into the active window + **local/self-hosted** + **open source**. No one has all three. SpeechKit Hybrid is on-prem but closed enterprise; Диктуй is a ready client but cloud; the Whisper guides are local but not a product.

**Positioning (honest, for Habr):** NOT "first dictation without VPN" (Win+H, Диктуй, local Whisper all work without VPN — would be mocked). Instead: **open source push-to-talk you can point at any provider — Groq free, others per-usage, or a local server for zero data egress; and the code is open so a company's security team can verify it.** Three audiences: ordinary (Groq free / ~30 ₽ proxy, ~7× cheaper than Диктуй for raw transcript), private (local model, offline, on a laptop without a GPU), company (self-hosted, whole office via LAN IP, SB reads the code).

**Honest gaps to state first:** no AI-formatting (SpeakFlow has it); setup requires effort (key + provider) vs Диктуй's install-and-go; on CPU it's fine for short push-to-talk phrases, slow for long recordings.

**Cost math (300 min/mo, Диктуй Профи tier as baseline):** Диктуй 299 ₽/mo (3588 ₽/yr) vs Glagol ~30 ₽/mo proxy + Groq free tier (~360 ₽/yr) — ~7–10× cheaper for a raw transcript. Groq TTS ruled out (no Russian); Groq STT is real and in the UI.

### Local-model privacy — the strongest angle, needs a proof run

The settings already ship a **"Локальный сервер"** provider preset (base URL `localhost:8000/v1`) — the product is *designed* for a local model, not a hack. Status: **implemented, not yet verified** ("Ключ: сохранён (не проверен)").

**Hardware check (done this session):** local Whisper needs no GPU for push-to-talk. `large-v3-turbo` int8 on CPU ≈ 1.5 GB RAM, 1.6 GB disk, ~0.3–2 s for a short phrase. `large-v3` full on CPU = 143 s/clip — unusable; turbo or small only.

**Maintainer's own hardware:** Lunnen Ground 16 C1 2025 (Ryzen 7 7730U 8c/16t, 16 GB RAM, no discrete GPU) — enough for turbo int8 on short phrases. Using the weak laptop as the *lower bound* proof: "works even on this" is stronger than "needs an RTX."

**Planned test topology (client-server, for the Habr proof):** whisper server on the strong desktop (Xeon + NVIDIA 6 GB → GPU inference, <15 ms/chunk); clients = second desktop + Ground 16, both pointing Glagol at the server's LAN IP; disconnect client internet → dictate → works = offline-privacy proof. Gotchas to expect: server must bind `0.0.0.0` (not localhost), Windows firewall on port 8000, and the model name in Glagol must match the server's (not `whisper-1`).

**For Habr:** only claim what's run. `localhost` offline works (once tested) = "works locally, offline, on a laptop without a GPU." Multi-client LAN = "architecture allows it, tested two clients" — don't claim "whole office" without running it.

---

## Notable this session — the feedback loop

The maintainer dictated his bug reports and fix confirmations **through Glagol itself**. The v0.2.1 fix ("multiple instances") was confirmed by a message typed via Glagol's dictation. This is the truest test a tool can pass: its author uses it, voluntarily, for real work, including the work of building it. "10/10 for myself" (the maintainer's rating) is not a number — it's this loop.

---

## Carry-over registry (session additions)

| → | Item | Priority |
|---|---|---|
| **next PR** | **pdfium non-fatal:** `env!` → `option_env!` or cache in-repo. Blocked a release build this session (4th egress hit). A release must not die on GitHub reachability. | **raised** |
| **next** | Local-model test (client-server: Xeon+GPU server, Ground 16 + 2nd desktop clients, LAN IP, offline) — Habr proof | — |
| **next** | Habr draft — positioning above, honest gaps stated, cost math, local-privacy with proof (not before the test) | — |
| **post-tag** | Icons: trace prototype B (needs `icon_concepts.png` in repo + tracing toolchain) | — |
| **next quest** | TTS migration Sber → aitunnel/Groq; note Groq TTS has no Russian (STT only) | — |
| **post-MVP** | Threshold centring (≥1wk data); `Type` mode (line-42 amend); adaptive SNR | — |
| **note** | `whisper-1` in the local preset must be changed to the server's model name when testing local | — |

---

## Stats — Day 12 Session 3

| Metric | Value |
|---|---|
| Releases shipped | 1 (v0.2.1, manual) |
| PRs merged | 1 (#45) |
| Tests | 344 (unchanged — OS-level lock unit-untestable) |
| Regressions / hotfixes | 0 / 0 |
| New `unsafe` | 0 |
| New deps | 1 (`tauri-plugin-single-instance =2.4.3`, unifies with graph) |
| Composition-mine avoidance | 1 (reused `show_main_window`, not duplicated) |
| Build-blocking issues | 1 (pdfium egress — worked around, priority raised) |
| Skills/conventions shipped | `glagol-docs` skill + CLAUDE.md Documentation invariants |

---

## Lessons — Day 12 Session 3

1. **The author using the product is the best QA.** The first production bug was found by real usage, and the fix confirmed by dictating through the fixed product. No synthetic test finds "I opened it twice and got two copies" — only using it does.

2. **A cached dependency hides a fragile build.** pdfium "worked" through v0.2.0 only because it was cached. The fragility (compile-time `env!` + GitHub download) was always there; the cache masked it until a network hiccup exposed it mid-release. Non-fatal degradation beats a hard compile-time requirement for a non-core feature.

3. **Same protocol for a patch as for a feature.** v0.2.1 is a small fix, but it got Phase 0, graph verification, sanity, green CI, and hardware QA. The discipline that produced 0 hotfixes across 6 marathon PRs held on the post-release patch too — that's what keeps it 0.

4. **Position on the empty intersection, not the crowded axis.** "Without VPN" is crowded (many do it). "Ready push-to-talk + local + open source" is empty. The honest, specific claim beats the loud, contestable one — especially on Habr.

---

## What's next

1. **pdfium non-fatal** (next PR) — so release builds stop dying on GitHub reachability.
2. **Local-model test** (client-server on the maintainer's three machines) — Habr proof.
3. **Habr draft** — after the test, with real numbers and a proven local-privacy section.
4. Later: icons, TTS migration.

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **v0.2.1 release:** https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.2.1 — Latest, verified signature, `Glagol_0.2.1_x64-setup.exe`
- **PR #45:** `fix: prevent multiple instances (single-instance lock)` · merged `1a5cb4f`
- **Plugin:** `tauri-plugin-single-instance =2.4.3` (registered first, callback reuses `show_main_window`)
- **pdfium workaround:** browser-download `pdfium-win-x64.tgz` → `D:\pdfium\bin\pdfium.dll` → `$env:PDFIUM_LIBRARY_PATH`
- **`main` HEAD:** `1a5cb4f`, tag `v0.2.1`, 344 tests, working tree clean
- **Version:** 0.2.1 (`tauri.conf.json`, `Cargo.toml`)
- **Local-model hardware:** Lunnen Ground 16 C1 2025 (Ryzen 7 7730U, 16 GB, no dGPU) — turbo int8 sufficient for short push-to-talk
- **Skill:** `.claude/skills/glagol-docs/SKILL.md` (local) + CLAUDE.md Documentation invariants (`cb5005d`)
