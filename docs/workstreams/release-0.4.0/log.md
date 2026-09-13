# Журнал публикации 0.4.0

## Начало — 2026-09-13T14:27:50.246Z

Авторизация: прямое указание пользователя опубликовать 0.4.0 и обновить GitHub.
main на GitHub проверен git ls-remote: 2fa523c34c520dd4fa4fe6df41802bbd3e4b2e2c;
тег v0.4.0 отсутствует. GitHub Releases API подтверждает последний v0.2.1.
Локальный HEAD e05c852; изменения контекста/ранбуков/QA ещё не закоммичены.
Артефакт .scratch/qa-v2-kit/Glagol_0.4.0_x64-setup.exe повторно проверен:
SHA-256 569d707ff2d3bdb0b5f83c6686c6d66889aa6d135ed2161f20bdaa9e2f62f7d3.
Второй ПК подтвердил чистую установку, обновление 0.2.1, бэкапы, пять голосов,
короткий синтез и экспорт. Остаток QA раскрывается в release notes.

## Документация и проверки — 2026-09-13T14:33:48.493Z

Подготовлены README RU/EN, обе USER_GUIDE, CHANGELOG с датой публикации
13 сентября и пометкой непубликовавшейся 0.3.0, двуязычные release notes,
About (описание, homepage Releases/latest, 12 topics), SHA256SUMS.txt.
Использованы оригинальные скриншоты первого запуска и пустой библиотеки.
QA приостановлена на T60; ручные результаты T01–T59 сохранены с границами.

Выполнено на Node v24.19.0, Windows x64:
- node --test scripts/runbook-check.test.mjs: 49/49 PASS, 53.28 s, exit 0.
- node scripts/runbook-check.mjs: OK (3 series), exit 0.
- node scripts/check-version.mjs: 0.4.0 версии совпадают, exit 0.
- node node_modules/typescript/bin/tsc --noEmit: exit 0.
- git diff --check: exit 0.
- Проверка 47 локальных Markdown-ссылок и SHA-256 13 PNG: PASS.
- git diff --exit-code e05c852 -- src src-tauri pnpm-lock.yaml public index.html:
  exit 0, продукт неизменён относительно исходников проверенного установщика.
- Установщик: 9 742 481 байт, SHA-256 совпадает с ручным прогоном.

GitHub API через настроенный credential helper подтвердил admin/push-доступ;
секреты не выводились и не записывались. Следующее: commit, fast-forward main
и тег v0.4.0, draft Release с установщиком/checksums и About; перед публичным
Release дождаться CI. Скрипт публикации проверяет hash и привязку тега.

## GitHub staging — 2026-09-13T14:41:00.133Z

Commit 063ba935fd8dedd8badb76f800aec5dd2bae548a отправлен fast-forward в main;
легковесный тег v0.4.0 указывает на тот же commit. Draft Release 387928440
содержит установщик (9742481 байт, GitHub digest совпал с ожидаемым SHA-256)
и SHA256SUMS.txt. About: описание/homepage/topics обновлены по v0.4.0-about.json.
CI: https://github.com/dimasiksuleyman-sudo/glagol/actions/runs/34763158935
Перед публикацией ожидается его завершение. В staged diff обнаружена
унаследованная строка с пробелами в историческом снимке CLAUDE, строка 276;
снимок сохранён без редактирования. Это замечание форматирования, не ошибка
тестов или продукта; ранний git diff --check относился к tracked-изменениям.

## Публикация и проверка — 2026-09-13T15:17:30.259Z

Release: https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.4.0
Тег v0.4.0: 063ba935fd8dedd8badb76f800aec5dd2bae548a. CI этого commit успешно завершился до
публикации: https://github.com/dimasiksuleyman-sudo/glagol/actions/runs/34763158935
Включая контекст/49 тестов, TypeScript, fmt, Clippy, Rust tests, NSIS build.
Публичный asset — проверенный локальный установщик; CI-сборка отдельный артефакт.

node .scratch/github-release-api.mjs verify: exit 0. Через публичную ссылку
заново скачаны установщик и SHA256SUMS; оба побайтово совпали с исходниками.
Установщик 9742481 байт, SHA-256
569d707ff2d3bdb0b5f83c6686c6d66889aa6d135ed2161f20bdaa9e2f62f7d3.
Release не draft и не prerelease, отмечен latest; тег/описание выпуска совпали.
About: описание, homepage и 12 тем совпали с подготовленным JSON.
README, две USER_GUIDE, CHANGELOG, release notes и манифест скриншотов в main
сверены с локальными текстами (с нормализацией CRLF/LF); SHA-256 всех 13 PNG
на теге совпали с оригиналами.

D1–D3 завершены. QA остаётся paused после T59, продолжение T60 NOT_RUN;
публикация не закрывает V3/V4/V5. Финальный checkpoint меняет только контекст
доставки, его отдельный docs commit использует [skip ci], чтобы не повторять
полную сборку неизменного приложения. Перед коммитом context:refresh и
runbook:check выполняются повторно.
