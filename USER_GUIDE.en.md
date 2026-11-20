# Glagol user guide — 0.6.6

[Русский](USER_GUIDE.ru.md) · [README](README.md)

Glagol reads English and Russian documents aloud and turns dictation into text.
This guide describes [Glagol 0.6.6](docs/releases/v0.6.6.md).
The GitHub link at the bottom of the sidebar opens the author's profile in your browser.

## Installation

Windows 10/11 x64 is required. Run the locally built `Glagol_0.6.6_x64-setup.exe`.
The installer offers English and Russian, starts with Windows' language and uses
English as fallback. The unsigned installer may trigger SmartScreen: check the
artifact's source and SHA-256 before choosing More info → Run anyway. Installation
and removal do not download speech models or accept Silero's terms.

On first launch choose **English / Русский** using the large buttons. Your choice
is saved immediately. Then set up dictation. A new installation starts with text to
speech for documents turned off: instead of its card you see a note that it can be
turned on later in Settings → Text to speech; Silero models are neither offered nor
verified, and there is no Synthesize page. After an upgrade your previous choice is
kept: if text to speech was on, both cards are shown at this step. You can install
nothing; Continue opens the app without requiring a download. Closing and reopening resumes the saved setup step. With no
network you can still skip setup and use the library or existing local components.

On a new installation, both speech languages initially match the chosen interface;
dictation starts in local mode. Changing the interface later changes neither speech
language. Switch interface language in the shell or Settings without restarting.

## Updating an existing installation

From 0.6.0 the current version is shown in the sidebar and in Settings → General → Updates.
Check for updates asks GitHub whether a newer version exists. Check automatically at
startup is off by default; when on, Glagol checks once after it starts and shows a
notification about a new version. While it is off, a reminder with Check now and
Enable auto-check buttons appears at startup at most once a month; a manual check
postpones it. Install update shows what is new in that version (from 0.6.1) and,
after confirmation, downloads the
installer, verifies its digital signature, closes Glagol, installs the new version and
starts it again; documents, settings, keys and speech components are kept. Installation
does not start during dictation or a speech operation. A file with an invalid signature
is never installed. Versions before 0.6.0 update manually as described below.

Back up your library, exit through the tray, run the new installer and retain the
existing directory (choose “Do not uninstall” on the existing-installation page).
The first upgrade from 0.4.1 also asks for interface language. Existing local/server/cloud
modes, models, voices, addresses and credentials are retained, including old implicit
Russian/cloud settings. The speech cards show the saved configuration. A Russian
voice previously kept in the webview is transferred once into saved preferences.

Existing WAVs need no installed TTS engine to play. Old document metadata is preserved;
new synthesis stores its speech language. Restoring an older backup remains supported.
Installation, live-microphone and listening checks of this candidate remain separate
from automated tests; see the [work log](docs/workstreams/english-first/log.md).
[Current 0.5.0 screenshots](docs/screenshots/0.5.0/README.md) show English synthesis, local
dictation and the library, plus completed synthesis in the Russian interface. Older [0.4.0 screenshots](docs/screenshots/windows10-0.4.0/README.md) are historical.

## Interface and help

The sidebar has Synthesize, Library, Dictation, Settings and Help. The Dictation page
starts with a readiness line (where speech is recognized, model, device, shortcut) with a
Change button, followed by insertion mode, shortcut, microphone and history. Settings are
split into Dictation, Text to speech and General tabs (interface language, updates,
backup, diagnostics); rarely used parts are collapsed: Manage models, Advanced, and
Components and license.

Help collects short steps for common tasks in Dictation, Text to speech and General
groups. Show opens the right screen and highlights the real button or switch; Got it, a
click outside the hint or Esc closes it. A «Did you know?» card on Synthesize, Dictation
and Library occasionally describes a feature you have not used yet: at most once a week
in the first month, then once a month. Close it, or turn tips off with the card's check
box or the Show tips switch in Help. What you have seen is stored only on this computer.

## Qwen3-TTS synthesis (lively voice)

In Settings → Text to speech → «Speech engine» choose **Qwen3-TTS** and click
«Download and enable». Qwen3-TTS models are Apache 2.0 and the qwentts.cpp
runtime is MIT: **commercial use is allowed** and no separate acknowledgement is
needed. About 1.3 GB is downloaded (the 0.6B model, the codec and the runtime)
from pinned addresses and checked by SHA-256 before anything runs.

