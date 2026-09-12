# Silero TTS 0.4.0 — integration master log, 2026-09-12

## Scope and authorization

The user approved replacing SaluteSpeech with optional downloaded Silero TTS v5.5
for noncommercial use, preserving independent GigaAM/office/cloud dictation.
Yandex SpeechKit v3 is a future commercial TTS stage. The user supplied the local
model after the previous session could not reach the official model server.
Local commits only; no PR, push or published release. Work follows the
[approved plan](../plans/silero-tts-migration.md), on `codex/silero-tts` after
`77d95bd` (verified runtime preparation). Previous logs remain historical.

## Verified input and implementation

- Model `v5_5_ru.pt`: 145,420,684 bytes, SHA-256
  `50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437`.
  The supplied Downloads file was read, checked and used for isolated tests;
  the user's file was not moved or modified.
- Portable Python 3.11.9 / torch 2.7.1+cpu: 13 pinned official-source archives,
  249,346,278 bytes downloaded; 1,319,448,970 bytes / 14,483 files extracted.
  Combined download 394,766,962 bytes, about 1.9 GB including cached archives.
  Weights/runtime remain outside Git and the application installer. Full hashes,
  upstream licenses and reproduction: [runtime notes](../local-tts-runtime.md).
- Rust catalog and embedded runtime inventory, safe staging extraction,
  verification before activation/load, resumable downloader, cancel/remove/repair,
  exact-model import fallback. Explicit consent tied to model/license hash;
  installer notice does not opt in or download. No startup TTS worker/download.
- An owned hidden CPU worker uses bounded JSON plus private WAV files, two CPU
  threads, 90-second response timeout, active cancellation, parent-death watcher
  and three-minute idle unload. No pip/hub/SAPI/network inference. This is not
  an OS sandbox. Original adapter code remains MIT; model code stays in its
  separately downloaded NC package.
- Five voices with preview. Numbers, explicit stress and Latin input are handled
  before inference; date/number grammar and unknown Latin pronunciation are
  intentionally heuristic. Sequential chunks stream into WAV; errors/cancel do
  not create successful library rows. A provider interface leaves room for the
  separately scoped SpeechKit work.
- Append-only migration 5 adds document provider; existing metadata becomes
  `salutespeech-legacy`. Legacy WAV, voice labels, library playback/export,
  dictation profiles/keys/counters and STT model storage are preserved.
- Removed SaluteSpeech client, OAuth, commands, custom root CA, TTS credentials
  context and quota UI. Narrow one-time cleanup deletes only the old TTS key,
  with an on-disk success marker and retry on OS failure; it never reads the key.
- TTS consent/components stay outside backups; voice preview is in a subdirectory
  excluded from library backup enumeration. Main MIT license remains unchanged.
- Version synchronized to 0.4.0 in package/Tauri/Cargo/lockfile. NSIS carries a
  bilingual optional-component notice and versioned original icon references for
  existing shortcuts. README, paired user guides, architecture, contributor and
  security docs, issue templates, runtime notes and changelog updated together.

## Evidence

Windows x64, Ryzen 7 7730U, 16 GB RAM. All test text is public fixture text;
tests use `.scratch`/temporary directories, not the installed app's database.

- Rust suite: **325 passed, 3 ignored** (328 total). Baseline 366 tests minus 45
  obsolete Salute/credentials/quota tests plus 7 new tests: consent, archive paths,
  two pipeline tests, migration preservation, backup isolation, ignored native
  model pipeline. The native model test is explicitly exercised below; the other
  two pre-existing ignored tests are not claimed as run.
- `cargo fmt`; `cargo clippy --all-targets -- -D warnings`; version check;
  TypeScript check and production Vite build; NSIS release build.
- Python assembler tests: 4 passed. Adapter tests: 4 passed, including numeric
  data, Latin tokens, explicit/combining stress markers and bounded segmentation.
- Native real-model pipeline test passed in debug mode. Separately, release mode
  with `GLAGOL_TTS_ASSEMBLE_SMOKE=1` passed in **58.34 s**, including full Rust
  runtime assembly/inventory verification, multi-chunk WAV, persisted provider /
  measured duration and active cancellation below three seconds. Test runtime
  directory is isolated; archive/model hardlinks are read-only inputs.
- Direct model probes: 32 cases across five voices and 2/4 CPU threads produced
  valid nonzero WAV. Two threads: Python/torch/model load 3.25 s **excluding app
  runtime verification**, first inference 1.48 s / 5.15 s audio, warm Xenia 0.27 s /
  4.53 s audio, long paragraph 1.81 s / 22.39 s audio. Peak worker working set
  about 752 MB. Four threads: load 3.16 s, warm Xenia 0.28 s, paragraph 1.78 s,
  peak about 753 MB. Two threads selected for similar throughput with less CPU.
- Concurrent GigaAM CTC inference: 0.291/0.301/0.320 s alone versus
  0.385/0.441/0.385 s during TTS, same transcript. Adapter WAV recognized as
  “Сегодня 12 сентября 2026 года. Проверяем Windows и USB. Ты уже готов?”
  This is an inference regression check, not a live microphone or listening test.

Local raw reports: `.scratch/silero/probe-2/report.json`, `probe-4/report.json`,
`concurrency/report.json`. No raw binaries or generated user data are committed.
Standard commands are documented in the runtime notes. The installed pnpm 12.4.1
attempted an inaccessible dependency reinstall; existing Node entrypoints were
used for TypeScript/Vite/Tauri instead. The sandbox also could not download
Pdfium, so builds used `PDFIUM_LIBRARY_PATH` pointing to the existing verified
project `src-tauri/resources/pdfium.dll`. No dependency upgrade was needed.

## Delivery and remaining manual checks

Local artifact: `src-tauri/target/release/bundle/nsis/Glagol_0.4.0_x64-setup.exe`.
Size: **9,742,481 bytes**. SHA-256:
`569d707ff2d3bdb0b5f83c6686c6d66889aa6d135ed2161f20bdaa9e2f62f7d3`.
The user's installed 0.3.0 application and real data have not been updated by
this coding session. A successful installer build does not prove installation.

The computer-use skill was applied for intended UI verification, but `sky.list_apps`
failed with “Computer Use native pipe is unavailable … os error 2”. No alternative
UI automation was used. Accordingly, these checks remain explicitly unperformed:
clean install/update UI, visual notice/consent/preview/shortcut behavior, live
dictation after updating, and human listening to all pronunciations. Process crash,
90-second timeout and parent-death handling exist in code but were not separately
fault-injected in this session. These are manual follow-ups, not claimed passes.

The official model endpoint timed out from the shell; browser-supplied exact-model
import is supported. Automatic model download under the user's network still
needs a real endpoint connection. Runtime archive sources and actual Rust
assembly were verified. No model/adapter/runtime publication, PR or push occurred.
