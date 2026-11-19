# Журнал: диктовка по модификаторам

## 2026-11-10T07:21:29.000Z — серия создана как TODO

По просьбе пользователя «Да, запиши». Работа не начата; серия на паузе до
выпуска 0.6.5 (серия ux-help). Перед M1 согласовать объём и порядок с
пользователем.

## 2026-11-19T00:00:00.000Z — место в очереди

Пользователь собрал побочные задачи в план (docs/roadmap.md) и выбрал порядок:
озвучка по умолчанию выключена → Ctrl+Win → сервер в сети. Эта серия вторая;
статус не меняется, старт по просьбе пользователя.

## 2026-11-19T12:00:00.000Z — старт M1

Пользователь: «го контрол+вин». Решения (AskUserQuestion): Ctrl+Win работает
вместе с обычной горячей клавишей (она — запасной путь в окнах от
администратора, куда LL-хук не получает ввод); другая клавиша при удержании —
диктовка молча отменяется, начало записи отложено ~200 мс. План — машина
состояний (кроссплатформенная, с тестами), WH_KEYBOARD_LL в отдельном потоке
без поглощения и журналирования нажатий, маска «Пуска» VK 0xE8 через
SendInput, отмена в конвейере, переключатель по умолчанию выкл. M1 занят.

## 2026-11-19T13:30:00.000Z — M1 выполнен, тестовая сборка 0.6.7

Код: `dictation/modifier_trigger.rs` — чистая машина состояний (Ctrl+Win
удерживаются без других клавиш ≥ 200 мс → Press; отпускание Ctrl или Win →
Release; другая клавиша после Press → Cancel, до Press — сброс без действия;
повторный старт только после отпускания обеих), 7 тестов; Windows `imp`:
WH_KEYBOARD_LL в потоке «ctrl-win-hook» с циклом GetMessageW, колбэк отдаёт
только класс клавиши в mpsc и всегда CallNextHookEx, пропускает LLKHF_INJECTED;
рабочий поток «ctrl-win-trigger» с recv_timeout; при Press — SendInput VK 0xE8
(маска «Пуска»); stop — очистка канала и WM_QUIT. Конвейер: `StopSignal
{Release, Cancel}`, `DiscardReason::Cancelled`, `session::cancel_dictation`,
тест «cancel discards without recognition». Настройка `dictation_ctrl_win`
(по умолчанию false) применяется сразу (`session::apply_ctrl_win`, через
run_on_main_thread) и при запуске (`lib.rs`); переключатель «Также по удержанию
Ctrl+Win» под редактором клавиши (только если `ctrl_win_supported`). Новые
features `windows`: Win32_System_Threading, Win32_UI_Input_KeyboardAndMouse
(Cargo.lock без изменений). Документы: USER_GUIDE RU/EN, README обе части,
CHANGELOG [Unreleased], SECURITY.md EN/RU, CLAUDE.md, PROJECT_STRUCTURE.md,
справка (helpDictateHint). Версия 0.6.7 (тестовая сборка).
Гейты: cargo fmt --check; clippy --all-targets -D warnings; cargo test — 384
passed, 6 ignored; Windows-код — clippy -D warnings для x86_64-pc-windows-msvc в
отдельном мини-крейте с windows =0.61.3 (OK); pnpm exec tsc --noEmit; pnpm build —
I18N PASS 356, ✓ built. Хук на Windows не запускался — M2 (ручная проверка).