Qwen3-TTS reads with natural intonation and pauses. The voices are Qwen's named
speakers (Ryan, Aiden, Serena, Vivian and others); the same voice reads Russian and
English text, and «Speech language» selects which. Try them with «Preview voice».
Numbers, the pronunciation dictionary and AI normalization work as with Silero.

**Reading style.** On the Synthesize page, below the Qwen3-TTS voice, «Reading
style» offers «Even, without emotion» (the default: Qwen reads emotionally, which
news and documents do not need), «As the model decides» (natural intonation),
«Calm and slow», «News anchor», «Warm and friendly», «Cheerful and energetic» or your
own description of up to 200 characters. The description is sent to the model with
each text fragment; the effect depends on the voice, so click «Listen» before
synthesizing a long document. The choice is remembered on this computer. It does
not apply to Silero.

Speed depends on the computer. With a dedicated graphics card synthesis is faster
than real time; on a modern laptop's integrated graphics it is about 1.5 times
slower than real time (a minute of news in a minute and a half), and on old
computers without AVX2 many times slower. Glagol picks the graphics card itself;
if the card runs out of memory it moves to a safer mode and then to the processor,
and remembers this for the computer. Keep Silero for weak computers. The model
runs in a separate hidden process with no network and no open ports; removing it
in Settings deletes all Qwen3-TTS files.

## Optional Silero synthesis

Both Silero **v3_en (English)** and **v5.5 (Russian)** are noncommercial components
under [CC BY-NC-SA 4.0](docs/third-party/Silero-LICENSE.txt), by Silero Team. Glagol
itself is MIT licensed; dictation does not require Silero. Cloud providers have their
own terms. This version has no commercial TTS provider or SaluteSpeech integration.

1. If text to speech is off (a new installation starts that way), turn on the Text to
   speech switch in Settings → Text to speech.
2. Choose the synthesis language on Synthesize or in Settings.
3. Read and explicitly acknowledge the selected model's license.
4. Download and enable, or select the exact previously downloaded model file.
5. Pick a voice and preview it. English offers EN 0, EN 1, EN 2, EN 3 (default EN 0).
   Russian offers Aidar, Baya, Kseniya, Xenia and Eugene. Numerical English voice
   IDs do not imply a guaranteed gender or US/UK accent.

English model: **57.2 MB** (`v3_en.pt`); Russian: **145.4 MB** (`v5_5_ru.pt`).
Both use **one 249.3 MB runtime**. First English install downloads about 306.5 MB;
first Russian install 394.8 MB. With valid RU Silero installed, adding English needs
only its 57.2 MB model. The app accounts for existing files. Initial setup/staging
needs 1.9 GB free. No system Python, pip, CUDA or SAPI registration is required.

Changing a language does not install anything. Interrupted downloads can resume;
size and SHA-256 must match before activation. Repair checks damaged components.
If the model server is unreachable, import the exact pinned file; arbitrary `.pt`
files are rejected. Removing a language model removes its acknowledgement but keeps
the other model, shared runtime and library. Conditions are acknowledged per model.

A full-verification receipt lasts 30 days. Expiry, changes to key files or a worker
failure trigger full verification. Opening Synthesize preloads the selected model;
status shows preparation. Warm models unload after 15 idle minutes or on exit.
Installed local synthesis works offline. This is process isolation, not an OS sandbox.

If you only dictate, turn off the Text to speech switch in Settings → Text to speech. Glagol then stops verifying Silero files at startup and preloading
the model, hides the Synthesize page and opens Dictation first; a loaded model is
unloaded immediately. Models, acknowledgement and your library are kept — turn the
switch back on to use speech again.

## Reading a document

Paste text or choose TXT, Markdown, DOCX or text-based PDF, then select a voice and
Synthesize and save to library. Input files are limited to 10 MB and 500,000 extracted
characters. Scanned PDFs need external OCR. Text stays in the editor when switching
languages. Language/model changes are disabled during the corresponding operation.

