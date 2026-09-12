# Local dictation runtime

Glagol downloads the runtime and weights only after a user action. Neither is
part of the NSIS installer. The source of truth is
`src-tauri/src/stt/local/catalog.rs`: URLs pin immutable releases/revisions,
byte counts and SHA-256 hashes. Increasing the catalog requires a reviewed
code change and a native smoke test on Windows x64.

## Components

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
