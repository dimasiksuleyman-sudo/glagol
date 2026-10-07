# Журнал

## 2026-10-07T12:19:44.000Z — начало

Ветка claude/pensive-wright-8jxin9, baseline local e5a5b64 (origin/main 16e3a87).
Серия overlay-resume закрыта: R1 автоматические проверки, R2 manual на ноутбуке.
Версия поднята до 0.5.1: package.json, Cargo.toml, Cargo.lock, tauri.conf.json;
`node scripts/check-version.mjs` — exit 0, «Glagol 0.5.1: package, Tauri and
Cargo versions match.» CHANGELOG: секция v0.5.1. Шаг P1 занят.

## 2026-10-07T12:24:03.000Z — P1 гейты

Linux-контейнер, rustc 1.97.0, Node v22.22.0, pnpm 10.28.0, cwd корень:

- node scripts/check-version.mjs — exit 0, Glagol 0.5.1 versions match.
- node scripts/i18n-check.mjs — exit 0, I18N PASS: 232 keys; i18n tests 3 pass / 0 fail.
- node --test scripts/runbook-check.test.mjs — exit 0, 49 pass / 0 fail.
- pnpm build (check-version + i18n + tsc + vite) — exit 0, built.
- cargo fmt --check, cargo clippy --all-targets -- -D warnings — exit 0 (src-tauri).
- cargo test — exit 0, 343 passed; 0 failed; 6 ignored.

Следующий шаг P2: пользователь собирает NSIS на Windows из этой ветки.

## 2026-10-07T12:38:33.000Z — P2 NSIS и обновление на Windows пользователя

Сборка пользователем на Windows, C:\Projects\glagol, ветка на f1356e2:
`pnpm tauri build` — завершилась, установщик
src-tauri/target/release/bundle/nsis/Glagol_0.5.1_x64-setup.exe:
9948410 байт (+984 к 0.5.0), LastWriteTimeUtc 2026-10-07 12:28:14,
SHA-256 01791f02ed250797d611d5ac723282f6d5e58e33f03d48c337f056f35b85a922
(Get-FileHash, сообщено пользователем). Код возврата сборки не передан;
наличие нового файла со свежим временем — решающее наблюдение.

Manual (сообщено пользователем): бэкап, «Выход» из трея, установка поверх 0.5.0;
установленная версия 0.5.1, таблетка видна, текст вставлен. Проверка сна на
установленной сборке не повторялась (выполнена на dev-сборке, overlay-resume R2).

Документация: docs/releases/v0.5.1.md (EN/RU), README (ссылка, размер, версия),
обе USER_GUIDE и USER_GUIDE.md (версия, ограничение короткого сна).
