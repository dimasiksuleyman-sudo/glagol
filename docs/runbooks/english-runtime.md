# Английская речь: Moonshine STT и Silero EN

Текущая поставка [english-first](../workstreams/english-first/README.md): Moonshine
Small Streaming и Silero v3_en. Kokoro/G2P заменены по решению пользователя;
старые записи manifest/probe остаются историей, в приложение не входят.

## Файлы и лицензии

[artifacts.json](../../scripts/english-runtime/artifacts.json): runtime v0.1.5,
ABI 30000; wheel 16 542 073 байта, две DLL 23 781 408 байт; восемь STT-файлов
142 300 974 байта, revision 0bf2f2e5aff22e6fbba4300b00a4e00bbc4f8aae.
Production generator выбирает только kind=stt. Windows SDK содержит статическую
библиотеку: DLL извлекаются как ZIP-данные из официального wheel, без исполнения
Python. OrtGetVersionString=1.23.2; Python английскому STT не нужен.

[notices.json](../../scripts/english-runtime/notices.json) закрепляет SHA, размер
и источники Moonshine LICENSE, ORT LICENSE и ThirdPartyNotices. Копии сохраняются
в каталоге компонента. English streaming — MIT.
[english-model.json](../../scripts/silero/english-model.json): Silero EN 57 194 546
байт, CC-BY-NC-SA-4.0, общий RU Python 3.11.9/torch 2.7.1+cpu. Origin timed out;
точная revision зеркала и origin_verified=false сохранены явно. Автоматической
подмены источника нет. Первая EN TTS загрузка 306 540 824 байта; к исправной RU
нужна только модель. Голоса en_0..en_3, без обещаний пола/акцента.

## Гейты

```powershell
node scripts/english-runtime/fetch.mjs .scratch/english-first
node scripts/english-runtime/selected-check.mjs .scratch/english-first
node --test scripts/english-runtime/verify.test.mjs
node scripts/i18n-check.mjs
node --test scripts/i18n-check.test.mjs
```

fetch по умолчанию получает только STT/wheel. Извлечь две DLL по закреплённым
member; точную EN PT положить в корень после проверки SHA. selected-check проверяет
выбранный комплект, notices и общий runtime ID. Отрицательные CLI fixtures — exit 1.
Старые native-smoke.py и verify.mjs licenses относятся к Kokoro/G2P; для их
повторения нужен явный fetch --historical-kokoro. Старый FAIL лицензии G2P не стал
PASS, эти файлы исключены из текущей поставки. Запрос автору не отправлялся.

## Worker приложения

Все входы — в отдельном scratch, не в пользовательском профиле. process-root
содержит native/ и stt/ по manifest. Синтетический PCM16 mono WAV — из Silero probe.
Не удалять рекурсивно scratch-junction на общий runtime.

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
$env:GLAGOL_MOONSHINE_ROOT = (Resolve-Path '.scratch/english-first/process-root').Path
$env:GLAGOL_MOONSHINE_TEST_EXE = (Resolve-Path 'src-tauri/target/debug/glagol.exe').Path
$env:GLAGOL_MOONSHINE_WAV = (Resolve-Path '.scratch/english-first/silero-en-smoke/en_0-plain-0.wav').Path
cargo test --manifest-path src-tauri/Cargo.toml native_moonshine_ -- --ignored --nocapture
node scripts/english-runtime/process-test.mjs src-tauri/target/debug/glagol.exe .scratch/english-first/process-root
$env:GLAGOL_TTS_SMOKE_ROOT = (Resolve-Path '.scratch/english-first/shared-silero').Path
cargo test --manifest-path src-tauri/Cargo.toml native_silero_ -- --ignored --nocapture
node scripts/silero/process-test.mjs .scratch/english-first/shared-silero
& .scratch/silero/runtime-271/python.exe -I -B scripts/silero/test_worker.py
& .scratch/silero/runtime-271/python.exe -I -B scripts/silero/voice-smoke.py --root .scratch/english-first/shared-silero --output .scratch/english-first/new-voice-smoke
& .scratch/silero/runtime-271/python.exe -I -B scripts/english-runtime/concurrent-smoke.py --exe src-tauri/target/release/glagol.exe --stt-root .scratch/english-first/process-root --tts-root .scratch/english-first/shared-silero --wav .scratch/english-first/silero-en-smoke/en_0-plain-0.wav --output .scratch/english-first/new-concurrent-smoke
```

Silero-корень содержит обе PT, runtime/ и закреплённые архивы. Тесты проверяют
streaming/resampler, начало/конец, warm reuse, тишину, crash/restart/cancel,
commit без повторного скачивания DLL и удаление модели с сохранением runtime.
Process-tests — oversized protocol и parent exit. Обычный cargo test — очередь,
границы/хвост ресемплера, таймаут с коротким test deadline и зависшим owned process;
в production остаётся 90 секунд. Finish latency начинается после последнего блока,
не с физического отпускания клавиши. Не логировать пользовательские тексты/аудио.

Inference без сетевого клиента не равно проверке с отключённой сетью Windows.
Прослушивание, живой микрофон/вставка, совместная отзывчивость, визуальные восемь
сочетаний языков, clean install/uninstall EN/RU Windows 10/11 — отдельная ручная
матрица; недоступные пункты NOT_RUN/BLOCKED. Обязательные команды: [Windows](windows-build.md).
voice-smoke проверяет текущий worker для всех девяти голосов; выходной каталог должен
быть новым. concurrent-smoke запускает native EXE STT и Silero EN одновременно,
используя синтетический файл, а не микрофон; он не подтверждает отзывчивость webview.
