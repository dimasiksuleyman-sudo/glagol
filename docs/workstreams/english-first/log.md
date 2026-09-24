# ?????? English first

## ?????? ? 2026-09-24T21:19:27.719Z

???????????? ??????? ?????????? ????? 0.5.0. Git baseline: main, HEAD 68d507f, ?? ?????? ?????? ??????; git status/log/diff/staged diff ???????. ????? E1. ??????? ?????? ???????? ?????; ?????????? ??? 0.4.1. ????????? ????????: ????????? ???????????? Moonshine v0.1.5 ? .scratch/english-first, ????????? DLL/ABI ? ?????? ? ????????. ?????????? PASS ? ?????? ?????? ?? ???????? ?????????? 0.5.0.

## Исправление кодировки и проверка runtime — 2026-09-24T21:25:26.456Z

Начальная запись и заголовки были повреждены при передаче кириллицы через PowerShell; README и state восстановлены из утверждённого плана, журнал сохранён append-only. Начало: main, HEAD 68d507f, чистое дерево до создания серии, текущий claim codex/E1. Продукт пока 0.4.1; изменения только в контексте и изолированных probe-файлах .scratch/english-first.

Официальный SDK v0.1.5 Windows содержит статическую moonshine.lib, а не DLL. Официальный wheel той же версии содержит moonshine.dll и onnxruntime.dll; извлечены только эти два проверенных файла. Python используется только как ctypes-инструмент проверки, не как будущая зависимость английского runtime. ABI 30000. Модели закреплены revision 0bf2f2e5aff22e6fbba4300b00a4e00bbc4f8aae официального mirror; скачанные размеры и SHA-256 сверены. STT 142300974 байта; TTS с четырьмя голосами 110554097 байт; wheel 16542073 байта.

Команда: & .scratch/silero/runtime-271/python.exe -I -B .scratch/english-first/native-smoke.py; Windows x64, Ryzen 7 7730U, Python 3.11 probe; exit 0, NATIVE SMOKE PASS. Четыре голоса, по два WAV 24 кГц, ненулевая амплитуда; две потоковые сессии STT на синтетической общедоступной фразе, ожидаемый фрагмент найден. Загрузка TTS 925–1166 мс, синтез 2362–3153 мс для 6.525–7.75 с аудио; STT загрузка 448 мс, финализация 1–2 мс при предварительной обработке блоков. Это smoke, не оценка WER или микрофона. Пиковая память пока не измерена. Живое прослушивание/микрофон NOT_RUN.

Первая probe-команда exit 1: неподдерживаемая опция num_threads вызывает C++ exception. Опция удалена; успешная проверка использует только voice/g2p_root и decode_incomplete_lines. Продолжение: аудит лицензий, воспроизводимый закреплённый manifest/probe, затем E2. E1 пока не завершён.

## E1 checkpoint: технический smoke PASS, лицензии BLOCKED — 2026-09-24T21:29:59.656Z

Созданы scripts/english-runtime/artifacts.json, fetch.mjs, verify.mjs, verify.test.mjs, native-smoke.py и docs/runbooks/english-runtime.md. Это воспроизводимые инструменты разработчика; приложение, пользовательские данные и сборочный pipeline не изменены. Все файлы моделей и wheel закреплены размерами/SHA-256; models revision 0bf2f2e5aff22e6fbba4300b00a4e00bbc4f8aae. Общая первичная загрузка 269397144 байта, установленные модели + две DLL 276636479 байт. Скачанные файлы и WAV остаются только в .scratch.

Среда: Windows 11 Home 10.0.26200 x64, AMD Ryzen 7 7730U, 16153120 KiB доступной физической памяти ОС; Node 24.19.0, trusted embedded Python 3.11 x64 для ctypes. Приложение/установщик не запускались.

Проверки выполнены из корня:

