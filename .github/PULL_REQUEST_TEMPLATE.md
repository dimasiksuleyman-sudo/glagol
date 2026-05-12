## Description / Описание

<!-- Briefly describe what this PR does and why. -->
<!-- Кратко опишите, что делает этот PR и зачем. -->



## Type of change / Тип изменения

<!-- Check ONE that best describes this PR / Отметьте ОДНО, что лучше всего описывает PR -->

- [ ] 🐛 Bug fix (non-breaking change which fixes an issue) / Исправление бага
- [ ] ✨ New feature (non-breaking change which adds functionality) / Новая функция
- [ ] 💥 Breaking change (fix or feature that would cause existing functionality to change) / Breaking change
- [ ] 📚 Documentation update / Обновление документации
- [ ] 🎨 Style (formatting, missing semicolons, etc; no logic change) / Стилистические правки
- [ ] ♻️ Refactor (no functional changes, no api changes) / Рефакторинг
- [ ] ⚡ Performance improvement / Улучшение производительности
- [ ] ✅ Test (adding missing tests, refactoring tests) / Тесты
- [ ] 🔧 Chore (build process, tooling, dependencies) / Tooling, зависимости

## Related issue / Связанный issue

<!-- Link the issue this PR closes. Use "Closes #123" or "Fixes #123" for auto-close. -->
<!-- Свяжите issue, который закрывает PR. "Closes #123" или "Fixes #123" автоматически закроет issue. -->

Closes #

## How has this been tested? / Как это было протестировано?

<!-- Describe the tests you ran. Include manual testing steps if relevant. -->
<!-- Опишите тесты, которые вы запустили. Включите ручное тестирование если применимо. -->

- [ ] `pnpm lint` passes / проходит
- [ ] `pnpm typecheck` passes / проходит
- [ ] `pnpm test` passes / проходит
- [ ] `cargo fmt --check` passes / проходит
- [ ] `cargo clippy -- -D warnings` passes / проходит
- [ ] `cargo test` passes / проходит
- [ ] Manual testing on Windows 10/11 / Ручное тестирование на Windows 10/11

### Manual testing steps / Шаги ручного тестирования

<!-- List concrete steps to verify the change works -->
<!-- Перечислите конкретные шаги для проверки изменения -->

1. 
2. 
3. 

## Screenshots / Скриншоты

<!-- For UI changes, include before/after screenshots. -->
<!-- Для UI-изменений приложите скриншоты до/после. -->

| Before / До | After / После |
|---|---|
|  |  |

## Security checklist / Чек-лист безопасности

<!-- These invariants are documented in CLAUDE.md and SECURITY.md -->
<!-- Эти инварианты задокументированы в CLAUDE.md и SECURITY.md -->

- [ ] No secrets in code, config, or env vars / Нет секретов в коде, конфиге или env
- [ ] No new network endpoints outside the allowlist (Sberbank + GitHub) / Нет новых сетевых эндпоинтов вне allowlist
- [ ] No telemetry, analytics, or data collection added / Не добавлены телеметрия, аналитика или сбор данных
- [ ] No new `unsafe` Rust blocks (or each has `// SAFETY:` comment) / Нет новых `unsafe` блоков (или каждый имеет `// SAFETY:` комментарий)
- [ ] No `dangerouslySetInnerHTML` or `eval` in React / Нет `dangerouslySetInnerHTML` или `eval` в React
- [ ] All new dependencies have OSI-approved licenses / Все новые зависимости имеют OSI-approved лицензии

## Breaking changes / Breaking changes

<!-- If "Breaking change" is checked above, describe migration path. -->
<!-- Если выше отмечен "Breaking change", опишите путь миграции. -->

N/A

## Additional context / Дополнительный контекст

<!-- Anything else reviewers should know? Performance implications, design alternatives considered, follow-up work needed? -->
<!-- Что ещё стоит знать ревьюерам? Влияние на производительность, рассмотренные альтернативы, нужная follow-up работа? -->



## Checklist / Финальный чек-лист

- [ ] My code follows the style guidelines of this project / Код соответствует style guide
- [ ] I have performed a self-review of my code / Я провёл(а) self-review своего кода
- [ ] I have commented my code, particularly in hard-to-understand areas / Я закомментировал(а) сложные места
- [ ] I have made corresponding changes to the documentation / Я обновил(а) документацию
- [ ] My changes generate no new warnings / Мои изменения не генерируют новых warnings
- [ ] I have added tests that prove my fix is effective or that my feature works / Я добавил(а) тесты
- [ ] New and existing unit tests pass locally / Новые и существующие тесты проходят локально
- [ ] I have updated CHANGELOG.md under "Unreleased" if applicable / Я обновил(а) CHANGELOG.md в секции "Unreleased" если применимо
