# Windows: проверка и сборка

Цель: воспроизводимый код и конкретный NSIS-артефакт. Результат сборки не
подтверждает установку или публикацию. Изменения только контекста проверяются
через Node-гейт, без повторной сборки приложения.

## Подготовка

Windows x64, Rust/MSVC, Node, pnpm и Git. Версии не обновлять в рамках проверки.
Сверить package.json, lockfile и текущую `.github/workflows/ci.yml`; CI и локальная
среда могут различаться. Зафиксировать:

```powershell
git status --short --branch
git rev-parse HEAD
node --version
pnpm --version
rustc --version
cargo --version
```

При необходимости первичной установки зависимостей — `pnpm install --frozen-lockfile`.
Не применять массовую переустановку как автоматическое лечение сбоя инструмента.

## Проверки по области изменения

```powershell
node scripts/check-version.mjs
node scripts/runbook-check.mjs
node --test scripts/runbook-check.test.mjs
pnpm exec tsc --noEmit
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
```

Нативные тесты с моделями запускаются отдельно по ранбукам TTS/STT.
Ignored-тесты не считать пройденными. ESLint/Vitest не настроены.

## Известные локальные ограничения

В сессии 2026-09-12 глобальный pnpm 12.4.1 пытался переустановить node_modules
и получал отказ. Если воспроизведён именно этот сбой, уже установленные entrypoints:

```powershell
node node_modules/typescript/bin/tsc --noEmit
node scripts/check-version.mjs
node node_modules/typescript/bin/tsc
node node_modules/vite/bin/vite.js build
```

Если build.rs не может скачать Pdfium, сначала проверить существующую
`src-tauri/resources/pdfium.dll`, её происхождение и соответствие сборке.
После проверки в текущем процессе:

```powershell
$env:PDFIUM_LIBRARY_PATH = (Resolve-Path 'src-tauri/resources/pdfium.dll').Path
```

Это локальный override, не доказательство исправления сетевой загрузки. Записать
его в validation, не фиксировать абсолютный путь разработчика в исходниках.

## NSIS

Штатно: `pnpm tauri build`. При подтверждённой проблеме pnpm выше создать локальный
`.scratch/build-context.json` с таким содержимым (JSON, не PowerShell):

```json
{
  "build": {
    "beforeBuildCommand": "node scripts/check-version.mjs && node node_modules/typescript/bin/tsc && node node_modules/vite/bin/vite.js build"
  }
}
```

```powershell
node node_modules/@tauri-apps/cli/tauri.js build --config .scratch/build-context.json
```

`&&` здесь — строка команды Tauri, а не цепочка команд PowerShell 5.
Использовать override только в этой сборке, конфигурация проекта остаётся pnpm.

После успешной сборки определить текущую версию из package.json и проверить
`src-tauri/target/release/bundle/nsis/Glagol_<version>_x64-setup.exe`:

```powershell
$buildVersion = (Get-Content package.json -Raw | ConvertFrom-Json).version
$installerPath = "src-tauri/target/release/bundle/nsis/Glagol_${buildVersion}_x64-setup.exe"
Get-Item -LiteralPath $installerPath | Select-Object FullName,Length,LastWriteTimeUtc
Get-FileHash -LiteralPath $installerPath -Algorithm SHA256
```

Записать SHA-256, размер, версию, Git baseline и изменения проверенного дерева.
Старая сборка с тем же именем не доказывает успех нового запуска: сопоставить
время/вывод сборки. Артефакт и его хеш — не Git commit.

Критерий: относящиеся проверки успешны, сборка завершилась с кодом 0,
идентифицирован итоговый файл. Установку закрывать отдельно по [release](release.md).
