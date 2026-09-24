# Glagol 0.5.0 — локальный кандидат и границы проверки

Дата: 2026-09-24 UTC. Windows 11 Home 26200 x64, Ryzen 7 7730U, 16 GB RAM.
Node 24.19.0 / Rust 1.98.0; embedded Python 3.11.9 / torch 2.7.1+cpu.
Git baseline указан структурированно в docs/context/project.json как local;
изменения незакоммичены. Основная установка и публичная 0.4.1 не обновлялись.

## Артефакт

- `src-tauri/target/release/bundle/nsis/Glagol_0.5.0_x64-setup.exe`
- Собран 2026-09-24T22:38:19.5659323Z, **9 947 426 байт**.
- SHA-256: `6d9d567975779eb9e157063ac34c7a132a5f88deb3c5107cbd22164eecdda762`.
- EXE FileVersion/ProductVersion: 0.5.0. NSIS English/Russian, selector включён,
  удаление читает сохранённый язык через MUI_UNGETLANGUAGE.
- Против опубликованной 0.4.1 (9 777 903 байта): +169 523 байта, **+1,73%**.
- Генерируемый NSIS-скрипт пакует EXE, иконку, MIT-текст и resources/Pdfium
  (плюс пустой .gitkeep). Speech DLL/PT/ORT/GGUF/Python/PyTorch и словарей нет;
  WebView bootstrapper/installer paths пусты. Это аудит сборки, не запуск установки.

## Выполненные проверки

| Область | Результат | Граница |
|---|---|---|
| Версии, TypeScript, production build | PASS | Прямые Node entrypoints по Windows runbook из-за ошибки глобального pnpm; JS chunk warning 516,25 kB |
| fmt, clippy all-targets -D warnings, cargo test | PASS | 343 passed, 6 ignored; ignored не объявлены PASS этим запуском |
| Context + i18n + artifact Node tests | PASS | 58/58: 49 context, 3 i18n, 6 artifact; отрицательные CLI действительно exit 1 |
| Словари/Intl/native ошибки | PASS | 232 UI-ключа; параметры EN/RU, числа/даты/множественные формы, сохранение пользовательских аргументов |
| Предпочтения и миграция | PASS | Новый профиль, старый implicit cloud/RU, прежние модели/серверы, однократный голос, все восемь комбинаций и повторное чтение |
| SQLite/legacy WAV/backup | PASS | Миграция 6 с NULL для старых строк; существующие тесты restore/export/метаданных/исключений |
| Загрузчик | PASS | Mock HTTP: отмена/partial, Range/restart, неверный Range/hash/размер; неполный Moonshine не активируется |
| Общие файлы | PASS | RU/EN Silero имеют общий root/lock, отдельные consent/receipt; удаление языка сохраняет соседнюю модель/runtime; native Moonshine commit без wheel/сохранение DLL |
| 30-дневная проверка | PASS | Unit: срок, fingerprint, invalidation после ошибки и reuse; это не ожидание реального календарного месяца |
| Streaming recorder/resampler | PASS | Блоки 1/997/4096/целый, 16/44,1/48/96/192 kHz, начало/хвост/длина, overflow loud failure, single finish, retry; остальные early-release/silence/cap/insert тесты |
| Moonshine через release EXE | PASS | 2 native tests: начало/конец синтетической фразы, warm repeat, silence, crash/restart/cancel; модели и DLL проверены |
| GigaAM RU | PASS | CTC/RNNT дважды на публичном example.wav, 1 native test; не микрофон |
| Silero RU/EN pipeline | PASS | 2 native tests: long WAV/library, cancel; EN language/provider metadata, crash/restart; выполнялись ранее в этой серии |
| Текущий Silero worker, 9 голосов | PASS | 5 RU + 4 EN, synthetic числа/даты/сокращения, mono PCM24k WAV; прослушивание отдельно |
| Протокол/parent exit/timeout | PASS | Реальные hidden workers reject oversized / exit with parent; зависшие owned processes timeout+kill с коротким test deadline; production 90 с |
| Совместные EN workers | PASS | Реально перекрывающиеся Moonshine release + Silero EN, сохранены начало/конец и корректный WAV; без webview/микрофона |
| NSIS | PASS | build exit 0, файл/версия/хеш/размер определены; не установка и не публикация |

## Замеры этой машины

Release Moonshine child ready **1019 мс**, finish после последнего блока **312/332 мс**.
Loaded worker peak **292 MiB** (отдельный process test). Silero EN loaded worker
peak **260 MiB** (не максимальная память длинного синтеза). В current-voices smoke:
RU ready 11 575 мс, EN ready 3430 мс; RU-фрагменты 544–2211 мс, EN 642–3027 мс.
В отдельном concurrent smoke распознавание готового файла 1727 мс, TTS 3830 мс,
overlap=true. Это не задержка живой речи, не суммарный peak приложения, не обещание
качества и не показатели из внешнего исследования. В разных smoke разные условия
кеша/нагрузки; минимальные числа не подменяют пользовательские измерения.

## Остаётся до полной готовности

| Проверка | Статус | Продолжение |
|---|---|---|
| UI/browser, восемь визуальных комбинаций, overlay/tray live switch, accessibility | BLOCKED | Browser bootstrap/list вернул No browser is available / []; выполнить на доступном изолированном UI |
| Windows 10/11: clean install/remove EN/RU, OS language fallback, upgrade synthetic library | NOT_RUN | Изолированная Windows QA; прежнюю пользовательскую установку не использовать |
| Первый запуск: закрытие, повторный запуск, skip, offline через настоящий UI | NOT_RUN | Настройки покрыты unit, пользовательские клики не проверены |
| Живой микрофон, начало/конец речи, хоткей/вставка/clipboard | NOT_RUN | После сигнала готовности на согласованной тестовой машине |
| Прослушивание всех голосов, длинного текста, чисел/дат/аббревиатур | NOT_RUN | Валидный WAV не равен правильному произношению |
| Отзывчивость UI при одновременных STT/TTS, общий peak памяти, cold/warm физического hotkey | NOT_RUN | Headless concurrent smoke не закрывает эту строку |
| Отмена/докачка настоящих EN endpoints в UI, физическая нехватка диска | NOT_RUN | Mock downloader покрыт; реальные сетевые/дисковые условия не симулировались |
| Silero EN прямой origin | BLOCKED | models.silero.ai timeout; закреплённый mirror/import прошёл, происхождение отмечено origin_verified=false |
| Полностью отключённая сеть Windows после установки, 15 минут idle для обоих новых workers | NOT_RUN | Inference без сетевых вызовов и код idle есть; новый wall-clock/manual прогон не выполнен |
| Доступ к реальному office/cloud серверу | NOT_RUN | Профили сохранены/моки пройдены; чужие сервисы не вызывались |

E5 остаётся открытым по ручной матрице. Следующий шаг — проверка этого конкретного
артефакта в изолированной Windows-среде по строкам выше. Публикация и обновление
основной установки в это задание не входят.
