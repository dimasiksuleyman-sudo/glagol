# Постоянный контекст

Вход: [AGENTS](../../AGENTS.md) → [STATUS](../STATUS.md) → выбранная серия.
Система работает из файлов репозитория; не требует истории чата, модели или сети.

## Источники

- `project.json`: выбранная серия, датированные наблюдения доставки и Git-ссылки.
- `../workstreams/<id>/state.json`: единственные статусы шагов и NEXT.
- README серии: цель, решения, границы. log.md: последовательные checkpoint и результаты.
- STATUS.md: производный обзор; `pnpm context:refresh` обновляет его,
  `pnpm runbook:check` проверяет без записи.
- `../day-logs/`: исторические результаты; не переносить их как новые PASS.

## Формат v1

JSON разбирается строго, неизвестные поля верхних уровней и записей — ошибка.
Шаблон: [state.json](../templates/workstream/state.json). Скопировать каталог
шаблона в workstreams/<id>, заменить id/title/шаги, затем выбрать его в project.json.
Все обычные каталоги workstreams проверяются, включая каталог с пропавшим state.

Проект: schema_version=1, observed_at (UTC ISO), active_workstream (id или null),
links (пути от корня), observations, git_refs. Наблюдение: id, label, status
(`observed`, `reported`, `unknown`), summary, observed_at, source (путь evidence).
Не превращать пользовательский отчёт о релизе в observed без проверки GitHub.

Серия: schema_version=1, id, title, status (`active`, `paused`, `done`, `cancelled`),
next (ID шага или null), reason (строка или null), claim (объект или null),
links, steps. Выбранная серия должна быть active. Другие серии — paused/done/cancelled.
Если active_workstream=null, активных серий нет. У paused есть причина и точка
продолжения; у done/cancelled NEXT=null и нет незавершённых шагов.

Шаг: id, title, status (`pending`, `in_progress`, `blocked`, `done`, `cancelled`),
depends_on (ID), required (ID проверок), checks. Заблокированный шаг имеет reason.
NEXT указывает на первый незавершённый шаг по порядку. Активный шаг только один,
он совпадает с NEXT. Зависимости должны быть выполнены; циклы запрещены.
Шаг done имеет хотя бы одну обязательную проверку и все обязательные PASS.

Claim: owner, at (UTC ISO), branch. Он необходим только для in_progress;
не сверяется с текущим именем checkout-ветки на CI. Возраст больше суток — заметка
для проверки исполнителем, не автоматическое снятие. Паузить можно после checkpoint,
вернув незавершённый шаг в pending/blocked и очистив claim.

Проверка: id, status (`PASS`, `FAIL`, `NOT_RUN`, `BLOCKED`), method
(`command`/`manual`), command (команда либо описание ручной процедуры), cwd
(путь от корня, `.` разрешён), at (UTC ISO), environment (непустая карта строк),
exit_code (integer для выполненной command, null для manual/NOT_RUN/BLOCKED),
output (решающая строка или причина), evidence (путь записи журнала).
FAIL command должен иметь ненулевой код, PASS command — ноль.
Если тест не запускался, не создавать для него искусственный успешный код.

Пути links/source/evidence — существующие файлы внутри репозитория, с `/`;
можно `#heading-slug` или явный HTML-якорь. Историческая проза не сканируется.
Внешние URL и локальные абсолютные пути пишутся в журнале, не в этих полях.

Git-ссылка: id, sha (7–40 hex), scope:

- local: commit существует и достижим из HEAD;
- main: достижим из origin/main, при его отсутствии из main с явной заметкой;
- release: требует ref=`refs/tags/<tag>` и достижимость из этого тега;
- historical_unresolved: требует reason и evidence; sha только описывает прошлое,
  не используется как актуальное доказательство. После squash добавить причину
  и новую проверенную ссылку, не исправлять незаметно старый журнал.

Git-ссылки проекта описывают baseline/историю, не автоматически доказывают PASS
шага. Доказательства проверки и состояние проверенного дерева записываются в log.
SHA будущего commit не требуется: до commit записывается baseline + изменённые
файлы/dirty state, после commit ссылка может быть добавлена отдельной записью.

## Команды и границы

```powershell
node scripts/context-status.mjs write
node scripts/context-status.mjs check
node scripts/runbook-check.mjs
node --test scripts/runbook-check.test.mjs
```

Соответствующие pnpm scripts: context:refresh, runbook:check, runbook:test.
Node >=20 и Git должны быть доступны в PATH. Новые зависимости не нужны.
Гейт read-only, без fetch. Код 0 — согласовано, 1 — ошибка, 2 — INCOMPLETE
(shallow history/нет нужного ref/недоступен Git). Любой ненулевой код блокирует CI.
CI получает полную историю. Локальный main может быть устаревшим: его проверка
подтверждает только локальное наблюдение, не свежесть удалённого сервера.

Валидатор проверяет структуру и согласованность. Он не доказывает правдивость
прозы, фактический запуск ручной проверки или свежесть установки/релиза.
Сохранённые наблюдения всегда имеют дату; старое observed не значит «проверено сейчас».
Отсутствие старого EXE на CI нормально; артефакт проверяется процедурой сборки отдельно.
Не сканировать все старые SHA/пути в журналах: среди них хеши моделей и удалённый код.
