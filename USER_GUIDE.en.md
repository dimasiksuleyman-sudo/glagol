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
is saved immediately. Then choose dictation and synthesis independently on two
speech cards. Install either, both or neither; Continue opens the app without
requiring a download. Closing and reopening resumes the saved setup step. With no
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

## Optional Silero synthesis

Both Silero **v3_en (English)** and **v5.5 (Russian)** are noncommercial components
under [CC BY-NC-SA 4.0](docs/third-party/Silero-LICENSE.txt), by Silero Team. Glagol
itself is MIT licensed; dictation does not require Silero. Cloud providers have their
own terms. This version has no commercial TTS provider or SaluteSpeech integration.

1. Choose the synthesis language on Synthesize or in Settings.
2. Read and explicitly acknowledge the selected model's license.
3. Download and enable, or select the exact previously downloaded model file.
4. Pick a voice and preview it. English offers EN 0, EN 1, EN 2, EN 3 (default EN 0).
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
English text uses English number/abbreviation handling; Russian retains its existing
stress and Latin-letter conversion. Dates and unusual notation may be read component
by component: listen to important text. Choosing a language is not translation.

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
  optional. Example `http://192.168.1.10:8000/v1`. Your administrator installs the
  server. HTTP is allowed only for localhost/private IPs on a trusted network; audio
  and keys are unencrypted there. Hostnames need trusted HTTPS. System proxies are
  bypassed; an explicit proxy is available.
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