- node scripts/english-runtime/fetch.mjs .scratch/english-first — exit 0, PROBE DOWNLOADS VERIFIED (license gate is separate); повторная проверка уже скачанных точных файлов.
- & .scratch/silero/runtime-271/python.exe -I -B scripts/english-runtime/native-smoke.py .scratch/english-first — exit 0, NATIVE SMOKE PASS. Четыре голоса × два WAV, 24 кГц, амплитуда; STT две последовательные сессии с блоками 4800/997, проверены начало/конец общей синтетической фразы. Загрузка TTS 893–1053 мс; синтез 2187–3045 мс, аудио 6.525–7.75 с. STT загрузка 432 мс; обработка всех блоков 2100/1494 мс; финализация 2/493 мс. Peak working set всего последовательного Python probe 801955840 байт. Это не память двух workers и не пользовательская задержка после отпускания клавиши.
- node scripts/english-runtime/verify.mjs files .scratch/english-first — exit 0, ARTIFACT INTEGRITY PASS.
- node --test scripts/english-runtime/verify.test.mjs — exit 0, 6/6 PASS. Отрицательные CLI-проверки повреждённого runtime и отсутствующих лицензионных условий реально завершаются кодом 1.
- node scripts/english-runtime/verify.mjs licenses — exit 1, License terms unresolved: moonshine-g2p. Это FAIL гейта, не PASS на основании ожидаемого отказа.
- node scripts/check-version.mjs — exit 0, Glagol 0.4.1: package, Tauri and Cargo versions match. Версия ещё не поднята: функциональность 0.5.0 не реализована.

Блокер: LICENSE Moonshine v0.1.5 явно исключает TTS/G2P models/data из общего MIT. en_us README сообщает, что OOV обучен авторами, но лицензии весов не указывает. Проверены также исходные moonshine-tts (dba0b7cb5857ff4b0a08fad47f6fbb8630a3ca70) и moonshine-g2p-training (252e7dbbaad9789de80d7c9464bf9ef4a269bb8c): корневого LICENSE нет; общая фраза training README о permissive licenses не закрепляет условия собственных OOV-весов. Для Kokoro model card указывает Apache-2.0; исходный CMUdict — BSD-2-Clause. В manifest неопределённые G2P-условия оставлены null, не подменены MIT. Первичные ссылки и процедура продолжения — в ранбуке english-runtime.

E1 blocked, claim очищен; серия active, NEXT E1. E2–E5 pending по утверждённой зависимости. Для продолжения нужен проверяемый источник явных условий G2P/OOV от правообладателя; затем уточнить notices/manifest и повторить лицензионный гейт. Движок не заменён. Запросы автору не отправлялись. Живой микрофон, прослушивание, интеграция процессов, RU/EN UI, миграция и NSIS 0.5.0 NOT_RUN. Прежняя verification-0.4.0 остаётся paused. Продуктовые README/USER_GUIDE не изменены, поскольку поведения продукта пока не меняли.

## Проверки контекста после checkpoint — 2026-09-24

Из корня, Windows PowerShell, Node 24.19.0:

- `node --test scripts/runbook-check.test.mjs` — exit 0, 49 tests / 49 pass.
- `pnpm context:refresh` — exit 0, `context-status write: OK`.
- `pnpm runbook:check` — exit 0, `runbook-check OK (7 series)`.
- `git diff --check` — exit 0; только обычные предупреждения Git о LF/CRLF.

Git status: изменения контекста, новый ранбук и каталог probe-скриптов; файлы
приложения/версии не менялись. Rust/TypeScript/NSIS не запускались: E2–E5 pending,
изменений соответствующего кода нет. Новые файлы не staged; commit не создавался.

## Выбор Silero для EN — 2026-09-24T21:37:24.094Z

