# Local dictation runtime

Glagol downloads the runtime and weights only after a user action. Neither is
part of the NSIS installer. The source of truth is
`src-tauri/src/stt/local/catalog.rs` (RU) and `src-tauri/src/stt/moonshine/catalog.rs` (EN): URLs pin immutable releases/revisions,
byte counts and SHA-256 hashes. Increasing the catalog requires a reviewed
code change and a native smoke test on Windows x64.

## Components

- English: Moonshine Small Streaming, runtime v0.1.5 / ABI 30000; eight model files
  at revision `0bf2f2e5aff22e6fbba4300b00a4e00bbc4f8aae`, 142,300,974 bytes. Native
  Windows wheel archive: 16,542,073 bytes; only two pinned DLLs are extracted.
  ONNX Runtime reports 1.23.2. No Python execution is involved in this STT path.
  Total first download: 158,843,047 bytes. English streaming models/runtime are
  MIT; [Moonshine](third-party/Moonshine-LICENSE.txt), [ORT license](third-party/ONNX-Runtime-LICENSE.txt)
  and [third-party notices](third-party/ONNX-Runtime-ThirdPartyNotices.txt) are retained.

- [transcribe.cpp 0.2.3](https://github.com/handy-computer/transcribe.cpp/releases/tag/v0.2.3),
  Windows x64 CPU/Vulkan archive, 20,077,848 bytes. Glagol explicitly selects CPU.
  Its package retains its MIT license and bundled third-party notices.
- [GigaAM v3 E2E CTC Q8_0](https://huggingface.co/handy-computer/gigaam-v3-e2e-ctc-gguf),
  revision `075dff81f843cf23d22b4ce943ffdc4dd8650cd7`, 272,151,136 bytes.
- [GigaAM v3 E2E RNNT Q8_0](https://huggingface.co/handy-computer/gigaam-v3-e2e-rnnt-gguf),
  revision `f719d70812344f4d0fb8c11c0887b190501a7465`, 273,724,832 bytes.
- Models are conversions of [Sber GigaAM](https://github.com/salute-developers/GigaAM),
  MIT licensed. Keep the upstream model license with redistributed weights.

## Lifetime and storage

English uses `speech_models/moonshine-0.1.5` and a hidden child mode of the same EXE,
before Tauri/SQLite/hotkeys initialize. Every model/DLL hash is verified before
loading; the pinned ORT is explicitly loaded by absolute path before Moonshine.
The recorder sends bounded blocks through a 32-packet queue, resamples outside
the audio callback to mono 16 kHz and flushes the tail once on release. Only final
text is inserted. Queue overflow/worker failure aborts the attempt. JSON is bounded
to 512,000 bytes, a transaction times out at 90 seconds, and a Windows parent handle
terminates the child on exit/crash. Warm models unload after 15 idle minutes.
English STT is independent of Silero's Python processes and operation lock.

The following in-process lifetime applies only to the existing Russian path.

`paths::local_models_root` resolves the app-local `speech_models` directory.
`.part` files remain after cancellation/disconnection and use HTTP Range to resume.
Complete files are installed only after size and SHA-256 checks. Native archive
extraction rejects absolute paths, parent components and links. Before the first
load, runtime files are compared against the verified archive.

The native library remains loaded for the process lifetime: ggml keeps global
backend registrations. A mutex owns one session/model at a time, and inference
runs in `spawn_blocking`. Session destruction precedes freeing weights. No model
is downloaded during recognition. Logs include model ID and timing, never audio
or recognized text. Short utterances remain intact; long audio is partitioned
near low-energy boundaries between 16 and 24 seconds without losing samples.

## Reproduce native verification

Place both pinned GGUFs, `runtime-0.2.3.tar.gz`, and Sber's public
[example.wav](https://cdn.chatwm.opensmodel.sberdevices.ru/GigaAM/example.wav)
in a test directory. Set `GLAGOL_LOCAL_SMOKE_ROOT` to its absolute path, then run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml native_gigaam_smoke -- --ignored --nocapture
```

This opt-in test extracts the runtime, loads both models, and recognizes the
public sample twice with each. It does not access the microphone or download
files. Normal CI tests use mock HTTP and need no weights or native runtime.