Long text is split and synthesized sequentially with progress and cancellation.
Cancellation does not publish a partial recording as a completed library document.
Before synthesis Glagol turns numbers, amounts, percentages, dates, years, centuries,
times and abbreviations into words. Russian text gets the grammatical case from the
context («продал 980 млн акций» → «девятьсот восемьдесят миллионов акций», «в 2026 г.» →
«в две тысячи двадцать шестом году», «IPO» → «ай-пи-о»); English text follows English
rules («$81.3 billion» → «eighty-one point three billion dollars», «in 2026» → «in
twenty twenty-six»). The case is inferred from neighbouring words and can occasionally
be off; listen to unusual notation. Choosing a language is not translation.

**Improve number reading with AI** is off by default. In Settings → Text to speech,
enter the address of any OpenAI-compatible service (usually ending with `/v1`; the
RouterAI and AITunnel buttons fill it in) and its API key, then click **Load models**:
Glagol reads the service's model list and suggests a fast inexpensive text model
(Gemini Flash Lite when available); you can pick another one. **Save** with the switch
on first checks the address, key and model with a built-in sample and saves only if
the check passes; otherwise a message names the problem (address not answering, key
not accepted, no such model). **Test AI connection** repeats the check later — useful
because providers retire and rename models. Once enabled, document preview and
synthesis send short excerpts near numbers, amounts and abbreviations, including
neighboring words — at most a tenth of the text. Names and other words in Latin
letters, links, email addresses, phone numbers and dictionary words stay on the
computer. Use this only for documents you are comfortable sharing with the selected
provider. The key and choice are stored in the system credential store outside backups.

**Preview how this is read** on the Synthesize page shows the prepared document.
If the provider is unreachable or a response changes the surrounding words,
Glagol uses local rules for the affected excerpts. The audio itself is synthesized
locally.

**Do not read text in brackets** (Settings → Text to speech, off by default): asides
in round and square brackets — explanations, references like [1], sources — are
skipped in synthesis, nested ones too. An unclosed bracket or one that spans
paragraphs stays as it is. The document text in the library does not change; see
the result with «Preview how this is read».

**Pronunciation dictionary** (Settings → Text to speech, below the Silero card) tells
Glagol how to read names, abbreviations and terms: «Written as» → «Read as» (for example
«Anthropic» → «Энтропик»). It is applied before the built-in rules, so your choice always
wins. Each entry has a language (Russian, English or any), «Whole word» (on by default:
«ИИ» leaves «ИИСУС» alone) and «Match case»; an entry can be switched off without
deleting it. When several entries match, the longest wins («Нью-Йорк Таймс» before
«Нью-Йорк»), and a replacement is never processed again. For Russian voices mark stress
with «+» before the stressed vowel («з+амок»); English voices skip it. «Check a phrase»
shows how text will be read, and «Listen» (and ▶ next to an entry) speaks up to 200
characters with the selected voice, which must be installed. «Import» accepts a CSV
exported by Glagol or a simple two-column table «written as;read as» (comma or semicolon);
existing words are updated. «Export» saves the dictionary as CSV. The dictionary lives in
Glagol's database and is included in backups; up to 5000 entries. Documents already
synthesized do not change — synthesize them again.

The Library is a compact list: ▶ next to a document plays it in the shared player at the
bottom (pause, seek, 0.5–2× speed, volume); the icons rename, export the WAV or delete.
With six or more documents a search by title appears. User titles, document contents and
historical records are not translated when changing the interface.

## Dictation

Hold `Ctrl+Shift+Space`. Wait for Preparing microphone to change to the red recording
indicator and level bars; then speak and release. You receive one final text, without
partial text or hands-free capture. Early release cancels preparation. The microphone
is closed between attempts. The maximum recording duration is 60 seconds; silence
is filtered. Queue overflow or an engine failure discards the attempt with an error
instead of inserting a truncated transcript.

