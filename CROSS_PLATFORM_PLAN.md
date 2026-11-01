# Klipparkiv: план сборки и обновлений для Windows и macOS

Статус: базовый macOS-билд и апдейтер реализованы; нужен финальный QA
на Apple Silicon, ключ подписи обновлений и прогон Windows CI.
Ограничения: без Apple Developer ID, без notarization, без App Store.
Распространение: GitHub Releases (portable ZIP для Windows, DMG для macOS).
Обновления: внутри приложения на обеих платформах.

## Что уже реализовано

- `AppPaths` с раздельными `root`/`support` и платформенными путями.
- macOS ad-hoc подпись, DMG и `.app.tar.gz`.
- macOS helper с атомарной заменой бандла (`renamex_np(RENAME_SWAP)`),
  fallback на две переименовки и backup/rollback.
- `update.json` с `windows-x86_64`, `darwin-x86_64`, `darwin-aarch64`
  и assisted DMG.
- macOS CI/release jobs, скрипты бинарей, манифеста и версии.
- Проверка версии учитывает `src-tauri/Cargo.lock`.
- Кроссплатформенные тесты и Unix process group.
- Очистка старых `.part`/распаковок, ротация backup и лога.
- Tray-template иконка macOS.
- Проверено: `cargo check/clippy/test`, `.app` + DMG + tar, `codesign`,
  распаковка и проверка подписи архива, launch в изолированном `HOME`.
- Не проверено: реальный Windows CI после изменений, Apple Silicon,
  App Management TCC, подпись приватным ключом обновлений.
- Тесты: 82 Rust + 5 helper + 47 frontend, clippy/fmt без ошибок.

---

## 1. Цели и границы

### Цели

1. Сохранить текущую Windows-сборку: portable ZIP, `klipparkiv.exe`, обновление одним exe.
2. Добавить macOS-сборку: `.app` в DMG, Intel x86_64 и Apple Silicon arm64.
3. Сохранить автообновление на Windows и добавить его на macOS.
4. Не требовать Apple Developer ID и notarization.
5. Сделать так, чтобы позже можно было включить Developer ID + notarization без переделки архитектуры.
6. Не ломать существующих пользователей Windows 1.2.0 и их данные.

### Не входит в этот этап

- App Store / Mac App Store.
- Windows ARM64.
- Linux.
- Автоматическое обновление yt-dlp без участия пользователя.
- Нотаризация Apple.
- Установщик MSI/NSIS для Windows.
- Подпись Windows Authenticode (можно добавить позже отдельно).

---

## 2. Текущее состояние: аудит

### Что уже работает

- React + Tauri 2, Rust-бэкенд компилируется под macOS без ошибок.
- `cargo check --all-targets` на macOS проходит.
- Tauri собирает `.app` (проверено в debug: `Klipparkiv.app`, x86_64, 49 MiB).
- Windows CI и release уже есть.
- Обновление Windows: манифест, minisign, SHA-256, helper с `ReplaceFileW`, backup, rollback.
- Иконка `.icns` в проекте есть.
- `tauri-plugin-notification`, `dialog`, `opener`, `single-instance` поддерживают macOS.

### Что мешает

