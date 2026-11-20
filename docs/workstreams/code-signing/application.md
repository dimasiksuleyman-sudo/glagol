# Черновик заявки в SignPath Foundation

Поля сверены с действующей [формой Apply](https://signpath.org/apply) 20 ноября 2026
года. Имя, email и согласия заполняет сопровождающий при подаче.

| Поле формы | Значение |
|---|---|
| Project Name | Glagol |
| Repository URL | https://github.com/dimasiksuleyman-sudo/glagol |
| Homepage URL | https://github.com/dimasiksuleyman-sudo/glagol |
| Download URL | https://github.com/dimasiksuleyman-sudo/glagol — главная страница содержит ссылку на установщик и политику подписи |
| Privacy Policy URL | https://github.com/dimasiksuleyman-sudo/glagol#code-signing-policy — после публикации политики |
| Wikipedia URL (optional) | Оставить пустым, если у проекта нет статьи в английской Википедии |
| Tagline | Offline English and Russian dictation and text-to-speech for Windows. |
| Category | Accessibility |
| Maintainer Type | Individual maintainer(s) |
| Build System | GitHub Actions |
| First Name, Last Name, Email | Контакты сопровождающего для учётной записи SignPath и ответа на заявку |
| Company Name | Заполнить, если применимо |
| Primary Discovery Channel | Фактический источник, через который сопровождающий узнал о Foundation |

## Description

Glagol is a free, MIT-licensed Windows desktop app for English and Russian
dictation and text-to-speech. It lets users dictate into other apps, listen to
documents, and keep recordings in a local library. Speech can be processed
offline on the user's computer. Users can also choose to configure their own
office or cloud provider for dictation.

## Reputation

Glagol is an independently maintained project with public releases and
documented installation and update instructions. On November 20, 2026, GitHub
reported 48 installer asset downloads across 12 public stable releases. Four
were for v0.6.6, the latest release at the time.
[Releases](https://github.com/dimasiksuleyman-sudo/glagol/releases) ·
[v0.6.6 release](https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.6.6) ·
[User guide](https://github.com/dimasiksuleyman-sudo/glagol/blob/main/USER_GUIDE.en.md).

Перед отправкой обновить счётчики или оставить дату снимка. Можно добавить
публичные отзывы или обсуждения, если есть ссылки.

## Сборка и компоненты

Текст для пояснения к заявке или ответа на уточнения Foundation.

Glagol's application source is available under the MIT License. The existing
Windows GitHub Actions workflow checks the code and builds the NSIS installer.
The currently published Windows installer is built separately by the maintainer.
We are requesting Authenticode signing for the Windows x64 NSIS installer
built by GitHub Actions from this repository. The maintainer will review changes
from contributors without commit access and manually approve every release
signing request. Team members must use MFA on GitHub and SignPath.

The installer includes the application and the upstream open-source PDFium
library for document import. The requested Authenticode signature covers the
outer installer. Any upstream library retains its own license and signature
status. Speech weights and speech runtimes are separate optional downloads.
This keeps the installer small and lets users install only the speech
components they need under the applicable licenses.

| Model | License and availability |
|---|---|
| Silero v5.5 RU and v3 EN | CC BY-NC-SA 4.0 for noncommercial use. Each model is an optional download requiring acknowledgement. [Silero license](../../third-party/Silero-LICENSE.txt) |
| GigaAM v3 CTC/RNNT | MIT. Optional Russian dictation models. [Upstream license](https://github.com/salute-developers/GigaAM/blob/main/LICENSE) · [Download sources](../../local-dictation-runtime.md) |
| Moonshine Small Streaming (English) | MIT. Optional English dictation model. The upstream license notice describes separate terms for legacy non-streaming models in some other languages. [Moonshine license notice](../../third-party/Moonshine-LICENSE.txt) |
| Qwen3-TTS | Apache 2.0. Planned as an optional separate download. The current installer does not include it. [Upstream repository](https://github.com/QwenLM/Qwen3-TTS) · [Model license](https://huggingface.co/Qwen/Qwen3-TTS-12Hz-0.6B-Base) |

Please confirm whether the optional, separately downloaded Silero models are
compatible with Foundation eligibility given their noncommercial license
and their exclusion from the signed installer.

[MIT license](../../../LICENSE) · [Windows CI workflow](../../../.github/workflows/ci.yml) ·
[Current release process](../../runbooks/release.md) ·
[Proposed code signing policy](../../../README.md#code-signing-policy).
