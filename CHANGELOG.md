# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project adheres to [Semantic Versioning](https://semver.org/).

## [1.0.0] — 2026-11-01

First public release.

### Added

- Portable Windows GUI for `yt-dlp` (Tauri 2 + React 19).
- Audio downloads to MP3 (128–320 kbps) with embedded cover art and metadata.
- Video downloads to MP4 (480p–2160p).
- "Audio + Video" mode: two files from one link, kept in separate archives.
- Timecode clips for both audio and video.
- Batch downloads from a `.txt` list with per-item counters.
- Queue with configurable concurrency (1–4), pause, resume and cancel.
- Metadata preview (title, channel, duration, thumbnail, formats) before download.
- Download archive support to skip already saved items.
- Library screen: covers, search, filters, sorting, open in the system player,
  reveal in Explorer.
- One-click `yt-dlp` update.
- Windows toast notification when a batch finishes.
- Portable layout: settings and index in `data\`, media in `audio\` and `video\`.

[1.0.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.0.0
