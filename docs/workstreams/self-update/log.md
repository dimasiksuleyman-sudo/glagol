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
