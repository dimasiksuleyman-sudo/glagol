# Журнал

## 2026-10-09T07:28:25.000Z — начало

Baseline local c9480b3 (main 95d5643, v0.5.2). Пользователь выбрал полный
updater (вариант 2), переключатель автопроверки и показ версии. Пользователю
отправлена инструкция создать ключи `pnpm tauri signer generate` локально;
ожидается открытый ключ. Шаг U1 занят.

## 2026-10-09T07:36:36.000Z — U1 реализация (ожидает открытый ключ)

Зависимость: tauri-plugin-updater =2.12.0, default-features=false, rustls-tls.
2.13.x отклонён: требует tauri 2.12 (обновление фреймворка 2.11→2.12, wry 0.55→0.57)
и добавляет windows 0.62 рядом с 0.61 — класс сбоя D1-B. С 2.12.0 Cargo.lock только
дополняется (tauri-plugin-updater, minisign-verify, rustls-platform-verifier и
платформенные зависимости); версии существующих пакетов не меняются.

Rust: commands/updates.rs — get_update_settings (версия из package_info,
флаг update_auto_check, по умолчанию выключен), set_update_auto_check,
check_for_update (PendingUpdate), install_update (Channel-прогресс; отказ во время
диктовки, при занятых TTS/STT operation-локах; подпись проверяет плагин; на Windows
процесс завершается установщиком). Плагин без webview-разрешений.
tauri.conf.json: plugins.updater — endpoint releases/latest/download/latest.json,
installMode passive (NSIS-хуки уже пропускают свою страницу в passive);
pubkey — временная заглушка до получения ключа пользователя.
tauri.release.conf.json: createUpdaterArtifacts для подписанной сборки; CI без ключа.
UI: версия «v…» в боковом меню; «Настройки» → «Обновления» (версия, кнопка,
переключатель, подтверждение, прогресс); автопроверка один раз при запуске,
уведомление с переходом в настройки. scripts/release-assets.mjs: latest.json и
SHA256SUMS.txt; 3 теста, мутация (удаление ключа windows-x86_64-nsis) даёт 1 fail.
Документация: обе USER_GUIDE, SECURITY EN/RU, CHANGELOG, ранбук release, карта кода.

Проверки (Linux-контейнер): i18n 250 keys PASS; pnpm build exit 0;
release-assets tests 3 pass; cargo fmt/clippy exit 0; cargo test 345 passed,
0 failed, 6 ignored; runbook-check OK (13 series). Не проверено: реальная проверка,
загрузка и установка обновления — нужен ключ и Windows (U2/U3).

## 2026-10-09T08:43:12.000Z — U1 завершён

Добавлено после первого checkpoint: открытый ключ minisign 82D55017631804B2 (8f7c4e1),
версия 0.6.0 (593f423), .gitattributes LF для Cargo.toml/Cargo.lock (1de1d7f затёр
правило лицензии Silero, восстановлено в ca43f83 — итог только добавления),
ежемесячное напоминание при выключенной автопроверке (fae9f73), тост с текстом на
всю ширину и кнопками строкой ниже, «Позже», без автоскрытия, debug-переменная
GLAGOL_FORCE_UPDATE_REMINDER (2696189), кнопки слева, основная первой (2a029e7).

Manual (пользователь, `pnpm tauri dev`, Windows): «Glagol v0.6.0» в боковом меню;
раздел «Обновления» с версией, переключателем (выключен) и кнопкой; проверка до
публикации latest.json даёт ожидаемую ошибку; напоминание при первом запуске,
повторный запуск без него; «Включить автопроверку» из тоста включает переключатель;
вид тоста подтверждён скриншотом.

Автоматика (Linux-контейнер, rustc 1.97.0, Node v22.22.0, pnpm 10.28.0): cargo fmt/clippy
(dev и release) exit 0; cargo test 346 passed, 0 failed, 6 ignored; i18n 256 keys PASS;
pnpm build exit 0; release-assets tests 3 pass. Следующий шаг U2 — подписанная NSIS.

## 2026-10-09T08:56:36.000Z — U2 подписанная сборка и ручная установка 0.6.0

