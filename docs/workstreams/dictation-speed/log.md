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
