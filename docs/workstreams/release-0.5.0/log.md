# Журнал выпуска 0.5.0

## Начало — 2026-09-24T23:26:24.923Z

Явное разрешение пользователя на публикацию. main, baseline 68d507fe04273743932a17a1c5da87a28fb4ef4f; staged diff пуст, изменения english-first сохранены. git fetch origin exit 0. SHA локального NSIS повторно совпал: 6d9d567975779eb9e157063ac34c7a132a5f88deb3c5107cbd22164eecdda762. Исторические автоматические проверки и текущая manual QA находятся в english-first/qa.md и live-qa.md; не запускались заново. Computer Use list_apps: native pipe unavailable, os error 2. gh отсутствует в PATH; используется прежний GitHub REST helper с credential helper, без вывода секрета.

## Подготовка материалов

Обновлены README, оба руководства, CHANGELOG и двуязычные release notes. Повторно: context refresh/check exit 0 (8 series), version gate exit 0, i18n 232 keys и 9 i18n/artifact tests PASS, git diff --check exit 0. Установщик прежний проверенный, без новых изменений приложения. Перечень untracked содержит только исходники/документацию/тесты; модели и runtime не добавляются. Следующий шаг: commit, atomic push main/tag, CI, draft/assets/public verify.
