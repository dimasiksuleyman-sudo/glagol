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
- Windows 10/11 x64, NVIDIA GeForce GTX 1060 6 GB (Pascal), driver: <version>
- CPU: Intel Xeon E5-2650 v2 (AVX, F16C, no AVX2)
- Model: `qwen-talker-0.6b-customvoice-Q8_0.gguf` + `qwen-tokenizer-12hz-Q8_0.gguf`
  (Serveurperso/Qwen3-TTS-GGUF @ b7ee2e8c), speaker `ryan`, language `Russian`

**What happens**
The model loads on `Vulkan0` (~9.5 s), synthesis starts, and after 10–23 s the
process terminates (no error status returned from `qt_synthesize`). Same with
`clamp_fp16 = true` and `codec_chunk_sec = 6`. With `device = "CPU"` the same text
synthesizes correctly. On an AMD RX 550 (2 GB, `fp16: 0`) and on an integrated
Radeon (Ryzen 7 laptop) the Vulkan backend works (the RX 550 needs `--clamp-fp16`
and runs out of memory in the codec on long texts).

**Reproduce**
```
qwen-tts.exe --model qwen-talker-0.6b-customvoice-Q8_0.gguf --codec qwen-tokenizer-12hz-Q8_0.gguf ^
  --device Vulkan0 --speaker ryan --lang Russian -o out.wav < text.txt
```
Output / last lines before the crash:
```
<paste here>
```

**Expected**
Synthesis completes on Vulkan0, or a clean error status if the device is unsupported.

Thanks for qwentts.cpp — it runs beautifully on the other cards we tried.
