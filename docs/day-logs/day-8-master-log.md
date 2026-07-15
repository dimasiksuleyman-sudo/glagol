# Glagol — Day 8 Master Log

**Period:** May 26, 2026 ~19:00 local → May 26, 2026 ~23:15 local (public release day)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** None — publication day, not a feature sprint
**Status at end of Day 8:** Glagol is publicly downloadable. GitHub Release `v0.1.0-rc.7` published with NSIS installer attached, bilingual USER_GUIDE shipped, README rewritten to match shipped reality, Discussions enabled, GitHub Profile README created.

> Day 7 Session 2 (`day-7-session-2-master-log.md`) covers Sprint 5d closure preceding this day.
> This file covers the publication day: no code changes, documentation and release artifacts only.

---

## TL;DR

Day 8 crossed the line from "development-only" to "publicly available product." After 7 days of development (14 sprint closures, tag `v0.1.0-rc.7`), the application was published for real users.

Day 8 delivered:

1. **Fresh installer build verified** — `Glagol_0.1.0_x64-setup.exe`, 7.67 MB download, ~26 MB installed (pdfium.dll ~7 MB + glagol.exe ~20 MB). Confirmed offline installer: system WebView2, no download during install. Prior guesses of "80–120 MB" were wrong by an order of magnitude — Tauri's footprint is a selling point, not a caveat.

2. **Three UI screenshots captured** — `docs/screenshots/{library-page,synthesize-page,settings-page}.png`. The settings screenshot alone documents three sprints of work simultaneously (5d credentials polish + 5d usage counter + 5c backup/restore).

3. **English synthesis honestly tested and honestly rejected.** SaluteSpeech reads Latin script poorly. Rather than list "multi-language support," documentation states plainly: Russian only, other languages are "hit or miss." Honest scope beats a longer feature list.

4. **Bilingual USER_GUIDE shipped** — `USER_GUIDE.md` (language selector) + `USER_GUIDE.ru.md` + `USER_GUIDE.en.md`, mirroring the README's existing selector pattern. Marketing-forward human tone, not a technical longread.

5. **README rewritten for shipped reality** — corrected voice count (6, named, was incorrectly "7 + one English"), added shipped features absent from the old text (backup/restore, usage counter, inline rename, file formats), replaced "installer coming, grab from CI" with a real release link and real sizes, removed the signed-MSI plan (project ships unsigned NSIS by design), fixed the roadmap, removed a broken image reference.

6. **GitHub Release `v0.1.0-rc.7` published** — installer attached, pre-release flag set (semver `-rc` is honest), bilingual release notes.

7. **Discussions enabled** — a feedback channel alongside Issues.

8. **GitHub Profile README created** — new repository `dimasiksuleyman-sudo/dimasiksuleyman-sudo`, rendering a card above Popular repositories.

9. **Claude credited openly** across README footer, USER_GUIDE, and release notes: "Created by Dmitriy + Claude — AI as a tool under human control." Not hidden. AI-assisted development is normal in 2026; the project documents its process rather than obscuring it.

No code shipped on Day 8. Test count unchanged at 168. Working tree remained clean throughout; all changes were direct commits to `main` (documentation-only, per solo-project convention).

**Glagol is now installable by anyone with Windows 10/11 and a free SaluteSpeech key.**

---

## Artifact inventory — what shipped

**Installer:**
- Path: `src-tauri/target/release/bundle/nsis/Glagol_0.1.0_x64-setup.exe`
- Download size: 8 037 428 bytes (7.67 MB)
- Installed size: ~26 MB
- Composition: `pdfium.dll` ~7 MB, `glagol.exe` ~20 MB
- WebView2: system (not bundled) — explains the small footprint
- Offline: confirmed. No download prompts observed during install.

**Screenshots (`docs/screenshots/`):**
- `library-page.png` — hero shot. Four real documents with meaningful titles, three different voices (Тарас/Борис/Наталья), sizes 4 533 → 33 737 chars. Shows play/download/rename/delete affordances.
- `synthesize-page.png` — entry point. Text field with character counter, voice dropdown, disabled button state (empty-text UX from Sprint 5d Scenario 4).
- `settings-page.png` — all three sections visible. Key field shows placeholder only (`Base64(client_id:client_secret)`); the real key lives in Windows Credential Manager and never renders in UI. Usage counter showing live data ("Использовано в мае: 92 759 / 200 000 символов", 46,4%).

**Documentation:**
- `USER_GUIDE.md` — language selector (Русский · English)
- `USER_GUIDE.ru.md` — Russian guide
- `USER_GUIDE.en.md` — English guide
- `README.md` — rewritten, bilingual, with hero screenshot and USER_GUIDE links