Пользователь явно выбрал английский Silero и переиспользование runtime русской версии, включая существующую 30-дневную проверку файлов. Это согласованная замена Kokoro/G2P Moonshine, а не автоматическая подмена. Английский STT Moonshine Small Streaming сохраняется. Прежний блокер G2P относится к исключённому кандидату. Возобновлён claim codex/E1 на main; Git baseline и собственные изменения сверены, чужих изменений не обнаружено.

Далее: проверить v3_en на закреплённом Python 3.11/PyTorch 2.7.1, закрепить модель и условия; общий runtime хранить единожды, RU/EN модели/отметки/согласия независимо. Не удалять runtime при удалении одной модели, если другая установлена. Пользовательский профиль не изменять.

## E1: Silero EN на общем runtime, переход к E2 — 2026-09-24T21:41:22.113Z

v3_en: 57194546 байт, SHA-256 02b71034d9f13bc4001195017bac9db1c6bb6115e03fea52983e8abcff13b665. Прямой models.silero.ai недоступен: Node connect timeout exit 1, curl --connect-timeout 25 timeout (wrapper exit 1), Invoke-WebRequest -TimeoutSec 40 exit 1. Загрузили кандидат из Alevxis/silero_v3_en revision 23e0774444b3caf601a13d24ee4fc9a75608a0dd; SHA совпадает с LFS-записью Derur/silero-models. Это сверка зеркал, не доказательство совпадения с недоступным origin. Перед запуском просмотрены Python-код пакета и pickle globals; сетевых/системных вызовов на пути загрузки и apply_tts не обнаружено. Происхождение явно записано в english-model.json; URL приложения остаётся официальным с обязательным закреплённым SHA.

Лицензия Silero на commit d9355348e2781dc8fa25a135d1602c530afae24c: CC-BY-NC-SA-4.0, SHA 1349a4b6148492b44f629e64eed676612e234fe9a839e4f3b277c1482c8849f1 — совпала с уже поставляемым текстом для RU.

Команда & .scratch/silero/runtime-271/python.exe -I -B scripts/silero/probe-english.py --model .scratch/english-first/v3_en.pt --output .scratch/english-first/silero-en-smoke — exit 0, SILERO EN SHARED RUNTIME PASS. Windows 11 26200 x64/Ryzen 7 7730U; torch 2.7.1+cpu без новых пакетов. Получены 12 валидных WAV 24 кГц (en_0/en_1/en_2/en_3/en_117 ×2, числа словами, абзац). В модели 118 обычных голосов + random; random не предлагаем как сохраняемый голос. Загрузка 10.346 с, синтез 0.446–2.509 с для 5.2–17.125 с аудио. Гендер/акценты по числовым ID не объявляем; прослушивание NOT_RUN.

node scripts/english-runtime/selected-check.mjs .scratch/english-first — exit 0, SELECTED EN ARTIFACTS PASS. Проверены точные DLL/STT-файлы и EN-модель, совпадение версии общего Silero runtime и license hash. Moonshine streaming ABI smoke этого же закреплённого комплекта уже пройден ранее (см. предыдущую запись). Kokoro/G2P больше не входят в выбранную поставку; старые скрипты и результаты остаются свидетельством отвергнутого кандидата.

E1 закрыт по технической совместимости и закреплению выбранного комплекта. Недоступность прямого Silero endpoint остаётся ограничением проверки доставки, а не PASS загрузки. E2 занят codex: локализация, SQLite-предпочтения, первый запуск. Для E3: общий runtime, отдельные модели/consent/verification stamps, докачка только недостающего, сохранение runtime при удалении одного языка. Installer и приложение ещё 0.4.1, изменены пока только инструменты проверки и контекст.

## E2 промежуточный checkpoint — 2026-09-24T21:47:50.866Z

Добавлены preferences.rs и IPC для языка/онбординга/однократного импорта старого голоса. Инициализация различает существующую БД и новую до её открытия, сохраняет старый неявный cloud и RU; последующая смена UI не меняет речь. Новые типизированные EN/RU словари, React context, подписка overlay, переключатель в оболочке, первый выбор English/Русский и экран настройки. Переведены строки TSX, форматирование дат/чисел через Intl; исходные пользовательские документы/тексты не менялись. Экран настройки пока использует прежние карточки: подключение языковых каталогов и компактный вид ещё впереди.

