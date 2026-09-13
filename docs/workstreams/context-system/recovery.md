# Контроль восстановления — 2026-09-13

Проверка выполнена в текущей сессии повторным чтением файлов. Отдельный новый
чат или независимый агент не запускался; это проверка маршрута, не эксперимент
с полной потерей памяти. Приложение не запускалось.

## Маршрут и факты

Прочитаны AGENTS → STATUS → state/README/log текущей серии → README/state
верификации 0.4.0. Сверены `git status --short --branch`, `git log -8`,
`git diff --cached --stat`, ранее текущий diff и runtime-тесты.

| Вопрос | Восстановленный ответ | Основание |
|---|---|---|
| Текущая задача и NEXT | Во время проверки context-system/R6 in_progress. После её закрытия — verification-0.4.0/V1: проверить установленную версию и выбрать тестовую среду | STATUS, оба state.json |
| Решение об STT/TTS | Диктовка local/server/cloud независима от Silero; лицензия и согласие модели не блокируют STT | CLAUDE: current TTS contract; план Silero |
| Изменения после 0.2.1 | Готовность микрофона, GigaAM/серверные профили, иконка, runtime и Silero 0.4.0; семь технических/плановых commits после main | git log -8 и журнал verification-0.4.0 |
| Сборка/установка/публикация | Локальная 0.4.0 обнаружена. Текущая установленная версия unknown. Публичная 0.2.1 reported пользователем, онлайн-проверка не выполнена | project.json и датированная исходная запись log |
| Что проверено и что осталось | Исторически Rust 325 passed/3 ignored и отдельный native Silero smoke; ручная установка/UI/голоса/STT остаются открытыми. Сейчас проверены только система контекста и CI-конфигурация | журнал интеграции, context log, findings |
| Как продолжить после обрыва | Прочитать NEXT/claim/checkpoint, сверить Git status/diff/staged diff; продолжить только незавершённый шаг | AGENTS и docs/context/README |

Git: ветка codex/silero-tts, HEAD e05c852. В индексе изменений нет. В рабочем
дереве изменены только инструкции, карта проекта, scripts/package.json и CI;
добавлены документы контекста и гейт. Продуктовые src и Rust-модули не менялись.
Полный старый CLAUDE совпал с baseline после нормализации CRLF при отдельной
проверке архива. Исторические day-logs не редактировались.

## Проверки системы

Окружение: Windows x64, Node v24.19.0. Команды из корня:

```text
node --test scripts/runbook-check.test.mjs
tests 49; pass 49; fail 0; skipped 0; exit 0

node scripts/runbook-check.mjs
runbook-check OK (2 series); exit 0

node .scratch/context-ci-review.mjs
CI YAML: OK; 14 existing steps preserved, 2 context steps added;
full history and read-only permissions verified. CLAUDE archive matches baseline.
exit 0
```

Отрицательные CLI-fixtures проверяют exit 1; недоступная Git-история — exit 2.
Первый прогон: 47/47. После появления реальной серии с версией в ID гейт отверг
точки в имени. Формат исправлен; добавлены тест dotted ID и missing version,
повторный итог — 49/49. Ошибка и исправление не скрыты старым PASS.

`context-ci-review.mjs` — локальная разовая проверка через уже установленный
js-yaml 4.1.1; production-гейт не зависит от него. Python/PyYAML оказался
недоступен, поэтому YAML проверен существующей JS-библиотекой без установки.
Воспроизводимые постоянные команды — context:refresh, runbook:check, runbook:test.

Ограничения: GitHub CI не запускался; там сохранён Node 20, локальные тесты
выполнены на Node 24.19.0. Полная сборка приложения и ручная QA здесь NOT_RUN,
так как менялись процесс и инструменты документации. pnpm wrappers не запускались;
использованы прямые Node-команды из их определений, без переустановки зависимостей.
