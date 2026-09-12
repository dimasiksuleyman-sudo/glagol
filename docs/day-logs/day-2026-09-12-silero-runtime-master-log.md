# Silero migration — runtime preparation, 2026-09-12

## Scope and current state

User authorized implementation of `docs/plans/silero-tts-migration.md`.
Branch: `codex/silero-tts`. No PR, push, release or application installation.
This commit covers the reproducible preparation part of stage 1 only.
The required real-model gate is **not passed**. Stages 2–6 remain outstanding.

The installed app and its production source paths remain at 0.3.0. SaluteSpeech
has not yet been removed. GigaAM, office/cloud dictation, user credentials,
database, library and hotkeys have not been modified. The 0.4.0 bump belongs to
the application migration, not a development-only preparation commit.

## Changes

- `scripts/silero/runtime-manifest.json`: immutable artifact filenames, URLs,
  sizes and hashes for embedded Python 3.11.9, torch 2.7.1+cpu and dependencies.
- `download.mjs`: bounded verified developer downloads; no model download,
  no pip, no user data or external publication.
- `assemble.py`: verify all archives first, reject traversal/links, extract
  into new staging, preserve upstream licenses, isolate Python import paths,
  inventory files, activate only after completion. Docopt's pinned source
  archive supplies only its module/license without running setup.py.
- `test_assemble.py`: four offline tests, no native/PyTorch dependency.
- `probe.py`: exact-hash guard and opt-in real-model corpus for five voices,
  punctuation/questions, stress/homographs, numbers/dates, Latin and a paragraph;
  mono 24 kHz WAV, per-case timings/RTF and Windows peak working set.
- Paired runtime notes and upstream Silero CC BY-NC-SA 4.0 license; Python
  bytecode ignored. Migration plan status updated honestly.

## Evidence

Target: Windows x64, Ryzen 7 7730U, 8 cores / 16 threads, 16 GB RAM.

All 13 catalog artifacts downloaded and SHA-256 verified. Catalog download
size is 249,346,278 bytes. Full assembled payload: 1,319,448,970 bytes across
14,483 files, excluding inventory JSON. Development-only runtime under
`.scratch/silero/runtime-271`, not in Git or installer.

`python.exe -I -B` imported torch/numpy/num2words/docopt successfully;
torch reported `2.7.1+cpu`, CPU tensor values `[1.0, 1.0, 1.0, 1.0]`,
Russian number conversion succeeded, docopt reported `0.6.2`.
PyTorch 2.8.0+cpu also imported in a separate scratch runtime but its wheel
is 619,392,861 bytes, versus 216,031,210 bytes for 2.7.1+cpu.

Validation:

- Python AST checks and `node --check scripts/silero/download.mjs`: pass.
- Assembler suite: 0 previous + 4 new − 0 deleted = 4 passing Python tests.
- Bad model input: rejected by SHA-256 before PyTorch import/output creation.
- Existing Rust/React tests unchanged; full app builds are unnecessary for
  these development-only tools and have not been rerun as Silero validation.
- No real-model WAV, performance or pronunciation result is claimed.

## Blocking evidence and next action

Repeated official model requests (including an elevated request and an 8-second
TCP timeout retry) fail to connect to `models.silero.ai:443`; Python.org,
PyTorch, GitHub and PyPI downloads work. No global proxy/VPN changes were made.

The official GitHub Silero SAPI release was downloaded and inspected without
running its installer: it contains `v5_5_ts.bin` plus separate accent/homograph
modules, not the planned Python package. It is not substituted for v5.5 `.pt`.
No unverified mirror or different model was executed.

The provisionally pinned model digest agrees with two public integrations,
but the model itself is still absent. Asked user asynchronously for an existing
file path or browser/VPN download. Continue by obtaining the exact artifact,
confirming its size/hash/provenance, then running probe in separate 2/4-thread
processes with an external timeout. Listen to samples and measure concurrent
dictation before replacing the production pipeline. Remaining implementation
and final 0.4.0 release checks are recorded in the approved migration plan.