pnpm exec tsc --noEmit — exit 0 после исправления generic translator. cargo fmt выполнен. Первый cargo test preferences::tests — FAIL: недоступна загрузка Pdfium, compile-time PDFIUM_LIBRARY_PATH отсутствует. Существующий resources/pdfium.dll сверён (SHA A487E1D2A18F164ADC3A17AACEE158787FA86049E6D91D3712B0A43F745E6905; происхождение в предыдущих Windows QA). Повтор с процессным PDFIUM_LIBRARY_PATH: cargo test --manifest-path src-tauri/Cargo.toml preferences::tests -- --nocapture — exit 0, 3 passed. Это только тесты предпочтений, не полный Rust gate.

E2 in_progress/claim codex сохраняется. Не готово: IPC-языки речи и сохранение нового выбора голоса, подключение EN Silero/shared runtime, оставшиеся native-строки/трей/плеер, словарный гейт, полные тесты и документация. UI и runtime интеграция не запускались; NSIS 0.5.0 отсутствует. Старый localStorage пока сохраняется до перевода обоих UI-потребителей на SQLite, чтобы промежуточная миграция не теряла голос. Далее: подключить модельные каталоги Silero и общий runtime, затем закрыть локализацию/проверки E2–E3. Ветка всё ещё main с локальными незакоммиченными изменениями.

## E2/E3: общие компоненты Silero и потоковый путь STT — 2026-09-24T22:09:42.310Z

Silero RU/EN используют один runtime и общий mutex операций; отдельные модель, согласие и 30-дневная отметка проверки для каждого языка. При установке второго языка исправный runtime проверяется и пропускается, скачивается модель; удаление модели сохраняет общие файлы. Удаление, установка и выбор языка не выполняются поверх синтеза. Голоса переехали из localStorage в SQLite однократно. Миграция 6 добавляет nullable speech_language; старые документы остаются NULL. Настройки/библиотека/бэкапы проходят существующие тесты.

Windows 11 x64/Ryzen 7 7730U; процессный PDFIUM_LIBRARY_PATH на ранее проверенный resources/pdfium.dll. cargo test --manifest-path src-tauri/Cargo.toml native_silero_ -- --ignored --nocapture с GLAGOL_TTS_SMOKE_ROOT=.scratch/english-first/shared-silero — exit 0, 2 passed за 133.81 с. Реальные RU/EN дочерние worker, общий runtime Python 3.11.9/torch 2.7.1+cpu: RU pipeline/cancel; EN четыре голоса, длинный WAV в библиотеке, язык документа, crash/restart/cancel. Это синтетические тексты; прослушивание и живой микрофон NOT_RUN. В scratch/runtime находится junction на ранее проверенный runtime-271; не удалять его рекурсивно как обычную папку.

node --test scripts/i18n-check.test.mjs — exit 0, 3 passed (ключи/параметры, live Intl/plural, негативные CLI fixtures exit 1). Native messages.json переводит ошибки только на IPC/event границе, аргументы/пользовательские данные не переводятся. Трей обновляется при смене языка. cargo test --manifest-path src-tauri/Cargo.toml i18n::tests -- --nocapture — exit 0, 1 passed. cargo test --manifest-path src-tauri/Cargo.toml --lib — exit 0, 336 passed, 4 ignored; включает независимый streaming resampler: блоки 1/997/4096/целый, 16/44.1/48/96/192 кГц, начало/хвост/длина. Более поздние тесты recorder и native Moonshine ещё не запускались на момент записи.

