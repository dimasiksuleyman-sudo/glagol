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
