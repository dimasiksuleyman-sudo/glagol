# Журнал: скорость диктовки и отключение озвучки

## 2026-11-09T07:31:28.000Z — старт серии и разбор кода

Ветка claude/optimistic-franklin-cvvbsr от 8c59250 (main, 0.6.1). Серия
self-update поставлена на паузу (см. её журнал).

Reported (пользователь): 0.6.1 установлена и работает; на ПК локальное
распознавание очень долгое, на ноутбуке быстрое. Модель, язык диктовки и
железо обеих машин не уточнены.

Наблюдения по коду (не причина, а места измерения):
- RU (GigaAM, transcribe.cpp 0.2.3) работает в процессе, сессия создаётся
  с NULL session params: потоки = min(8, CPU в маске процесса), источник
  transcribe.cpp src/transcribe-batch-util.cpp `default_n_threads`.
- `transcribe_init_backends_default` загружает все модули рядом с DLL:
  ggml-vulkan.dll и девять вариантов ggml-cpu (x64, sse42, sandybridge,
  haswell, skylakex, cannonlake, cascadelake, icelake, alderlake); ggml
  выбирает CPU-вариант по возможностям процессора. `transcribe_log_set(NULL)`
  глушит все логи движка, поэтому ни выбранный вариант, ни устройства, ни
  разбивка времени в журнал сейчас не попадают.
- Первая загрузка модели после запуска проверяет SHA-256 модели (≈272 МБ) и
  распаковывает архив движка для сверки файлов; время этих шагов не пишется.
- Стартовый экран — «Озвучка»: при открытии окна он прогревает worker Silero
  (Python/PyTorch, 2 потока); при запуске программы фоново проверяются файлы
  Silero. Это может пересекаться с диктовкой по CPU и диску.
- EN (Moonshine) работает в дочернем процессе; время запуска worker и
  финального ответа не пишется.

Архив движка скачан для сверки (SHA-256 dac5b603…a879e49 совпал с каталогом);
экспорты transcribe_device_*, transcribe_get_timings, transcribe_model_backend,
ggml_backend_reg_* проверены `objdump -p`.

## 2026-11-09T07:45:39.000Z — S1 диагностика и переключатель озвучки, автоматические гейты

Сделано (база 8c59250, рабочее дерево до commit):
- `diagnostics.rs`: строка «system diagnostics» при запуске — версия, CPU
  (brand по CPUID), логические CPU, расширения (sse4.2/avx/avx2/fma/f16c/bmi2/
  avx512*), потоки STT по умолчанию min(8, CPU). Настройка
  `diagnostics_detailed` (по умолчанию выкл.).
- `logging.rs`: фильтр за reload-слоем; «Подробный журнал» меняет его на
  `warn,glagol_lib=debug` без перезапуска; явный RUST_LOG не перезаписывается.
- GigaAM: «local STT runtime initialized» (init_ms), «local STT compute devices»
  (устройства через transcribe_device_*, признаки CPU-варианта через
  ggml_backend_get_features), «local STT session opened» (backend), «local STT
  model loaded» (+verify_model_ms, verify_runtime_ms, total_ms), «local STT
  completed» (+rtf, prepare_ms, lock_wait_ms, segments, mel/encode/decode_ms,
  threads). Колбэк логов движка остаётся NULL.
- Moonshine: «local EN STT worker started» (холодный старт), «local EN STT
  completed» (finish_ms).
- Конвейер: «dictation context» (окно видно/в фокусе/свёрнуто, worker Silero
  загружен/операция TTS занята) и «dictation latency» (stop_ms, stt_ms,
  insert_ms, release_to_text_ms).
- Настройка `tts_enabled` (по умолчанию вкл.): выключение закрывает worker
  RU/EN, пропускает стартовую проверку Silero, `Silero::prepare` отказывает;
  экран «Озвучить» скрыт, стартовый — «Диктовка». Модели/согласие сохраняются.
- UI: переключатель в TtsSection, новая секция «Диагностика» (переключатель,
  «Открыть папку журналов» через tauri-plugin-opener из Rust, путь папки).
- Документация: обе USER_GUIDE, обе части README, CHANGELOG [Unreleased],
  PROJECT_STRUCTURE, local-dictation-runtime, CLAUDE.md (инвариант прогрева).

