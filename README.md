# Klipparkiv

**Portable Windows GUI for yt-dlp** — save audio, video and timecode clips, then keep
them in a local library. No installer, no registry, no admin rights.

**[English](#english)** · **[Русский](#русский)**

[![CI](https://github.com/xmalinov/klipparkiv/actions/workflows/ci.yml/badge.svg)](https://github.com/xmalinov/klipparkiv/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

| Download | Library | Settings |
|:---:|:---:|:---:|
| ![Download](assets/screenshots/download.png) | ![Library](assets/screenshots/library.png) | ![Settings](assets/screenshots/settings.png) |

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
- **Update yt-dlp** from the UI.
- **Russian and English UI** — switch the language in Settings; the choice is saved.
- **Portable** — settings and index in `data\`, media in `audio\` and `video\`;
  nothing is written to the registry or `%APPDATA%`.

### Requirements

Windows 10/11 x64 · WebView2 (preinstalled) · ~250 MB of disk space.

### Install

1. Download `klipparkiv-<version>-win-x64.zip` from
   [Releases](https://github.com/xmalinov/klipparkiv/releases).
2. Unpack the folder completely — do not run the exe from inside the archive.
3. Run `klipparkiv.exe`.

The build is unsigned, so SmartScreen may warn you: **More info → Run anyway**.
The folder must be writable. FAT32 drives cannot store files over 4 GB — use
exFAT/NTFS.

### Build from source

Node.js 24+, Rust stable (MSVC target), MSVC Build Tools, WebView2.

```powershell
npm ci
npm run tauri dev        # dev mode
npm run build:portable   # src-tauri\target\release\klipparkiv.exe
npm run pack:portable    # release\klipparkiv-<version>-win-x64.zip
```

The app drives external binaries that are **not** in this repository. Put them in
`bin\` next to the exe:

```
bin\yt-dlp.exe
bin\ffmpeg.exe
bin\ffprobe.exe
```

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