On Dictation choose language, microphone, hotkey and Auto-paste or Clipboard only.
Also hold Ctrl+Win (under the shortcut, off by default, Windows only) adds dictation
by holding Ctrl and Win together, as in Wispr Flow; the main shortcut keeps working.
Recording starts once both keys are held for about 0.2 s with no other key; release
either and the text is inserted. Pressing another key meanwhile (for example
Ctrl+Win+D or Ctrl+Win+←) cancels the dictation silently and the Windows shortcut
works as usual. Start does not open when Win is released after a dictation. Windows
does not pass this chord to Glagol in windows run as administrator — use the main
shortcut there. Glagol installs a keyboard hook for this: it only tells Ctrl, Win
and "another key" apart and stores or logs nothing.
Remove hesitation sounds (on by default) drops the sounds people make while thinking:
"uh", "um", "er", "hmm" and the Russian «эм», «э-э», «м-м», «хм», «аа». Spaces, commas
and capitalization are repaired afterwards; if a dictation contained only such sounds,
nothing is inserted. Words such as "like" or «ну», «вот», «типа» are kept: they are real
words and cannot be told apart from their ordinary use without understanding the
sentence. Single «м», «а», «у» are kept too. Cleaning runs on the computer, without the
network, for every recognition method; history receives the cleaned text.
History is off by default; enabling it retains the last 10 transcripts. Turning it
off stops additions; Clear history removes existing entries. Total dictated is a
lifetime duration counter. UI, dictation and TTS languages are independent; each
speech language remembers its local model and voice.

- **On this computer:** English Moonshine Small Streaming (158.8 MB initial download,
  including 16.5 MB native runtime); Russian GigaAM v3 CTC/RNNT (272.2–273.7 MB model
  plus 20.1 MB runtime). Windows x64: processor or, for the Russian model, graphics card
  through Vulkan. Models install by explicit choice, work offline after verification and
  survive updates. Other-language installed models remain listed.
  Remove an inactive model to free space; shared runtime is retained. Local speech uses
  explicit EN/RU; mixed-language detection and translation are not included.
- **Organization server:** OpenAI-compatible `/audio/transcriptions`; `/models` is
  optional. Example `http://192.168.1.10:8000/v1`. The server can be another Glagol —
  see Server for other computers; then pasting its connection code is enough. HTTP is allowed only for localhost/private IPs on a trusted network; audio
  and keys are unencrypted there. Hostnames need trusted HTTPS. System proxies are
  bypassed; an explicit proxy is available.
  Enter the address and the API key if required, then click **Load models**.
  Filter the list and pick a model or enter its name if the server provides no list.
  **Save and use** tests the chosen model with a short built-in WAV of silence.
  Glagol saves the profile and key only if the check succeeds.
- **Cloud service:** choose a preset or compatible endpoint, model, key and optional
  dictation-only proxy. The existing remote `auto` language option is retained.

**Russian local model speed.** On a computer with an older processor (for example,
without AVX2) GigaAM recognizes at roughly the speed of speech. In Settings → Dictation →
Recognition speed, choose Compute on: Graphics card (Vulkan) and, if you like, under Advanced the number
of Processor threads (Automatic by default, up to 8). The change applies immediately, and
Now computing on shows the actual device. If the graphics card cannot be used, Glagol
uses the processor and shows a warning; the reason is in the log. At startup Glagol
detects the processor (AVX2 or not) and a discrete graphics card without loading the
model (when the Russian dictation engine is already downloaded) and shows a
recommendation for this computer: GigaAM CTC on the graphics card if there is one;
otherwise CTC on the processor; without AVX2 and without a graphics card, an honest
note that local recognition will be slow and a suggestion to try an organization server
or the cloud. Apply recommendation downloads CTC if needed and selects the device;
nothing changes until you click. Recent dictations shows how many seconds of waiting
there are per 10 seconds of speech. The status line on the Dictation page tells you when
a faster setup is available for the computer. The full SHA-256 check
of the GigaAM files runs on the first load and is then reused for up to 30 days while file
sizes and dates are unchanged; Verify files and repair, a load error or a recognition
error trigger the full check again.

Office/cloud profiles and keys stay separate. Save and use activates an edited profile;
changing endpoints does not send the old key to the new service. Local Moonshine streams
audio internally while you hold the hotkey; GigaAM/server profiles keep their batch path.
Large GigaAM recordings are split at quiet boundaries; punctuation and rare terms may suffer.
Dictation and synthesis can run independently.
After the computer wakes from sleep, the display turns back on, the session is unlocked,
a remote desktop connects or disconnects, the display configuration changes, or the
indicator's WebView2 process fails, Glagol rebuilds the hidden indicator window within
a few seconds (on Windows). The window is also rebuilt when the indicator has not been
shown for more than 10 minutes, and after each show Glagol reads a few screen pixels
within the indicator itself to confirm it is really visible — on light, dark and black
backgrounds. If the indicator is not on screen, Glagol logs a warning, rebuilds the window
at once and shows the indicator again during the same dictation (in about half a second;
once per dictation). Only a background of almost the same colour as the indicator cannot
be judged. The pixels are not stored. The
indicator has a light and a dark outline, so it stands out on light and dark backgrounds. If the indicator ever stays invisible, restart Glagol from
the tray and send the log from that day: it records these events and the indicator state.

