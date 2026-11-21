# Черновик issue для qwentts.cpp: падение Vulkan на GTX 1060

Куда: https://github.com/ServeurpersoCom/qwentts.cpp/issues/new
Публикует пользователь со своего аккаунта. Перед отправкой вставить вывод
`qwen-tts.exe` из шага воспроизведения (без личных данных).

---

**Title:** Vulkan backend crashes mid-synthesis on NVIDIA Pascal (GTX 1060 6 GB), CPU works

**Environment**
- qwentts.cpp `2edb35f87b709a070b2d5379f77e5c52a1daa585` (ggml submodule `ef4555b7`),
  built on GitHub Actions `windows-latest` with `-DQWEN_SHARED=ON -DGGML_VULKAN=ON
  -DGGML_CPU_ALL_VARIANTS=ON -DGGML_BACKEND_DL=ON`, Vulkan SDK 1.4.313.2
- Windows x64, NVIDIA GeForce GTX 1060 6 GB (Pascal), driver 581.80;
  ggml_vulkan reports `fp16: 0 | bf16: 0 | warp size: 32 | shared memory: 49152 | int dot: 1 | matrix cores: none`
- CPU: Intel Xeon E5-2650 v2 (AVX, F16C, no AVX2)
- Model: `qwen-talker-0.6b-customvoice-Q8_0.gguf` + `qwen-tokenizer-12hz-Q8_0.gguf`
  (Serveurperso/Qwen3-TTS-GGUF @ b7ee2e8c), speaker `ryan`, language `Russian`

**What happens**
The model loads on `Vulkan0` (~9.5 s) and the talker finishes generation
(`[Pipeline] EOS at step 25`), then — apparently during the codec decode — the
driver reports page faults and the device is lost; the process ends with
0xC0000409 (-1073740791) instead of returning an error status. Same with
`clamp_fp16 = true` and `codec_chunk_sec = 6`. With `device = "CPU"` the same text
synthesizes correctly. On an AMD RX 550 (2 GB, `fp16: 0`) and on an integrated
Radeon (Ryzen 7 laptop) the Vulkan backend works (the RX 550 needs `--clamp-fp16`
and runs out of memory in the codec on long texts).

**Reproduce**
```
qwen-tts.exe --model qwen-talker-0.6b-customvoice-Q8_0.gguf --codec qwen-tokenizer-12hz-Q8_0.gguf ^
  --device Vulkan0 --speaker ryan --lang Russian -o out.wav < text.txt
```
Exit code `-1073740791`. Log (abridged):
```
[GGML] ggml_vulkan: 0 = NVIDIA GeForce GTX 1060 6GB (NVIDIA) | uma: 0 | rebar: 0 | fp16: 0 | bf16: 0 | fp4: 0 | warp size: 32 | shared memory: 49152 | int dot: 1 | matrix cores: none
[Load] Talker backend: Vulkan0 (CPU threads: 8)
[KVCache] Allocated: 28 layers, 8 KV heads, head_dim 128, max_seq_len 4096, 1 sets -> 896 MB
[Pipeline] Loaded: arch=0b6 variant=custom_voice tokenizer=qwen3_tts_tokenizer_12hz codebooks=16 speaker_encoder=absent speakers=9 fa=on clamp_fp16=off max_batch=1
[Pipeline] EOS at step 25, stopping (slot 0)
[GGML]   address fault 0: type=4 address=0x28d5f07b8 precision=0x8
[GGML]   address fault 1: type=4 address=0x28d5ee190 precision=0x8
...
[GGML]   address fault 169: type=4 address=0x28d5eef70 precision=0x8
[GGML] ggml_vulkan: device lost on Vulkan0
```
(The same happens with `--clamp-fp16` and a 6 s codec chunk; `--no-fa` was not
tried on this card.)

**Expected**
Synthesis completes on Vulkan0, or a clean error status if the device is unsupported.

Thanks for qwentts.cpp — it runs beautifully on the other cards we tried.
