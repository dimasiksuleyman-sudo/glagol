# Подготовка локальной озвучки Silero

[English](local-tts-runtime.md)

Это подготовка к 0.4.0, **не готовая функция приложения**. Глагол 0.3.0 пока
не изменён. План перехода: [silero-tts-migration.md](plans/silero-tts-migration.md).
Yandex SpeechKit v3 для коммерческой озвучки — следующий отдельный этап.
Диктовка, включая GigaAM и офисный сервер, независима от этой работы.

## Проверено 2026-09-12

На Windows x64, Ryzen 7 7730U / 16 ГБ RAM собран переносимый runtime:

- Python 3.11.9 embedded и PyTorch 2.7.1+cpu; системный Python, pip и CUDA не нужны.
- Все 13 архивов проверены по размерам и SHA-256 до распаковки.
- Загрузка: **249 346 278 байт** (~249 МБ), модель сюда не входит.
- Распаковка: **1 319 448 970 байт**, 14 483 файла, без JSON-описи.
  Это полная распаковка, включая SDK-файлы PyTorch; уменьшение ещё не проверено.
- Из изолированного интерпретатора успешно импортированы torch, numpy,
  num2words и docopt; проверены CPU-тензор и преобразование русского числа.
- Четыре теста сборщика прошли: выход из каталога, повреждение артефакта,
  очистка неудачной распаковки с сохранением соседних файлов, активация/опись.
- Проба модели отвергает неверный SHA-256 до импорта PyTorch и создания WAV.

**Синтез пока не проверен.** TCP-соединение с официальным сервером модели
`models.silero.ai` завершается таймаутом. Проверенная работоспособность PyTorch
не доказывает совместимость с конкретной моделью. Холодный старт модели,
скорость, RAM при синтезе, голоса и влияние на диктовку пока не измерены.

Официальный SAPI-установщик с GitHub изучен без запуска/установки: в нём
`v5_5_ts.bin` и отдельные модули, а не `v5_5_ru.pt`. Он не используется как
подмена. PyTorch 2.8.0+cpu тоже запускался, но его Windows-архив весит
619 392 861 байт, поэтому в подготовительный каталог выбран 2.7.1+cpu.

## Источники и лицензии

Каталог разработчика: [runtime-manifest.json](../scripts/silero/runtime-manifest.json).
Python загружается с python.org, PyTorch — из официального CPU-индекса,
остальные пакеты — с PyPI. Версии/размеры/хеши закреплены, метаданные не
обновляются автоматически. Сборка сохраняет upstream-лицензии в пакетах.

| Компонент | Версия | Лицензия |
|---|---|---|
| Python | 3.11.9 | PSF и лицензии включённых компонентов |
| PyTorch CPU | 2.7.1 | BSD-3-Clause и bundled third-party notices |
| NumPy | 2.2.6 | BSD-3-Clause и bundled third-party notices |
| filelock | 3.16.1 | Unlicense |
| typing_extensions | 4.15.0 | PSF-2.0 |
| SymPy | 1.14.0 | BSD-3-Clause |
| NetworkX | 3.4.2 | BSD-3-Clause |
| Jinja2 | 3.1.6 | BSD-3-Clause |
| fsspec | 2025.7.0 | BSD-3-Clause |
| mpmath | 1.3.0 | BSD-3-Clause |
| MarkupSafe | 3.0.3 | BSD-3-Clause |
| num2words | 0.5.14 | LGPL-2.1 |
| docopt | 0.6.2 | MIT |

Silero TTS v5.5 — отдельный необязательный компонент **для некоммерческого
использования**, CC BY-NC-SA 4.0, авторы Silero Team.
[Исходный проект](https://github.com/snakers4/silero-models),
[полная лицензия](third-party/Silero-LICENSE.txt).
SHA-256 сохранённого текста лицензии:
`1349a4b6148492b44f629e64eed676612e234fe9a839e4f3b277c1482c8849f1`.
Лицензия модели не заменяет MIT-лицензию Глагола и не ограничивает сама по
себе независимую диктовку. Условия выбранного STT-провайдера действуют отдельно.

Официальный URL модели задан в upstream `models.yml`:
`https://models.silero.ai/models/tts/ru/v5_5_ru.pt`.
В пробе предварительно закреплён SHA-256
`50081637b602126ee06cb3bc8a744d25651d2da149ee8864b9a379bfdd934437`,
совпадающий в двух независимо опубликованных интеграциях:
[bootstrap](https://github.com/ganiushin/parakeet-stt-silero-tts-addons-haos/blob/main/wyoming_silero_tts/silero/scripts/bootstrap.py)
и [bridge](https://github.com/Krablante/silero-tts-bridge).
Это **не заявление о проверке скачанной нами модели**: файл ещё не получен.
Перед включением в продукт подтвердить источник, размер и совместимость.

## Воспроизведение разработчиком

Эти инструменты не вызываются установленным Глаголом. Все файлы хранить в
`.scratch`; не добавлять веса/интерпретатор в Git или NSIS.

```powershell
node scripts/silero/download.mjs .scratch/silero/downloads
# download.mjs уже проверил SHA-256 архива Python.
Expand-Archive .scratch/silero/downloads/python-3.11.9-embed-amd64.zip .scratch/silero/bootstrap-python
& .scratch/silero/bootstrap-python/python.exe -I -B scripts/silero/assemble.py --downloads .scratch/silero/downloads --destination .scratch/silero/runtime-271
& .scratch/silero/runtime-271/python.exe -I -B scripts/silero/test_assemble.py
```

Каталог назначения должен быть новым: существующий runtime сборщик не
перезаписывает. Распаковка идёт в соседний `.staging`, ссылки и опасные пути
отвергаются, активация происходит после завершения. `python311._pth` отключает
пользовательские site-packages и переменные Python-путей. `setup.py` не выполняется.

После получения точного файла запустить `probe.py` из этого runtime с
`--model <полный путь к v5_5_ru.pt> --output <новый каталог> --threads 2`,
потом повторить с 4 потоками в другом процессе/каталоге. Ограничить процесс
таймаутом снаружи. Проба пишет WAV PCM mono 24 кГц и `report.json` с временем,
RTF и пиковым working set. Она использует только публичные тестовые фразы.
Прослушать WAV: техническая валидность не подтверждает качество произношения.
Отдельно проверить одновременную диктовку до замены pipeline приложения.
