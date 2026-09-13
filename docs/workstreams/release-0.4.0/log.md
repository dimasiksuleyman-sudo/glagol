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