| Проблема | Файл | Влияние |
|---|---|---|
| Корень данных = папка exe | `src-tauri/src/paths.rs:23-47` | В `.app` это `Contents/MacOS`: запись в бандл, падение из `/Applications`, поломка подписи |
| `appDirectoriesOverride="./data/appdata"` | `src-tauri/tauri.conf.json:13` | На macOS резолвится рядом с `.app`, для установленного приложения не работает |
| Имена `yt-dlp.exe`, `ffmpeg.exe`, `ffprobe.exe` | `src-tauri/src/paths.rs:73-83` | На macOS бинари не находятся |
| `kill_tree` через `taskkill` | `src-tauri/src/process.rs:24-33` | На macOS не убивает yt-dlp/ffmpeg |
| `UPDATE_TARGET = "windows-x86_64"` | `src-tauri/src/updater.rs:20` | macOS не видит обновления |
| Manifest требует `kind == "exe"` | `src-tauri/src/updater.rs:200-214` | macOS-архив не принимается |
| Staged-файл `klipparkiv.exe` | `src-tauri/src/updater.rs:402-407` | macOS-архив некуда класть |
| Helper только Windows | `src-tauri/src/bin/klipparkiv-updater.rs:14-17` | Нет замены `.app` |
| Лимит 128 МБ, таймаут 300 с | `src-tauri/src/updater.rs:22, 550` | macOS-бандл с ffmpeg не скачается |
| `bundle.active=false` | `src-tauri/tauri.conf.json:29-38` | Нет `.app`/DMG в релизе |
| CI/release только Windows | `.github/workflows/*.yml` | Нет macOS-артефактов |
| Windows-пути в тестах | `src-tauri/src/args.rs:279`, `paths.rs:154` | 7 тестов падают на macOS |
| Warning'и в Windows helper | `src-tauri/src/bin/klipparkiv-updater.rs` | `clippy -D warnings` падает на macOS |
| Жёсткий `\` в UI | `src/features/download/DownloadScreen.tsx:172` | Косметика на macOS |
| «Проводник» в i18n | `src/i18n/index.tsx:119` | На macOS нужен Finder |

### Проверенные факты

- Ad-hoc подпись `.app` и вложенных Mach-O проходит `codesign --verify --deep --strict`.
- `spctl --assess` на ad-hoc возвращает `rejected` — это ожидаемо, использовать его нельзя.
- Без подписи на Apple Silicon arm64-код не запустится.
- На macOS 15+ первый запуск скачанного приложения подтверждается через
  Settings → Privacy & Security → Open Anyway.
- На macOS 14 и старше работает правый клик → Open.

---

## 3. Целевая архитектура

### 3.1 Платформы и артефакты

| Платформа | Target triple | Установка | Основной артефакт | Update-артефакт |
|---|---|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | Portable ZIP | `klipparkiv-<ver>-win-x64.zip` | `klipparkiv-<ver>-win-x64.exe` |
| macOS Intel | `x86_64-apple-darwin` | DMG | `klipparkiv-<ver>-mac-x64.dmg` | `klipparkiv-<ver>-mac-x64.app.tar.gz` |
| macOS ARM | `aarch64-apple-darwin` | DMG | `klipparkiv-<ver>-mac-arm64.dmg` | `klipparkiv-<ver>-mac-arm64.app.tar.gz` |

Решение: отдельные сборки под архитектуры, не universal2.

Причины:
- ffmpeg/ffprobe проще брать под конкретную архитектуру.
- Update-артефакты меньше.
- Не нужен `lipo`.

Universal2 можно добавить позже как отдельный target `universal-apple-darwin`, если появится запрос.

Минимальная macOS: **11.0 Big Sur**.
Windows: 10/11 x64.

### 3.2 Конфигурация Tauri

Разделить конфиг на общий и платформенные файлы.

`src-tauri/tauri.conf.json` — общее:

```json
{
  "productName": "Klipparkiv",
  "version": "1.3.0",
  "identifier": "com.klipparkiv.app",
  "build": {
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:1420",
    "beforeBuildCommand": "npm run build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Klipparkiv",
        "width": 1000,
        "height": 700,
        "minWidth": 820,
        "minHeight": 560,
        "center": true,
        "theme": "Dark"
      }
    ],
    "security": { "csp": null }
  },
  "bundle": {
    "active": false,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

`src-tauri/tauri.windows.conf.json` — сохранить текущее поведение:

```json
{
  "app": {
    "appDirectoriesOverride": "./data/appdata"
  },
  "bundle": {
    "active": false
  }
}
```

`src-tauri/tauri.macos.conf.json`:

```json
{
  "app": {
    "appDirectoriesOverride": "$DATA/Klipparkiv"
  },
  "bundle": {
    "active": true,
    "targets": ["app"],
    "macOS": {
      "minimumSystemVersion": "11.0",
      "bundleName": "Klipparkiv",
      "bundleVersion": null
    },
    "resources": {
      "resources/bin/": "bin/"
    }
  }
}
```

Примечания:

- `$DATA/Klipparkiv` = `~/Library/Application Support/Klipparkiv`.
  Это же будет `support`-каталогом приложения.
- `bundle.resources` копирует бинари в `Contents/Resources/bin`.
- `bundle.active` и `resources` в platform-specific конфиге заменяют значения,
  а не дополняют массивы — это ожидаемо.
- Tauri-подпись (`signingIdentity`) не задаём: ad-hoc подпись делает отдельный
  скрипт после сборки. Так проще гарантировать, что подписаны и resources.
- `hardenedRuntime` пока не включаем. Он понадобится при переходе на
  Developer ID + notarization; для ad-hoc он не обязателен.
- `targets: ["app"]` — DMG собирает `hdiutil`, а не Tauri, чтобы успеть
  подписать вложенные бинари до упаковки.

Позже, при переходе на Developer ID, добавляются только:

```json
"macOS": {
  "signingIdentity": "Developer ID Application: ...",
  "hardenedRuntime": true,
  "entitlements": "entitlements.plist"
}
```

и Apple-секреты в CI. Архитектура не меняется.

### 3.3 Пути и данные

Ввести `AppPaths` вместо `PortablePaths`.

```rust
pub enum InstallKind {
    Portable,
    Installed,
}

pub struct AppPaths {
    root: PathBuf,     // данные и медиа
    support: PathBuf,  // обновления, backup, логи, копия yt-dlp
    kind: InstallKind,
}
```

#### Windows (без изменений)

| Что | Где |
|---|---|
| root | папка с exe |
| support | `root/data` |
| bin | `root/bin` |
| БД | `root/data/library.db` |
| настройки | `root/data/settings.json` |
| thumbs | `root/data/thumbs` |
| audio/video | `root/audio`, `root/video` |
| updates | `root/data/update` |

#### macOS installed

| Что | Где |
|---|---|
| root | `~/Movies/Klipparkiv` |
| support | `~/Library/Application Support/Klipparkiv` |
| bin | `Klipparkiv.app/Contents/Resources/bin` |
| БД | `~/Movies/Klipparkiv/data/library.db` |
| настройки | `~/Movies/Klipparkiv/data/settings.json` |
| thumbs | `~/Movies/Klipparkiv/data/thumbs` |
| audio/video | `~/Movies/Klipparkiv/audio`, `.../video` |
| updates | `~/Library/Application Support/Klipparkiv/updates` |
| backups | `~/Library/Application Support/Klipparkiv/backups` |
| логи | `~/Library/Application Support/Klipparkiv/logs` |
| копия yt-dlp | `~/Library/Application Support/Klipparkiv/bin/yt-dlp` |

Почему `~/Movies/Klipparkiv`:
- медиа лежат в видимой пользователю папке;
- `~/Movies` не защищён TCC, в отличие от Documents/Desktop/Downloads;
- не нужно объяснять, где лежат файлы.

Альтернатива: `~/Library/Application Support/Klipparkiv` для всего. Это
скрытая папка, зато строго «правильная» для macOS. Решение нужно подтвердить
до реализации (см. раздел 11).

#### macOS portable (опционально, не в MVP)

Если рядом с `.app` лежит маркер `.klipparkiv-portable`:

| Что | Где |
|---|---|
| root | папка, содержащая `.app` |
| support | `root/data` |
| bin | `Contents/Resources/bin` |
| updates | `root/data/update` |

Ограничения:
- App Translocation: если приложение запущено из `/private/var/.../AppTranslocation/...`,
  папка рядом недоступна на запись. Нужно показать ошибку и предложить
  перетащить `.app` в `/Applications` или `~/Applications`.
- `/Applications` у обычного пользователя может быть не writable.
  Обновление тогда уходит в assisted-режим.
- Portable-режим на macOS не обязателен для первого релиза.

#### Определение путей

- Debug: project root (как сейчас), чтобы `bin/` и `data/` были в репозитории.
- Windows release: `exe.parent()`.
- macOS release:
  - если `current_exe` = `.../Klipparkiv.app/Contents/MacOS/klipparkiv`,
    то `.app` = `current_exe` минус 3 компонента;
  - portable-маркер проверяется рядом с `.app`;
  - иначе installed root.
- macOS `bin`:
  - dev: `project_root/bin`;
  - bundled: `.../Contents/Resources/bin`;
  - portable: `.../Contents/Resources/bin` (внутри `.app`).
- `ytdlp()`:
  - Windows: `bin/yt-dlp.exe`;
  - macOS: `support/bin/yt-dlp` (копия, см. 3.4);
  - если копии нет — fallback на bundled `bin/yt-dlp`.
- `ffmpeg()`, `ffprobe()`:
  - Windows: `bin/*.exe`;
  - macOS: bundled `Resources/bin/*`.
- `ensure_layout()` создаёт `root`, `support`, `data`, `thumbs`, `audio`, `video`
  и делает write-probe в `support`.

`resolve_inside` остаётся для медиа; для update-путей добавить
`resolve_inside_support`.

### 3.4 Внешние бинари

#### Windows

Без изменений:

```
bin/yt-dlp.exe
bin/ffmpeg.exe
bin/ffprobe.exe
```

#### macOS

```
Klipparkiv.app/Contents/Resources/bin/yt-dlp
Klipparkiv.app/Contents/Resources/bin/ffmpeg
Klipparkiv.app/Contents/Resources/bin/ffprobe
```

Источники (версии и SHA-256 зафиксировать в `scripts/binaries.json`):

- `yt-dlp_macos` — universal2 с GitHub yt-dlp;
- `ffmpeg`, `ffprobe` — статические сборки под x86_64 и arm64
  (evermeet.cx / osxexperts.net; проверить лицензии и архитектуру).

Требования:
- `file` показывает нужную архитектуру;
- нет динамических зависимостей от Homebrew (`otool -L`);
- executable bit;
- ad-hoc подпись каждого бинаря.

#### Самообновление yt-dlp на macOS

Проблема: `yt-dlp --update` пишет в `Contents/Resources/bin/yt-dlp`, то есть
внутрь подписанного `.app`. Это ломает подпись.

Решение:
1. При старте (или при первом `update_ytdlp`) копировать bundled `yt-dlp`
   в `support/bin/yt-dlp`.
2. Выставить `0o755`.
3. Подписать копию ad-hoc: `codesign --force --sign - support/bin/yt-dlp`.
4. Все запуски `yt-dlp` на macOS вести из `support/bin/yt-dlp`.
5. `yt-dlp --update` обновляет эту копию, а не бандл.
6. При обновлении приложения сравнить версии bundled и support:
   - если bundled новее — заменить копию;
   - если support новее (пользователь обновил вручную) — оставить.
7. Если копирование/подпись не удались — работать из бандла и отключить
   кнопку обновления yt-dlp с понятной ошибкой.

ffmpeg/ffprobe всегда из бандла; они обновляются вместе с приложением.

#### Скачивание бинарей в CI

`scripts/fetch-binaries.mjs`:

- принимает `win-x64 | mac-x64 | mac-arm64`;
- читает `scripts/binaries.json` (url, version, sha256);
- скачивает, проверяет SHA-256;
- распаковывает;
- проверяет `file`/`otool`;
- кладёт:
  - Windows: `bin/`
  - macOS: `src-tauri/resources/bin/`
- `chmod 755` на macOS.

Зафиксированные версии важны для воспроизводимости релизов.

### 3.5 Процессы

`src-tauri/src/process.rs`:

- Windows: оставить `CREATE_NO_WINDOW` и `taskkill /PID /T /F`.
- Unix:
  - при spawn: `command.process_group(0)` (tokio / std);
  - `kill_tree(pid)`: `libc::kill(-(pid as i32), SIGKILL)`;
  - fallback: `kill(pid, SIGKILL)`;
  - логировать, если группа не убилась.
- Добавить `libc` в `[target.'cfg(unix)'.dependencies]`.

Тест на macOS:
- запустить `sh -c 'sleep 60 & wait'`, получить pid, убить группу,
  проверить, что оба процесса исчезли.

### 3.6 Ad-hoc подпись macOS

Скрипт `scripts/macos/sign-app.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail
APP="$1"
IDENTITY="${IDENTITY:--}"

# 1. Все Mach-O внутри Resources и MacOS, изнутри наружу.
find "$APP/Contents/Resources/bin" -type f -perm -111 -print0 |
  while IFS= read -r -d '' f; do
    codesign --force --sign "$IDENTITY" --timestamp=none "$f"
  done

for f in "$APP/Contents/MacOS/"*; do
  codesign --force --sign "$IDENTITY" --timestamp=none "$f"
done

# 2. Сам бандл.
codesign --force --sign "$IDENTITY" --timestamp=none "$APP"

# 3. Проверка.
codesign --verify --deep --strict --verbose=2 "$APP"
```

Правила:
- подписывать изнутри наружу;
- `--deep` использовать только для проверки, не для подписи;
- не использовать `spctl` как критерий;
- если в `Contents/MacOS` нет `klipparkiv-updater`, скопировать его из
  `src-tauri/target/release/klipparkiv-updater` и подписать;
- логировать `codesign -dv --verbose=4` для каждого бинаря.

DMG:

```bash
hdiutil create \
  -volname "Klipparkiv" \
  -srcfolder "$STAGING" \
  -ov -format UDZO \
  "$DMG"
```

В `$STAGING`:
- `Klipparkiv.app` (уже подписан);
- symlink `Applications -> /Applications`;
- `README.txt` с инструкцией первого запуска.

Update-архив:

```bash
COPYFILE_DISABLE=1 tar -czf "$TARBALL" -C "$BUNDLE_DIR" Klipparkiv.app
tar -tzf "$TARBALL" | head
```

### 3.7 Gatekeeper и первый запуск

Без Developer ID:

- DMG и `.app` получают quarantine;
- macOS 15+ блокирует первый запуск;
- пользователь открывает:
  - System Settings → Privacy & Security → Open Anyway;
  - либо правый клик → Open на старых macOS;
  - либо `xattr -dr com.apple.quarantine /Applications/Klipparkiv.app`.
- После подтверждения приложение запускается.
- Обновления, скачанные самим приложением, не получают quarantine,
  поэтому повторное подтверждение не требуется.
- Ad-hoc подпись не даёт «известного разработчика»; README и release notes
  должны это прямо говорить.
- Ad-hoc CDHash меняется при каждой сборке. TCC-разрешения (например,
  уведомления) могут сбрасываться после обновления. Это нужно проверить
  и, при необходимости, повторно запрашивать permission.

---

## 4. Обновления

### 4.1 Манифест

Оставить `schema: 1` для совместимости со старыми Windows-клиентами.
Добавить macOS-ключи. Старые клиенты читают только `windows-x86_64`
и игнорируют остальное.

Пример:

```json
{
  "schema": 1,
  "channel": "stable",
  "version": "1.3.0",
  "pubDate": "2026-11-01T12:00:00Z",
  "notes": "Cross-platform release",
  "minVersion": "1.2.0",
  "platforms": {
    "windows-x86_64": {
      "kind": "exe",
      "url": "https://github.com/xmalinov/klipparkiv/releases/download/v1.3.0/klipparkiv-1.3.0-win-x64.exe",
      "size": 18234234,
      "sha256": "…",
      "signature": "…"
    },
    "darwin-x86_64": {
      "kind": "app-tar-gz",
      "url": "https://github.com/xmalinov/klipparkiv/releases/download/v1.3.0/klipparkiv-1.3.0-mac-x64.app.tar.gz",
      "size": 145000000,
      "sha256": "…",
      "signature": "…",
      "minVersion": "1.3.0",
      "manual": {
        "kind": "dmg",
        "url": "https://github.com/xmalinov/klipparkiv/releases/download/v1.3.0/klipparkiv-1.3.0-mac-x64.dmg",
        "size": 150000000,
        "sha256": "…",
        "signature": "…"
      }
    },
    "darwin-aarch64": {
      "kind": "app-tar-gz",
      "url": "https://github.com/xmalinov/klipparkiv/releases/download/v1.3.0/klipparkiv-1.3.0-mac-arm64.app.tar.gz",
      "size": 145000000,
      "sha256": "…",
      "signature": "…",
      "minVersion": "1.3.0",
      "manual": {
        "kind": "dmg",
        "url": "https://github.com/xmalinov/klipparkiv/releases/download/v1.3.0/klipparkiv-1.3.0-mac-arm64.dmg",
        "size": 150000000,
        "sha256": "…",
        "signature": "…"
      }
    }
  }
}
```

Правила:

- `windows-x86_64.kind` не менять: старые клиенты требуют `exe`.
- `darwin-*` используют `app-tar-gz`.
- `manual` опционален; клиент не должен падать, если его нет.
- `minVersion` на уровне платформы переопределяет глобальный.
- `channel`/`pubDate` клиент может игнорировать.
- `signature` — minisign, как сейчас.
- Манифест лежит по стабильному URL:
  `https://github.com/xmalinov/klipparkiv/releases/latest/download/update.json`.

Расширить `ManifestAsset`:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestAsset {
    kind: String,
    url: String,
    size: u64,
    sha256: String,
    signature: String,
    #[serde(default)]
    min_version: Option<String>,
    #[serde(default)]
    manual: Option<ManualAsset>,
}
```

### 4.2 Выбор цели

Вместо `const UPDATE_TARGET`:

```rust
pub struct UpdateTarget {
    os: &'static str,
    arch: &'static str,
}

impl UpdateTarget {
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS,   // "windows" | "macos"
            arch: std::env::consts::ARCH, // "x86_64" | "aarch64"
        }
    }

    pub fn key(&self) -> String {
        let os = match self.os {
            "macos" => "darwin",
            other => other,
        };
        format!("{os}-{}", self.arch)
    }

    pub fn expected_kind(&self) -> &'static str {
        if self.os == "windows" { "exe" } else { "app-tar-gz" }
    }
}
```

Важно: `std::env::consts::ARCH` в universal2-сборке соответствует
запущенному слайсу, если когда-нибудь перейдём на universal.

### 4.3 Скачивание и проверка

Общий поток:

1. `check`:
   - HTTPS manifest;
   - schema 1;
   - version > current;
   - `minVersion`;
   - `platforms[target.key()]`;
   - `kind == expected_kind`;
   - size 1..=`MAX_UPDATE_BYTES`;
   - sha256 64 hex;
   - signature непустая;
   - URL https.
2. `download`:
   - staged path:
     - Windows: `support/update/<ver>/klipparkiv.exe`;
     - macOS: `support/update/<ver>/Klipparkiv.app.tar.gz`;
   - `.part` рядом;
   - поток в файл, hash, progress;
   - size limit;
   - flush + sync;
   - сравнить размер и SHA-256;
   - проверить minisign + версию в trusted comment;
   - rename `.part` → staged.
3. `install`:
   - проверить staged ещё раз;
   - Windows — helper;
   - macOS — auto или assisted.

Лимиты:

- `MAX_MANIFEST_BYTES = 256 KiB`;
- `MAX_UPDATE_BYTES = 512 MiB`;
- connect timeout 15 s;
- total download timeout 30 min;
- проверка свободного места: archive + ~2.5× (по возможности).

Коды ошибок должны быть локализованы (ru/en), как сейчас.

### 4.4 Windows: без изменения поведения

- `support = root/data`;
- staged `klipparkiv.exe`;
- helper `klipparkiv-updater.exe` рядом с exe;
- backup `data/update/backup/klipparkiv-<current>.exe`;
- helper:
  - ждёт PID;
  - `ReplaceFileW` с retry;
  - fallback rename;
  - проверяет `MZ`;
  - запускает новый exe;
  - rollback при ошибке.
- Проверка «портативной сборки»: parent of target == root.
- Всё остальное сохранить, чтобы обновление 1.2.0 → 1.3.0 работало.

### 4.5 macOS: автоматическая установка

Шаги:

1. Проверить, что `current_exe` находится в `.app`:
   `.../Klipparkiv.app/Contents/MacOS/klipparkiv`.
2. Если путь содержит `/AppTranslocation/` → assisted.
3. Проверить, что папка с `.app` доступна на запись (write-probe).
   Если нет → assisted.
4. Распаковать `app.tar.gz` в `support/update/<ver>/extract/`.
   - использовать crates `flate2` + `tar`;
   - валидировать entries:
     - только относительные пути;
     - нет `..`, абсолютных путей, пустых компонентов;
     - ровно один top-level `Klipparkiv.app`;
     - symlink'и только внутрь `Klipparkiv.app`;
   - сохранить mode, executable bit.
5. Проверить распакованный `.app`:
   - `CFBundleIdentifier == com.klipparkiv.app`;
   - `CFBundleShortVersionString == asset.version`;
   - `codesign --verify --deep --strict`;
   - каждая Mach-O имеет подпись (`codesign -dv`).
6. Скопировать helper:
   - `Contents/MacOS/klipparkiv-updater` →
     `support/update/<ver>/klipparkiv-updater`;
   - проверить `codesign --verify`.
7. Запустить helper:
   ```
   klipparkiv-updater \
     --wait-pid <pid> \
     --source <extract>/Klipparkiv.app \
     --target <current app> \
     --backup <support>/backups/Klipparkiv-<current>.app \
     --log <support>/logs/updater.log \
     --support <support> \
     --relaunch
   ```
8. `app.exit(0)`.

Helper на macOS:

- `parse_args` общий, без `cfg(windows)`.
- Валидирует:
  - `source` — `.app`-директория;
  - `target` — `.app`-директория;
  - `backup`/`log`/`source` внутри `--support`;
  - `target` в своей родительской папке.
- Ждёт PID:
  - `libc::kill(pid, 0)`;
  - `ESRCH` → вышел;
  - `EPERM` → жив;
  - timeout 60 s → ошибка.
- Замена:
  1. удалить старый backup;
  2. атомарно обменять `target` и `source` через
     `renamex_np(RENAME_SWAP)`;
  3. если ФС не поддерживает swap — `fs::rename(target, backup)`,
     `fs::rename(source, target)` с восстановлением при ошибке;
  4. если `EXDEV` — `ditto` (или рекурсивное копирование) + удаление source;
  5. сохранить старый бандл (после swap он лежит в `source`) в `backup`
     для rollback;
  6. снять `com.apple.quarantine` с нового `.app` (после проверки minisign);
  7. `open -n target`;
  8. подождать 5 с и проверить процесс (`pgrep -f "<target>/Contents/MacOS/klipparkiv"`);
  9. если не запустился — атомарно вернуть backup и `open -n target`.
- Логировать каждый шаг в `support/logs/updater.log`.
- Backup хранить до следующего успешного обновления.

Ограничение: App Management (TCC) на macOS 13+ может запретить замену
чужого `.app`. Это главный риск. Проверяется в spike. Если запрещает —
включается assisted-режим.

### 4.6 macOS: assisted update (fallback)

Если auto невозможен:

1. Скачать `manual.dmg` (или использовать уже скачанный).
2. Проверить размер, SHA-256, minisign.
3. `open <dmg>`.
4. Показать в UI:
   - «Перетащите Klipparkiv в Applications и запустите».
5. Пользователь перетаскивает, закрывает старое приложение, запускает новое.
6. Данные не затрагиваются.

Это не «полуручное скачивание с GitHub»: приложение само скачивает и
проверяет DMG, пользователь делает только drag-and-drop.

UI-состояние `manual`:

- текст;
- кнопка «Открыть DMG»;
- кнопка «Позже»;
- ошибка, если DMG не скачался.

Причина assisted:
- read-only `/Applications`;
- App Translocation;
- App Management TCC;
- ошибка замены бандла.

### 4.7 Состояния и интерфейс

Существующие состояния сохранить:

`idle`, `checking`, `current`, `available`, `downloading`, `ready`,
`deferred`, `installing`, `error`.

Добавить:

- `manual` — обновление скачано, нужен DMG;
- поле `installMode: "auto" | "manual"` в `UpdateStatus`;
- поле `target: "windows-x86_64" | "darwin-x86_64" | "darwin-aarch64"`
  для диагностики.

Команды:

- `get_update_status`;
- `check_app_update`;
- `download_app_update`;
- `install_app_update`;
- `open_manual_update` (новая);
- `cancel_pending_update`.

Поведение:

- `install_app_update`:
  - auto → `installing`, helper, exit;
  - manual → `manual`, вернуть статус;
- `open_manual_update`:
  - проверить DMG;
  - `open`;
  - `manual` остаётся, приложение не завершается;
- `maybe_install_when_idle`:
  - auto → установка;
  - manual → не открывать DMG автоматически, только выставить баннер.

Frontend:

- `UpdateBanner` добавляет ветку `manual`;
- i18n: «Обновление готово», «Открыть DMG», «Перетащите в Applications»;
- `appInfo` показывает:
  - `installKind: "portable" | "installed"`;
  - `dataRoot`, `supportRoot`;
  - `updateTarget`;
  - `pathSeparator`;
- на macOS заголовок «portable mode» заменить на «installed»/«portable».

### 4.8 Очистка и логи

- При старте:
  - удалить `.part` старше 7 дней;
  - удалить `support/update/<ver>` для версий старше текущей;
  - оставить один backup.
- После успешной установки helper удаляет `extract` и старый staged-архив.
- Лог: `support/logs/updater.log`, ротация по размеру (например, 1 МБ).
- В UI ошибка содержит путь к логу.

### 4.9 Безопасность

Цепочка доверия:

1. HTTPS GitHub.
2. `update.json`.
3. minisign (offline private key) для артефакта.
4. SHA-256.
5. Проверка версии в trusted comment.
6. macOS: `codesign --verify --deep --strict` (ad-hoc — целостность,
   не авторство).
7. Windows: PE header + helper.

Дополнительно:

- не разрешать HTTP в release;
- ограничить URL хостами `github.com` и `objects.githubusercontent.com`
  (или как минимум требовать HTTPS и GitHub Releases path);
- отклонять архив с path traversal и symlink escape;
- не запускать `spctl` для ad-hoc;
- не удалять quarantine у исходного DMG/приложения;
- удалять quarantine только у нового `.app` после minisign-проверки;
- не логировать содержимое ключей (их и нет в клиенте);
- лимиты размера и таймауты;
- rollback при любом сбое.

---

## 5. Изменения по файлам

### Rust

| Файл | Изменения |
|---|---|
| `src-tauri/src/paths.rs` | `AppPaths`, installed/portable root, support, bin, exe suffix, тесты без Windows-путей |
| `src-tauri/src/process.rs` | Unix process group + kill, тест |
| `src-tauri/src/binaries.rs` | без существенных изменений; тесты |
| `src-tauri/src/lib.rs` | `AppPaths`, `support` init, `app_info` поля, tray template, macOS menu, notification permission |
| `src-tauri/src/commands.rs` | пути yt-dlp, копия yt-dlp на macOS, notification, open data folder |
| `src-tauri/src/library.rs` | root из `AppPaths`; без изменения логики |
| `src-tauri/src/settings.rs` | без изменений (возможно, путь настроек) |
| `src-tauri/src/updater.rs` | разбить на модули `updater/*` |
| `src-tauri/src/updater/manifest.rs` | схема, target, minVersion, manual |
| `src-tauri/src/updater/download.rs` | streaming, лимиты, timeout, retry |
| `src-tauri/src/updater/verify.rs` | minisign, sha256, version |
| `src-tauri/src/updater/install/windows.rs` | текущая логика |
| `src-tauri/src/updater/install/macos.rs` | extract, codesign, helper, assisted |
| `src-tauri/src/bin/klipparkiv-updater.rs` | macOS swap, общий parse_args, тесты |
| `src-tauri/Cargo.toml` | `flate2`, `tar`, `plist`, `libc` (unix), `tempfile` dev |
| `src-tauri/tests/fixtures/*` | манифесты, tar-фикстуры |
| `src-tauri/capabilities/default.json` | `notification:default` при JS-разрешении |

### Tauri/конфиги

| Файл | Изменения |
|---|---|
| `src-tauri/tauri.conf.json` | убрать override, оставить общее |
| `src-tauri/tauri.windows.conf.json` | новый |
| `src-tauri/tauri.macos.conf.json` | новый |
| `src-tauri/resources/bin/` | mac-бинари (gitignored) |
| `src-tauri/icons/` | tray template, проверить icns |
| `rust-toolchain.toml` | убрать Windows target из общего файла, ставить targets в CI |

### Frontend

| Файл | Изменения |
|---|---|
| `src/api/updates.ts` | `manual`, `installMode`, `openManualUpdate` |
| `src/api/appInfo.ts` | новые поля |
| `src/features/updates/UpdateBanner.tsx` | ветка manual |
| `src/App.tsx` | portable/installed label, platform warnings |
| `src/features/settings/SettingsDialog.tsx` | путь данных, кнопка «Открыть папку» |
| `src/features/download/DownloadScreen.tsx` | `pathSeparator` |
| `src/i18n/index.tsx` | Finder, DMG, Gatekeeper, manual |
| `src/App.css` | стили manual-баннера |

### Скрипты

| Файл | Назначение |
|---|---|
| `scripts/fetch-binaries.mjs` | скачивание и проверка бинарей |
| `scripts/binaries.json` | версии, URL, SHA-256 |
| `scripts/merge-update-manifest.mjs` | сборка единого `update.json` |
| `scripts/check-version.mjs` | сверка версий package/Cargo/tauri/tag |
| `scripts/macos/sign-app.sh` | ad-hoc подпись |
| `scripts/macos/verify-app.sh` | codesign + Info.plist + arch |
| `scripts/macos/make-dmg.sh` | hdiutil + README + Applications |
| `scripts/macos/make-update-archive.sh` | tar.gz + проверка |
| `scripts/pack-portable.ps1` | без изменений |
| `scripts/make-update-manifest.ps1` | заменить Node-скриптом |

### Документация

| Файл | Изменения |
|---|---|
| `README.md` | macOS, Gatekeeper, DMG, данные, сборка |
| `README.txt` | то же для Windows ZIP |
| `UPDATING.md` | два платформы, манифест, подпись, CI, QA |
| `CHANGELOG.md` | 1.3.0 cross-platform |
| `CONTRIBUTING.md` | dev на macOS, тесты, ad-hoc |
| `SECURITY.md` (опционально) | модель доверия обновлений |

---

## 6. CI/CD

### 6.1 `ci.yml`

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
  workflow_dispatch:

jobs:
  frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-node@v7
        with:
          node-version: 24
          cache: npm
      - run: npm ci
      - run: npm run typecheck
      - run: npm run test:run
      - run: npm run build

  rust:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: windows-latest
          - os: macos-15-intel
          - os: macos-15      # arm64
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-node@v7
        with:
          node-version: 24
          cache: npm
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - run: npm ci
      - run: npm run build
      - run: cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
      - run: cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
      - run: cargo test --manifest-path src-tauri/Cargo.toml
      - run: cargo test --manifest-path src-tauri/Cargo.toml --bin klipparkiv-updater
```

Требования:
- Node 24 (не 22.9): иначе Vitest/rolldown падают.
- Исправить 7 Windows-тестов и 6 warning'ов helper'а.
- `npm run build` до `cargo` — нужен `dist` для `generate_context!`.

### 6.2 `release.yml`

Структура:

```yaml
name: Release

on:
  push:
    tags: ["v*"]
  workflow_dispatch:

concurrency:
  group: release-${{ github.ref }}
  cancel-in-progress: false

permissions:
  contents: read

jobs:
  build-windows:
    runs-on: windows-latest
    # build + pack + sign exe + manifest fragment
  build-macos-x64:
    runs-on: macos-15-intel
    # build + sign + dmg + tar.gz + sign + manifest fragment
  build-macos-arm64:
    runs-on: macos-15
    # same
  publish:
    needs: [build-windows, build-macos-x64, build-macos-arm64]
    runs-on: ubuntu-latest
    permissions:
      contents: write
    # merge + verify + draft release
```

#### Windows job

1. checkout, Node 24, Rust, cache.
2. `scripts/fetch-binaries.mjs win-x64`.
3. `npm ci`.
4. `npm run build:portable` (tauri --no-bundle + helper).
5. `npm run pack:portable`.
6. Подготовить update exe:
   `release/klipparkiv-<ver>-win-x64.exe`.
7. `npx tauri signer sign --app-version <ver> <exe>`.
8. Сгенерировать фрагмент `update-windows.json`:
   - size, sha256, signature, url.
9. Upload artifacts + fragment.

#### macOS job

1. checkout, Node 24, Rust, cache.
2. `scripts/fetch-binaries.mjs mac-x64|mac-arm64`.
3. `npm ci`.
4. `npx tauri build --bundles app`.
5. `scripts/macos/sign-app.sh`.
6. `scripts/macos/verify-app.sh`.
7. `scripts/macos/make-dmg.sh`.
8. `scripts/macos/make-update-archive.sh`.
9. Подписать:
   - `npx tauri signer sign --app-version <ver> <tar.gz>`;
   - `npx tauri signer sign --app-version <ver> <dmg>`.
10. Фрагмент `update-macos-<arch>.json` (основной + manual).
11. Upload.

#### Publish job

1. Скачать все фрагменты и артефакты.
2. `scripts/check-version.mjs`:
   - `package.json` = `Cargo.toml` = `tauri.conf.json`;
   - tag = `v<version>`.
3. `scripts/merge-update-manifest.mjs`:
   - объединить `platforms`;
   - проверить наличие `windows-x86_64`, `darwin-x86_64`, `darwin-aarch64`;
   - size/hash/sha256;
   - signature непустая;
   - URL соответствует имени артефакта.
4. Проверить minisign:
   - прогнать Rust-тест `verifies_the_release_artifact_when_configured`
     для каждого артефакта;
   - либо отдельный verifier-скрипт.
5. Создать `update.json`.
6. Создать draft release со всеми файлами:
   - `*.zip`, `*.exe`, `*.exe.sig`;
   - `*.dmg`, `*.dmg.sig`;
   - `*.app.tar.gz`, `*.app.tar.gz.sig`;
   - `update.json`.
7. Опубликовать вручную после проверки.

Секреты:

- build jobs: `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`;
- publish: `GITHUB_TOKEN`;
- Apple-секреты не нужны.

### 6.3 Именование артефактов

Ровно эти имена должны совпадать в манифесте и в release:

```
klipparkiv-<ver>-win-x64.zip
klipparkiv-<ver>-win-x64.exe
klipparkiv-<ver>-win-x64.exe.sig
klipparkiv-<ver>-mac-x64.dmg
klipparkiv-<ver>-mac-x64.dmg.sig
klipparkiv-<ver>-mac-x64.app.tar.gz
klipparkiv-<ver>-mac-x64.app.tar.gz.sig
klipparkiv-<ver>-mac-arm64.dmg
klipparkiv-<ver>-mac-arm64.dmg.sig
klipparkiv-<ver>-mac-arm64.app.tar.gz
klipparkiv-<ver>-mac-arm64.app.tar.gz.sig
update.json
```

---

## 7. Тесты и QA

### Unit / integration

- `paths`:
  - Windows root;
  - macOS installed root (mock HOME/temp dirs);
  - macOS portable marker;
  - exe suffix;
  - bin resolution;
  - writable probe.
- `process`:
  - Unix kill tree (два процесса);
  - Windows taskkill (существующий).
- `manifest`:
  - windows/mac target;
  - wrong kind;
  - missing target;
  - minVersion global/per-platform;
  - size/hash/url/signature;
  - schema.
- `verify`:
  - minisign fixture;
  - wrong version;
  - tampered data.
- `archive`:
  - корректный `.app.tar.gz`;
  - path traversal;
  - absolute path;
  - symlink escape;
  - два top-level;
  - неверный bundle id/version.
- `helper`:
  - parse args на всех ОС;
  - mac swap на temp-директориях;
  - rollback;
  - отсутствие source/target;
  - backup/log вне support.
- `download`:
  - слишком большой размер;
  - неверный hash;
  - обрыв (по возможности);
  - повторное использование staged.

### Manual QA

Матрица:

| Сценарий | Windows 10/11 | macOS 15 Intel | macOS 26 ARM |
|---|---|---|---|
| Чистая установка | ZIP | DMG + Gatekeeper | DMG + Gatekeeper |
| Первый запуск | OK | Open Anyway | Open Anyway |
| Аудио | OK | OK | OK |
| Видео | OK | OK | OK |
| Клипы | OK | OK | OK |
| Batch | OK | OK | OK |
| Pause/resume/cancel | OK | OK | OK |
| Tray/menu | OK | OK | OK |
| Notifications | OK | OK + permission | OK + permission |
| Single instance | OK | OK | OK |
| yt-dlp update | OK | support copy | support copy |
| App update 1.3.0 → 1.4.0 | OK | auto/assisted | auto/assisted |
| Update while queue active | deferred | deferred | deferred |
| Rollback | битый exe | битый app | битый app |
| Read-only location | — | assisted/manual | assisted/manual |
| Translocation | — | assisted/manual | assisted/manual |
| Permission reset after update | — | проверить | проверить |

### Проверка обновления

1. Собрать 1.3.0 и 1.4.0.
2. Установить 1.3.0.
3. Поднять тестовый manifest:
   - release URL на GitHub draft;
   - либо debug `KLIPPEARKIV_UPDATE_URL`.
4. Проверить:
   - check;
   - download;
   - signature;
   - install;
   - relaunch;
   - данные на месте;
   - backup;
   - rollback.
5. Повторить с:
   - повреждённым архивом;
   - неверной подписью;
   - отсутствующим ассетом;
   - read-only каталогом;
   - `/AppTranslocation/`.

### Проверка совместимости Windows

- Взять реальный 1.2.0 portable.
- Скормить ему `update.json` 1.3.0.
- Убедиться, что старый клиент видит только `windows-x86_64`,
  не падает на mac-ключах и обновляется.

---

## 8. Совместимость и миграция

- Windows 1.2.0 продолжает обновляться: schema 1, `windows-x86_64`,
  `kind: "exe"`, лимит < 128 МБ.
- macOS 1.3.0 — первая mac-версия; миграции нет.
- Данные macOS создаются заново в `~/Movies/Klipparkiv`.
- Обновление приложения не трогает `root` и `support`, кроме
  `support/updates`, `support/backups`, `support/logs`.
- Windows portable root не меняется.
- При удалении приложения данные остаются; добавить в README инструкцию
  удаления обеих папок.

---

## 9. Этапы и оценка

### M0. Spike (1–2 дня)

- Собрать ad-hoc `.app`.
- Запустить на macOS 15 Intel и Apple Silicon.
- Проверить Gatekeeper-флоу.
- Собрать два бандла 1.3.0/1.4.0 и вручную заменить `.app` helper'ом.
- Проверить App Management TCC:
  - замена в `/Applications`;
  - замена в `~/Applications`;
  - поведение после обновления.
- Проверить, сбрасываются ли notifications/TCC после смены ad-hoc CDHash.
- Решение: auto или assisted по умолчанию.

**Выход:** отчёт с результатами и выбранным режимом.

### M1. Кроссплатформенная сборка (2–3 дня)

- `AppPaths`, root/support, bin resolution.
- exe suffix, macOS binaries.
- Unix kill tree.
- Tauri config split.
- Ad-hoc sign script, DMG, verify.
- mac build запускается из DMG.
- Windows сборка не меняется.

**Выход:** `.dmg` и `.app` собираются локально и в CI; приложение работает.

### M2. Рефакторинг апдейтера + Windows (2 дня)

- `updater/*` модули.
- `UpdateTarget`, manifest с mac-ключами.
- per-target staged path/kind.
- лимиты, timeout.
- Windows 1.2.0 → 1.3.0 regression.
- manifest merge script.

**Выход:** Windows-обновление работает как раньше; манифест готов к mac.

### M3. macOS auto update (2–3 дня)

- tar extraction + validation.
- codesign verify.
- helper macOS.
- backup/rollback/relaunch.
- cleanup/logs.
- auto/assisted переключение.

**Выход:** 1.3.0 → 1.4.0 на Intel и ARM.

### M4. Assisted + release pipeline (2 дня)

- `manual` state, DMG download, UI.
- release jobs, artifacts, publish.
- docs, README, UPDATING.
- Homebrew Cask (опционально).

**Выход:** публичный draft release со всеми артефактами.

### M5. Hardening (1–2 дня)

- fault injection.
- QA matrix.
- логи, ошибки.
- финальный релиз.

Итого: **8–12 рабочих дней** при параллельной работе.
Если делать только assisted update на macOS: **6–8 дней**.

---

## 10. Риски и меры

| Риск | Вероятность | Мера |
|---|---|---|
| App Management TCC блокирует замену `.app` | Средняя | Spike; assisted DMG fallback |
| Ad-hoc CDHash меняется → TCC/уведомления сбрасываются | Средняя | Проверить; повторно запрашивать permission; документировать |
| Gatekeeper пугает пользователей | Высокая | Чёткий README, release notes, Open Anyway |
| Приложение в `/Applications` не writable | Средняя | Write-probe; assisted; рекомендация `~/Applications` |
| App Translocation | Средняя | Детект `/AppTranslocation/`; assisted |
| yt-dlp self-update ломает подпись | Высокая | Копия в support/bin |
| Вложенные бинари не подписаны | Средняя | sign-app.sh + verify + CI |
| tar path traversal | Низкая | Валидация entries + тесты |
| Обновление > лимита | Средняя | 512 МБ, проверка размера |
| Медленная сеть / timeout | Средняя | 30 мин, progress, retry |
| Ошибка замены → потеря приложения | Низкая | Backup + rollback |
| Старый Windows-клиент ломается | Низкая | Schema 1, kind exe, regression-тест |
| CI runner labels меняются | Низкая | `macos-15-intel`, `macos-15`; pin |
| Нет Apple Silicon для QA | Средняя | Найти Mac или не выпускать arm64 |
| Notarization понадобится позже | Низкая | Конфиг и секреты добавляются без кода |

---

## 11. Открытые решения

1. **Корень данных macOS:** `~/Movies/Klipparkiv` (рекомендуется) или
   `~/Library/Application Support/Klipparkiv`.
2. **Режим обновления macOS:** auto + assisted fallback (рекомендуется)
   или только assisted в первом релизе.
3. **Архитектуры:** отдельные x64/arm64 (рекомендуется) или universal2.
4. **Минимальная macOS:** 11.0 (рекомендуется) или 10.15.
5. **Portable macOS:** не делать в MVP (рекомендуется) или делать сразу.
6. **Homebrew Cask:** добавить позже (опционально).
7. **Windows Authenticode:** не делать сейчас; SmartScreen остаётся.
8. **Уведомления:** запрашивать permission на macOS при первом запуске.

---

## 12. Первые шаги

1. Подтвердить решения из раздела 11.
2. Сделать M0 spike на Intel и Apple Silicon.
3. По результатам spike зафиксировать auto/assisted.
4. Реализовать M1 (пути, бинари, kill, ad-hoc, DMG).
5. Затем M2–M4.

Главный критерий готовности: пользователь скачивает DMG с GitHub,
один раз подтверждает запуск, а дальше обновляется из приложения
без Developer ID и без повторного drag-and-drop.
