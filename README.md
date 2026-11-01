# Klipparkiv

**Portable GUI for yt-dlp on Windows and macOS** — save audio, video and timecode
clips, then keep them in a local library. No installer on Windows, no registry,
no admin rights.

**[English](#english)** · **[Русский](#русский)**

[![CI](https://github.com/xmalinov/klipparkiv/actions/workflows/ci.yml/badge.svg)](https://github.com/xmalinov/klipparkiv/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

| Download | Library | Updates |
|:---:|:---:|:---:|
| ![Download](assets/screenshots/download.png) | ![Library](assets/screenshots/library.png) | ![Updates](assets/screenshots/updates.png) |

---

## English

*klipparkiv* (Swedish) — an archive of clippings.

### Features

- **Audio** — MP3 128–320 kbps with cover art and tags.
- **Video** — MP4 480p–2160p.
- **Audio + Video** — two files from one link, kept in separate archives.
- **Clips** — cut audio or video by timecode, e.g. `00:01:30-00:05:00`.
- **Batch** — one URL per line in a `.txt`; queue with 1–4 parallel downloads,
  pause, resume and cancel.
- **Metadata preview** — title, channel, duration, thumbnail, formats.
- **Download archive** — items you already saved are skipped.
- **Library** — covers, search, filters, sorting, open in the system player.
- **App updates** — in-app banner, signed download, restart on your command.
- **Update yt-dlp** from the UI.
- **Russian and English UI** — switch the language in Settings; the choice is saved.
- **Portable on Windows** — settings and index in `data\`, media in `audio\`
  and `video\`; nothing is written to the registry or `%APPDATA%`.
- **Native macOS app** — `.app` in a DMG; data lives in `~/Movies/Klipparkiv`.

### Requirements

- Windows 10/11 x64 with WebView2 (preinstalled).
- macOS 11+ on Intel or Apple Silicon.
- ~250 MB of disk space (plus the media you download).

### Install

**Windows**

1. Download `klipparkiv-<version>-win-x64.zip` from
   [Releases](https://github.com/xmalinov/klipparkiv/releases).
2. Unpack the folder completely — do not run the exe from inside the archive.
3. Run `klipparkiv.exe`.

The build is unsigned, so SmartScreen may warn you: **More info → Run anyway**.
The folder must be writable. FAT32 drives cannot store files over 4 GB — use
exFAT/NTFS.

**macOS**

1. Download `klipparkiv-<version>-mac-x64.dmg` (Intel) or
   `klipparkiv-<version>-mac-arm64.dmg` (Apple Silicon).
2. Open the DMG and drag Klipparkiv into Applications.
3. The app is ad-hoc signed, not notarized by Apple. On first launch:
   - macOS 15+: System Settings → Privacy & Security → **Open Anyway**;
   - macOS 14 and older: right-click Klipparkiv → **Open**.
   - Terminal alternative: `xattr -dr com.apple.quarantine /Applications/Klipparkiv.app`.
4. In-app updates do not require this confirmation again.

### Build from source

Common: Node.js 24+, Rust stable, npm.

**Windows** (MSVC Build Tools, WebView2):

```powershell
npm ci
npm run tauri dev        # dev mode
npm run build:portable   # src-tauri\target\release\klipparkiv.exe
npm run pack:portable    # release\klipparkiv-<version>-win-x64.zip
```

**macOS** (Xcode Command Line Tools):

```bash
npm ci
npm run tauri dev        # dev mode
node scripts/fetch-binaries.mjs mac-arm64   # or mac-x64
npm run build:macos      # .app bundle
npm run pack:macos -- 1.3.0 arm64           # DMG + update archive
```

The app drives external binaries that are **not** in this repository.

Windows: put them in `bin\` next to the exe:

```
bin\yt-dlp.exe
bin\ffmpeg.exe
bin\ffprobe.exe
```

macOS: `node scripts/fetch-binaries.mjs mac-x64|mac-arm64` downloads pinned
`yt-dlp`, `ffmpeg` and `ffprobe` into `src-tauri/resources/bin`; they are
bundled into `Contents/Resources/bin`.

For the release and update-signing flow see [UPDATING.md](UPDATING.md).

Checks:

```powershell
npm run typecheck
npm run test:run
cargo test  --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
```

### Project layout

| Path | Purpose |
|---|---|
| `src\` | React 19 + TypeScript frontend |
| `src-tauri\` | Rust backend: processes, progress, SQLite library |
| `scripts\` | Portable packaging and screenshot helpers |
| `assets\screenshots\` | Images used in this README |

### Legal

Klipparkiv is a GUI: it runs [yt-dlp](https://github.com/yt-dlp/yt-dlp) (Unlicense)
and [FFmpeg](https://ffmpeg.org/legal.html) (LGPL/GPL depending on the build).
Download only content you own or are allowed to save; respect the terms of service
and the authors' copyright. Not affiliated with YouTube, Google, yt-dlp or FFmpeg.

### License

[MIT](LICENSE) © Klipparkiv contributors. See [CONTRIBUTING.md](CONTRIBUTING.md)
for setup and checks.

---

## Русский

*klipparkiv* (швед.) — архив вырезок.

### Возможности

- **Аудио** — MP3 128–320 кбит/с, с обложкой и тегами.
- **Видео** — MP4 от 480p до 2160p.
- **Аудио + видео** — два файла по одной ссылке, в разных архивах.
- **Клипы** — вырезание аудио или видео по таймкоду, например `00:01:30-00:05:00`.
- **Пакетно** — по одной ссылке в строке `.txt`; очередь на 1–4 параллельные
  загрузки, пауза, продолжение, отмена.
- **Предпросмотр** — название, канал, длительность, обложка, форматы.
- **Архив загрузок** — уже сохранённое пропускается.
- **Библиотека** — обложки, поиск, фильтры, сортировка, запуск в системном плеере.
- **Обновления приложения** — баннер в окне, подписанная загрузка, перезапуск по команде.
- **Обновление yt-dlp** из интерфейса.
- **Русский и английский интерфейс** — язык переключается в настройках и сохраняется.
- **Портативность** — настройки и индекс в `data\`, медиа в `audio\` и `video\`;
  реестр и `%APPDATA%` не затрагиваются.

### Требования

Windows 10/11 x64 · WebView2 (есть по умолчанию) · ~250 МБ на диске.

### Установка

1. Скачайте `klipparkiv-<версия>-win-x64.zip` со страницы
   [Releases](https://github.com/xmalinov/klipparkiv/releases).
2. Распакуйте папку целиком — не запускайте exe из архива.
3. Запустите `klipparkiv.exe`.

Сборка без подписи, поэтому SmartScreen может предупредить:
**«Подробнее» → «Выполнить в любом случае»**. Папка должна быть доступна для
записи. На FAT32 файлы больше 4 ГБ не поместятся — используйте exFAT/NTFS.

### Сборка из исходников

Node.js 24+, Rust stable (цель MSVC), MSVC Build Tools, WebView2.

```powershell
npm ci
npm run tauri dev        # режим разработки
npm run build:portable   # src-tauri\target\release\klipparkiv.exe
npm run pack:portable    # release\klipparkiv-<версия>-win-x64.zip
```

Приложению нужны внешние бинарники, которых **нет** в репозитории. Положите их в
`bin\` рядом с exe:

```
bin\yt-dlp.exe
bin\ffmpeg.exe
bin\ffprobe.exe
```

Процесс релиза и подписи обновлений описан в [UPDATING.md](UPDATING.md).

Проверки:

```powershell
npm run typecheck
npm run test:run
cargo test  --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
```

### Структура

| Путь | Назначение |
|---|---|
| `src\` | фронтенд на React 19 + TypeScript |
| `src-tauri\` | бэкенд на Rust: процессы, прогресс, библиотека на SQLite |
| `scripts\` | упаковка portable-сборки и скриншоты |
| `assets\screenshots\` | изображения для этого README |

### Правовое

Klipparkiv — только интерфейс: он запускает [yt-dlp](https://github.com/yt-dlp/yt-dlp)
(Unlicense) и [FFmpeg](https://ffmpeg.org/legal.html) (LGPL/GPL в зависимости от
сборки). Скачивайте только тот контент, на который у вас есть права; соблюдайте
условия сервисов и авторские права. Проект не связан с YouTube, Google, yt-dlp
и FFmpeg.

### Лицензия

[MIT](LICENSE) © Klipparkiv contributors. Настройка и проверки описаны в
[CONTRIBUTING.md](CONTRIBUTING.md).