Проверки (Linux container, rustc 1.97.0, node v22.23.4, pnpm 10.28.0):
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  в src-tauri — exit 0; «test result: ok. 350 passed; 0 failed; 6 ignored».
  PDFIUM_LIBRARY_PATH указывал на libpdfium.so из предыдущей успешной загрузки
  build.rs в target/ (повторная загрузка давала 504, ср. GL-004).
- `pnpm build` — exit 0, «I18N PASS: 264 keys», «✓ built»;
  `node --test scripts/release-assets.test.mjs` — pass 3.
- Windows-only код (загрузка ggml.dll с флагами поиска): отдельный мини-крейт
  с тем же фрагментом и libloading 0.8.9 — `cargo clippy --target
  x86_64-pc-windows-msvc -- -D warnings` exit 0. Полный крейт под Windows здесь
  не проверялся (ring требует MSVC C-компилятор).
- Manual (headless Chromium, заглушка IPC): настройки RU с озвучкой вкл./выкл.;
  при выкл. меню «Библиотека | Диктовка | Настройки», «/» → /dictation.

Не проверено: работа на Windows (реальные строки журнала, открытие папки,
выгрузка worker). Это S2 вместе с журналами на ПК и ноутбуке.

## 2026-11-09T08:16:33.000Z — S1b диагностика индикатора (GL-006)

Reported (пользователь): на ноутбуке процессор с AVX2, на ПК без AVX2; проверить
на ПК до выпуска; «постоянно пропадает пилюля», причину выяснить по журналам.
Отсутствие AVX2 согласуется с гипотезой о медленном CPU-варианте ggml, но это
не доказано: «local STT compute devices» и «system diagnostics» с ПК покажут
выбранный вариант.

Код: окно индикатора прозрачное; если его WebView перестаёт рисовать или
перезагружается, индикатор невидим, а диктовка работает (совпадает с GL-006).
Добавлено:
- Rust: «overlay window built», «overlay page load» (Started/Finished — повтор
  означает перезагрузку страницы), «overlay window destroyed», «overlay scale
  changed», «overlay took focus»; «overlay positioned» (монитор, их число,
  геометрия, масштаб, x/y); «overlay shown» (поколение окна, show_ok, visible,
  minimized, x/y, размер); warn при отсутствии окна или ошибке show.
- Страница индикатора → команда report_overlay_event: mounted, state_received,
  frame_painted (задержка), frame_missing (нет кадра за 2 с — warn), hidden,
  script_error (без текста ошибки). Принимаются только перечисление событий,
  имя состояния из белого списка и задержка ≤ 600 000 мс; тест
  overlay_report_accepts_only_known_states_and_bounded_delays.
- Руководства RU/EN и CHANGELOG дополнены описанием этих записей.

Проверки (Linux container, rustc 1.97.0, node v22.23.4, pnpm 10.28.0;
PDFIUM_LIBRARY_PATH как в S1): cargo fmt --check, clippy -D warnings,
cargo test — exit 0, «351 passed; 0 failed; 6 ignored»; pnpm build — exit 0,
«I18N PASS: 264 keys», «✓ built». На Windows не запускалось.

Далее S2: установка сборки этой ветки на ПК (без выпуска), подробный журнал,
одинаковая фраза на ПК и ноутбуке, отметить время пропажи индикатора.

## 2026-11-09T10:44:48.000Z — S2 сборка на ПК, сведения о железе

Reported (скриншоты пользователя): на ПК установлена сборка этой ветки — в
настройках есть раздел «Диагностика», «Подробный журнал» включён, путь журналов
…\AppData\Local\app.glagol.desktop\logs (совпадает с руководствами). Способ
сборки/установки не уточнён. ПК: AMD FX-8300 Eight-Core 3,3 ГГц, 16 ГБ DDR3,
Radeon RX 550/550 Series 2 ГБ. Модель диктовки: GigaAM v3 · RNNT. Ноутбук —
с AVX2 (модель CPU не сообщена).

Ожидания до журнала (не выводы): FX-8300 (Piledriver) поддерживает AVX, FMA3,
F16C, SSE4.2, но не AVX2/BMI2, поэтому ggml должен выбрать вариант без AVX2
(ожидаемо sandybridge); 8 ядер = 4 модуля с общим FPU на модуль, при 8 потоках
по умолчанию матричные операции могут конкурировать за FPU. RX 550 — GCN,
Vulkan-устройство должно появиться в «local STT compute devices». RNNT
декодирует последовательно, CTC — нет. Всё это проверяется журналом.