**GitHub:**
- Release `v0.1.0-rc.7` — installer in Assets, pre-release flag, bilingual notes
- Discussions enabled (auto-created Welcome #33 in Announcements)
- Profile README repository `dimasiksuleyman-sudo/dimasiksuleyman-sudo`

---

## Corrections made to README — reality vs. documentation drift

The README had drifted significantly from the shipped product during Sprints 4–5d. Day 8 reconciled it:

| Claimed (stale) | Reality (v0.1.0-rc.7) |
|---|---|
| "7 neural voices (plus one English voice)" | **6 voices**: Наталья, Борис, Марфа, Тарас, Александра, Сергей. No dedicated English voice. |
| Backup/restore not mentioned | Shipped in Sprint 5c |
| Usage counter not mentioned | Shipped in Sprint 5d |
| Inline rename not mentioned | Shipped in Sprint 5b |
| File formats listed under "planned" | Shipped in Sprint 4 (TXT/MD/DOCX/PDF) |
| "Signed MSI installer — Sprint 5" | Project ships **unsigned NSIS** by design (signing skipped; SmartScreen documented instead) |
| "Installer will ship with v0.1.0; until then grab the unsigned .exe from CI artifacts" | **Public release exists**, direct link + real sizes |
| `docs/images/smartscreen-warning.png` referenced | File doesn't exist — reference removed, text instructions kept |
| Roadmap: "v0.1.0 Release ✅" | First public release is `v0.1.0-rc.7`; stable `v0.1.0` still ahead |

**Lesson banked:** README drift accumulates silently across sprints because nothing gates it. Feature work updates code and CHANGELOG; the README's prose claims go stale unnoticed. Worth a pre-release reconciliation pass every time, not just once.

---

## The English synthesis test

Tested SaluteSpeech with an English phrase before writing the language section of USER_GUIDE. Result: poor — the engine reads Latin script through Russian phonetics.

Decision: document honestly. USER_GUIDE and README both state Russian-only, with the phrasing "латиница и другие языки озвучиваются «на любителя»" / "Latin script and other languages come out 'hit or miss'."

Rationale: an honest limitation is more useful to users than an inflated feature list, and cheaper to maintain than a support burden from disappointed English-language users. The product does one thing well; saying so plainly is a feature.

*(Adding a second synthesis engine for English is backlog, not a Day 8 concern.)*

---

## Bilingual pattern applied to USER_GUIDE

The README already used a language selector (`Русский · English · Disclaimer` anchors) from earlier work. Day 8 extended the same pattern to the user guide:

```
USER_GUIDE.md      → selector (Русский | English)
USER_GUIDE.ru.md   → Russian
USER_GUIDE.en.md   → English
```

Both language versions reference the same screenshots (`docs/screenshots/*.png`), so no asset duplication.

**Tone decision:** short, human, marketing-forward — not a technical longread. Opens with user benefit ("listen anywhere: on a walk, on the road, while doing chores"), not with a feature table. This matches the project's existing documentation voice and the maintainer's stated preference.

---

## Commit sequence (all direct to `main`, documentation-only)

1. `docs: add bilingual user guide and screenshots` — USER_GUIDE ×3 + screenshots ×3
2. `docs: rewrite README for v0.1.0-rc.7 reality + for-hire signaling` — README rewrite

Per project convention: **code goes through branch + PR + squash-merge; documentation-only changes commit directly to `main`.** This is a deliberate solo-project deviation from CLAUDE.md's stricter "even docs via PR" guidance — a PR for a prose change is overhead without a reviewer.

---

## Release notes structure

Bilingual (RU + EN), covering:
- What it does (6 voices, file import, library, backup/restore, usage counter, local key storage)
- Installation (download link, SmartScreen instructions, key setup pointer to USER_GUIDE)
- Requirements (Windows 10/11 x64, ~26 MB, free SaluteSpeech key, internet for synthesis)
- Known limitations — stated plainly:
  - Windows only (macOS/Linux on request)
  - Russian only
  - No auto-update
  - Installer not commercially signed (hence SmartScreen)

Pre-release flag set: `-rc.7` is not a stable release and the notes don't pretend otherwise.

---

## Deferred from Day 8

**ADR index (`docs/architecture/README.md`)** — deliberately not rushed.

Context: the project has no formal ADR directory, but it has an equivalent that is arguably better — 13 public master logs in `docs/day-logs/` documenting every architectural decision as it happened, plus 42 banked conventions in CLAUDE.md.

The gap is navigation, not content. A reader landing in `docs/day-logs/` faces 13 files with no entry point.

Three options were weighed:
1. **Do nothing** — logs are already public. Rejected: no entry point, reader drowns.
2. **Write a clean `docs/architecture/README.md`** that indexes 6–7 key decisions (context → decision → consequence) with links into the master logs where each is discussed in depth. Doesn't duplicate, navigates.
3. **Rewrite the master logs themselves** to be more presentable. Rejected: ~400 KB across 13 files, and polishing would cost the raw authenticity that makes them evidence rather than marketing.

**Option 2 selected, execution deferred.** The maintainer's instinct — "это нужно обдумать" — was correct; an architecture index written in a hurry at the end of a long publication day would be worse than one written deliberately.

Note: the internal kickoff documents in `.scratch/` stay internal permanently. They contain working-session material not intended for publication. The architecture index will be written fresh, not extracted.

---

## Day 8 in numbers

- Code changes: **0**
- Tests: **168** (unchanged)
- Documentation files added: **4** (3 guides + selector)
- Screenshots added: **3**
- README: rewritten (~9 factual corrections)
- GitHub Releases published: **1** (`v0.1.0-rc.7`)
- Repositories created: **1** (Profile README)
- Installer size discovered to be **10× smaller** than assumed

---

## State at close of Day 8

**Public artifacts:**
- Release: https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.1.0-rc.7
- Installer: `Glagol_0.1.0_x64-setup.exe` (7.67 MB)
- Docs: README (bilingual), USER_GUIDE (bilingual), 13 master logs, CHANGELOG, CONTRIBUTING, SECURITY, CODE_OF_CONDUCT
- Channels: Issues + Discussions

**Repository:**
- `main` clean, HEAD at documentation commits
- Tag `v0.1.0-rc.7` (Sprint 5d closure)
- 168 tests, zero open bugs
- 42 banked conventions in CLAUDE.md

**Next:** ADR index (deliberate, not rushed). Then continued development toward stable `v0.1.0`.

---

*Created by Dmitriy + Claude — AI as a tool under human control.*
