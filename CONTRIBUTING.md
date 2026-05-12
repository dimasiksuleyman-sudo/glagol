# Contributing to Glagol / Вклад в Glagol

[English](#english) · [Русский](#русский)

---

## English

First off — **thank you for considering contributing to Glagol!** ✨

Glagol is a community-driven project. Every bug report, feature idea, documentation fix, and code contribution makes a difference.

### Code of Conduct

By participating, you agree to abide by our [Code of Conduct](CODE_OF_CONDUCT.md). In short: be respectful, be patient, be kind.

### How can I contribute?

#### 🐛 Found a bug?

1. Check [existing issues](https://github.com/dimasiksuleyman-sudo/glagol/issues) — it might already be reported
2. If not, open a [new bug report](https://github.com/dimasiksuleyman-sudo/glagol/issues/new?template=bug_report.yml)
3. Include: Windows version, Glagol version, steps to reproduce, expected vs actual behavior, logs if available

#### 💡 Have a feature idea?

1. Check [existing feature requests](https://github.com/dimasiksuleyman-sudo/glagol/issues?q=is%3Aissue+label%3Aenhancement)
2. Open a [feature request](https://github.com/dimasiksuleyman-sudo/glagol/issues/new?template=feature_request.yml) or [start a discussion](https://github.com/dimasiksuleyman-sudo/glagol/discussions)
3. Describe the problem you're solving, not just the solution

#### 📖 Improving docs?

Documentation PRs are always welcome. No issue needed — just open a PR.

#### 💻 Want to write code?

1. **Comment on the issue first** — let us know you're working on it (avoids duplicate work)
2. **For larger changes**, open a discussion before coding to align on approach
3. **Fork → branch → PR** workflow (see below)

### Development setup

#### Prerequisites

- **Windows 10/11** (primary target; macOS/Linux contributions also welcome)
- **Node.js 20+** (we use 22 LTS in CI)
- **pnpm 10+** (`npm install -g pnpm`)
- **Rust stable** (≥1.77, install via [rustup](https://rustup.rs/))
- **MSVC Build Tools** (Visual Studio 2022 with "Desktop development with C++")
- **Git**

#### Get started

```bash
# Fork the repo on GitHub, then:
git clone https://github.com/YOUR-USERNAME/glagol.git
cd glagol
pnpm install
pnpm tauri dev
```

#### Get SaluteSpeech credentials for testing

1. Register at [developers.sber.ru](https://developers.sber.ru/studio)
2. Create a SaluteSpeech API project
3. Copy your `Authorization Key`
4. Enter it in Glagol's Settings on first launch

### Branch and commit conventions

#### Branches

- `main` — stable, always releasable
- `feat/short-description` — new features
- `fix/short-description` — bug fixes
- `docs/short-description` — documentation
- `refactor/short-description` — code restructuring
- `chore/short-description` — tooling, CI, deps

#### Commits — Conventional Commits

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add support for EPUB file format
fix: handle empty SSML tags in chunker
docs: clarify SaluteSpeech setup steps
refactor: extract OAuth logic into separate module
chore: bump Tauri to 2.10
test: add unit tests for text chunker
perf: parallelize chunk synthesis
```

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `build`.

Keep the subject line under 72 characters. Use the body for context if needed.

### Pull Request process

1. **One PR = one concern.** Don't mix refactoring with feature work
2. **Write tests** for new logic (Rust: `cargo test`, TS: `vitest`)
3. **Run quality checks locally** before pushing:
```bash
   pnpm lint
   pnpm typecheck
   pnpm test
   cd src-tauri && cargo clippy -- -D warnings && cargo test && cargo fmt --check
```
4. **Fill the PR template** — describe what, why, how to test
5. **Link the issue** with `Closes #123` or `Fixes #123`
6. **Be patient** — maintainers review PRs in their free time

### Code style

#### Rust

- `cargo fmt` (rustfmt with default config) — enforced in CI
- `cargo clippy -- -D warnings` — no warnings allowed
- Public APIs documented with `///` doc comments
- No `unsafe` without a `// SAFETY:` comment explaining why

#### TypeScript / React

- ESLint + Prettier — config in repo
- Functional components only
- Hooks > class components
- Use shadcn/ui primitives where possible
- Tailwind utility classes preferred over custom CSS

#### Translations

UI strings are i18n-ready. New strings:

1. Add English key to `src/locales/en.json`
2. Add Russian translation to `src/locales/ru.json`
3. Reference via `t('key.path')` in components

### What we WON'T accept

To keep the project focused:

- ❌ Telemetry, analytics, or any non-opt-in data collection
- ❌ Bundling additional API providers besides SaluteSpeech (without prior discussion)
- ❌ Code obfuscation or anti-modification measures
- ❌ Dependencies with non-OSI-approved licenses
- ❌ Features that compromise user privacy
- ❌ Closed-source binary blobs (except officially distributed libpdfium.dll and Russian Ministry of Digital Development certificate)

### License

By contributing, you agree your contributions will be licensed under the [MIT License](LICENSE).

---

## Русский

Прежде всего — **спасибо, что рассматриваете возможность вклада в Glagol!** ✨

Glagol — community-driven проект. Каждый bug report, идея фичи, исправление в документации и строчка кода имеют значение.

### Кодекс поведения

Участвуя, вы соглашаетесь с нашим [Кодексом поведения](CODE_OF_CONDUCT.md). Если кратко: будьте уважительны, терпеливы и доброжелательны.

### Как помочь?

#### 🐛 Нашли баг?

1. Проверьте [существующие issues](https://github.com/dimasiksuleyman-sudo/glagol/issues) — возможно, уже репортнули
2. Если нет — откройте [bug report](https://github.com/dimasiksuleyman-sudo/glagol/issues/new?template=bug_report.yml)
3. Включите: версию Windows, версию Glagol, шаги воспроизведения, ожидаемое vs фактическое поведение, логи если есть

#### 💡 Есть идея фичи?

1. Проверьте [существующие feature requests](https://github.com/dimasiksuleyman-sudo/glagol/issues?q=is%3Aissue+label%3Aenhancement)
2. Откройте [feature request](https://github.com/dimasiksuleyman-sudo/glagol/issues/new?template=feature_request.yml) или [начните обсуждение](https://github.com/dimasiksuleyman-sudo/glagol/discussions)
3. Описывайте проблему, которую решаете, а не только решение

#### 📖 Улучшения документации?

PR с документацией всегда приветствуются. Issue не обязателен — просто открывайте PR.

#### 💻 Хотите написать код?

1. **Сначала прокомментируйте в issue** — дайте знать, что вы взялись (избегаем дублирующей работы)
2. **Для крупных изменений** откройте discussion для согласования подхода
3. **Fork → branch → PR** workflow (см. ниже)

### Локальная разработка

#### Что нужно

- **Windows 10/11** (основная платформа)
- **Node.js 20+** (в CI используем 22 LTS)
- **pnpm 10+** (`npm install -g pnpm`)
- **Rust stable** (≥1.77, установка через [rustup](https://rustup.rs/))
- **MSVC Build Tools** (Visual Studio 2022 с «Разработка классических приложений на C++»)
- **Git**

#### Старт

```bash
# Сначала fork на GitHub, затем:
git clone https://github.com/ВАШ-ЛОГИН/glagol.git
cd glagol
pnpm install
pnpm tauri dev
```

#### Получить credentials SaluteSpeech для тестирования

1. Зарегистрируйтесь на [developers.sber.ru](https://developers.sber.ru/studio)
2. Создайте проект SaluteSpeech API
3. Скопируйте `Authorization Key`
4. Введите его в настройках Glagol при первом запуске

### Conventions для веток и коммитов

#### Ветки

- `main` — стабильная, всегда готова к релизу
- `feat/short-description` — новые фичи
- `fix/short-description` — исправления багов
- `docs/short-description` — документация
- `refactor/short-description` — реструктуризация кода
- `chore/short-description` — tooling, CI, зависимости

#### Коммиты — Conventional Commits

Мы используем [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add support for EPUB file format
fix: handle empty SSML tags in chunker
docs: clarify SaluteSpeech setup steps
```

Типы: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `build`.

Subject — до 72 символов. Body — если нужен контекст.

### Процесс Pull Request

1. **Один PR = одна задача.** Не смешивайте рефакторинг с новой фичей
2. **Пишите тесты** для новой логики
3. **Запустите проверки локально** перед push:
```bash
   pnpm lint
   pnpm typecheck
   pnpm test
   cd src-tauri && cargo clippy -- -D warnings && cargo test && cargo fmt --check
```
4. **Заполните шаблон PR** — что, зачем, как тестировать
5. **Привяжите issue** через `Closes #123`
6. **Будьте терпеливы** — maintainers ревьюят PR в свободное время

### Стиль кода

#### Rust

- `cargo fmt` — обязательно
- `cargo clippy -- -D warnings` — никаких warnings
- Публичные API задокументированы через `///`
- Никакого `unsafe` без `// SAFETY:` комментария

#### TypeScript / React

- ESLint + Prettier
- Только функциональные компоненты
- Hooks вместо class components
- Используйте примитивы shadcn/ui где возможно
- Tailwind utility classes предпочтительнее кастомного CSS

### Что мы НЕ примем

Чтобы проект оставался сфокусированным:

- ❌ Телеметрия, аналитика, любой не-opt-in сбор данных
- ❌ Дополнительные API-провайдеры кроме SaluteSpeech (без предварительного обсуждения)
- ❌ Обфускация кода или anti-modification меры
- ❌ Зависимости с non-OSI лицензиями
- ❌ Фичи, нарушающие приватность пользователя
- ❌ Закрытые бинарные blob'ы (кроме официально распространяемых libpdfium.dll и сертификата НУЦ Минцифры)

### Лицензия

Делая вклад в проект, вы соглашаетесь, что ваши контрибьюции будут лицензированы под [MIT License](LICENSE).

---

*Questions? Open a [discussion](https://github.com/dimasiksuleyman-sudo/glagol/discussions) — we're happy to help newcomers!*

*Вопросы? Откройте [обсуждение](https://github.com/dimasiksuleyman-sudo/glagol/discussions) — мы рады помочь новичкам!*
