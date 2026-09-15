# Журнал публикации Glagol 0.4.1

Журнал append-only. Даты UTC.

## Начало — 2026-09-15T11:03:00.000Z

Пользователь подтвердил UI подготовки озвучки и явно поручил подготовить и
опубликовать GitHub Release. Выбран patch release v0.4.1: опубликованный v0.4.0
не изменяется. Перед началом удалённый main совпадает с локальным dbb56d5;
refs/tags/v0.4.1 отсутствует; GitHub API показывает latest v0.4.0 и права push.

Серия verification-tts-warm-start завершена. Direct-run candidate подтвердил
30-дневную отметку, прогрев около 3,5 с, повторную подготовку 0 мс, видимую
индикацию и idle unload через 901 926 мс. P1 занят исполнителем codex для
версии, changelog, двуязычных руководств и release notes.

## P1, версия и документация — 2026-09-15T11:07:00.000Z

Версия синхронизирована как 0.4.1 в package.json, Cargo.toml, Cargo.lock и
tauri.conf.json. Изменения перенесены из Unreleased в секцию v0.4.1. README,
русское/английское руководство и обе runtime-инструкции обновлены; создано
двуязычное описание выпуска с замерами, лицензией Silero и явным NOT_RUN для
свежей ручной диктовки. Исторические скриншоты и evidence 0.4.0 не переименованы.

`node scripts/check-version.mjs` — exit 0, версии 0.4.1 совпадают;
49 тестов runbook-checker — PASS; `context:refresh` и `runbook:check` — PASS
для 6 серий; `git diff --check` — exit 0 с информационными LF→CRLF.
P1 завершён, P2 занят codex для полной проверки, NSIS и обновления установленной
копии с заранее проверенным бэкапом библиотеки.

## P2, полная проверка и NSIS — 2026-09-15T11:15:00.000Z

Финальный pipeline 0.4.1 завершён exit 0: context/runbook/version PASS;
TypeScript/Vite PASS (1914 modules); cargo fmt и clippy `-D warnings` PASS;
cargo test — 330 PASS, 0 FAIL, 3 ignored hardware/native. Release build
использовал документированный локальный PDFium override: попытка сетевой загрузки
дала curl exit 7, существующий проверенный `src-tauri/resources/pdfium.dll`
позволил продолжить. Tauri/NSIS build завершён exit 0.

Итоговый `Glagol_0.4.1_x64-setup.exe`: 9 777 903 байта,
FileVersion/ProductVersion 0.4.1, SHA-256
5e23d37d606b8fa529e96ab2dc139e4c195c9f6b81f74f0424ef5b174e45df21.
Release EXE: 26 688 512 байт, SHA-256
920f7821571c74f5eb9476f7ce5d95e10a10a45b5eb15ab7a9ae947b0b5ce7c4.

Штатный тихий update установленной 0.4.0 выполнен через NSIS `/S`, exit 0.
Установленный EXE имеет FileVersion/ProductVersion 0.4.1; его SHA отличается
от release EXE из-за документированной NSIS bundle-метки. Короткий запуск
установленной копии: отметка 2 мс, UI 699 мс, worker/warmup 3872/3873 мс;
затем остановлен только тестовый процесс. `glagol.db`, verification receipt и
Silero runtime после обновления присутствуют; содержимое пользовательских данных
не читалось. Бэкап до теста уже проверен отдельно.

После добавления точного размера/SHA в README и release notes повторно прошли
context:refresh, runbook:check, version check, TypeScript `--noEmit`,
`git diff --check` и Get-FileHash. Новая clean install и свежая live-microphone
диктовка остаются NOT_RUN. P2 завершён; P3 занят codex для commit/tag/push и CI.

## P3, Git и CI — 2026-09-15T11:23:00.000Z

Проверенные изменения закоммичены как
`6bcb18691ad3aa9980722b697b0986f04ea27763`
(`feat: speed up Silero warm start`). Lightweight tag v0.4.1 создан на том же
commit. `git push --atomic origin main refs/tags/v0.4.1` — exit 0: удалённый
main обновлён dbb56d5→6bcb186, новый тег отправлен вместе с ним.

GitHub Actions run 34962399690 для этого SHA завершён `success`:
https://github.com/dimasiksuleyman-sudo/glagol/actions/runs/34962399690
Quality gates (Windows) прошли, включая контекст/ранбуки, TypeScript,
fmt, clippy, Rust tests, Tauri/NSIS build и CI artifact. Публичный Release на
этом checkpoint ещё не создавался. P3 завершён; P4 занят codex для draft,
assets, публикации и повторной публичной сверки.

## P4, GitHub Release и публичная сверка — 2026-09-15T11:26:27.788Z

После успешного CI создан draft GitHub Release 389086058. Загружены:

- `Glagol_0.4.1_x64-setup.exe` — 9 777 903 байта, GitHub digest и локальный
  SHA-256 совпадают:
  5e23d37d606b8fa529e96ab2dc139e4c195c9f6b81f74f0424ef5b174e45df21;
- `SHA256SUMS.txt` — 93 байта, SHA-256
  1659ec3eccfa157ce100bcf7ebc383d23f9392ce881b03820c104f7c6451b655.

Draft опубликован 2026-09-15T11:26:09Z как public/latest, не prerelease:
https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.4.1

`node .scratch/github-release-v041.mjs verify` — exit 0. Оба asset заново
скачаны по публичным ссылкам и побайтово совпали с локальными данными; installer
повторно дал ожидаемый SHA-256. Latest = v0.4.1, тег указывает на 6bcb186.
README, обе USER_GUIDE, CHANGELOG и release notes на теге совпадают с локальными
текстами после нормализации CRLF/LF.

P1–P4 завершены. Серия закрыта. Финальный docs-only checkpoint обновляет только
состояние доставки и отправляется отдельным Conventional Commit с `[skip ci]`;
код/версия/релизный артефакт после успешного CI не изменялись.
