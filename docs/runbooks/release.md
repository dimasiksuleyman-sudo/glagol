# Подготовка и выпуск Windows-релиза

Это процедура доставки, не разрешение публиковать. Актуальная очередь:
[STATUS](../STATUS.md). Подготовка reviewable артефакта предшествует запросу
разрешения на действие, если публикация ещё не была согласована.

## Исходники и артефакт

1. Проверить Git status/ветку/commit, выбранную серию и scope. В журнале отделить
   локальные изменения от вошедших в main. Локальный тег не доказывает GitHub Release.
2. Синхронизировать package.json, Cargo.toml, Cargo.lock и tauri.conf.json;
   выполнить `node scripts/check-version.mjs`. Проверить CHANGELOG и обе версии
   руководства на фактическое поведение этой сборки.
3. Выполнить относящиеся проверки и [сборку Windows](windows-build.md).
   Записать версию, SHA-256 и размер конечного установщика.
4. Выполнить [Silero QA](tts-smoke.md) и [диктовку](dictation-smoke.md)
   в объёме изменённых сценариев. Не закрывать ручную QA по unit-тестам.

## Подпись обновлений (с 0.6.0)

Установленные копии принимают обновление только с подписью ключа, открытая часть
которого записана в `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). Закрытый
ключ и пароль хранит сопровождающий вне репозитория (с резервной копией); при потере
ключа установленные версии больше не обновятся автоматически. CI собирает без подписи.
Ключ задаётся только в текущем процессе PowerShell и удаляется после сборки:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content "$env:USERPROFILE\.tauri\glagol-updater.key" -Raw
$secure = Read-Host -AsSecureString "Updater key password"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Net.NetworkCredential]::new('', $secure).Password
pnpm tauri build --config src-tauri/tauri.release.conf.json
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
pnpm release:assets --notes "Glagol <version>"
```

Ключ можно не хранить файлом, а вставить его текст в `Read-Host -AsSecureString`.
У ключа Глагола **есть пароль** (запись 2026-11-10; вывод 2026-11-09 «пароля нет»
был ошибочным). В 0.6.3 подпись внутри `pnpm tauri build` спросила пароль и упала
с «Wrong password for that key», хотя пароль был задан в переменной процесса;
причина не выяснена. Надёжный порядок с 0.6.5: собрать `pnpm tauri build`, затем
задать `TAURI_SIGNING_PRIVATE_KEY` и `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` через
`Read-Host -AsSecureString` и подписать готовый установщик:
`pnpm tauri signer sign src-tauri\target\release\bundle\nsis\Glagol_<version>_x64-setup.exe`,
затем `pnpm release:assets`. Без пароля подпись падает с той же ошибкой. ID ключа в
`.sig` должен совпасть с pubkey в tauri.conf.json (82D55017631804B2).

Рядом с установщиком появятся `.exe.sig`, `latest.json` и `SHA256SUMS.txt`. К Release
прикрепить установщик, `latest.json` и `SHA256SUMS.txt`; Release должен быть отмечен
latest, потому что программы читают `releases/latest/download/latest.json`. После
публикации скачать `latest.json` по этому адресу и сверить версию и URL установщика.
Проверка самообновления — установленная предыдущая версия находит и ставит новую.

## Чистая установка и обновление

Использовать согласованную тестовую среду/профиль. Перед обновлением реальной
библиотеки подготовить резервную копию и проверить её; не проводить destructive
проверки на единственном экземпляре данных.

Проверить установку, ярлыки/иконку, версию установленного EXE, single instance,
пояснение об отдельных компонентах, отсутствие их автоматической установки.
При обновлении проверить старые документы, воспроизведение/экспорт, настройки
и ключи STT. Согласие Silero должно оставаться отдельным, не переноситься бэкапом.
Отдельно проверить ручные сценарии диктовки после обновления.

Сохранить результаты и сведения об установленном EXE; при сравнении бинарников
учесть документированную метку Tauri/NSIS. Нельзя объявлять одинаковый SHA
установщика и установленного EXE: это разные файлы.

## Публикация

Подготовить release notes, перечень проверок/ограничений, точный commit/tag и
артефакт. Выполнить push/PR/merge/tag/release только в согласованном порядке.
Не добавлять веса или Python/PyTorch в Git или установщик.

После публикации прочитать GitHub Release и сверить tag/commit, имя asset и его
размер; проверить скачанный asset по SHA-256. Только затем обновить наблюдение
published на observed с URL/датой/evidence. Без доступа — reported/unknown.
CI artifact, локальный NSIS и публичный Release не взаимозаменяемы.

Закрыть серию после всех её обязательных проверок; `context:refresh` и
`runbook:check` должны пройти на итоговом состоянии. Непроверенные crash/timeout,
произношение или endpoint явно перечислить; не прятать их за общим «готово».