## Server for other computers

One computer with Glagol can recognize Russian speech for other computers in your
local network — for example, a powerful PC with a graphics card serving office
computers. Speech does not go to the internet; it travels only over the local network.

**On the server computer:**

1. Install a Russian GigaAM model (Settings → Dictation → Manage models). If there is
   a graphics card, turn it on in Recognition speed: the server uses the same device.
2. In Server for other computers, turn on Serve recognition over the network. The
   first time, Windows asks whether Glagol may accept connections — allow private
   networks.
3. Click Copy connection code and give the code to your colleagues. It contains the
   address and the access key: without the key the server serves nobody.

**On an office computer:** Settings → Dictation → method Organization server → Glagol
on another computer, paste the code and click Connect. Glagol checks the server and
saves the settings; then dictate as usual. Hesitation-sound removal and text insertion
run on the office computer.

Good to know:

- The server recognizes Russian only. It serves private-network addresses only (home
  or office network, Tailscale) and is off by default.
- Requests run one after another. If more than 32 are waiting at once, Glagol on the
  office computer retries by itself after a couple of seconds (up to three times). On
  a test PC with a GTX 1060 a 6-second phrase took about 0.4 s.
- While the server is on, the computer does not go to sleep (the screen may still turn
  off) — you can change this in Advanced, together with the port (8765 by default).
- New key disconnects everyone with the old code — they need to paste the new one.
- Audio and the key travel over the local network unencrypted (HTTP), so use the
  server only on a trusted network.
- The server log holds numbers only (duration, recognition time, status code): no
  speech, text or computer addresses. The "computers in the last 10 minutes" count
  lives in memory only.
- If computers cannot connect, open Computers cannot connect?: it has a PowerShell
  command (run as administrator) that opens the port for private networks.

## Windows limitations

- Auto-paste restores previous text clipboard contents, but cannot restore images/files.
  Clipboard managers changing line endings can prevent restoration. Windows Win+V
  history may retain transcripts independently of Glagol's history setting.
- Elevated windows may need Glagol at the same privilege level. Two identically named
  microphones are hard to distinguish. Background noise can pass the silence filter.
- `Ctrl+Shift+Space` conflicts with Office's nonbreaking space; choose another hotkey,
  for example `Alt+Shift+D`. A failed hotkey change retains the old binding.

## Backup, data and troubleshooting

Data remains in `%LOCALAPPDATA%\app.glagol.desktop\`: SQLite `glagol.db`, WAVs in
`audio_cache`, STT in `speech_models`, Silero in `tts_models`. Settings → General → Create backup
writes a ZIP of the library. Restore shows a confirmation, makes a protective backup
and restarts the app after replacing the library. Old libraries/backups remain readable.
Models, runtime, OS credentials, previews and Silero consent are excluded; enable speech
components separately on another PC. Credentials are stored in Windows Credential Manager.

If TTS cannot start, check selected language, model installation and acknowledgement;
repair if necessary. If a voice sounds wrong, check the speech language and try another
voice. For recognition errors, retry after checking microphone/model, and verify files
if needed. Close Glagol through the tray before collecting logs so they are flushed;
never attach private speech, documents or credentials.

Logs: Settings → General → Diagnostics → Open log folder (`%LOCALAPPDATA%\app.glagol.desktop\logs`,
one file per day, kept for 7 days). The log always records the processor, model load
time and the time of each recognition stage, and how the dictation indicator was shown:
on which monitor, where, and whether it painted. If dictation is slow or the indicator disappears, turn on Detailed log,
dictate the same phrase several times, close Glagol through the tray and attach that
day's file. The log contains no speech, recognized text, documents or keys. You can
turn the switch off afterwards.

[Report a bug](https://github.com/dimasiksuleyman-sudo/glagol/issues).
Glagol is independent open source software under MIT, created to listen to long texts.