Пользователь на Windows, ветка на d75c108: ключ и пароль заданы только в процессе
PowerShell и удалены после сборки. `pnpm tauri build --config
src-tauri/tauri.release.conf.json` — «Glagol 0.6.0: … versions match», «Finished 1
bundle», «Finished 1 updater signature» (Glagol_0.6.0_x64-setup.exe.sig).
`pnpm release:assets --notes "Glagol 0.6.0: in-app updates"` — latest.json и
SHA256SUMS.txt записаны. Установщик 10262171 байт, LastWriteTimeUtc 2026-10-09 08:47:43,
SHA-256 77ae4ec09fae11fa87989c02b9f588f62fe03fdc14c7cee5b981909837bb2a38.
latest.json: version 0.6.0, обе цели windows-x86_64-nsis/windows-x86_64 → URL v0.6.0;
подпись: trusted comment file:Glagol_0.6.0_x64-setup.exe, ID ключа совпадает с pubkey.

Manual (сообщено пользователем): установка поверх 0.5.2; «Glagol v0.6.0», раздел
«Обновления», диктовка работают. Напоминание не показано — ожидаемо: общая с dev
база настроек, напоминание уже показано в dev (месячная пауза).
Документация: docs/releases/v0.6.0.md, README, обе USER_GUIDE, USER_GUIDE.md.
Далее PR, merge, Release v0.6.0 с установщиком, latest.json и SHA256SUMS.txt.

## 2026-10-09T09:12:55.000Z — U2 PR, merge, публикация и сверка

PR https://github.com/dimasiksuleyman-sudo/glagol/pull/48 — CI «Quality gates (Windows)»
success (08:57–09:07 UTC, run 37908143267), review threads 0. Merge commit
0dc78220a2eded7ec196bbd1749c71d025dc521c в main.

Release https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.6.0 — создан
пользователем, published 2026-10-09T09:11:41Z, draft=false, prerelease=false;
v0.6.0^{commit} = 0dc7822. Assets: Glagol_0.6.0_x64-setup.exe 10262171 байт
(digest sha256:77ae4ec0…37bb2a38), latest.json 1304 байта, SHA256SUMS.txt 93 байта.
Скачаны curl, exit 0; `sha256sum -c` — OK. Адрес программы
releases/latest/download/latest.json отдаёт тот же файл (cmp), version 0.6.0, обе
цели → установщик v0.6.0. Подпись проверена независимо: `minisign -V` с pubkey из
tauri.conf.json — «Signature and comment signature verified», exit 0.
Остаётся U3: выпуск 0.6.1 и обновление из установленной 0.6.0.

## 2026-10-09T09:25:33.000Z — U3 подготовка 0.6.1

Manual (сообщено пользователем, десктоп с установленной 0.6.0): «Проверить
обновления» и кнопка тоста дают «У вас последняя версия» — проверка через
реальный GitHub latest.json; автопроверка включена пользователем.
0.6.1 по согласованию: блок «Что нового» (notes из подписанного latest.json, как
текст) в окне подтверждения; ранбук windows-build ссылается на release:assets.
Версия 0.6.1. Автоматика: i18n 257 keys, pnpm build, cargo fmt/clippy/test
exit 0. Далее подписанная сборка, PR, Release; критерий U3 — установленная 0.6.0
находит и ставит 0.6.1 из программы.

## 2026-10-09T09:34:59.000Z — U3 подписанная сборка 0.6.1

Первая попытка: git pull снова остановился на CRLF в Cargo.toml (рабочая копия
до .gitattributes), остальные команды блока выполнились на d75c108 и собрали
0.6.0 (d582eeb4…), перезаписав локальные Glagol_0.6.0_x64-setup.exe, latest.json
и SHA256SUMS.txt. Опубликованный v0.6.0 (77ae4ec0…) не затронут; локальные файлы
0.6.0 не использовать. Причина в инструкции: pull и сборка одним блоком; далее
pull отдельным шагом с проверкой.

git restore + pull до 1f680ee, чисто. Сборка с ключом в процессе: «Finished 1 bundle»
Glagol_0.6.1_x64-setup.exe и «Finished 1 updater signature»; ключ удалён.
release:assets: 82c127f467a22eadd17be8ac87601c8f547df1afaa792470c4cda9405ae2c618,
10263535 байт. После сборки `git status -sb` чистый — .gitattributes устранил
CRLF-изменения Cargo.toml. Документация: docs/releases/v0.6.1.md, README, обе
USER_GUIDE, USER_GUIDE.md.