Self-update U3: пользователь «точно не помнит, по-моему через обновление
программ» — недостаточно для PASS, U3 остаётся pending.
Далее: журнал с ПК (скорость — сразу, пропажа индикатора — когда случится).

## 2026-11-09T10:51:39.000Z — S2 разбор журнала ПК (glagol.2026-11-09.log)

Источник: файл журнала ПК, переданный пользователем; содержит записи 0.6.1 до
установки сборки (07:12–08:26 UTC) и сборки ветки (с 10:42 UTC).

Наблюдения (observed по журналу):
- «system diagnostics»: AMD FX(tm)-8300, logical_cpus=8,
  cpu_features=sse4.2,avx,fma,f16c (AVX2 нет), stt_threads=8.
- «local STT compute devices»: 0:vulkan Radeon RX550/550 Series 2147 MB;
  1:cpu FX-8300. «session opened» backend=CPU — Глагол явно запрашивает CPU.
  cpu_variant_features пусто: запрос признаков через реестр ggml ничего не
  вернул (причина unknown; transcribe.dll и ggml-cpu-*.dll импортируют тот же
  ggml.dll/ggml-base.dll). По набору инструкций ожидаемый вариант без AVX2 —
  вывод, не наблюдение.
- Все 6 диктовок сборки: rtf 1,04–1,13 (4,4–19,8 с аудио → 4,6–22,4 с).
  encode_ms ≈ 91–93 % времени (например 20 543 из 22 416 мс), decode_ms (RNNT)
  8–10 %, mel < 1 %, lock_wait_ms=0, segments=1. Записи 0.6.1 до обновления:
  то же соотношение ≈1,02–1,08.
- Первая диктовка после запуска: prepare_ms=3969 (verify_model 1764,
  verify_runtime 1326, open 878).
- Окно свёрнуто или нет, worker Silero загружен или идёт проверка Silero —
  rtf одинаковый (1,04–1,13): признаков фонового ограничения Windows или
  заметной конкуренции с Silero нет.
- Остальные этапы малы: stop 12–25 мс, вставка ≈303 мс, старт микрофона
  46–184 мс.
- Silero на ПК: прогрев worker 28 268 и 41 304 мс; полная проверка файлов
  после установки сборки — 210 587 мс, прогрев ждал её (219 404 мс).
- Индикатор: 6 показов, каждый — shown visible=true на DISPLAY1 1920×1080,
  страница получила все состояния, кадры за 16–42 мс, скрытие через 1500 мс.
  Пропажи в этом журнале нет; «starting» приходит дважды (on_pressed и
  конвейер) — безвредно.

Вывод: на ПК долгое распознавание — вычисление энкодера GigaAM на CPU
FX-8300 без AVX2, примерно в реальном времени (1 с речи ≈ 1,1 с ожидания).
Это не ожидание блокировок, не загрузка модели, не фон Windows и не Silero.
CTC вместо RNNT убрал бы не больше ~10 % (декодер). Главный кандидат для S3 —
видеокарта RX 550 через Vulkan, уже входящий в закреплённый runtime; также
проверить 4 потока вместо 8 (4 модуля FX делят FPU) и кеш проверки модели
(~3 с при первой диктовке). Исправление — после согласования с пользователем.
GL-006: ждём журнал с пропажей.

## 2026-11-09T11:03:13.000Z — S3 ускорение: Vulkan, потоки, квитанция проверки

Согласование: пользователь 2026-11-09 — «Да, гоу, все три и будем
тестировать» (видеокарта, потоки, кеш проверки). S2 закрыт по журналу ПК:
причина определена без журнала ноутбука, поэтому шаг сужен до ПК (заголовок
изменён, сравнение с ноутбуком не требуется для вывода).

Сделано:
- engine.rs: `transcribe_open` с `transcribe_session_params` (n_threads; 0 =
  по умолчанию библиотеки) и backend CPU(1) или VULKAN(3); при ошибке Vulkan —
  warn «local STT graphics card unavailable; using CPU» и открытие на CPU.
  «local STT session opened» пишет backend, gpu_requested, fell_back, threads.
- stt/local/mod.rs: EngineOptions {device cpu|gpu, threads 0..64} в
  app_settings (stt_local_device, stt_local_threads), читаются при запуске;
  prepare переоткрывает модель при смене опций; ошибка распознавания на Vulkan
  → warn, gpu_failed, повтор на CPU (до сохранения опций/перезапуска).
  Команды local_engine_state, set_local_engine_options (сразу prepare для
  активной русской локальной диктовки).
