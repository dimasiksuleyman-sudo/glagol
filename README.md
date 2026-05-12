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

### Возможности

- 🎙️ **6+ нейросетевых голосов** на русском
- 📁 **4 формата ввода:** TXT, Markdown, Word (.docx), PDF
- 📚 **Локальная библиотека** прослушанных документов с поиском
- ▶️ **Возобновление прослушивания** с точки остановки
- 🎚️ **Скорость воспроизведения** от 0.5x до 3x
- 🌙 **Темная и светлая темы**
- 🖱️ **Drag & drop** файлов
- 🔒 **Безопасность:** API-ключ в Windows Credential Manager, тексты не покидают вашу машину (кроме отправки в SaluteSpeech)

### Установка

> 🚧 Проект в активной разработке. Установщик появится в [Releases](https://github.com/dimasiksuleyman-sudo/glagol/releases) после первого релиза.

После релиза:

1. Скачайте `Glagol-Setup-x.x.x.msi` из последнего релиза
2. Запустите установщик
3. При первом запуске введите свой `Authorization Key` от SaluteSpeech (получается бесплатно на [developers.sber.ru](https://developers.sber.ru/studio))
4. Готово — можно загружать документы

### Технологический стек

- **Tauri 2.x** — фреймворк desktop-приложений
- **Rust** — backend (логика, парсинг, аудио)
- **React 19 + TypeScript** — frontend
- **Tailwind CSS + shadcn/ui** — стили и компоненты
- **SQLite** — локальная база данных
- **SaluteSpeech API** — синтез речи

### Дорожная карта

- [x] Sprint 0: Setup проекта
- [ ] Sprint 1: Backend клиент SaluteSpeech
- [ ] Sprint 2: Локальное хранилище + UI библиотеки
- [ ] Sprint 3: Парсинг файлов
- [ ] Sprint 4: Плеер + кэш
- [ ] Sprint 5: Polish + CI/CD
- [ ] v0.1.0 Release
- [ ] Sprint 6: Async синтез для больших документов
- [ ] Sprint 7: SSML-редактор, EPUB
- [ ] v1.0.0 Release

### Вклад в проект

Контрибьюторам рады! См. [CONTRIBUTING.md](CONTRIBUTING.md) и [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

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

### Features

- 🎙️ **6+ neural voices** in Russian
- 📁 **4 input formats:** plain text, Markdown, Word (.docx), PDF
- 📚 **Local library** of synthesized documents with search
- ▶️ **Resume playback** from where you stopped
- 🎚️ **Playback speed** 0.5x–3x
- 🌙 **Dark and light themes**
- 🖱️ **Drag & drop** files
- 🔒 **Security:** API key stored in Windows Credential Manager, your texts never leave your machine (except for synthesis requests to SaluteSpeech)

### Installation

> 🚧 Project is in active development. Installer will appear in [Releases](https://github.com/dimasiksuleyman-sudo/glagol/releases) after the first release.

### Tech Stack

- **Tauri 2.x** — desktop framework
- **Rust** — backend
- **React 19 + TypeScript** — frontend
- **Tailwind CSS + shadcn/ui** — styling
- **SQLite** — local database
- **SaluteSpeech API** — speech synthesis

### Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

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
