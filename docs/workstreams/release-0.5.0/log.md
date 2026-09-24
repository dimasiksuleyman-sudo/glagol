# Журнал выпуска 0.5.0

## Начало — 2026-09-24T23:26:24.923Z

Явное разрешение пользователя на публикацию. main, baseline 68d507fe04273743932a17a1c5da87a28fb4ef4f; staged diff пуст, изменения english-first сохранены. git fetch origin exit 0. SHA локального NSIS повторно совпал: 6d9d567975779eb9e157063ac34c7a132a5f88deb3c5107cbd22164eecdda762. Исторические автоматические проверки и текущая manual QA находятся в english-first/qa.md и live-qa.md; не запускались заново. Computer Use list_apps: native pipe unavailable, os error 2. gh отсутствует в PATH; используется прежний GitHub REST helper с credential helper, без вывода секрета.

## Подготовка материалов

Обновлены README, оба руководства, CHANGELOG и двуязычные release notes. Повторно: context refresh/check exit 0 (8 series), version gate exit 0, i18n 232 keys и 9 i18n/artifact tests PASS, git diff --check exit 0. Установщик прежний проверенный, без новых изменений приложения. Перечень untracked содержит только исходники/документацию/тесты; модели и runtime не добавляются. Следующий шаг: commit, atomic push main/tag, CI, draft/assets/public verify.

## Git / CI

Commit 5f0ac620fe3c62891b06f2c5fe70bff42fdd9b0a, tag v0.5.0. git push --atomic origin main refs/tags/v0.5.0 — exit 0. CI запущен: https://github.com/dimasiksuleyman-sudo/glagol/actions/runs/36072774471 . Публичный Release пока не создан.

## Публичная сверка — 2026-09-24T23:46:49.625Z

GitHub Release v0.5.0 опубликован как latest; CI success. Публичные installer и SHA256SUMS скачаны и побайтово совпали с локальными. Тег, README, обе USER_GUIDE, CHANGELOG и release notes сверены. Установщик 9947426 байт, SHA-256 6d9d567975779eb9e157063ac34c7a132a5f88deb3c5107cbd22164eecdda762.

https://github.com/dimasiksuleyman-sudo/glagol/releases/tag/v0.5.0

node .scratch/github-release-v050.mjs stage / publish / verify — exit 0. CI https://github.com/dimasiksuleyman-sudo/glagol/actions/runs/36072774471 — success. [{"name":"Glagol_0.5.0_x64-setup.exe","size":9947426,"sha256":"6d9d567975779eb9e157063ac34c7a132a5f88deb3c5107cbd22164eecdda762"},{"name":"SHA256SUMS.txt","size":93,"sha256":"b3ee793e0959e09ef892da09b99e6b71312a2bc53f3abfb3654282c4122098e5"}]

Закрытие серии: публикация разрешена и проверена. Приложение/installer после ручной QA не менялись. Новые скриншоты недоступны, старые помечены историческими. Расширенная QA остаётся отложенной по решению пользователя, прежняя verification-0.4.0 остаётся paused. Следующий шаг — обратная связь; активной серии нет. Итоговые статусы отправляются отдельным docs-only commit [skip ci].

Итоговые проверки контекста: context-status write, runbook-check (8 series), check-version 0.5.0 и git diff --check — exit 0. GitHub description/topics обновлены на EN/RU и Moonshine, повторное чтение подтвердило значения. Обновлены только внутренние статусы после тега; публичные пользовательские документы совпали с тегом при verify.