Добавлена интеграция Moonshine ABI 30000: закреплённые DLL и Small, отдельный режим того же EXE до запуска Tauri, ограниченный JSON, parent-death watchdog, таймауты, bounded очередь, live ресемплер и финализация без промежуточного текста. Встроено в существующий pipeline с фильтром тишины и единственной вставкой; пакетный путь GigaAM/server сохранён. Каталог и независимые языковые переключатели подключены. cargo check был exit 0 до последних UI/profile изменений; pnpm exec tsc --noEmit exit 0 до добавления этих переключателей. Полная повторная проверка текущего дерева ещё требуется.

E2 остаётся in_progress: компактный onboarding, финальный аудит строк/ошибок/плеера и UI QA не закрыты; E3-интеграция выполняется вместе с языковыми IPC-каталогами, готовность E3 не заявляется. Далее: native Moonshine через собранный EXE, crash/queue/parent-exit tests, все восемь сочетаний, fmt/clippy/test/TS/build, документация/лицензии/версия/NSIS. 0.5.0 ещё не собрана; основная установка и опубликованная 0.4.1 не менялись. Прямая доставка Silero origin по-прежнему не подтверждена, ручная Windows 10/11 QA остаётся NOT_RUN.

## E2 завершение / E3 claim — 2026-09-24T22:26:45.964Z

Компактные карточки onboarding, независимые языки, родной локализованный плеер, live EN/RU, сведения каталогов и перенос голоса реализованы. Новые настройки проходят все восемь сочетаний/повторное чтение; Markdown использует язык озвучки только для служебных подписей, не переводит текст. pnpm build — exit 0, i18n PASS и Vite build PASS (warning: JS chunk 516.25 kB). cargo clippy --all-targets -- -D warnings — exit 0. cargo test — exit 0, 340 passed, 5 ignored; PDFIUM_LIBRARY_PATH задан только процессу на ранее проверенный resources/pdfium.dll. Перед успешным повтором исправлен E0308: MutexGuard в commands/file.rs. Пять ignored не считаются PASS. Browser skill bootstrap/list вернул No browser is available / []; визуальная QA NOT_RUN, перенесена в E5. E2 закрыт по реализованным настройкам и автоматическим гейтам, это не ручной PASS.

Native Moonshine worker через debug EXE: native_moonshine_streaming_process — exit 0, 1 passed за 11.85 с; cold 3811 мс, finish 316/314 мс; warm reuse, silence, crash recovery, cancel. Первый запуск до исправления завершался с ошибкой системного ORT 1.17.1 вместо API 23: теперь явно предварительно загружается проверенный native/onnxruntime.dll. Его OrtGetVersionString подтвердил 1.23.2. node scripts/english-runtime/process-test.mjs src-tauri/target/debug/glagol.exe .scratch/english-first/process-root — exit 0, oversized protocol rejected / parent termination PASS, peak loaded worker 294 MiB. Синтетический WAV Silero EN, не живой микрофон. Память одной модели, не суммарный peak приложения.

E3 занят codex. Добавлены pinned notices Moonshine/ORT, повторное использование уже проверенных DLL без скачивания wheel, защита commit неполного набора; последние изменения ещё ожидают повторных гейтов. Silero warmup теперь ждёт общий lock фоновой проверки, чтобы не сообщать ложную ошибку первой подготовки. Пользовательские руководства и README обновляются EN-first с прямым указанием unpublished 0.5.0 и старого публичного 0.4.1. Далее: E3 native повтор/negative checks, версия и installer/doc E4, E5 итоговая сборка и остаток ручной матрицы.

## E3 завершение / E4 claim — 2026-09-24T22:33:37.739Z

Windows 11 26200 x64, Ryzen 7 7730U, Node 24.19.0, процессный PDFIUM_LIBRARY_PATH. cargo fmt --check / clippy --all-targets -- -D warnings — exit 0; cargo test — exit 0, 343 passed, 5 ignored. Включены timeout+kill тесты зависших owned процессов с test deadline 50 мс (production 90 с), отрицательный commit повреждённого комплекта. После этого добавлен ignored runtime reuse test: cargo test native_moonshine_reuses -- --ignored --nocapture — exit 0, 1 passed за 7.90 с: полный pinned набор без wheel, commit offline, удаление модели оставляет DLL. Итоговый полный набор после него имеет ещё один ignored тест.

