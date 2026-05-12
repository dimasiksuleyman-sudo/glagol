# Security Policy / Политика безопасности

[English](#english) · [Русский](#русский)

---

## English

### Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: (current development) |
| < 0.1   | :x: (pre-release)  |

Once we reach v1.0, we will commit to supporting the latest major version with security fixes.

### Reporting a Vulnerability

**Please do NOT open a public GitHub issue for security vulnerabilities.**

If you discover a security vulnerability in Glagol, please report it responsibly:

1. **Email:** security@glagol.app *(placeholder — see "Contact" section below for current contact)*
2. **GitHub Security Advisories:** Use the [private vulnerability reporting](https://github.com/dimasiksuleyman-sudo/glagol/security/advisories/new) feature
3. **Encrypted contact:** PGP key available upon request

We follow a **90-day responsible disclosure** policy. We aim to:

- Acknowledge your report within 72 hours
- Provide an initial assessment within 7 days
- Issue a fix or mitigation within 90 days
- Credit you in the security advisory (unless you prefer to remain anonymous)

### Threat Model — What We Protect Against

Glagol is a **local desktop application** that processes user documents and communicates with a third-party API (SaluteSpeech). Our threat model includes:

#### ✅ Threats we mitigate

- **API credential theft** — Authorization Keys stored in Windows Credential Manager (OS-level encryption), never in plain text, never in files
- **Man-in-the-middle attacks** on SaluteSpeech connections — TLS pinning with embedded Russian Ministry of Digital Development root certificate
- **Malicious file content** — DOCX/PDF parsed via memory-safe Rust crates; no JavaScript execution in PDFs; no `dangerouslySetInnerHTML` in React
- **Code injection** — strict Content Security Policy (CSP) restricts network requests to SaluteSpeech endpoints only
- **Supply chain attacks** — `pnpm-lock.yaml` and `Cargo.lock` committed; Dependabot enabled; release artifacts signed with Ed25519
- **Unsigned updates** — Tauri updater requires Ed25519 signature; cannot be disabled
- **Data exfiltration** — no telemetry by default; no data sent anywhere except SaluteSpeech (under user's own account)

#### ❌ Threats we do NOT mitigate

- **Compromised user machine** — if your Windows account is compromised, attacker has access to Windows Credential Manager
- **Compromised SaluteSpeech account** — security of your Sberbank account is your responsibility
- **Malicious contributors** — we review PRs but cannot guarantee zero-day in dependencies
- **Physical access** to your machine

### What We Don't Collect

Glagol does **NOT** collect, transmit, or store on any remote server:

- ❌ Your text content (sent only to SaluteSpeech under your own account)
- ❌ Generated audio (kept only on your machine)
- ❌ Document library metadata
- ❌ Usage telemetry
- ❌ Error reports (Sentry is **opt-in**, disabled by default)
- ❌ IP addresses, device fingerprints, hardware IDs

### Secrets Management

| Secret | Storage | Encryption |
| ------ | ------- | ---------- |
| SaluteSpeech `Authorization Key` | Windows Credential Manager (`keyring-rs`) | OS-level (DPAPI on Windows) |
| OAuth `access_token` | RAM only, never persisted | n/a (volatile) |
| User documents and audio | Local filesystem (`%LOCALAPPDATA%\Glagol\`) | No (user's choice for full-disk encryption) |

**We never log, transmit, or display secrets anywhere.**

### Network Boundaries

Glagol makes network requests **only** to these endpoints:

| Endpoint | Purpose | When |
| -------- | ------- | ---- |
| `https://ngw.devices.sberbank.ru:9443/api/v2/oauth` | Get OAuth access token | When token expires (every ~30 min) |
| `https://smartspeech.sber.ru/rest/v1/text:synthesize` | Synthesize speech | When user requests TTS |
| `https://api.github.com/repos/dimasiksuleyman-sudo/glagol/releases/latest` | Check for updates | On app start (can be disabled in settings) |

**No analytics services. No advertising networks. No CDN tracking.**

This is enforced via Tauri's CSP and capability allowlist — any attempt to add other endpoints requires a code change visible in the public repository.

### Dependencies Security

- `cargo audit` runs in CI on every PR
- `pnpm audit` runs in CI on every PR
- Dependabot enabled for both ecosystems
- Major dependency updates reviewed manually

### Build Reproducibility

Release builds are produced by GitHub Actions from a tagged commit. Build logs are public. Anyone can audit the build process at `.github/workflows/release.yml`.

### Disclosure of Third-Party Services

Glagol integrates with **SaluteSpeech API** by PJSC Sberbank. When you use the app:

- Your text is sent to Sberbank servers for synthesis
- Sberbank's [Privacy Policy](https://www.sberbank.com/privacy) and [EULA](https://developers.sber.ru/docs/ru/policies/eula) apply to that processing
- Your relationship with Sberbank is independent of your use of Glagol
- We have no visibility into or control over Sberbank's data handling

For SaluteSpeech-specific concerns, contact Sberbank directly: `SaluteSpeech@sberbank.ru`.

### Contact

Until a dedicated security email is configured:

- **GitHub Security Advisories** (preferred): https://github.com/dimasiksuleyman-sudo/glagol/security/advisories/new
- **GitHub Discussions** (for general questions): https://github.com/dimasiksuleyman-sudo/glagol/discussions

---

## Русский

### Поддерживаемые версии

| Версия  | Поддержка          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: (текущая разработка) |
| < 0.1   | :x: (pre-release)  |

После релиза v1.0 мы будем поддерживать актуальную мажорную версию с security-патчами.

### Как сообщить об уязвимости

**Пожалуйста, НЕ открывайте публичный issue для уязвимостей безопасности.**

Если вы обнаружили уязвимость:

1. **GitHub Security Advisories** (предпочтительный способ): [приватный отчёт](https://github.com/dimasiksuleyman-sudo/glagol/security/advisories/new)
2. **Email:** security@glagol.app *(placeholder)*
3. **PGP-шифрованный контакт:** доступен по запросу

Мы следуем **90-дневной политике ответственного раскрытия**:

- Подтверждение получения отчёта в течение 72 часов
- Первичная оценка в течение 7 дней
- Исправление или mitigation в течение 90 дней
- Упоминание исследователя в security advisory (если не предпочитает анонимность)

### Модель угроз — от чего защищаем

Glagol — **локальное desktop-приложение**, обрабатывающее документы пользователя и взаимодействующее со сторонним API (SaluteSpeech).

#### ✅ От чего защищаем

- **Кражу API-ключей** — Authorization Keys хранятся в Windows Credential Manager (шифрование на уровне ОС), никогда в plain text
- **MITM-атаки** на соединение с SaluteSpeech — TLS-пиннинг с встроенным корневым сертификатом НУЦ Минцифры РФ
- **Вредоносное содержимое файлов** — DOCX/PDF парсятся через memory-safe Rust crates; JavaScript в PDF не исполняется
- **Code injection** — строгая Content Security Policy ограничивает сетевые запросы только эндпоинтами SaluteSpeech
- **Supply chain атаки** — `pnpm-lock.yaml` и `Cargo.lock` закоммичены; Dependabot включён; release-артефакты подписаны Ed25519
- **Неподписанные обновления** — Tauri updater требует подпись Ed25519
- **Утечку данных** — никакой телеметрии по умолчанию; никаких данных не отправляется никуда кроме SaluteSpeech (под аккаунтом самого пользователя)

#### ❌ От чего НЕ защищаем

- **Скомпрометированный компьютер** — при компрометации Windows-аккаунта атакующий получает доступ к Credential Manager
- **Скомпрометированный аккаунт SaluteSpeech** — безопасность Сбер-аккаунта на ответственности пользователя
- **Вредоносных контрибьюторов** — мы ревьюим PR, но не можем гарантировать отсутствие zero-day в зависимостях
- **Физический доступ** к машине

### Что мы НЕ собираем

Glagol **НЕ** собирает, не передаёт и не хранит ни на каких удалённых серверах:

- ❌ Содержимое ваших текстов (отправляется только в SaluteSpeech под вашим аккаунтом)
- ❌ Сгенерированное аудио (хранится только на вашей машине)
- ❌ Метаданные библиотеки документов
- ❌ Телеметрию использования
- ❌ Отчёты об ошибках (Sentry — **opt-in**, отключён по умолчанию)
- ❌ IP-адреса, fingerprint устройства, hardware ID

### Управление секретами

| Секрет | Хранение | Шифрование |
| ------ | -------- | ---------- |
| `Authorization Key` от SaluteSpeech | Windows Credential Manager (`keyring-rs`) | На уровне ОС (DPAPI) |
| OAuth `access_token` | Только в RAM, не персистится | n/a |
| Документы и аудио | Локальная файловая система (`%LOCALAPPDATA%\Glagol\`) | Нет (на усмотрение пользователя — full-disk encryption) |

**Мы никогда не логируем, не передаём и не отображаем секреты нигде.**

### Сетевые границы

Glagol делает сетевые запросы **только** к этим адресам:

| Эндпоинт | Назначение | Когда |
| -------- | ---------- | ----- |
| `https://ngw.devices.sberbank.ru:9443/api/v2/oauth` | Получение OAuth токена | При истечении токена (~30 мин) |
| `https://smartspeech.sber.ru/rest/v1/text:synthesize` | Синтез речи | По запросу пользователя |
| `https://api.github.com/repos/dimasiksuleyman-sudo/glagol/releases/latest` | Проверка обновлений | При запуске (отключаемо в настройках) |

**Никаких аналитических сервисов. Никаких рекламных сетей. Никакого CDN-трекинга.**

Это обеспечено CSP и allowlist'ом capability в Tauri — попытка добавить другие эндпоинты требует изменения кода, видимого в публичном репозитории.

### Сторонние сервисы

Glagol интегрируется с **SaluteSpeech API** от ПАО Сбербанк. При использовании:

- Ваш текст отправляется на серверы Сбербанка для синтеза
- Применяются [Политика конфиденциальности](https://www.sberbank.com/privacy) и [EULA](https://developers.sber.ru/docs/ru/policies/eula) Сбербанка
- Ваши отношения со Сбером независимы от использования Glagol
- Мы не имеем доступа к данным, обрабатываемым Сбербанком

По вопросам, специфичным для SaluteSpeech, обращайтесь в Сбер напрямую: `SaluteSpeech@sberbank.ru`.

### Контакты

До настройки выделенного email для безопасности:

- **GitHub Security Advisories** (предпочтительно): https://github.com/dimasiksuleyman-sudo/glagol/security/advisories/new
- **GitHub Discussions** (для общих вопросов): https://github.com/dimasiksuleyman-sudo/glagol/discussions

---

*Last updated: 2026-05*
