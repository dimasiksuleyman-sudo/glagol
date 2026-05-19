<div align="center">

# 📖 Glagol

**Бесплатная локальная озвучка длинных текстов на русском языке**
**Free local text-to-speech for long Russian documents**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-24c8db?logo=tauri)](https://tauri.app/)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)](https://www.rust-lang.org/)
[![React](https://img.shields.io/badge/React-19-61dafb?logo=react)](https://react.dev/)
[![Made with love](https://img.shields.io/badge/made_with-♥-red.svg)](https://github.com/dimasiksuleyman-sudo/glagol)

[Русский](#-русский) · [English](#-english) · [Disclaimer](#️-disclaimer--юридический-статус)

</div>

> ⚠️ **Glagol — независимый open source проект. Не аффилирован, не связан и не поддерживается ПАО Сбербанк.** Использует публичный API SaluteSpeech на условиях самого пользователя. Подробности — в [секции Disclaimer](#️-disclaimer--юридический-статус).

---

## 🇷🇺 Русский

### Что это?

**Glagol** — desktop-приложение для Windows, которое озвучивает длинные тексты и документы качественными русскими голосами и сохраняет аудио в локальную библиотеку, чтобы вы могли вернуться к прослушиванию когда угодно.

Работает поверх **SaluteSpeech API** от Сбера — это бесплатно до 200 000 символов в месяц на персональном тарифе.

### Зачем?

Существующие решения для озвучки русских текстов имеют проблемы:

- 🌐 **Speechify, NaturalReader** — платные ($99–$330/год), слабые русские голоса
- 🎙️ **Balabolka** — бесплатный, но устаревшие SAPI-голоса
- 💻 **SaluteSpeech App от Сбера** — отличные голоса, но без библиотеки документов и кэша
- 🤖 **Яндекс.Браузер «Прочитать вслух»** — только в браузере, требует Яндекс-аккаунт

**Glagol сочетает лучшее:** качественные нейросетевые голоса Сбера + локальная библиотека прослушанных документов + возможность возобновить прослушивание + полностью бесплатно для большинства пользователей.

### Кому это нужно?

- 📚 **Читателям книг и статей**, которые хотят слушать вместо чтения
- 💼 **Менеджерам и юристам** с длинными документами и отчётами
- 👁️ **Людям со сниженным зрением**, которым нужна альтернатива чтению
- 🎓 **Студентам и исследователям**, чтобы слушать научные статьи
- 🎧 **Тем, кто переучивает мозг** воспринимать через аудио

### Что уже работает (Sprint 1–3a, версии `alpha` / `rc.1` / `rc.2`)

- 🎙️ **7 нейросетевых голосов** на русском (плюс один англоязычный)
- 📋 **Вставка текста** для синтеза — paste-and-go
- 📚 **Локальная библиотека** прослушанных документов с автоматическим сохранением
- ▶️ **Воспроизведение** через нативный аудио-плеер с потоковой передачей из локального кэша
- 🎚️ **Скорость воспроизведения** 0.5x–2x (через нативные элементы управления)
- 💾 **Экспорт аудио** в WAV-файл в любую папку
- 🗑️ **Управление библиотекой** — удаление документов одним кликом
- 🧹 **Гуманизация текста** — URL'ы, email-адреса и распространённые аббревиатуры (`т.е.`, `и т.д.`, `т.к.`) произносятся естественно, а не побуквенно
- 🔒 **Безопасность:** Authorization Key хранится в Windows Credential Manager, тексты не покидают вашу машину (кроме отправки в SaluteSpeech для синтеза)

### Что планируется

- 📁 **4 формата ввода:** TXT, Markdown, Word (.docx), PDF — *Sprint 4*
- 🖱️ **Drag & drop** файлов — *Sprint 4*
- ▶️ **Возобновление прослушивания** с точки остановки — *Sprint 5*
- 🌙 **Тёмная и светлая темы** — *Sprint 5*
- 🔍 **Поиск по библиотеке** — *Sprint 5+*
- 📦 **Подписанный MSI-установщик** для публичного релиза — *Sprint 5*

### Установка

> 🚧 Проект в активной разработке. Достигнуты milestone'ы `v0.1.0-alpha` (Sprint 1), `v0.1.0-rc.1` (Sprint 2), `v0.1.0-rc.2` (Sprint 3a), `v0.1.0-rc.3` (Sprint 4). Подписанный установщик с проверенной цифровой подписью появится в [Releases](https://github.com/dimasiksuleyman-sudo/glagol/releases) к публичному релизу `v0.1.0`. До тех пор скачать актуальный неподписанный `.exe` можно из артефактов CI или из release-черновика.

1. Скачайте `Glagol_x.x.x_x64-setup.exe` из последнего [GitHub Release](https://github.com/dimasiksuleyman-sudo/glagol/releases).
2. Запустите файл.

#### При первом запуске Windows покажет предупреждение SmartScreen

Поскольку установщик пока не подписан сертификатом разработчика, Windows встретит вас синим окном «Система Windows защитила ваш компьютер»:

![Предупреждение SmartScreen](docs/images/smartscreen-warning.png)

Что делать:

1. Нажмите **«Подробнее»** в синем окне.
2. Появится кнопка **«Выполнить в любом случае»** — нажмите её.
3. Откроется обычный установщик NSIS, дальше — стандартная установка:
   - выбрать язык интерфейса установщика (русский или английский);
   - принять условия лицензии MIT;
   - выбрать папку установки (по умолчанию `%LOCALAPPDATA%\Programs\Glagol\`);
   - решить, создавать ли ярлыки в меню «Пуск» и на рабочем столе.

Права администратора не требуются — установка ставится для текущего пользователя.

После установки запустите Glagol из меню «Пуск», в Настройках вставьте свой `Authorization Key` от SaluteSpeech (получается бесплатно на [developers.sber.ru/studio](https://developers.sber.ru/studio)) — и можно загружать документы.

### Технологический стек

- **Tauri 2.x** — фреймворк desktop-приложений
- **Rust** — backend (логика, парсинг, аудио)
- **React 19 + TypeScript** — frontend
- **Tailwind CSS + shadcn/ui** — стили и компоненты
- **SQLite** (через `rusqlite` + `rusqlite_migration`) — локальная база данных
- **Tauri Asset Protocol** — потоковая передача аудио из локального кэша
- **SaluteSpeech API** — синтез речи

### Дорожная карта

- [x] Sprint 0: Setup проекта
- [x] Sprint 1: Backend клиент SaluteSpeech + минимальный UI (`v0.1.0-alpha`)
- [x] Sprint 2: Локальное хранилище + UI библиотеки + asset protocol playback (`v0.1.0-rc.1`)
- [x] Sprint 3a: Препроцессор текста (URL/email/аббревиатуры) (`v0.1.0-rc.2`)
- [ ] Sprint 4: Парсинг файлов (TXT, MD, DOCX, PDF)
- [ ] Sprint 5: Плеер + кэш + Polish + CI/CD + первый публичный релиз
- [ ] **v0.1.0 Release**
- [ ] Sprint 6: Async синтез для больших документов
- [ ] Sprint 7: SSML-редактор, EPUB
- [ ] **v1.0.0 Release**

### Вклад в проект

Контрибьюторам рады! См. [CONTRIBUTING.md](CONTRIBUTING.md) и [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

#### Где Glagol хранит данные

Дев-сборки (через `pnpm tauri dev`) и установленные через NSIS-установщик одинаково используют папку `%LOCALAPPDATA%\app.glagol.desktop\` для базы документов и аудио-кэша. Имя берётся из поля `bundle.identifier` в `src-tauri/tauri.conf.json` (исторически `app.glagol.desktop`; переименование в просто `Glagol` — техдолг, отложенный на будущий Sprint, чтобы не сломать существующие установки). Файл базы — `glagol.db`, аудио — в `audio_cache/{uuid}.wav`.

### Безопасность

Нашли уязвимость? Не открывайте публичный issue. См. [SECURITY.md](SECURITY.md).

---
## 🇬🇧 English

### What is it?

**Glagol** (Russian for "to speak", "verb") is a Windows desktop app that reads long texts and documents aloud using high-quality Russian neural voices, and saves audio to a local library so you can resume listening anytime.

Powered by **SaluteSpeech API** from Sberbank — free up to 200,000 characters/month on the personal tier.

### Why?

Existing Russian TTS solutions have gaps:

- 🌐 **Speechify, NaturalReader** — paid ($99–$330/year), weak Russian voices
- 🎙️ **Balabolka** — free but outdated SAPI voices
- 💻 **SaluteSpeech App by Sber** — great voices, no document library or cache
- 🤖 **Yandex Browser TTS** — only in the browser, requires a Yandex account

**Glagol combines the best:** quality neural voices from Sber + local library of synthesized documents + resume playback + completely free for most users.

### What already works (Sprint 1–3a, `alpha` / `rc.1` / `rc.2` milestones)

- 🎙️ **7 neural voices** in Russian (plus one English voice)
- 📋 **Paste text** for synthesis — paste-and-go workflow
- 📚 **Local library** of synthesized documents with automatic saving
- ▶️ **Playback** via native audio player with streaming from local cache
- 🎚️ **Playback speed** 0.5x–2x (via native controls)
- 💾 **Audio export** to WAV file to any folder
- 🗑️ **Library management** — single-click document deletion
- 🧹 **Text humanization** — URLs, emails, and common Russian abbreviations (`т.е.`, `и т.д.`, `т.к.`) are spoken naturally, not letter-by-letter
- 🔒 **Security:** Authorization Key stored in Windows Credential Manager; your texts never leave your machine (except for synthesis requests to SaluteSpeech)

### Planned features

- 📁 **4 input formats:** plain text, Markdown, Word (.docx), PDF — *Sprint 4*
- 🖱️ **Drag & drop** files — *Sprint 4*
- ▶️ **Resume playback** from where you stopped — *Sprint 5*
- 🌙 **Dark and light themes** — *Sprint 5*
- 🔍 **Library search** — *Sprint 5+*
- 📦 **Signed MSI installer** for public release — *Sprint 5*

### Installation

> 🚧 Active development. Milestones reached: `v0.1.0-alpha` (Sprint 1), `v0.1.0-rc.1` (Sprint 2), `v0.1.0-rc.2` (Sprint 3a), `v0.1.0-rc.3` (Sprint 4). A code-signed installer will ship in [Releases](https://github.com/dimasiksuleyman-sudo/glagol/releases) with the public `v0.1.0`. Until then the latest unsigned `.exe` is available from CI artifacts or release drafts.

1. Download `Glagol_x.x.x_x64-setup.exe` from the latest [GitHub Release](https://github.com/dimasiksuleyman-sudo/glagol/releases).
2. Run the file.

#### Windows SmartScreen warning on first launch

Because the installer is not yet signed with a developer certificate, Windows will greet you with a blue "Windows protected your PC" dialog:

![SmartScreen warning](docs/images/smartscreen-warning.png)

What to do:

1. Click **"More info"** in the blue dialog.
2. A **"Run anyway"** button will appear — click it.
3. The normal NSIS installer opens — standard install flow from there:
   - pick the installer UI language (English or Russian);
   - accept the MIT license terms;
   - choose an install location (default: `%LOCALAPPDATA%\Programs\Glagol\`);
   - decide whether to create Start Menu and/or Desktop shortcuts.

No administrator privileges are needed — install is per-user.

After installing, launch Glagol from the Start Menu, paste your SaluteSpeech `Authorization Key` in Settings (free at [developers.sber.ru/studio](https://developers.sber.ru/studio)), and you're ready to load documents.

### Tech Stack

- **Tauri 2.x** — desktop framework
- **Rust** — backend
- **React 19 + TypeScript** — frontend
- **Tailwind CSS + shadcn/ui** — styling
- **SQLite** (via `rusqlite` + `rusqlite_migration`) — local database
- **Tauri Asset Protocol** — streaming audio playback from local cache
- **SaluteSpeech API** — speech synthesis

### Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

#### Where Glagol stores data

Both dev builds (`pnpm tauri dev`) and NSIS-installed builds use `%LOCALAPPDATA%\app.glagol.desktop\` for the document database and audio cache. The folder name comes from `bundle.identifier` in `src-tauri/tauri.conf.json` (historically `app.glagol.desktop`; renaming to plain `Glagol` is tracked as tech debt for a future Sprint so existing installations don't lose their libraries). The database file is `glagol.db`; audio lives under `audio_cache/{uuid}.wav`.

### Security

Found a vulnerability? Don't open a public issue. See [SECURITY.md](SECURITY.md).

---

## ⚖️ Disclaimer / Юридический статус

### 🇷🇺 Русский

**Glagol — независимый open source проект**, созданный сообществом разработчиков (Glagol Contributors) и распространяемый под лицензией MIT.

Проект **НЕ является:**

- ❌ Официальным продуктом ПАО Сбербанк или его дочерних компаний
- ❌ Аффилированным с ПАО Сбербанк, SberDevices, SaluteSpeech или их сотрудниками
- ❌ Финансируемым, поддерживаемым или одобренным Сбером

**Проект использует** публичный API сервиса SaluteSpeech, доступный любому пользователю на условиях [Лицензионного соглашения](https://developers.sber.ru/docs/ru/policies/eula) и [Политики обработки персональных данных](https://www.sberbank.com/privacy) ПАО Сбербанк. Каждый пользователь Glagol самостоятельно регистрируется на [developers.sber.ru](https://developers.sber.ru) и получает свои собственные авторизационные данные.

**Товарные знаки.** «SaluteSpeech», «Сбер», «SberDevices» и связанные обозначения являются товарными знаками ПАО Сбербанк или связанных лиц. Упоминание этих знаков в проекте Glagol носит **исключительно информационный характер** в рамках добросовестного использования (fair use) и описания совместимости.

**Ответственность.** Программное обеспечение распространяется «как есть» (AS IS) без каких-либо гарантий. Разработчики Glagol не несут ответственности:

- За работоспособность API SaluteSpeech и изменения в его условиях
- За расходы пользователя, превысившие бесплатный лимит SaluteSpeech
- За соблюдение пользователем авторских прав на тексты, которые он озвучивает
- За использование сгенерированного аудио в коммерческих целях (правила определяются лицензией SaluteSpeech)

**Контакты для вопросов по API SaluteSpeech:** обращайтесь напрямую в Сбер — `SaluteSpeech@sberbank.ru` или через форму поддержки на developers.sber.ru.

### 🇬🇧 English

**Glagol is an independent open source project** developed by community contributors (Glagol Contributors) and distributed under the MIT License.

**This project is NOT:**

- ❌ An official product of PJSC Sberbank or its subsidiaries
- ❌ Affiliated with PJSC Sberbank, SberDevices, SaluteSpeech, or their employees
- ❌ Funded, supported, or endorsed by Sberbank in any way

**The project uses** the public SaluteSpeech API, available to any user under the terms of [PJSC Sberbank's License Agreement](https://developers.sber.ru/docs/ru/policies/eula). Each Glagol user independently registers at [developers.sber.ru](https://developers.sber.ru) and obtains their own credentials.

**Trademarks.** "SaluteSpeech", "Sber", "SberDevices", and related marks are trademarks of PJSC Sberbank or affiliated entities. Their use in this project is **for informational and interoperability purposes only** under fair use principles.

**Liability.** The software is provided "AS IS" without warranty of any kind. Glagol contributors are not responsible for:

- The operability or terms of the SaluteSpeech API
- Costs incurred by users exceeding SaluteSpeech free tier limits
- Copyright compliance for texts users choose to synthesize
- Commercial use of generated audio (governed by SaluteSpeech license)

**Contact for SaluteSpeech API questions:** Contact Sberbank directly — `SaluteSpeech@sberbank.ru` or via developers.sber.ru support.

---

<div align="center">

**Made with ♥ by Glagol Contributors**

[Report a bug](https://github.com/dimasiksuleyman-sudo/glagol/issues/new) ·
[Request a feature](https://github.com/dimasiksuleyman-sudo/glagol/issues/new) ·
[Discussions](https://github.com/dimasiksuleyman-sudo/glagol/discussions)

</div>
