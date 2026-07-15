# Glagol — Day 9 Master Log

**Period:** July 15, 2026 (Sprint 6 Dictation marathon — PR2 Q&A through PR2.1 runtime-QA closure)
**Maintainer:** dimasiksuleyman-sudo (Glagol Contributors)
**Sprint:** Sprint 6 — Dictation (system-wide push-to-talk dictation without a system VPN) — **PR2 of 5**
**Status at end of Day 9:** PR1 + PR2 + PR2.1 merged to `main`. **254 tests passing, 1 ignored, 0 regressions.** Recorder proven on real Windows hardware. Marathon tag deferred to PR5 closure per plan.

> Marathon PR1 (`feat/stt-client`, PR #35) landed July 14 and is covered by its own log.
> This file covers Day 9: the PR2 Q&A round, the PR2 kickoff, PR #37 (recorder), PR #38 (recorder hotfix), and the four-step runtime QA that closed PR2 honestly.

---

## TL;DR

Day 9 delivered the microphone-capture layer of the Dictation marathon: a dedicated recorder thread that yields 16 kHz / mono / S16LE audio to the rest of the pipeline, plus the hotfix that made it work on real hardware.

1. **PR2 Q&A round — 14 questions, all locked.** Dependency freshness verified against live crates.io rather than memory. User amendments landed on 6 of 14 answers (resampler flush latency, `SampleFormat` non-exhaustiveness, range-based config search, kebab-case event naming, best-effort `PermissionDenied`, `withGlobalTauri` QA recipe).

2. **Kickoff PR2 — 15 locked decisions (D1–D15)** covering dependency pins, thread-ownership model, config ladder, sample formats, the `PcmAudio` contract, resampling, the 60 s cap, device selection, error taxonomy, and verification strategy.

3. **PR #37 — recorder in 3 phases.** 218 → **250 tests** (+32). Negative regression cycle on the resampler flush verified live. All gates green.

4. **Two forced deviations from D1, both correct, both mine to own.** The `rubato 3.0.0` pin named an API that does not exist in that version; the `cpal 0.18.1` pin split the `windows` crate graph against Tauri and turned CI red on Windows while Linux stayed green. Landed as `rubato 0.16.2` and `cpal 0.17.3` with amendments D1-A / D1-B.

5. **Runtime QA found a real defect on real hardware.** `live_mic` failed with `UnsupportedConfig("U8")`: the config ladder selected by (channels, rate) while the sample format was validated only afterwards. The user's device advertises mono@16k **in U8** — the ladder grabbed the "ideal" step and killed the recording while a working 48k/stereo/F32 config sat one step below.

6. **PR #38 — hotfix, `recorder.rs` closed as one unit.** D3-A (format in the selection predicate), D8-A (`resample_to_16k` → `Result`; every fallible point audited; the silent-degradation fallback deleted), D12-A (`build_stream_error` mapped by variant, verified against the cpal source), plus a nitpick and hostile-format test fakes. 250 → **254 tests** (+4). Both mandatory negative cycles verified.

7. **Runtime QA closed honestly across 4 steps** — dependency graph, Windows gates, live microphone, application. `live_mic`: 30720 samples / 1920 ms / 16 kHz / `truncated=false` from `Capture Input terminal`.

**The Working Agreements protocol earned its keep four times in one day.** The PR2 kickoff shipped with four defects. Not one was caught by its author: CC caught two at implementation, CI caught one on Windows, the user's hardware caught the fourth. Each layer caught what only that layer could see.

---

## Sprint 6 Dictation — where the marathon stands after Day 9

```
PR1  feat/stt-client              ✅ merged (#35, Jul 14) + hotfix (#36, Jul 15)
PR2  feat/dictation-recorder      ✅ merged (#37) + hotfix (#38)
PR3  feat/dictation-hotkey-overlay   ← next
PR4  feat/dictation-paste
PR5  feat/dictation-page
                                     tag after PR5
```

**What exists after Day 9 (still no user-facing surface — by design):**

```
[PR3: hotkey] ──► [PR2: recorder thread] ──► PcmAudio (16k/mono/i16) ──► [PR1: wav.rs] ──► [PR1: SttProvider]
                        │
                        └──► RMS ~50 ms ──► LevelSink ──► emit "dictation-level" ──► [PR3: overlay]
```

The invariant PR2 establishes for the whole marathon: **everything leaving the recorder is already 16 kHz / mono / S16LE.** No downstream consumer handles audio format. `wrap_wav_s16le_mono()` from PR1 connects without an adapter.

PR2 is deliberately invisible: its only command, `list_audio_input_devices()`, has no consumer until PR5. That is the price of "one PR = one concern"; the compensation is an `#[ignore]` live-microphone test, which is exactly what caught the U8 defect.

---

## Entry state

- PR1 (`#35`) merged July 14, runtime-QA'd live: AITunnel `/models` responded, cost 0 ₽, test key revoked afterwards.
- **PR #36** merged July 15 at 15:57 local — a PR1 follow-up (validation-cache reset, probe timeout, atomic settings, 413 boundary), outside this session's transcript. Recorded here because it explains the baseline: its body states **218 passed (216 → 218)**.
- Baseline at PR2 branch point: **218 tests**.

> **Bookkeeping note.** The session opened quoting 216 tests; CC's paste-back claimed a 218 baseline. Both were right — PR #36 landed between them. The discrepancy was flagged rather than waved through, and closed by fetching the PR record. Arithmetic in a PR body must reconcile, or nobody can reconstruct it three sprints later.

---

## PR2 Q&A round — 14 questions

The marathon plan required pinning `cpal` and `rubato` "on a Q&A freshness check". Checked against live crates.io on July 15, 2026:

| Crate | Candidates | Data |
|---|---|---|
| cpal | **0.18.1** (Jun 7, 2026, MSRV 1.85, ~103k downloads in 5 weeks) vs 0.17.3 (Feb 18, 2026) | 0.17.3 still dominant by volume — ecosystem lag |
| rubato | **4.0.0** (Jul 9, 2026 — 6 days old, 2.7k) / **3.0.0** (May 20, 2026, 51k) / 1.0.1 (1.05M) | Four majors in six months: 1.0 → 4.0 |

**Answers (14/14 locked), with the user's amendments:**

| # | Question | Outcome |
|---|---|---|
| 1 | cpal pin | `0.18.1` — later overturned by CI (D1-B) |
| 2 | rubato pin | `3.0.0` — later overturned by CC (D1-A) |
| 3 | Resampler | `FftFixedIn` + tail trim **+ final flush** — *user amendment: FFT resamplers hold the last milliseconds in filter latency; without a drain the end of the last word is eaten* |
| 4 | Sample formats | F32 + I16, `_ => Err(UnsupportedConfig)` — *user amendment: `SampleFormat` is `#[non_exhaustive]`, the catch-all must not panic* |
| 5 | Callback transport | mpsc — *user amendment: `send(Vec<f32>)` allocates in the callback; acceptable for dictation, but the choice is recorded as deliberate* |
| 6 | Config ladder | *user amendment: search `supported_input_configs()` by **range** via `try_with_sample_rate`, not exact match — some drivers report ranges* |
| 7 | Levels | `LevelSink` trait + emit in PR2 — *user amendment: event name `dictation-level`, kebab-case per project convention (`synthesis-completed`, `backup-progress`), not `dictation://level`* |
| 8 | 60 s cap | Auto-finalize with `truncated: true` — losing 60 s of dictation is the worst outcome available |
| 9 | Recorder output | Ready `PcmAudio` at 16k/mono/i16 |
| 10 | <300 ms rejection | Stays in PR3 — rejection is pipeline *policy*, the recorder is *mechanism* |
| 11 | Device selection | `stt_input_device` in `app_settings`; missing device → fall back to default **as a warning, not an error** |
| 12 | Error taxonomy | *user amendment: `PermissionDenied` is honestly best-effort — WASAPI gives no clean "denied in Privacy settings" signal; the reliable winreg ConsentStore detector is deferred* |
| 13 | `DictationPhase` | `std::sync::Mutex`, block-scoped guard, never held across `.await` |
| 14 | QA without UI | `#[ignore]` live-mic test — *user amendment: `withGlobalTauri` is absent from `tauri.conf.json` (default false); Tauri 2 injects `__TAURI_INTERNALS__.invoke` regardless, so the flag is only needed for `listen`, and only for a local QA session* |

Test target set at **"not less than 230"** (baseline 218), per the project's "not less than X" convention.

---

## Kickoff PR2 — 15 locked decisions

D1 pins · D2 single-`RecorderMsg`-channel ownership model · D3 config ladder · D4 sample formats · D5 downmix · D6 RMS · D7 `PcmAudio` contract · D8 resampling · D9 60 s cap · D10 device selection · D11 `LevelSink` + `dictation-level` · D12 error taxonomy · D13 `DictationPhase` · D14 verification · D15 rejection stays in PR3.

Two decisions worth surfacing, because both proved load-bearing:

**D2 — one channel, not two.** `std::sync::mpsc` cannot `select` across channels. Merging commands, samples and stream errors into a single `RecorderMsg` queue gave the recorder thread one `recv_timeout(50ms)` loop — no polling, no extra crate — and the same tick drives the RMS emit and the cap check. `FakeSource` sends the same `Samples(..)` messages, so the test seam came for free. Reply channels are `tokio::oneshot` (its `send()` is sync and non-blocking from the recorder thread) while the command channel is `std::sync::mpsc` (the thread needs a blocking `recv_timeout`; `cpal::Stream` is not `Send` under WASAPI and must live on a plain `std::thread`).

**Directive that saved the day:** *"read docs.rs strictly for the pinned version; examples in the wild and in model memory are for cpal 0.17 and rubato 0.x — the APIs broke."* This directive caught its own author's error within hours (see D1-A).

---

## PR #37 — recorder implementation (3 phases)

**Branch:** `claude/dictation-recorder-backend-jm6ucx` · **merged** July 15, 15:24 UTC · **squash SHA** `858d14d`

| Phase | Content |
|---|---|
| 1 | `dictation/mod.rs` (types, seams, pure `downmix_to_mono` / `rms` / `f32_to_i16`) + `dictation/resample.rs` |
| 2 | `dictation/recorder.rs` — thread loop, config ladder, `CpalSource`, 60 s cap, state machine, `#[ignore]` live-mic gate |
| 3 | `state.rs` · `lib.rs` · `commands/dictation.rs` · `src/lib/tauri.ts` · Cargo.toml/lock |

**Tests: 218 + 32 = 250 passing (+1 ignored).** Target was ≥230.

**Negative regression cycle (D8, mandatory):** `MAX_FLUSH_ROUNDS → 0` makes `flush_keeps_the_tail_of_the_last_word` fail (output short of 16 000 frames); restored → pass. The fix is load-bearing, not a hopeful patch.

**Accepted deviation — incremental downmix.** Each `Samples` batch is downmixed on arrival rather than once at finalize: functionally identical, and it lets the RMS window and the sample-counted 60 s cap share one mono buffer. Better than the kickoff's version.

**Environment note (sandbox only):** the cloud sandbox cannot fetch Pdfium (proxy 403), so `build.rs` leaves `PDFIUM_LIBRARY_PATH` unset and `env!` hard-fails; baseline fails identically. Unrelated to PR2 and **not** a CI defect — later confirmed clean on Windows, where `pdfium-render v0.9.1` builds with no stubs. Kept out of the PR body.

---

## Deviation 1 — rubato (amendment D1-A)

CC declined the `rubato 3.0.0` pin and shipped `0.16.2`, flagged loudly and revertibly.

**Verified on docs.rs, July 15, 2026:**

| Version | Public structs |
|---|---|
| **0.16.2** | `FftFixedIn`, `FftFixedInOut`, `FftFixedOut`, `SincFixedIn`, `SincFixedOut`, `FastFixedIn`, `FastFixedOut` |
| **3.0.0** | `Async`, `Fft`, `Indexing`, `MissingCpuFeature`, `SincInterpolationParameters` |

**`FftFixedIn` does not exist in 3.0.0.** D8's implementation — `FftFixedIn` + `process_partial` + `output_delay` — is literally unbuildable on the pinned version. rubato 1.0 moved to an `audioadapter` / `audioadapter-buffers` paradigm (confirmed in the dependency manifests: 0.16.2 has neither; 1.0.1 and 3.0.0 both carry them).

**Verdict: accept 0.16.2.** D1's stated *reason* was "prefer proven over the newest major". By that reason 0.16.2 (2.28M downloads, 15 months in the field, narrow dep tree, no RustSec advisories, MSRV 1.61) fits better than 3.0.0 (51k, 8 weeks, already superseded by 4.0.0). The version string was wrong; the intent was right.

**Honest counterpoint, recorded for the future:** `Fft` in 3.0.0 exposes `process_all_into_buffer` + `process_all_needed_output_len`, which would collapse our offline pad/flush/trim dance into one call. Real, but not worth rewriting a green, tested module against an API that broke four times in six months. Revisit only if the 0.x line starts costing us.

**Root cause:** the pin was chosen by release date and download count. Nobody looked at the API surface of the pinned version.

---

## Deviation 2 — cpal (amendment D1-B) — the CI catch

CI turned **red on Windows while Linux stayed green**.

**Verified on crates.io manifests:**

| Version | Windows dependencies |
|---|---|
| **cpal 0.18.1** | `windows` `>=0.61, <=0.62` **and** `windows-core` `>=0.61, <=0.62` — two independent loose ranges |
| **cpal 0.17.3** | `windows` `>=0.59, <=0.62` — single declaration |

Alongside Tauri the resolver is free to split those ranges — `windows 0.61.3` for COM types, `windows-core 0.62.2` for the `#[implement]` macro — and cpal's macro-generated code then fails its trait bounds. Under 0.17.3 the split is structurally impossible: one declaration, one line.

**Correction recorded for accuracy:** "0.18.1 simply cannot compile" is too strong — Whispering ships 0.18.1 with Tauri, because *their* lockfile resolved both crates onto 0.62 coherently. The precise statement: **0.18.1 is reachable only via a lockfile constraint that fights Tauri's transitive resolution and can silently re-split after any `cargo update`.** 0.17.3 is the deterministic fix, and was D1's own explicit runner-up.

**API delta handled:** `name()` → `description()`, split `BuildStreamError` / `StreamError`, config passed by reference. `SampleRate` is `u32`, so the D3 ladder was unaffected.

**Root cause:** the pin was justified by "Whispering ships it in prod". True — and non-transferable. Prod validation of another project is a statement about *their* dependency graph.

**Graph verified on Windows after the fix:**

```
windows v0.61.3
├── cpal v0.17.3
└── tao / tauri 2.11.1 / wry / webview2-com / tauri-runtime-wry
```

cpal sits on `windows 0.61.3` → `windows-core 0.61.2`, the same line as the entire Tauri graph. `windows-core 0.62.2` has zero dependents on the Windows target — it exists only via `chrono` → `iana-time-zone`. Two lines coexist because their consumers never intersect; the fatal case was *one crate* straddling both.

> The initial pass/fail criterion for this check ("expect exactly one version of `windows-core`") was itself wrong — two versions in a Tauri lockfile are normal. The correct criterion is **cpal on one coherent line**.

---

## Runtime QA — the U8 finding (amendment D3-A)

First run of the live-microphone gate on the target platform:

```
thread 'dictation::recorder::tests::live_mic' panicked at src\dictation\recorder.rs:734:45:
microphone starts: UnsupportedConfig("U8")
```

**Not a hardware failure — a composition defect between D3 and D4, present in the kickoff.**

The ladder selected configs on two criteria: `channels == 1` and `16000 ∈ [min_rate, max_rate]`. `sample_format()` appeared nowhere in the predicate. The user's device honestly advertises mono@16k — **in U8**. The ladder grabbed step 1 ("ideal! no downmix, no resample!"), handed it to D4, and D4 said: F32 or I16, everything else is an `Err`. Two decisions written apart, never composed: D3 picks without looking at format, D4 assumes the format is already good.

**The irony: the fallback would have worked perfectly.** Step 3 (`default_input_config()`) returns the WASAPI mix format — 48k stereo F32. The ladder killed the recording chasing an "ideal" while a working path sat right there.

**The deeper error was the priority order.** The ladder optimised for "avoid resampling" and ignored bit depth entirely. But mono@16k in **U8** is 8-bit — ~48 dB of dynamic range, materially worse for Whisper than 48k stereo F32 with a resample. Even if U8 were supported, choosing it would be wrong. **Format outranks sample rate.**

**Why the unit tests missed it:** the ladder tests ran against fake `SupportedStreamConfigRange` values whose formats were all "nice". The fakes were polite; the hardware was not.

> **Lesson banked: a fake that only offers good inputs tests the author's optimism, not the code.**

---

## PR #38 — recorder hotfix (D3-A / D8-A / D12-A)

**Branch:** `fix/recorder-config-format` · **merged** July 15, 17:10 UTC · **squash SHA** `2afba89` · 2 files, +240 / −86

**Form decision — standalone hotfix, not folded into PR3.** The counter-proposal (fold D3-A into PR3, since the bug is unreachable until a hotkey calls `Start`) was reasonable and rejected on three grounds:

1. **PR2's runtime QA was open, and it is a protocol step, not a courtesy.** Folding it forward would leave the marathon's first hardware-touching PR permanently un-QA'd, with the verdict arriving retroactively inside someone else's PR.
2. **Squash-merge erases D3-A as a distinct commit.** A fix born from a QA failure would vanish into `feat: hotkey + overlay + pipeline` and cease to exist in git history. The kickoff records *intent*; git records *what happened*.
3. **D3-A is a precondition of PR3, not a companion.** The device is known to advertise U8 — PR3's own QA would hit this on the first hotkey press.

The valid part of the counter-proposal — "one reviewable unit for the `recorder.rs` tail" — was honoured by putting **all three nitpicks in the same hotfix**, closing `recorder.rs` entirely and opening PR3 on a clean recorder.

**Contents:**

- **D3-A** — `sample_format` is part of the selection predicate. Steps 1–2 consider only F32/I16 ranges, preferring F32; an unsupported-format range is **skipped**, so the ladder falls through to a working config rather than erroring.
  *Negative cycle:* U8 mono@16k + F32 stereo@48k → ladder picks the F32 step (`Some(1)`); drop the `format_rank` guard in `best_supported` → returns `Some(0)` (the U8 mono) → test fails. Restored → green.
- **D8-A** — `resample_to_16k` → `Result`. Every fallible point audited: constructor, each `process` / `process_partial`, and a short output after flush (tail lost) — all now **loud `Err`**. The old *"constructor failed → return the native-rate buffer tagged 16 kHz"* silent-degradation fallback is **deleted**. Identity and empty paths stay infallible in behaviour; only the signature changed.
  *Negative cycle:* `MAX_FLUSH_ROUNDS = 0` → `Err("resample produced 15903 of 16000 frames at 48000 Hz (tail lost)")` → the flush test fails. Restored → green.
- **D12-A** — `build_stream_error` mapped by variant, **verified against the cpal 0.17.3 source** rather than guessed: `windows_err_to_cpal_err_message` special-cases only `AUDCLNT_E_DEVICE_INVALIDATED` / `_IN_USE` → `DeviceNotAvailable`; `E_ACCESSDENIED` (a denied microphone from `IAudioClient::Initialize`) falls to `_ => BackendSpecific`. Hence `DeviceNotAvailable → DeviceLost`, `StreamConfigNotSupported → UnsupportedConfig`, `BackendSpecific → PermissionDenied` (the Privacy-settings hint stays where the denial actually lands), `InvalidArgument` / `StreamIdOverflow → BuildStream`. Still best-effort per D12; the winreg ConsentStore detector remains deferred.
- Nitpick — `mono` → `interleaved` in the I16 capture callback.
- Ladder test fakes now carry hostile formats (U8) as a rule.

**Tests: 250 → 254 (+4).** Both mandatory negative cycles verified live.

### Why D8-A had to be an error, not a comment

The proposed nitpick was "add a comment on the degradation path". It was upgraded to a hard error on a class-of-failure argument, and it is worth preserving verbatim:

**Loud and rare is acceptable; silent and rare is a landmine.** Returning native-rate samples tagged `16_000` means Whisper receives audio slowed by 2.75–3×, and returns *plausible nonsense* — grammatical Russian with punctuation and capitals — which is then pasted into the user's document. No point in the chain notices: not the recorder, not the pipeline, not the provider, not the user. Worse, the defect **masquerades as poor model quality**: the user concludes "dictation recognised it badly", not "the audio path is broken". A comment protects the reader of the code; it does not protect the user.

A third option — return `PcmAudio` with the honest native rate (Whisper endpoints happily eat 48 kHz, so nothing is lost) — was considered and rejected: it is a code path that never executes, but it would permanently burden every `PcmAudio` consumer with the invariant "the rate may be anything". The cost is forever; the benefit sits on an unreachable branch. **A simple loud error is cheaper and more honest.**

D9's "never lose dictation" is not violated in spirit: D9 protects a *good* recording (the cap, a lost `Released`). Here the audio path itself failed, and a recording that transcribes to garbage carries no value.

---

## Runtime QA closure — 4 steps

| Step | Check | Result |
|---|---|---|
| **0** | Dependency graph | ✅ cpal 0.17.3 → windows 0.61.3 → windows-core 0.61.2, one coherent line with Tauri; 0.62.2 inactive on Windows (chrono only) |
| **1** | Windows gates (**authoritative**, not Linux) | ✅ `254 passed; 0 failed; 1 ignored` · clippy `-D warnings` clean · fmt clean |
| **2** | Live microphone | ✅ `recording from: Capture Input terminal (fell_back_to_default=false)` / `captured 30720 samples, 1920 ms, truncated=false` |
| **3** | Application | ✅ `list_audio_input_devices()` → `['Capture Input terminal', 'Microphone']` · CPU ~0 % idle · clean exit · working tree clean |
| **4** | TTS regression (PR2 touched `credentials.rs`, `synthesize.rs`) | ✅ synthesis unaffected |

**Arithmetic check on step 2:** 30720 / 16000 = 1.92 s — exactly the reported `1920 ms`. The D7 contract holds on real hardware, not only in fakes. `254 filtered out` confirmed the fixed binary was actually under test.

**Confirmed in passing:** `__TAURI_INTERNALS__.invoke` works **without** `withGlobalTauri` — Tauri 2 always injects internals. The flag is needed only for `listen`, which was dropped from this QA with no loss: `dictation-level` cannot fire before PR3 supplies a recording trigger, so subscribing would test Tauri's event system rather than our code.

**What this QA deliberately did not prove:** which ladder step won on real hardware. 30720 output samples arise identically from 30720 native samples and from 92160 divided by three, so the numbers cannot distinguish "step 1 F32 mono" from "step 3, downmix + resample". For a marathon where resampling quality converts directly into transcription quality, that is worth knowing → carried to PR3.

---

## Carry-over registry

| → | Item | Origin |
|---|---|---|
| **PR3** | `UnsupportedConfig` now carries three meanings (device format / `from_rate = 0` / resampler tail loss). The third is *our* bug, but the Russian user-facing text blames the device. Loud, but pointing the wrong way. | PR #38 sanity |
| **PR3** | `live_mic` should print the chosen config (rate/channels/format) — the field-diagnostics dump for three more Windows-audio PRs | QA step 2 |
| **PR3** | Question for CC: what is `MAX_FLUSH_ROUNDS`, and what is the margin at 44.1k (2.75625 — the tightest ratio)? If generous, the tail-loss branch is truly dead. | PR #38 sanity |
| **PR3 QA** | `listen('dictation-level')` — observable only once a hotkey exists | QA step 3 |
| **PR5** | **Revisit D10.** Device names are generic: `Capture Input terminal`, `Microphone`. Name-based identity risks a silent first-match collision, and a picker built from these strings cannot answer "which one is my headset". The stable WASAPI endpoint ID is not exposed by cpal 0.17.3 (COM + `unsafe` via windows-rs). Decide deliberately, not by default. | QA step 3 |

**On D10:** the locking argument was "cpal device names on Windows are stable". Stability held. Stability is not the same as **distinguishability**, and the two were conflated.

---

## Stats — Day 9

| Metric | Value |
|---|---|
| PRs merged | 3 (#36 PR1 follow-up, #37 PR2, #38 PR2.1) |
| Tests | 216 → 218 (#36) → 250 (#37) → **254** (+1 ignored) |
| Regressions | **0** |
| New dependencies | `cpal 0.17.3`, `rubato 0.16.2` (+ transitive `realfft`, `rustfft`, `dasp_sample`) |
| Migrations | **none** — `app_settings` arrived in PR1 |
| Security invariants touched | **none** — the recorder makes no network requests; invariant #3 was reformulated back in PR1 |
| New `unsafe` | none |
| Mandatory negative cycles | 3 (flush in #37; format ladder + flush in #38) |
| Amendments to a shipped kickoff | 5 (D1-A, D1-B, D3-A, D8-A, D12-A) |
| Kickoff defects found by the author | **0 of 4** |

---

## Lessons learned — Day 9

1. **A version pin means verifying the API surface and dependency graph of the pinned version.** Release date and download count are not evidence. Cost: two forced deviations in one PR.

2. **For platform-specific crates (cpal / enigo / global-shortcut / windows-rs), Linux gates are advisory; Windows is authoritative.** Green on ALSA, red on WASAPI, twice in one day — once at compile (windows-core split), once at runtime (U8). PR3 and PR4 live entirely in this zone.

3. **Prod validation of someone else's project does not transfer.** "Whispering ships cpal 0.18.1" was true and useless: it describes their lockfile.

4. **Test fakes must carry hostile variants.** A fake offering only good formats validates the author's optimism.

5. **Classify failures before choosing a response.** Loud-and-rare is acceptable; silent-and-rare is a landmine — especially when the silent failure produces *plausible* output that masquerades as low model quality.

6. **A protocol's value is not that the kickoff is right — it is that every layer has someone who checks it.** Four kickoff defects, four different catchers: CC (rubato API, at implementation), CI (windows-core split, on Windows), hardware (U8, at runtime), and the review of CC's own report (the mapping mismatch). Zero caught by the author.

7. **Rigor applies to one's own reasoning too.** The claim "identical binary hash proves a stale build" was wrong — the hash derives from crate metadata, not source contents, and stayed identical across a genuine 2m42s rebuild. The conclusion was right (`250 filtered out` and the absent recompile proved it); one of its three legs was rotten. Demanding sources from others while arguing from plausibility oneself is the same defect the day cost twice.

---

## What's next

**PR3 — `feat/dictation-hotkey-overlay`:** the largest and riskiest PR of the marathon. `tauri-plugin-global-shortcut` + `arboard`, tray icon (`+tray-icon, image-png`), a runtime-created transparent always-on-top overlay window that must not steal focus, the first real pipeline (`run_dictation<P: SttProvider>`), the <300 ms / RMS-silence rejection, the 60 s watchdog for a lost `Released`, and a product change (closing the main window minimises to tray). First PR of the marathon with a user-facing surface: **dictation to clipboard**.

Everything in it sits in the "Windows authoritative" zone. The PR3 kickoff opens with a hard Q&A round and inherits four carry-over items.

Then PR4 (paste via `enigo`), PR5 (Dictation page, history, migration), the 18-point manual QA checklist, and the marathon tag.

---

## Reference

- **Repo:** https://github.com/dimasiksuleyman-sudo/glagol
- **PR #35 (Dictation PR1 — STT client):** merged July 14, 2026 · `603ae01`
- **PR #36 (PR1 follow-up):** merged July 15, 2026 12:57 UTC · `da473ab` · 216 → 218
- **PR #37 (Dictation PR2 — recorder):** merged July 15, 2026 15:24 UTC · `858d14d` · 218 → 250
- **PR #38 (Dictation PR2.1 — recorder hotfix):** merged July 15, 2026 17:10 UTC · `2afba89` · 250 → 254
- **Marathon plan:** `PlanofRUWhisper.md`
- **PR2 kickoff (D1–D15 + amendments D1-A / D1-B / D3-A / D8-A / D12-A):** `docs/day-logs/kickoff-dictation-pr2-recorder.md`
- **cpal 0.17.3 audit point:** `cpal-0.17.3/src/host/wasapi/mod.rs::windows_err_to_cpal_err_message`
- **Marathon tag:** deferred to PR5 closure per plan
- **`main` HEAD at Day 9 close:** `2afba89` (post-#38), working tree clean

---

*Day 9 captures: PR2 Q&A round (14 questions, 6 user amendments) + PR2 kickoff (D1–D15) + PR #37 recorder in 3 phases (+32 tests) + two forced D1 deviations verified against live sources (rubato API surface, cpal windows-core split) + runtime QA finding a real composition defect on real hardware (U8) + PR #38 hotfix closing `recorder.rs` as one unit (D3-A / D8-A / D12-A + nitpick + hostile fakes) + four-step runtime QA closure including the marathon's first live microphone capture.*
*254 tests, 1 ignored, 0 regressions. Five amendments to a shipped kickoff, none of them found by its author — and all five found by the protocol.*
*Last updated: July 15, 2026 ~21:30 local time*

---

*Created by Dmitriy + Claude*