Повтор native_moonshine_streaming_process — exit 0, 1 passed за 13.12 с; cold ready 4355 мс, final 381/329 мс, теперь проверяются и начало hello, и конец listen на синтетическом WAV. Это debug EXE, не микрофон. node scripts/silero/process-test.mjs .scratch/english-first/shared-silero — exit 0, oversized protocol / parent termination PASS, peak loaded EN worker 260 MiB. node scripts/english-runtime/selected-check.mjs .scratch/english-first — exit 0, включая закреплённые notices. Реальные два RU/EN pipeline smoke ранее записаны выше, заново в этой записи не заявляются.

E3 интеграция завершена по автоматическим гейтам. Микрофон, прослушивание, визуальная матрица, Windows 10/11 installation и прямой Silero endpoint не объявлены PASS, остаются в E5. E4 занят codex: EN-first документы, архитектура, bilingual NSIS, синхронизация 0.5.0. README и обе USER_GUIDE уже описывают изменения и различают unpublished candidate/публичный 0.4.1; уточнение старых runtime-документов продолжается. Прежняя QA-серия остаётся paused, основная установка не запускалась/не обновлялась.

## E4 завершение / E5 claim — 2026-09-24T22:36:00.673Z

Версия 0.5.0 синхронизирована в package.json, Cargo.toml/lock и tauri.conf.json. ProductName/identifier/data path прежние; title Glagol, EN описания, NSIS English/Russian selector и отдельные bilingual notice/header с условиями Silero для обоих языков. README EN-first, оба руководства и индекс, CHANGELOG Unreleased, architecture/security/runtime/runbooks обновлены; опубликованный installer в README явно остаётся 0.4.1, история неизменна. Resources содержит только Pdfium и .gitkeep; речевых компонентов нет.

node --test scripts/runbook-check.test.mjs scripts/i18n-check.test.mjs scripts/english-runtime/verify.test.mjs — exit 0, 58/58 (49 context, 3 i18n, 6 artifact negative gates). node scripts/check-version.mjs — exit 0, 0.5.0 versions match; context refresh/check — exit 0. node node_modules/typescript/bin/tsc --noEmit и vite build — exit 0; 232 i18n keys; JS chunk warning 516.25 kB оставлен без ослабления порога.

Штатный pnpm context:refresh после смены manifest снова вызвал известный ERR_PNPM_PACKAGE_MANAGER_REMOVE_MODULES_DIR access denied; последующий pnpm автоматически перелинковал 468 пакетов из кеша, затем runbook-check FAIL из-за stale STATUS. Это не зелёный запуск pnpm. Lockfile не изменён. Прямые Node entrypoints из Windows runbook успешно обновили STATUS и прошли гейт. Для NSIS используется локальный beforeBuild override с теми же version/i18n/TS/Vite проверками, без изменения global settings.

E5 занят codex: финальные Rust gates, RU native regression, NSIS build/hash/размер и QA matrix. Последний небольшой fix локализует новые исторические ошибки на момент записи (старые строки не меняет), оба Silero cancel выставляются на Exit. Полная ручная Windows 10/11 установка, live UI/микрофон/прослушивание пока NOT_RUN.

## E5: проверенный локальный NSIS, ручная QA не закрыта — 2026-09-24T22:41:57.739Z

