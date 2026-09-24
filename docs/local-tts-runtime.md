# Silero local TTS in Glagol 0.5.0

[Русский](local-tts-runtime.ru.md)

The 0.5.0 source candidate adds English v3_en alongside Russian v5.5, using one
existing Python 3.11.9 / PyTorch 2.7.1+cpu runtime. There is no Kokoro/G2P dependency.
Dictation is independent. This is not a publication or installation record;
see the [current workstream](workstreams/english-first/log.md).

## English and shared files — 0.5.0

[english-model.json](../scripts/silero/english-model.json) pins v3_en: 57,194,546 bytes,
SHA-256 `02b71034d9f13bc4001195017bac9db1c6bb6115e03fea52983e8abcff13b665`.
Official URL: `https://models.silero.ai/models/tts/en/v3_en.pt`. It timed out in this
environment; the manifest records the exact mirror revision, matching LFS hash
and `origin_verified: false`. No fallback mirror is silently used by the app.
The pinned file was inspected before execution and ran against the existing runtime.

First EN download: 306,540,824 bytes. Adding EN to a valid RU installation downloads
only 57,194,546 bytes. Runtime inventory/receipts stay in the same `tts_models` root.
Both models are CC BY-NC-SA 4.0 under the same pinned license, but acknowledgements
and verification receipts are per model: RU keeps `acknowledgement.json` and
`verification.json`; EN uses `acknowledgement-en.json` and `verification-en.json`.
Removal deletes only the selected model/partial/metadata, retaining the shared runtime.
A common operation mutex prevents runtime repair while either language synthesizes.

EN provider ID is `silero-en`; RU retains `silero`. EN voices are `en_0`–`en_3`,
default `en_0`. No gender/accent is inferred from IDs. English number/abbreviation
normalization is separate from Russian stress/Latin handling. Both use the existing
sequential temporary-WAV/library pipeline; new rows carry nullable `speech_language`
from migration 6, while old rows retain NULL. Backup exclusions are unchanged.

Native Windows tests exercised both model workers, four English voices, long text,
cancel, crash recovery, WAV/library metadata and independent language preferences.
Process tests separately exercise parent exit, bounded protocol and timeout handling.
Commands and actual results, including NOT_RUN listening/UI/installation checks,
belong to the [work log](workstreams/english-first/log.md). Historical measurements
below are for RU 0.4.x, not English 0.5.0 performance promises.

## Verified on 2026-09-12

An isolated Windows x64 runtime was assembled on Ryzen 7 7730U / 16 GB RAM:

- Embedded Python 3.11.9 and PyTorch 2.7.1+cpu; no system Python, pip or CUDA.
- All 13 archives verified against pinned sizes and SHA-256 before extraction.
- Runtime download: **249,346,278 bytes**. Model: **145,420,684 bytes**.
  Combined download: **394,766,962 bytes** (~395 MB); about 1.9 GB on disk
  including retained archives. Neither weights nor Python/PyTorch enter NSIS.
- Extracted: **1,319,448,970 bytes**, 14,483 files, excluding the JSON inventory.
  This includes PyTorch SDK files; reducing it has not been validated yet.
- Isolated imports of torch, numpy, num2words and docopt succeeded, as did a
  CPU tensor operation and Russian number conversion.
- Four assembler tests pass: traversal, corruption, failed extraction cleanup
  preserving neighbouring files, and activation/inventory.
- The model probe rejects a wrong SHA-256 before importing PyTorch or writing WAV.

The user-supplied `v5_5_ru.pt` matches the pinned SHA-256 below. Real model tests
produced valid WAV for all five voices and 32 cases across two/four CPU threads.
With two threads, Python/torch/model loading took 3.25 s (excluding the app's
runtime integrity scan). First inference took 1.48 s for 5.15 s of audio;
warm Xenia took 0.27 s for 4.53 s. A longer paragraph took 1.81 s for 22.39 s.
Peak worker working set was about 752 MB. Four threads offered little benefit;
the app uses two. These are measurements on this laptop, not minimum requirements.

GigaAM CTC inference on the same public test WAV took about 0.30 s alone and
0.40 s during TTS, with identical transcripts. This tests concurrent inference,
not live microphone readiness. Adapter-produced numbers and Windows/USB words
also survived the STT check. Number/date grammar and Latin pronunciation remain
heuristic; valid audio and ASR checks do not replace listening.

The release-mode native test also assembled all runtime archives through the
application's Rust code, verified its embedded inventory, synthesized a multi-chunk
document into the library and cancelled active inference. Migration and backup
tests cover preservation of existing documents and exclusion of TTS consent,
components and previews. Visual UI, clean-install/update and listening checks
remain manual: the computer-use helper was unavailable during implementation.

The official model server timed out from the development shell. Settings support
both automatic download and importing the exact verified model file downloaded
in a browser; an arbitrary `.pt` file is rejected. Runtime downloads use the
verified sources below. No promise is made that the network issue is resolved.

The official GitHub SAPI installer was inspected without executing/installing
it: it contains `v5_5_ts.bin` and separate modules, not `v5_5_ru.pt`. It is not
used as a substitute. PyTorch 2.8.0+cpu also ran, but its Windows wheel is
619,392,861 bytes; the preparation catalog therefore uses 2.7.1+cpu.

## Sources and licenses

Developer catalog: [runtime-manifest.json](../scripts/silero/runtime-manifest.json).
Python comes from python.org, PyTorch from its official CPU index, and other
packages from PyPI. Versions, sizes and hashes are pinned; no automatic metadata
updates. Assembly preserves upstream license files.

