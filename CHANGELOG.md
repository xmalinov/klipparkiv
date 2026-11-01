# Changelog / История изменений

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/).

**[English](#english)** · **[Русский](#русский)**

---

## English

### [1.1.0] — 2026-11-01

#### Added

- Russian and English interface with a language switch in Settings; the choice is
  saved in `data\settings.json` and also localizes notifications and the tray menu.
- Localized number, size, speed, date and plural formats in both languages.
- Bilingual changelog: the English section stays in English, the Russian one in
  Russian.

### [1.0.0] — 2026-11-01

First public release.

#### Added

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

## Русский

### [1.1.0] — 2026-11-01

#### Добавлено

- Русский и английский интерфейс с переключателем в настройках; выбор сохраняется
  в `data\settings.json` и применяется также к уведомлениям и меню в трее.
- Локализованные форматы чисел, размеров, скорости, дат и склонений в обоих языках.
- Двуязычный changelog: английский раздел остаётся на английском, русский — на
  русском.

### [1.0.0] — 2026-11-01

Первый публичный релиз.

#### Добавлено

- Портативный интерфейс для `yt-dlp` под Windows (Tauri 2 + React 19).
- Загрузка аудио в MP3 (128–320 кбит/с) с обложкой и метаданными.
- Загрузка видео в MP4 (480p–2160p).
- Режим «Аудио + видео»: два файла по одной ссылке, в разных архивах.
- Клипы по таймкоду для аудио и видео.
- Пакетная загрузка из списка `.txt` со счётчиками по каждому элементу.
- Очередь с настраиваемым параллелизмом (1–4), паузой, продолжением и отменой.
- Предпросмотр метаданных (название, канал, длительность, обложка, форматы)
  перед загрузкой.
- Архив загрузок, чтобы пропускать уже сохранённые элементы.
- Библиотека: обложки, поиск, фильтры, сортировка, запуск в системном плеере,
  показ в проводнике.
- Обновление `yt-dlp` в один клик.
- Уведомление Windows по завершении пакета.
- Портативная структура: настройки и индекс в `data\`, медиа в `audio\` и `video\`.

[1.1.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.1.0
[1.0.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.0.0
