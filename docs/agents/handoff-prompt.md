# Как поручить задачу агенту / Handing a task to an agent

Глагол — MIT. Этот файл и [AGENTS.md](../../AGENTS.md) — готовая система работы
для любого ИИ-агента (Claude Code, Codex и др.) и для форков: агент получает
короткий промпт, сам находит состояние проекта и не тратит контекст на изучение
всего кода.

Glagol is MIT-licensed. This file and [AGENTS.md](../../AGENTS.md) are a ready
workflow for any AI coding agent and for forks: the agent gets a short prompt,
finds the project state itself and does not spend its context reading all the code.

## Что уже есть в репозитории / What the repository provides

| Файл | Зачем |
|---|---|
| [AGENTS.md](../../AGENTS.md) | Рабочий процесс: вход, ветки, учёт, проверки, доставка |
| [CLAUDE.md](../../CLAUDE.md) | Архитектурный контракт: стек, инварианты безопасности и данных |
| [docs/STATUS.md](../STATUS.md) | Сводка серий (генерируется из state.json) |
| [docs/workstreams/](../workstreams/) | Серии задач: README (план), state.json (шаги, claim, проверки), log.md (журнал) |
| [docs/roadmap.md](../roadmap.md) | Очередь серий и отложенные идеи |
| [PROJECT_STRUCTURE.md](../../PROJECT_STRUCTURE.md) | Карта кода: где что лежит |
| [docs/runbooks/](../runbooks/README.md) | Процедуры: сборка, проверки, выпуск; [шпаргалка команд](../runbooks/commands.md) |
| [docs/findings/](../findings/README.md) | Реестр найденных проблем |
| [docs/templates/workstream/](../templates/workstream/) | Шаблон новой серии |

## Перед тем как отдать задачу / Before handing over

1. Всё, на что опирается задача, должно быть в `main`. Если нужная работа ещё в
   чужой ветке — сначала слить её в `main` (PR + зелёный CI).
2. Шаг задачи есть в `state.json` серии, его статус `pending`, claim пуст.
3. Заполнить поля промпта ниже: `<SERIES>`, `<STEP>`, `<AGENT>`, `<TASK>`.

## Промпт (русский)

```text
Репозиторий: <URL репозитория>. Ты работаешь над шагом <STEP> серии <SERIES>.

1. Начни от свежего origin/main. Создай свою ветку <AGENT>/<SERIES>-<STEP>
   (например codex/tts-normalization-n3). Пушь только в неё; в чужие ветки
   не пушь никогда.
2. Прочитай AGENTS.md (процесс), CLAUDE.md (архитектурные правила),
   docs/STATUS.md и docs/workstreams/<SERIES>/ (README, state.json, log.md).
   Код читай по карте PROJECT_STRUCTURE.md только там, где он нужен для шага.
3. Займи шаг: в state.json статус in_progress и claim (кто ты, UTC-время, ветка);
   в log.md запиши, от какого коммита main начал.
4. Задача: <TASK>
5. Перед коммитом прогони проверки из docs/runbooks/commands.md («Проверки
   перед коммитом»). Изменения, которые заметит пользователь, опиши в
   USER_GUIDE.ru.md и USER_GUIDE.en.md, README (обе части) и CHANGELOG.
6. Заверши шаг по AGENTS.md: проверки в state.json, запись в log.md (что
   сделано по пунктам плана, что не проверено), очисти claim,
   pnpm context:refresh, pnpm runbook:check.
7. Открой PR из своей ветки в main. Не сливай сам, если тебя об этом не просили.
Отвечай пользователю по-русски, просто и коротко.
```

## Prompt (English)

```text
Repository: <repository URL>. You are working on step <STEP> of series <SERIES>.

1. Start from a fresh origin/main. Create your own branch <AGENT>/<SERIES>-<STEP>
   (e.g. codex/tts-normalization-n3). Push only to it; never push to another
   agent's branch.
2. Read AGENTS.md (workflow), CLAUDE.md (architecture rules), docs/STATUS.md and
   docs/workstreams/<SERIES>/ (README, state.json, log.md). Read code only where
   the step needs it, using the map in PROJECT_STRUCTURE.md.
3. Claim the step: in state.json set in_progress and claim (who you are, UTC time,
   branch); in log.md record the main commit you started from.
4. Task: <TASK>
5. Before committing, run the checks from docs/runbooks/commands.md. Describe
   user-visible changes in USER_GUIDE.en.md and USER_GUIDE.ru.md, both halves of
   README and CHANGELOG.
6. Finish the step per AGENTS.md: checks in state.json, a log.md entry (what was
   done per plan point, what was not verified), clear the claim, run
   pnpm context:refresh and pnpm runbook:check.
7. Open a PR from your branch into main. Do not merge it unless asked.
```

## Для форков / For forks

Процесс работает без изменений. Замените в промпте адрес репозитория, ведите
свои серии в `docs/workstreams/` (шаблон — `docs/templates/workstream/`) и
очередь в `docs/roadmap.md`. Ключи подписи обновлений и выпуски — свои
(см. [ранбук выпуска](../runbooks/release.md)); ключ автора Глагола не нужен и
недоступен.

The workflow works unchanged. Replace the repository URL in the prompt, keep your
own series in `docs/workstreams/` (template in `docs/templates/workstream/`) and
your queue in `docs/roadmap.md`. Update-signing keys and releases are your own
(see the [release runbook](../runbooks/release.md)).