Windows 11 Home 26200 x64 / Ryzen 7 7730U / 16 GB; Node 24.19.0, Rust 1.98.0. Финальные cargo fmt --check, cargo clippy --all-targets -- -D warnings, cargo test --manifest-path src-tauri/Cargo.toml — exit 0: 343 passed, 6 ignored, doc tests 0. PDFIUM_LIBRARY_PATH только у процесса, проверенный Pdfium из resources. node .scratch/english-first/diff-check.mjs — exit 0 (реальный git diff --check exit 0); первая PowerShell-комбинация с 2>$null дала wrapper exit 1 на stderr-предупреждениях CRLF, дефекты whitespace не найдены. Python test_worker.py — exit 0, 5 tests OK.

node node_modules/@tauri-apps/cli/tauri.js build --config .scratch/english-first/build-context.json — exit 0, beforeBuild version/i18n/TS/Vite PASS, release compile 1m51s, Finished 1 bundle. NSIS 9947426 байт, SHA-256 6d9d567975779eb9e157063ac34c7a132a5f88deb3c5107cbd22164eecdda762, LastWriteTimeUtc 2026-09-24T22:38:19.5659323Z. FileVersion/ProductVersion EXE 0.5.0. Рост относительно опубликованной 0.4.1 — 169523 байта / 1.7337%. Generated installer.nsi содержит English/Russian, language selector, MUI_UNGETLANGUAGE; resources только Pdfium/.gitkeep, нет speech weights/runtime/dictionaries. WebView installer paths пусты. Иконка и license — обычные упаковочные файлы. Это static packaging audit, установщик не запускался. Создан соседний .exe.sha256.

Отдельно: cargo test native_gigaam_smoke -- --ignored --nocapture с GLAGOL_LOCAL_SMOKE_ROOT=.scratch/local-stt — exit 0, 1 passed / 30.49 с, CTC/RNNT по два распознавания публичного WAV. cargo test native_moonshine_ -- --ignored --nocapture с GLAGOL_MOONSHINE_TEST_EXE=target/release/glagol.exe — exit 0, 2 passed / 9.93 с; cold 1019 мс, final 312/332 мс. node scripts/english-runtime/process-test.mjs release EXE process-root — exit 0, protocol/parent PASS, loaded peak 292 MiB. Worker запускался только внутренним режимом, без Tauri/main profile.

python scripts/silero/voice-smoke.py --root .scratch/english-first/shared-silero --output .scratch/english-first/current-voices-050 — exit 0: текущий worker создал валидные WAV для всех 5 RU и 4 EN голосов с числами/датами/сокращениями. python scripts/english-runtime/concurrent-smoke.py --exe target/release/glagol.exe --stt-root process-root --tts-root shared-silero --wav synthetic EN --output concurrent-050 — exit 0: overlap=true, STT fixture 1727 мс / TTS 3830 мс, начало/конец сохранены; это два процесса без webview и живого микрофона. Полные реальные пути/команды воспроизводятся по english-runtime runbook. Никаких пользовательских текстов/аудио/ключей в evidence.

node scripts/check-version.mjs и selected-check.mjs — exit 0, версии/выбранные файлы/notices совпали. Фактическая матрица, артефакт и ограничения — qa.md. E5 blocked только по недоступной ручной QA; обязательный manual-qa BLOCKED с exit_code=null, не PASS. Claim очищен; серия остаётся active/NEXT E5 для продолжения, прежняя verification-0.4.0 остаётся paused. Исходники/установщик готовы к изолированной проверке, полная готовность релиза не объявляется. Прямой Silero origin недоступен; импорт закреплённого файла проверен, он не закрывает реальную доставку. Локальный scratch UI-server остановлен; primary installation, commit/push/PR и публикация не выполнялись.

## Пользователь разрешил проверку на основной машине — 2026-09-24T22:46:41.524Z

Пользователь отверг VM/отдельный профиль и попросил установить/тестировать на машине использования, оставив ему только UI/микрофон/прослушивание. Это расширяет прежнюю границу задания: обновление основной установки теперь разрешено. Публикация не разрешалась. E5 возобновлён, claim codex/main. Git baseline прежний, все локальные изменения сохранены; staged diff пуст. Реестр показывает Glagol 0.4.1 в LOCALAPPDATA/Glagol; процессов glagol при проверке нет. Перед обновлением создаётся и проверяется локальная резервная копия.

