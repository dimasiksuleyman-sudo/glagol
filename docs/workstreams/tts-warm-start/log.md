# Быстрый запуск Silero — журнал

Журнал append-only. Даты UTC.

## Начало — 2026-09-15T06:11:36.750Z

Пользователь разрешил реализацию безопасного ускорения первой озвучки.
Baseline: dbb56d5, ветка main. `git status --short --branch` перед началом чистый;
`git diff` и `git diff --cached` пусты. Серия verification-0.4.0 остаётся paused
по прежнему пользовательскому решению и не изменяется.

Выбран строгий вариант: полный SHA-256 контроль модели и 14 483 файлов runtime
сохраняется перед первым исполнением в каждом процессе Glagol. Проверка переносится
в фон после запуска и повторно используется подготовкой; модель прогревается при
входе на экран синтеза. Дисковый месячный кеш и проверка только после ошибки не
входят в работу, потому что они позволяют исполнить незамеченный изменённый код.

R1 занят исполнителем codex. Следующий шаг: реализовать согласованный жизненный
цикл и модульные проверки, не затрагивая скачивание или формат runtime.

## R1 — 2026-09-15T06:17:32.279Z

Добавлен процессный кеш успешной полной проверки с async gate: фоновая проверка
и команда подготовки не хешируют файлы параллельно, ошибка не кешируется, а
установка и удаление инвалидируют результат. Установка после собственной полной
сборки помечает компоненты проверенными. При запуске Glagol уже установленный и
принятый Silero проверяется в фоне, но worker и 752 МБ рабочего набора пока не
поднимаются. Idle timeout изменён с 180 до 900 секунд.

Первый `cargo test --manifest-path src-tauri/Cargo.toml tts::silero::tests
--no-fail-fast` завершился exit 101: сеть для build.rs недоступна, а compile-time
`PDFIUM_LIBRARY_PATH` не был задан. Это FAIL окружения, не результат тестов.
После проверки существующего `src-tauri/resources/pdfium.dll` повтор выполнен с
документированным override из windows-build.md: exit 0, 3 passed, 0 failed,
0 ignored, 327 filtered out. Новые тесты подтверждают разделение одной проверки
между конкурентными вызывающими и повтор после ошибки/инвалидации.

`cargo fmt --manifest-path src-tauri/Cargo.toml --check` и
`node node_modules/typescript/bin/tsc --noEmit` завершились exit 0. Окружение:
Windows x64, rustc/cargo 1.98.1, Node v24.19.0. `git diff --check` exit 0 с
информационными предупреждениями LF→CRLF. Native Silero smoke с реальной моделью
на этом шаге NOT_RUN; формат runtime и синтез не менялись.

R1 завершён. R2 занят исполнителем codex: закончить frontend-прогрев и обновить
обязательную русскую/английскую пользовательскую документацию.

## R2 — 2026-09-15T06:20:43.786Z

Зарегистрирована команда `prepare_tts`; экран «Озвучить» вызывает её один раз
после обнаружения установленного и принятого Silero. Во время подготовки экран
показывает отдельный статус и ошибку backend. Фоновая проверка сама worker не
запускает; модель поднимается только при входе на экран или явной операции TTS.

Обновлены обе языковые части README, USER_GUIDE.ru/en, CHANGELOG, технические
runtime/structure/CLAUDE документы и TTS runbook. Зафиксированы: полная проверка
в фоне, отсутствие повторной загрузки/распаковки, процессный срок кеша,
предзагрузка модели, примерно 752 МБ рабочего набора и idle timeout 15 минут.

Validation: `pnpm.cmd build` — exit 0, TypeScript и Vite production build,
1914 modules transformed; `node scripts/check-version.mjs` — exit 0, версия
0.4.0 согласована; `git diff --check` — exit 0 с информационными LF→CRLF.
Окружение: Windows x64, Node v24.19.0, pnpm 12.4.1, Vite 7.3.3.

R2 завершён. R3 занят исполнителем codex: полный Rust/TS/context gate,
проверка итогового diff и честная фиксация ограничений native/manual QA.

## R3 — 2026-09-15T06:26:36.969Z

Итоговые проверки на Windows x64 с rustc/cargo 1.98.1, Node v24.19.0 и
pnpm 12.4.1: cargo fmt — PASS; cargo clippy all-targets с `-D warnings` — PASS;
полный cargo test — 327 passed, 0 failed, 3 ignored; runbook test — 49 passed,
0 failed; TypeScript/Vite production build — PASS, 1914 modules transformed;
version check и diff whitespace — PASS. Rust-команды использовали существующий
`src-tauri/resources/pdfium.dll` через документированный `PDFIUM_LIBRARY_PATH`;
build.rs сообщил, что сетевое скачивание Pdfium пропущено, но локальная сборка
завершилась успешно.

`pnpm.cmd tauri build` завершился exit 0 и создал новый локальный NSIS
`src-tauri/target/release/bundle/nsis/Glagol_0.4.0_x64-setup.exe`: 9 754 593
байта, LastWriteTimeUtc 2026-09-15 06:26:25, SHA-256
3798649e722789a32cf7a42cca39fc6b88018be34b0cb208ba0cbdd6d7275ee8.
Это локальная сборка поверх dirty tree, не установка, commit или публикация.

Промежуточный `pnpm.cmd runbook:check` до закрытия серии вернул exit 1 только
с `docs/STATUS.md stale`; это ожидаемый контроль сгенерированного файла после
checkpoint. Финальный refresh/check выполняется после перевода R3 и серии в done.

Native Silero smoke с закреплённой моделью/runtime, запуск GUI, измерение времени
на реальной установке и ручная проверка 15-минутной выгрузки NOT_RUN. Три ignored
теста полного cargo test: live microphone, GigaAM native smoke и Silero native
pipeline. Они не считаются PASS. Серия завершена, claim очищен, активной серии нет.

## Финальная проверка контекста — 2026-09-15T06:28:31.351Z

После закрытия серии: `pnpm.cmd context:refresh` — exit 0,
`context-status write: OK`; `pnpm.cmd runbook:check` — exit 0,
`runbook-check OK (4 series)`; `node scripts/check-version.mjs` — exit 0,
версия 0.4.0 согласована; `git diff --check` — exit 0 с информационными
LF→CRLF. STATUS теперь показывает отсутствие активной серии и отдельное
наблюдение локальной сборки; verification-0.4.0 остаётся paused без изменений.
