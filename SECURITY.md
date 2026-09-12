# Security Policy / Политика безопасности

## English

Report vulnerabilities privately through [GitHub Security Advisories](https://github.com/dimasiksuleyman-sudo/glagol/security/advisories/new), not public issues.
We aim to acknowledge within 72 hours, assess within seven days and address
reports within the existing 90-day responsible disclosure window. Do not send
API keys, private documents or microphone recordings in public reports.

Current development line: 0.4.x. Historical releases may still use retired
providers; use the current code/release when testing fixes.

- Glagol processes local documents and audio. Local Silero TTS and local GigaAM
  inference require no network after component installation. Office/cloud STT
  sends recordings to the explicitly configured endpoint under its own terms.
- Python, CPU PyTorch, dependencies and Silero weights download only by choice
  from pinned HTTPS URLs with byte-count/SHA-256 checks. A `.pt` package contains
  executable model code: only the pinned v5.5 file can be imported.
- Archives extract into staging with path/link checks. Runtime files are compared
  with a compiled inventory before first process launch. No hub, pip or arbitrary
  script execution; no system Python search; no SAPI registration or HTTP listener.
- The worker uses bounded JSON over private stdin/stdout; WAV stays on disk.
  Input/output bounds, cancellation, timeouts and parent-lifetime monitoring limit
  failures. Idle workers unload. These are process controls, not an OS sandbox.
- Logs contain operational metadata, not document text, audio, transcripts or keys.
  Optional dictation history is off by default and stored locally when enabled.
- STT credentials use OS keyring slots separate from server/cloud profiles.
  The retired TTS key is narrowly deleted without reading it; failures retry at
  startup without blocking the app. Salute OAuth and custom root CA are removed.
- Webview CSP permits app/IPC/asset traffic; audio scope is limited to the cache.
  Production frontend does not execute remote scripts or call TTS providers.
- Backups include SQLite/library WAV, not downloaded runtimes/models, OS secrets
  or Silero licence acknowledgement. Existing library metadata is migrated, not
  rewritten as a new provider. Make backups before upgrading important libraries.

Downloaded components retain their own licences: Glagol code is MIT; Silero
v5.5 is CC BY-NC-SA 4.0 for noncommercial use. Licence acknowledgement is local
and provider-specific; it does not restrict independent organizational dictation.
Future Yandex SpeechKit v3 is not yet integrated.

This app cannot protect against malware running as the same user/admin,
compromised OS/devices, hostile edits to installed executable code, or a breach
of a user-selected external STT server. OS permissions and provider selection
remain part of the deployment's security boundary.

## Русский

Сообщайте об уязвимостях конфиденциально через [GitHub Security Advisories](https://github.com/dimasiksuleyman-sudo/glagol/security/advisories/new), а не в публичных Issues.
Цель: подтвердить получение за 72 часа, оценить за семь дней и исправить в рамках
90-дневного ответственного раскрытия. Не публикуйте ключи, личные документы и записи.

Текущая ветка разработки — 0.4.x; в исторических выпусках могли использоваться
удалённые провайдеры. Для проверки исправлений используйте актуальную версию.

- Silero и GigaAM после загрузки работают локально без сети. При office/cloud STT
  аудио уходит на явно выбранный пользователем сервер на условиях этого сервиса.
- Python, CPU PyTorch, зависимости и веса скачиваются по выбору с закреплённых
  HTTPS-адресов. Размеры и SHA-256 проверяются. `.pt` содержит исполняемый код:
  импортируется только точный закреплённый файл v5.5.
- Распаковка в staging проверяет пути/ссылки; перед запуском файлы сравниваются с
  встроенной описью. Нет hub/pip, поиска системного Python, регистрации SAPI,
  произвольных скриптов или HTTP-порта для озвучки.
- Ограниченный JSON идёт по stdin/stdout, WAV остаётся на диске. Отмена, таймаут,
  ограничения ввода/вывода, завершение с родителем и выгрузка по простою управляют
  процессом. Это не полноценная песочница операционной системы.
- В логах нет текста, аудио, распознанной речи и ключей. История диктовок локальна,
  необязательна и по умолчанию выключена.
- STT-ключи находятся в OS keyring и разделены по профилям. Старый TTS-ключ
  удаляется адресно без чтения; ошибка не блокирует запуск. OAuth и корневой
  сертификат SaluteSpeech больше не используются.
- CSP ограничивает webview приложением/IPC/asset; аудио доступно только из кэша.
  Бэкап содержит SQLite и WAV библиотеки, а не модели/runtime, ключи и подтверждение
  условий Silero. Старые данные сохраняют своего провайдера.

Лицензия кода — MIT, модели Silero v5.5 — CC BY-NC-SA 4.0 для некоммерческого
использования. Локальное подтверждение относится только к Silero и не ограничивает
независимую диктовку организаций. Yandex SpeechKit v3 пока не внедрён.

Приложение не защищает от вредоносного ПО с правами пользователя/администратора,
скомпрометированной ОС или устройств, подмены исполняемого кода и взлома выбранного
внешнего STT-сервера. Для важных библиотек делайте резервную копию перед обновлением.
