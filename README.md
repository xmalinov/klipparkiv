# Klipparkiv

**Portable Windows GUI for [yt-dlp](https://github.com/yt-dlp/yt-dlp).** Download audio,
video and timecode clips, then browse everything in a built-in library.

*klipparkiv* (Swedish) — an archive of clippings.

![Download screen](assets/screenshots/download.png)
![Library](assets/screenshots/library.png)
![Settings](assets/screenshots/settings.png)

## Features

- **Audio** — MP3 128–320 kbps with cover art and tags.
- **Video** — MP4 from 480p up to 2160p.
- **Audio + Video** — one click, two files, two separate archives (so the audio
  download never blocks the video one).
- **Clips by timecode** — `00:01:30-00:05:00` for both audio and video.
- **Batch mode** — drop a `.txt` with one URL per line and walk away.
- **Queue** — 1–4 parallel downloads, pause/resume, cancel, per-item progress and logs.
- **Metadata preview** — title, channel, duration, thumbnail and available formats
  before downloading.
- **Download archive** — items already saved are skipped automatically.
- **Library** — covers, search, filters, sorting, "show in Explorer", open in the
  system player. The index is a local SQLite database.
- **Update yt-dlp** from the UI (YouTube changes often).
- **Truly portable** — settings, library and thumbnails live in `data\` next to the
  executable; media goes to `audio\` and `video\`. Nothing is written to the registry
  or to `%APPDATA%`.

## Requirements

| | |
|---|---|
| OS | Windows 10/11 x64 |
| Runtime | [WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) (preinstalled on Windows 10/11) |
| Disk | ~250 MB of free space for the portable build |

## Download

Grab the latest `klipparkiv-<version>-win-x64.zip` from the
[Releases](https://github.com/xmalinov/klipparkiv/releases) page.

1. Unpack the whole folder (do not run the exe from inside the archive).
2. Run `klipparkiv.exe`.
3. Paste a YouTube link and press **Скачать** (*Download*).

The folder must be writable. The build is unsigned, so SmartScreen may warn you:
**More info → Run anyway**.

## Build from source

Prerequisites: Node.js 24+, Rust (stable, MSVC target), MSVC Build Tools, WebView2.

```powershell
npm ci
npm run tauri dev        # dev mode (Vite + Tauri window)
```

Portable release:

```powershell
npm run build:portable   # src-tauri\target\release\klipparkiv.exe
npm run pack:portable    # dist-portable\ + release\klipparkiv-<version>-win-x64.zip
```

The application drives external binaries, which are **not** stored in this repository.
Put them into `bin\` next to the exe (in dev mode — into the project root):

```
bin\
  yt-dlp.exe
  ffmpeg.exe
  ffprobe.exe
```

`yt-dlp` can be replaced with a newer build at any time without losing settings
or the library.

### Checks

```powershell
npm run typecheck
npm run test:run
cargo test  --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
```

## Project layout

| Path | Purpose |
|---|---|
| `src\` | React 19 + TypeScript frontend (screens, API bindings) |
| `src-tauri\` | Rust backend: process control, progress parsing, SQLite library |
| `src-tauri\src\args.rs` | Builds yt-dlp argument lists (audio, video, clips, batch) |
| `src-tauri\src\process.rs` | Spawns and supervises yt-dlp/ffmpeg processes |
| `src-tauri\src\library.rs` | Local media index and thumbnail cache |
| `assets\screenshots\` | Screenshots used in this README |
| `scripts\` | Portable packaging and WebView2 screenshot helpers |

## Legal

- Klipparkiv is a GUI. It does not download anything by itself — it runs
  [yt-dlp](https://github.com/yt-dlp/yt-dlp) (Unlicense) and
  [FFmpeg](https://ffmpeg.org/legal.html) (LGPL/GPL depending on the build).
- Download only content you own or are allowed to save. Respect the terms of service
  of the sites you use and the copyright of the authors.
- This project is not affiliated with YouTube, Google, yt-dlp or FFmpeg.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Bug reports and feature requests are welcome
through the issue templates.

## License

[MIT](LICENSE) © Klipparkiv contributors

---

## По-русски

**Klipparkiv** — портативное GUI для Windows поверх `yt-dlp`: MP3 320k, MP4 до 2160p,
клипы по таймкоду, пакетная загрузка из списка и библиотека скачанного с обложками.

Приложение не устанавливается: распакуйте zip, запустите `klipparkiv.exe`, положите
`bin\` рядом. Настройки и индекс библиотеки хранятся в `data\`, медиа — в `audio\`
и `video\` внутри той же папки. Реестр и профиль пользователя не затрагиваются.

Скачайте свежую сборку на странице [Releases](https://github.com/xmalinov/klipparkiv/releases).

Скачивайте только тот контент, на который у вас есть права.
