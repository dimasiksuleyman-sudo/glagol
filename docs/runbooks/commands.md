# Шпаргалка команд

Короткий набор того, что запускается чаще всего. Подробности и причины — в
[windows-build](windows-build.md) и [release](release.md). Все команды для
PowerShell, из папки репозитория `glagol`.

## Собрать тестовую версию с ветки

```powershell
git fetch origin
git checkout <ветка>            # например claude/optimistic-franklin-cvvbsr
git pull
pnpm install --frozen-lockfile
pnpm tauri build
Start-Process .\src-tauri\target\release\bundle\nsis\Glagol_<версия>_x64-setup.exe
```

Версию смотреть в `package.json` (`"version"`). Установка поверх сохраняет
библиотеку и настройки.

## Где что лежит

| Что | Путь |
|---|---|
| Установщик после сборки | `src-tauri\target\release\bundle\nsis\` |
| Журналы Глагола (7 дней) | `%LOCALAPPDATA%\app.glagol.desktop\logs` |
| Открыть папку журналов | `explorer "$env:LOCALAPPDATA\app.glagol.desktop\logs"` |

## Проверки перед коммитом (гейты)

```powershell
node scripts/check-version.mjs
pnpm exec tsc --noEmit
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

## Учёт работы (серии в docs/workstreams)

```powershell
pnpm context:refresh   # пересобрать docs/STATUS.md из state.json
pnpm runbook:check     # проверить, что state.json серий в порядке
```

## Сервер для других компьютеров: замер

```powershell
node scripts/lan-server-bench.mjs "<код подключения>" 10 3
```

Аргументы: код подключения, число клиентов (10), число кругов (3).

## Qwen3-TTS: замер скорости (tts-qwen Q2)

Скачать артефакт `qwentts-windows-x64` из последнего запуска «Qwen3-TTS spike
build» в GitHub Actions, распаковать, затем:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\qwen\bench.ps1 -BinDir <папка с tts-server.exe>
```

Модели скачиваются один раз в `%LOCALAPPDATA%\glagol-qwen-bench\models`
(≈ 1,5 ГБ по умолчанию), результаты и WAV — в `...\glagol-qwen-bench\out`.

## Выпуск

Только по [ранбуку выпуска](release.md): сборка обычной командой `pnpm tauri build`
(не с `--config src-tauri/tauri.release.conf.json` — она требует ключ во время сборки
и падает без него, 0.7.0), затем отдельная подпись обновления ключом
82D55017631804B2 (пароль только через `Read-Host -AsSecureString`, новый ключ
не создавать), затем `pnpm release:assets --notes-file docs/releases/v<версия>.notes.txt`.

## Git для агентов

```powershell
git status --short --branch
git log -5 --oneline
git fetch origin
git switch -c <агент>/<серия>-<шаг> origin/main   # своя ветка от main
git push -u origin <агент>/<серия>-<шаг>
```