| Component | Version | License |
|---|---|---|
| Python | 3.11.9 | PSF and included component licenses |
| PyTorch CPU | 2.7.1 | BSD-3-Clause and bundled third-party notices |
| NumPy | 2.2.6 | BSD-3-Clause and bundled third-party notices |
| filelock | 3.16.1 | Unlicense |
| typing_extensions | 4.15.0 | PSF-2.0 |
| SymPy | 1.14.0 | BSD-3-Clause |
| NetworkX | 3.4.2 | BSD-3-Clause |
| Jinja2 | 3.1.6 | BSD-3-Clause |
| fsspec | 2025.7.0 | BSD-3-Clause |
| mpmath | 1.3.0 | BSD-3-Clause |
| MarkupSafe | 3.0.3 | BSD-3-Clause |
| num2words | 0.5.14 | LGPL-2.1 |
| docopt | 0.6.2 | MIT |

Silero TTS v5.5 and v3_en are optional components **for noncommercial use**, licensed
CC BY-NC-SA 4.0, by the Silero Team.
[Upstream](https://github.com/snakers4/silero-models),
[full license](third-party/Silero-LICENSE.txt).
SHA-256 of the saved license:
`1349a4b6148492b44f629e64eed676612e234fe9a839e4f3b277c1482c8849f1`.
The model license does not replace Glagol's MIT license or itself restrict
independent dictation. The selected STT provider's own terms still apply.

The official model URL appears in upstream `models.yml`:
`https://models.silero.ai/models/tts/ru/v5_5_ru.pt`.
The catalog pins SHA-256
`50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437`,
matching two independently published integrations:
[bootstrap](https://github.com/ganiushin/parakeet-stt-silero-tts-addons-haos/blob/main/wyoming_silero_tts/silero/scripts/bootstrap.py)
and [bridge](https://github.com/Krablante/silero-tts-bridge).
The supplied model was checked against this hash and the exact byte count above
before execution. Its model code remains in the separately downloaded package.

## Application behavior

Settings require explicit noncommercial-use acknowledgement tied to the model
and license hash before installation/import; Rust checks it again before loading
or synthesis. The installer shows an informational notice without opting in.
Consent stays in the per-model files described above and is excluded from backups.
Dictation, library playback/export and file import do not depend on consent.

Artifacts download with progress, cancellation, resume and SHA-256 validation.
Safe staging extraction is activated only after every expected file matches the
compiled inventory. The app launches an isolated portable interpreter with no
pip, hub, SAPI registration or inference network calls. It exchanges bounded
JSON metadata and private WAV files, not audio over IPC. This is process ownership
and Python path isolation, not an OS security sandbox. A full verification writes
`tts_models/verification.json` with its time, compiled-inventory identity and
metadata for the model, `python.exe` and `python311.dll`. The receipt remains valid
for 30 days; startup checks it without hashing the entire runtime. A missing,
stale or mismatched receipt triggers a background full check, as does a worker
failure. The first upgraded launch creates the receipt after one full check. It
contains no user data and is excluded from backups. Opening Synthesize preloads
the worker without another download or extraction. The worker exits with its
parent, on cancellation/error, or after 15 minutes idle.

Text is chunked sequentially and WAV is written incrementally. A cancelled or
failed job does not create a successful library row. Old WAV/voice metadata stays
playable with provider `salutespeech-legacy`; new documents use `silero` / `silero-en`. OAuth,
SaluteSpeech API/TLS material and TTS quota UI are removed. Only the legacy TTS
credential is deleted once; dictation profiles, keys and usage remain independent.

## Developer reproduction

The installed application never invokes these tools. Keep all binaries under
`.scratch`; do not commit weights/interpreter or bundle them in NSIS.

```powershell
node scripts/silero/download.mjs .scratch/silero/downloads
# download.mjs has already verified the Python archive SHA-256.
Expand-Archive .scratch/silero/downloads/python-3.11.9-embed-amd64.zip .scratch/silero/bootstrap-python
& .scratch/silero/bootstrap-python/python.exe -I -B scripts/silero/assemble.py --downloads .scratch/silero/downloads --destination .scratch/silero/runtime-271
& .scratch/silero/runtime-271/python.exe -I -B scripts/silero/test_assemble.py
```

Use a new destination; existing runtimes are never overwritten. Extraction uses
an adjacent `.staging`, rejects links/unsafe paths, and activates only on success.
`python311._pth` excludes user site-packages and Python environment paths.
No `setup.py` is executed.

Run `probe.py` with this runtime and arguments
`--model <absolute v5_5_ru.pt path> --output <new directory> --threads 2`, then
repeat with four threads in another process/directory. Apply an external process
timeout. The probe writes mono 24 kHz PCM WAV and `report.json` with timings,
RTF and peak working set. Only public test sentences are used. Listen to the
WAVs: valid audio does not establish pronunciation quality.

Run adapter checks with `python.exe -I -B scripts/silero/test_worker.py`.
To regenerate Rust's pinned catalog/inventory after intentionally updating and
verifying the runtime, run `node scripts/silero/generate-catalog.mjs <runtime.json>`.
For the native smoke test, prepare an isolated `GLAGOL_TTS_SMOKE_ROOT` containing
the model and all 13 archives at its top level, set `GLAGOL_TTS_ASSEMBLE_SMOKE=1`,
and run `cargo test --release --manifest-path src-tauri/Cargo.toml native_silero_pipeline_and_cancel -- --ignored --nocapture`.
Set `PDFIUM_LIBRARY_PATH` to the absolute local `src-tauri/resources/pdfium.dll`
if the normal build-time download is unavailable. Never point smoke tests at user data.
