# Журнал

## 2026-10-07T09:41:37.000Z — начало

Изучены AGENTS, STATUS, context format, CLAUDE, карта кода. Ветка
claude/pensive-wright-8jxin9, чистое дерево, baseline local 16e3a87 (= origin/main).
Пользователь подтвердил: после сна текст вставляется, перезапуск возвращает
таблетку. Окно overlay создаётся один раз скрытым (lib.rs) и далее только
show/hide. Среда: Linux-контейнер, Windows-проверка сна здесь недоступна.
Шаг R1 занят.

## 2026-10-07T09:48:02.000Z — R1 реализация и проверки

Изменено: dictation/session.rs (build_overlay, overlay_label, spawn_resume_watch,
rebuild_overlay, resumed_from_sleep + 2 unit-теста), lib.rs (создание overlay
через build_overlay, запуск наблюдателя), capabilities/default.json (overlay-*),
src/main.tsx (метка overlay-N), CHANGELOG Fixed. Руководства не меняются:
восстанавливается уже описанное поведение, новых настроек нет.

Наблюдатель: поток каждые 3 с сравнивает ход системных часов; скачок больше
3 + 10 с считается выходом из сна. Через один следующий интервал, вне сеанса
диктовки, на главном потоке строится новое скрытое окно overlay-N, затем
старое уничтожается; при ошибке сборки старое окно остаётся.

Проверки, Linux-контейнер (не Windows), rustc 1.97.0, Node v22.22.0, pnpm 10.28.0:

- cargo fmt --check (cwd src-tauri) — exit 0 после cargo fmt.
- cargo clippy --all-targets -- -D warnings (src-tauri) — exit 0, Finished.
- cargo test (src-tauri) — exit 0, 343 passed; 0 failed; 6 ignored.
- Мутация: `>` → `>=` в resumed_from_sleep — cargo test --lib resume_detected
  exit 101, 1 failed; условие возвращено.
- pnpm exec tsc --noEmit — exit 0. pnpm build — exit 0, built.
- node scripts/runbook-check.mjs — exit 0, OK (10 series) после git fetch --unshallow --tags.

Не проверено: сам сценарий сна/пробуждения на Windows и NSIS-сборка — недоступны
в этой среде; это шаг R2 (manual на ноутбуке пользователя).
