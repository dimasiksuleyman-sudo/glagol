# Находки и ограничения

Одна причина — один ID. Неизвестную причину обозначать unknown. Пробел QA
не является доказанным дефектом. Состояния: open, planned, resolved, accepted.
Для accepted нужна причина; для resolved — evidence проверки. Новые независимые
работы не добавлять молча в текущую серию.

| ID | Вид / статус | Наблюдение, модуль и сценарии | Источник | Критерий закрытия / действие |
|---|---|---|---|---|
| GL-001 | QA gap / planned | 0.4.0: установка/обновление, согласие, preview, ярлыки, живой STT и прослушивание не проверены полностью | [Silero log](../day-logs/day-2026-09-12-silero-integration-master-log.md) | Серия verification-0.4.0: отдельные результаты UI, обновления и звука |
| GL-002 | Network observation / open | models.silero.ai был недоступен из shell; причина unknown. Импорт точной модели не доказывает работу авто-download | [Runtime](../local-tts-runtime.ru.md) | Повторная проверка официального endpoint/загрузки с датой; сохранить импорт как отдельный путь |
| GL-003 | Tooling observation / open | pnpm 12.4.1 пытался переустановить зависимости, получал отказ; использованы существующие Node entrypoints | [Integration](../day-logs/day-2026-09-12-silero-integration-master-log.md) | Отдельная задача воспроизводимости инструментов; успешный стандартный build без неожиданной переустановки |
| GL-004 | Build observation / open | Pdfium download в build.rs был недоступен; использован PDFIUM_LIBRARY_PATH к существующей DLL | [Integration](../day-logs/day-2026-09-12-silero-integration-master-log.md) | Проверить чистую сборку с получением закреплённой DLL; до этого фиксировать override |
| GL-005 | QA gap / open | Crash, response timeout и parent-death worker реализованы, но отдельно не fault-injected | [Integration](../day-logs/day-2026-09-12-silero-integration-master-log.md) | Изолированные сценарии с наблюдаемым завершением worker; не считать обычную отмену их заменой |

Якоря GL-001: installer/hooks.nsh, TtsSection, DictationSection и runtime worker;
GL-002: stt/local downloader + tts/silero/catalog; GL-003: package manager/build;
GL-004: src-tauri/build.rs; GL-005: tts/silero worker. Это места проверки, а не
установленные причины всех наблюдений. Исторические замеры относятся к 12 сентября.
