# Changelog

All notable changes to Glagol are documented in this file.

The format follows [Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/)
and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Entries describe **user-visible** behaviour. Internal refactors, test counts,
and dependency version bumps that don't change what the user sees are kept in
the per-session master logs under [`docs/day-logs/`](docs/day-logs/).

## [Unreleased]

### Added
- **Счётчик использования SaluteSpeech** (Настройки → «Использование
  SaluteSpeech»). Показывает потраченные символы за текущий месяц на
  бесплатном тарифе (200 000 символов/месяц) с прогресс-баром и
  локализованным названием месяца. Обновляется автоматически после
  каждой успешной озвучки.
- **Дружественные сообщения об ошибках SaluteSpeech** (closes #16). Сетевые
  сбои, истёкшие токены, превышение лимита, неверные credentials и
  ошибки сервера Сбера теперь показываются пользователю на русском с
  конкретной подсказкой что делать, вместо «`network error: ...`».

### Changed
- **Ошибка восстановления резервной копии больше не дублируется.**
  Toast «Этот файл не является корректной резервной копией Glagol»
  теперь идёт с конкретной причиной один раз, не дважды.
- **Правильные русские формы множественного числа** в счётчиках
  диалогов восстановления — «1 документ», «2 документа», «5 документов».

## [v0.1.0-rc.6] — 2026-05-20

### Added
- **Резервное копирование и восстановление библиотеки одним архивом**
  (Настройки → «Резервное копирование»).
  - Создание `.zip`-архива с базой данных, всеми аудиофайлами и манифестом.
  - Восстановление из архива с автоматической резервной копией текущего
    состояния перед перезаписью.
  - Поддержка переноса библиотеки между компьютерами без ручной работы с
    файлами.

### Fixed
- **Восстановление на Windows больше не падает с «file in use».**
  SQLite-соединение явно закрывается перед удалением `glagol.db` через
  `std::mem::replace`, что устраняет `ERROR_SHARING_VIOLATION` (os error
  32) на восстановлении.

## [v0.1.0-rc.5] — 2026-05-20

### Added
- **Inline-редактирование названий документов в Библиотеке.** Карандаш
  рядом с заголовком → click → редактирование, Enter сохраняет, Esc
  отменяет, blur коммитит. Поведение F2-rename из Проводника Windows.

### Changed
- **Имя инсталлятора и метка в «Программы и компоненты» с правильным
  регистром бренда.** Установщик теперь `Glagol_0.1.0_x64-setup.exe`
  (было `glagol_…`); запись в «Программах и компонентах» отображается
  как «Glagol».

### Removed
- **Настраиваемая папка библиотеки удалена.** Sprint 5b dual-root
  fallback не решал реальную user pain (управление местом на диске —
  файлы оставались на C: + добавлялись в новой папке). Возможно вернётся
  в Sprint 5c+ с физической миграцией файлов, если будет реальный сигнал.

## [v0.1.0-rc.4] — 2026-05-19

### Added
- NSIS Windows installer — per-user install (no admin elevation), MIT license
  acceptance step, customizable install location, optional Start Menu and
  Desktop shortcuts, language selector for English / Russian (PR #26).
- Pdfium PDF engine bundled inside the installer so PDF parsing works
  out-of-the-box in release builds without any extra download or system-wide
  Pdfium install (PR #26).
- GitHub Actions CI workflow on `windows-latest`: runs `cargo fmt` / `cargo
  clippy` / `cargo test`, `pnpm tsc --noEmit`, and a full `pnpm tauri build`
  on every PR and main-branch push. NSIS installer attached as a CI artifact
  for 14 days after each green run (PR #26).
- This `CHANGELOG.md` file — historical releases backfilled from Sprint master
  logs (PR #26).

## [v0.1.0-rc.3] — 2026-05-19

### Added
- **File input on the Synthesize page.** New «Выбрать файл» button opens a
  native file picker; selected files are parsed and their text loaded into
  the textarea, ready to synthesize (PR #24).
- **TXT parsing** with smart encoding detection — UTF-8 BOM, plain UTF-8, and
  legacy Windows-1251 (covers ~99% of real Russian `.txt` files).
- **Markdown parsing** with conservative defaults — bold / italic / link
  markup is stripped to plain text, code blocks are replaced with the
  placeholder «фрагмент кода», image alt-text is dropped, footnotes are
  collected and appended at the end under a «Сноски:» heading.
- **DOCX parsing** — paragraphs and tables are extracted; tables read row by
  row with cells joined by spaces. Headers, footers, comments, footnotes,
  tracked changes, and embedded images are skipped.
- **PDF parsing** via the Pdfium engine (same library Chromium uses).
  Scanned image-only PDFs are detected automatically and surface an OCR
  guidance dialog instead of loading an empty textarea.
- **File size limit** of 10 MB and **content limit** of 500 000 characters
  (Cyrillic-aware), both with friendly Russian-language error messages.

## [v0.1.0-rc.2] — 2026-05-18

### Added
- **Narration humaniser.** Text now flows naturally for SaluteSpeech instead
  of being read mechanically (PR #22):
  - URLs (e.g. `https://github.com/...`, `www.site.com`, bare domains like
    `github.com`) are pronounced as the single word «ссылка».
  - Email addresses are pronounced as the single word «email».
  - Common Russian abbreviations are expanded: `т.е.` → «то есть», `и т.д.`
    → «и так далее», `и т.п.`, `т.к.`, `т.н.`, `т.о.`, `и др.`, `и пр.`.
- Conservative bare-domain detection backed by a curated TLD whitelist so
  version numbers (`1.5`), filenames (`report.pdf`), and abbreviations
  (`т.е.`) are never mistaken for URLs.

## [v0.1.0-rc.1] — 2026-05-18

### Added
- **Persistent local library.** Every synthesised document is now saved
  automatically: a database row plus the WAV file land atomically in
  `%LOCALAPPDATA%\app.glagol.desktop\`, so closing and reopening the app
  brings everything back (PR #15-18).
- **Library page** lists every stored document, newest first, with native
  HTML5 audio playback streamed straight from the local cache via the Tauri
  asset protocol. Each row offers a one-click delete (row + file removed
  instantly) and a one-click export-to-disk (system Save As dialog).
- **Discriminated empty / loading / error states** on the Library page so
  the first visit, an empty library, and a backend hiccup all show clear
  guidance instead of a blank screen.
- **Synthesize page toast** after a successful synthesis offers a one-click
  jump straight to the Library page.

### Fixed
- **Ctrl+R no longer resets the credentials status.** Refreshing the dev
  WebView (or any future mount-time probe) used to map a valid stored key
  to "не настроен или не работает" if the network blipped during the
  Sberbank handshake. The probe now trusts the in-process auth cache and
  only contacts Sberbank when the user explicitly clicks «Проверить»
  (closes GitHub issue #15).

## [v0.1.0-alpha] — 2026-05-17

### Added
- **MVP synthesis pipeline** end-to-end (PR #11-13):
  - Paste Russian text on the Synthesize page, pick a voice, click «Озвучить
    и сохранить» — the text is split into chunks under the SaluteSpeech
    4000-character per-request limit, each chunk synthesised in turn, and
    the resulting WAV pieces joined into a single playable file.
  - Settings page for storing the SaluteSpeech Authorization Key. Stored in
    the Windows Credential Manager via `keyring-rs`, never written to
    config files or environment variables.
  - One-click test of the stored key against the real Sberbank OAuth
    endpoint.
- **Six native Russian voices** exposed in the picker: Наталья (default),
  Борис, Марфа, Тарас, Александра, Сергей.
- **TLS pinning** to the Russian Ministry of Digital Development root
  certificate (`НУЦ Минцифры`) — the embedded cert is the only trusted root
  for Sberbank calls.
- **Live progress** during long synthesis: per-chunk progress events drive a
  visible progress bar so the user can see «Озвучиваем фрагмент 5 из 12».
- **System Save As dialog** for choosing where the resulting WAV goes.

[Unreleased]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-rc.6...HEAD
[v0.1.0-rc.6]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-rc.5...v0.1.0-rc.6
[v0.1.0-rc.5]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-rc.4...v0.1.0-rc.5
[v0.1.0-rc.4]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-rc.3...v0.1.0-rc.4
[v0.1.0-rc.3]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-rc.2...v0.1.0-rc.3
[v0.1.0-rc.2]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-rc.1...v0.1.0-rc.2
[v0.1.0-rc.1]: https://github.com/dimasiksuleyman-sudo/glagol/compare/v0.1.0-alpha...v0.1.0-rc.1
[v0.1.0-alpha]: https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.1.0-alpha
