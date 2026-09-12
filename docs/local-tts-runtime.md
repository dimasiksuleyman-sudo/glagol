# Silero local TTS preparation

[Русский](local-tts-runtime.ru.md)

This is preparation for 0.4.0, **not an available application feature**.
Glagol 0.3.0 is unchanged. See the [migration plan](plans/silero-tts-migration.md).
Yandex SpeechKit v3 for commercial TTS is a separate future stage. Dictation,
including GigaAM and the office server, is independent of this work.

## Verified on 2026-09-12

An isolated Windows x64 runtime was assembled on Ryzen 7 7730U / 16 GB RAM:

- Embedded Python 3.11.9 and PyTorch 2.7.1+cpu; no system Python, pip or CUDA.
- All 13 archives verified against pinned sizes and SHA-256 before extraction.
- Download: **249,346,278 bytes**, excluding the model.
- Extracted: **1,319,448,970 bytes**, 14,483 files, excluding the JSON inventory.
  This includes PyTorch SDK files; reducing it has not been validated yet.
- Isolated imports of torch, numpy, num2words and docopt succeeded, as did a
  CPU tensor operation and Russian number conversion.
- Four assembler tests pass: traversal, corruption, failed extraction cleanup
  preserving neighbouring files, and activation/inventory.
- The model probe rejects a wrong SHA-256 before importing PyTorch or writing WAV.

**Real synthesis remains untested.** Connections to the official
`models.silero.ai` server time out. A working PyTorch import does not establish
model compatibility. Model startup, synthesis speed/RAM, voices and concurrent
dictation have not been measured.

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

Silero TTS v5.5 is an optional component **for noncommercial use**, licensed
CC BY-NC-SA 4.0, by the Silero Team.
[Upstream](https://github.com/snakers4/silero-models),
[full license](third-party/Silero-LICENSE.txt).
SHA-256 of the saved license:
`1349a4b6148492b44f629e64eed676612e234fe9a839e4f3b277c1482c8849f1`.
The model license does not replace Glagol's MIT license or itself restrict
independent dictation. The selected STT provider's own terms still apply.

The official model URL appears in upstream `models.yml`:
`https://models.silero.ai/models/tts/ru/v5_5_ru.pt`.
The probe provisionally pins SHA-256
`50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437`,
matching two independently published integrations:
[bootstrap](https://github.com/ganiushin/parakeet-stt-silero-tts-addons-haos/blob/main/wyoming_silero_tts/silero/scripts/bootstrap.py)
and [bridge](https://github.com/Krablante/silero-tts-bridge).
This **does not claim verification of our downloaded model**: no model file has
been obtained yet. Confirm provenance, size and compatibility before product use.

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

Once the exact model is available, run `probe.py` with this runtime and arguments
`--model <absolute v5_5_ru.pt path> --output <new directory> --threads 2`, then
repeat with four threads in another process/directory. Apply an external process
timeout. The probe writes mono 24 kHz PCM WAV and `report.json` with timings,
RTF and peak working set. Only public test sentences are used. Listen to the
WAVs: valid audio does not establish pronunciation quality. Also test concurrent
dictation before replacing the application pipeline.
