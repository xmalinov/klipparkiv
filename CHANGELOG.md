# Changelog / История изменений

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/).

**[English](#english)** · **[Русский](#русский)**

---

## English

### [Unreleased]

#### Added

- macOS build: `.app` in a DMG for Intel and Apple Silicon, ad-hoc signed.
- In-app macOS updates: signed `.app.tar.gz`, bundle replacement, backup and
  rollback, with a signed-DMG fallback when automatic replacement is blocked.
- Platform-specific data layout: Windows stays portable, macOS uses
  `~/Movies/Klipparkiv` and `~/Library/Application Support/Klipparkiv`.
- Managed yt-dlp copy on macOS so `yt-dlp --update` does not break the app
  signature.
- macOS CI and release jobs, DMG packaging, update manifest fragments and
  version checks.
- The version check now also covers `src-tauri/Cargo.lock`.

#### Fixed

- macOS updates exchange the `.app` bundle atomically, so the install path is
  never missing during replacement.
- Cross-platform path handling in tests and thumbnail output templates.
- Process-tree cancellation on Unix (yt-dlp + ffmpeg).
- Opening a library item no longer depends on the opener plugin ACL: the app
  validates the file against the library root and opens it from Rust.
- Video downloads prefer H.264/AAC in MP4, so QuickTime plays them instead of
  showing audio over a black frame (AV1/VP9 are not decoded by QuickTime).

### [1.2.0] — 2026-11-01

#### Added

- In-app update banner for the application itself.
- Signed single-exe updates with checksum and version binding.
- Updates wait for active downloads to finish before restarting.
- Automatic signature check against the embedded public key in the release workflow.

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

### [Unreleased]

#### Добавлено

- Сборка для macOS: `.app` в DMG для Intel и Apple Silicon, ad-hoc подпись.
- Обновления macOS из приложения: подписанный `.app.tar.gz`, замена бандла,
  backup и rollback, а также fallback через подписанный DMG.
- Платформенные пути: Windows остаётся портативным, macOS использует
  `~/Movies/Klipparkiv` и `~/Library/Application Support/Klipparkiv`.
- Управляемая копия yt-dlp на macOS, чтобы `yt-dlp --update` не ломал подпись.
- macOS CI и release jobs, сборка DMG, фрагменты манифеста и проверка версий.
- Проверка версии теперь учитывает `src-tauri/Cargo.lock`.

#### Исправлено

- Обновление macOS меняет `.app` атомарно, поэтому путь установки не исчезает
  во время замены.
- Кроссплатформенные пути в тестах и шаблонах миниатюр.
- Отмена дерева процессов на Unix (yt-dlp + ffmpeg).
- Открытие файла из библиотеки больше не зависит от ACL плагина opener:
  приложение проверяет путь внутри корня библиотеки и открывает файл из Rust.
- Видео скачивается в H.264/AAC (MP4), поэтому QuickTime показывает картинку,
  а не только звук (AV1/VP9 QuickTime не декодирует).

### [1.2.0] — 2026-11-01

#### Добавлено

- Баннер обновления самого приложения в окне.
- Подписанные обновления одним exe с проверкой контрольной суммы и версии.
- Обновление ждёт завершения активных загрузок перед перезапуском.
- Автоматическая проверка подписи по встроенному публичному ключу в release-workflow.

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

[1.2.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.2.0
[1.1.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.1.0
[1.0.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.0.0
