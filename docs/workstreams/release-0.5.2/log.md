# Журнал

## 2026-10-09T06:49:51.000Z — начало и P1

Ветка claude/pensive-wright-8jxin9, baseline local 04ce255 (origin/main 1b3ed4e, тег v0.5.1).
Изменения после 0.5.1: d9c5386 fix(i18n) — «Настройки» без кавычек в меню и
заголовке; 04ce255 feat(ui) — ссылка GitHub в боковом меню (openUrl, opener:default).
Manual (сообщено пользователем, `pnpm tauri dev` на Windows): подпись без кавычек;
ссылка открывает профиль на EN и RU. Версия поднята до 0.5.2 в четырёх источниках.

## 2026-10-09T06:54:03.000Z — P1 гейты

Linux-контейнер, rustc 1.97.0, Node v22.22.0, pnpm 10.28.0, cwd корень: check-version exit 0
(0.5.2 match); i18n 235 keys PASS, i18n tests 0 fail; runbook tests 49 pass;
pnpm build exit 0; cargo fmt --check, clippy -D warnings exit 0; cargo test exit 0 —
343 passed; 0 failed; 6 ignored. Следующий шаг P2 — NSIS на Windows пользователя.

## 2026-10-09T07:15:19.000Z — P2 NSIS и обновление на Windows пользователя

Первая попытка сборки шла с HEAD 04ce255 (git pull не выполнен) и собрала 0.5.1;
она перезаписала локальный Glagol_0.5.1_x64-setup.exe пользователя. Опубликованный
asset v0.5.1 не затронут; локальный файл с этим именем не использовать.
git pull блокировало изменение src-tauri/Cargo.toml — только LF/CRLF (git diff пуст,
предупреждение о CRLF); отменено git restore, pull до 054904d.

Повторная сборка пользователем: `pnpm tauri build` — «Glagol 0.5.2: package, Tauri
and Cargo versions match», «Finished 1 bundle». Glagol_0.5.2_x64-setup.exe:
9943738 байт (−4672 к 0.5.1), LastWriteTimeUtc 2026-10-09 07:03:34,
SHA-256 e99d29b9aff915b0d0540caad3273e860799fabe562451024af98b907a88bccb.
SHA256SUMS.txt подготовлен пользователем, строка совпадает.

Manual (сообщено пользователем): установка поверх, запущенный процесс
FileVersion/ProductVersion 0.5.2. Подпись и ссылка проверены ранее на dev-сборке
(EN/RU); функциональная проверка установленной 0.5.2 запрошена отдельно.

Документация: docs/releases/v0.5.2.md (EN/RU, известная проблема GL-006),
README, обе USER_GUIDE и USER_GUIDE.md. Решение пользователя: самообновление
(tauri-plugin-updater, переключатель автопроверки, показ версии) — отдельная
серия для 0.6.0 после этого релиза.