- receipt.rs: verification-<model>.json с размером и mtime модели, архива и
  всех файлов runtime; повтор полной проверки не дольше 30 дней; сброс при
  загрузке/восстановлении, удалении, ошибке открытия и распознавания.
  «local STT model loaded» пишет verification_reused и backend.
- UI: блок «Скорость распознавания» (где считать, потоки, «Сейчас считает»,
  предупреждение об откате на CPU) для русской локальной модели.
- Документация: обе USER_GUIDE, обе части README, CHANGELOG, runtime doc,
  PROJECT_STRUCTURE, CLAUDE.md.

Проверки (Linux container, rustc 1.97.0, node v22.23.4, pnpm 10.28.0;
PDFIUM_LIBRARY_PATH как в S1): cargo fmt --check, clippy -D warnings,
cargo test — exit 0, «354 passed; 0 failed; 6 ignored» (новые тесты:
receipt — повтор/устаревание/изменение файла/отсутствующие файлы; опции
движка — значения по умолчанию, запись, отказ >64, откат gpu_failed).
pnpm build — exit 0, «I18N PASS: 274 keys». Manual (headless Chromium,
заглушка IPC): блок отображается, с предупреждением об откате.
Не проверено: Vulkan и потоки на реальном ПК — это pc-test (NOT_RUN).

План замера на ПК: та же фраза ~10 с по 3 раза — CPU авто (8), CPU 4,
видеокарта; сравнить rtf и encode_ms в «local STT completed».

## 2026-11-09T13:57:41.000Z — версия 0.6.2 для тестовой сборки

По просьбе пользователя («как я узнаю, что это та сборка — подними версию до
0.6.2») версия поднята в package.json, Cargo.toml, Cargo.lock, tauri.conf.json.
`node scripts/check-version.mjs` — «Glagol 0.6.2: … versions match»;
`cargo build --locked`, `cargo test --locked` (354 passed) и `pnpm build` —
exit 0. Это не выпуск: CHANGELOG остаётся в [Unreleased], заголовки
руководств, release notes, подписанная сборка и публикация — при выпуске.
Установщик тестовой сборки будет называться Glagol_0.6.2_x64-setup.exe,
версия видна в боковом меню и в «Обновлениях».

## 2026-11-09T14:11:26.000Z — S3 замер 0.6.2 на ПК (pc-test)

Источник: второй журнал ПК glagol.2026-11-09.log (сборка 0.6.2 с 14:06 UTC;
«system diagnostics version="0.6.2"»). Reported: «по-моему, на видюхе чуть быстрее».

Observed (local STT completed, GigaAM v3 RNNT):
| Режим | Аудио, мс | Время, мс | rtf | encode_ms | decode_ms |
|---|---:|---:|---:|---:|---:|
| CPU, 8 потоков | 3 860 | 4 034 | 1,05 | 3 644 | 367 |
| CPU, 8 потоков | 25 340 | 28 097 | 1,11 | 25 281 | 2 694 |
| CPU, 4 потока | 19 980 | 22 274 | 1,11 | 20 011 | 2 167 |
| Vulkan0, 4 потока | 26 920 | 5 515 | 0,20 | 2 561 | 2 775 |
| Vulkan0, 4 потока | 4 330 | 859 | 0,20 | 346 | 480 |

- Vulkan открылся с первого раза: backend=Vulkan0, fell_back=false, загрузка
  1 753 мс. Откат на CPU в этом прогоне не проверен (не понадобился).
- На видеокарте распознавание ≈5,5 раза быстрее (rtf 1,1 → 0,2), энкодер ≈10 раз.
  Теперь половину времени занимает декодер RNNT (decode_ms ≈ encode_ms).
- 4 потока против 8 на CPU: разницы нет (rtf 1,11 и 1,11).
- Квитанция: первая загрузка после установки — полная проверка
  (verification_reused=false, 3 074 мс), переоткрытия — reused=true, 0 мс.
  Повторное использование после перезапуска программы ещё не наблюдалось.
- Переключатель озвучки выключен в 14:06:39 («text to speech switch changed
  enabled=false»), строк Silero после него нет.
- Индикатор: FrameMissing/ScriptError — 0, перезагрузок страницы нет; пропажа
  в журнале не зафиксирована (GL-006 открыт).

Вывод: pc-test PASS для Vulkan. Следующие наблюдения: CTC на видеокарте
(у CTC нет последовательного декодера RNNT), reused=true после перезапуска,
откат на CPU — при случае.