Computer-use skill и guidance/confirmations прочитаны, @oai/sky импортирован. list_apps вернул Computer Use native pipe is unavailable (os error 2). Это недоступность UI helper, не отказ в разрешении. Обновление и техническая проверка выполняются штатным installer CLI; GUI оценивает пользователь. Старые NOT_RUN не превращаются в PASS, остаются историческим результатом до новых наблюдений.

## Основная установка обновлена, передача первого ручного шага — 2026-09-24T22:48:17.479Z

.backup-live.py через проверенный embedded Python — exit 0, 4 documents/4 WAV, SQLite integrity/ZIP CRC/payload SHA PASS; локальный бэкап вне репозитория, путь в live-qa.md. Проверенный installer SHA прежний; Start-Process NSIS /S /UPDATE -WindowStyle Hidden -Wait — exit 0. Установленный EXE 0.5.0, SHA 08775080c7f544ce71d5e5349167e6cc5bf7b1a7d172fb300b105f05355ec0d9. Byte compare: только offsets 22341330..22341332, bundle marker UNK→NSS; не объявляем SHA установленного и build EXE одинаковыми.

Запущен установленный EXE для пользовательской проверки. verify-live.py — exit 0: schema 6, 4 документа, прежние значения настроек и метаданных сохранены, WAV hashes unchanged, UI=en/onboarding=language. Никакие модели не переустанавливались. E5 in_progress; новая обязательная manual-qa соответствует пользовательскому сужению scope и пока NOT_RUN. Следующий шаг — большие кнопки English/Русский, Continue без изменения прежних RU параметров, затем пользовательская диктовка и слуховая проверка.

## Завершение согласованной ручной QA — 2026-09-24T23:17:37.835Z

Пользователь выбрал тестирование на основной машине и только сценарии, требующие его участия. Скриншоты подтверждают первый выбор English / Русский и английский экран настройки с сохранёнными GigaAM CTC / Русский и Silero RU / Xenia, обе загрузки 0 MB.

Результаты manual, источник — ответы пользователя в текущей сессии:

- PASS: русская диктовка с живым микрофоном.
- PASS: русская озвучка Xenia, прослушивание без замечаний.
- PASS: английская озвучка после инструкции выбрать Silero EN / EN 0, прослушивание без замечаний.
- PASS: английская диктовка Moonshine — присланный результат соответствует контрольной фразе. Отдельного измерения задержки или инструментального подсчёта вставок не было.
- PASS: переключение интерфейса EN → RU → EN сохраняет введённый текст и оба языка речи English, подтверждено пользователем.

Тексты речи и документов в evidence не сохраняются. Способ установки EN, объём сетевого трафика и успешность официального origin этим отчётом не подтверждаются. Полная изолированная Windows 10/11 install/uninstall матрица и остальные NOT_RUN из qa.md не объявляются PASS; пользователь отложил расширенную QA.

E5 и серия завершены в согласованном объёме: локальная сборка, обновление основной установки, короткая пользовательская RU/EN проверка. Следующий шаг — обычное использование и сбор обратной связи. Активной серии нет; verification-0.4.0 остаётся paused. Публикация/commit/push не выполнялись. Программный код и installer не изменялись; автоматические результаты предыдущих checkpoint повторно не выдаются за новые проверки.

Final context validation: node scripts/context-status.mjs write — exit 0; node scripts/runbook-check.mjs — exit 0 (7 series); node --test scripts/runbook-check.test.mjs — exit 0, 49 passed, 0 failed, including negative gate cases. node .scratch/english-first/diff-check.mjs — git diff --check exit 0. Windows 11, Node 24.19.0. Documentation-only checkpoint; application not rebuilt.
