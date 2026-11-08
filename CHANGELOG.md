# Changelog / История изменений

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/).

**[English](#english)** · **[Русский](#русский)**

---

## English

### [1.6.2] — 2026-11-09

Diagnostics release. Download and library behaviour is unchanged.

#### Changed

- Startup now reports every missing or broken binary separately, with the full
  path, the OS error and a likely cause. The warning names the `bin` folder and
  explains how to fix it.
- A failed `yt-dlp` launch shows the executable path and a hint for the specific
  error: missing file (2), missing folder (3), antivirus block (5/225), wrong
  architecture (193) or a too-long path (206). The same applies to the metadata
  probe, the download queue and `yt-dlp --update`.
- Settings → Updates lists the error and path for every binary whose version
  could not be read.

#### Fixed

- A missing `bin` folder is no longer reported only as "not found in bin": the
  UI now distinguishes a missing file from a missing folder or a blocked file.

### [1.6.1] — 2026-11-02

Documentation and release-hygiene release. No download or library behaviour
changed.

#### Changed

- The README, `README.txt` and landing page now lead with the offline-library
  use case and explain which release asset to download and how to start.
- README and landing screenshots were replaced with English and Russian sets,
  a neutral example path and original cover art.
- Security, privacy and third-party notices are documented. `LICENSE`,
  `THIRD_PARTY_NOTICES.md`, `PRIVACY.md` and `LICENSES/` are included in the
  Windows zip and macOS DMG.
- The app document starts with `lang="en"` before the saved interface language
  is loaded.
- macOS declares `minimumSystemVersion` 11.0 explicitly.

#### Fixed

- `CONTRIBUTING.md` no longer claims that UI text is Russian-only.
- Removed unsupported "any site" and "QuickTime" claims; clarified portable data
  locations and the WebView2 cache.

### [1.6.0] — 2026-11-02

#### Added

- Playlist links are detected during the metadata probe. `playlist?list=…` opens as
  "Whole playlist", `watch?…&list=…` defaults to "This video only"; the user can switch.
- Playlist range "From # / to #" with a hard cap of 500 items and a warning above 50.
  Playlist files go to `audio\<playlist title>\` or `video\<playlist title>\`.
- Clip UX: "From start" / "To end" / "Whole video" chips, live timecode validation,
  clip-length hint and two range sliders when the duration is known.
- Multi-link paste panel ("Add all" / "Keep the first") and drag & drop of links or
  `.txt` lists onto the download screen.
- Queue shows the video/playlist title, `playlist · 2/42` counters, and the
  "Already there" and "Partial" states.
- Batch downloads now honor the selected audio bitrate and destination folder.

#### Changed

- Clip files are named `ID - Title [HH-MM-SS-HH-MM-SS].ext` (open end: `[…-end]`);
  the library decodes the new and the legacy names.
- Playlist progress is aggregated across all items instead of showing the current item.

#### Fixed

- A second clip from the same video is no longer silently skipped by the download
  archive: clips bypass `--download-archive` and rely on the deterministic filename.
- Filenames are trimmed (`--trim-filenames 150`) to stay below the Windows path limit.

### [1.3.0] — 2026-11-01

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

### [1.6.2] — 2026-11-09

Диагностический релиз. Поведение загрузок и библиотеки не изменилось.

#### Изменено

- При старте каждый отсутствующий или нерабочий бинарник показывается отдельно:
  полный путь, ошибка Windows и вероятная причина. В предупреждении указана
  папка `bin` и как это исправить.
- При неудачном запуске `yt-dlp` выводятся путь к файлу и подсказка для
  конкретной ошибки: файл не найден (2), папка не найдена (3), блокировка
  антивирусом (5/225), неверная архитектура (193), слишком длинный путь (206).
  То же для проверки метаданных, очереди загрузок и `yt-dlp --update`.
- В «Настройки → Обновления» для каждого бинарника с нечитаемой версией
  показываются ошибка и путь.

#### Исправлено

- Отсутствие папки `bin` больше не сводится к «не найдены в bin»: интерфейс
  отличает отсутствующий файл от отсутствующей папки и заблокированного файла.

### [1.6.1] — 2026-11-02

Релиз документации и юридической гигиены. Поведение загрузок и библиотеки не
изменилось.

#### Изменено

- README, `README.txt` и лендинг теперь начинаются с задачи «офлайн-библиотека»
  и объясняют, какой файл релиза скачивать и с чего начать.
- Скриншоты в README и на лендинге заменены на английский и русский комплекты
  с нейтральным примером пути и собственной обложкой.
- Добавлены политики безопасности, конфиденциальности и уведомления о
  сторонних компонентах. `LICENSE`, `THIRD_PARTY_NOTICES.md`, `PRIVACY.md` и
  `LICENSES/` входят в Windows-zip и macOS-DMG.
- Документ приложения стартует с `lang="en"` до загрузки сохранённого языка
  интерфейса.
- Для macOS явно указан `minimumSystemVersion` 11.0.

#### Исправлено

- `CONTRIBUTING.md` больше не утверждает, что текст интерфейса только русский.
- Убраны неподтверждённые обещания «любой сайт» и «QuickTime»; уточнены
  расположение данных в портативной сборке и кэш WebView2.

### [1.6.0] — 2026-11-02

#### Добавлено

- Ссылки на плейлисты распознаются при проверке метаданных. `playlist?list=…`
  открывается как «Весь плейлист», `watch?…&list=…` — как «Только это видео»
  с возможностью переключить.
- Диапазон «С № / по №» с жёстким пределом 500 видео и предупреждением при списке
  длиннее 50. Файлы плейлиста попадают в `audio\<название плейлиста>\` или
  `video\<название плейлиста>\`.
- Клипы: чипы «С начала» / «До конца» / «Весь ролик», живая проверка таймкодов,
  подсказка длины клипа и два слайдера, когда длительность ролика известна.
- Панель «Список ссылок» («Добавить все» / «Оставить первую») и drag & drop ссылок
  или `.txt`-списков в экран загрузки.
- Очередь показывает название видео/плейлиста, счётчик `плейлист · 2/42` и статусы
  «Уже было» / «Частично».
- Пакетная загрузка учитывает выбранный битрейт аудио и папку назначения.

#### Изменено

- Клипы называются `ID - Название [чч-мм-сс-чч-мм-сс].ext` (открытый конец —
  `[…-end]`); библиотека понимает новые и старые имена.
- Прогресс плейлиста считается по всем видео, а не по текущему элементу.

#### Исправлено

- Второй клип из того же видео больше не пропускается архивом: клипы не используют
  `--download-archive`, а опираются на детерминированное имя файла.
- Имена файлов обрезаются (`--trim-filenames 150`) и не упираются в лимит пути Windows.

### [1.3.0] — 2026-11-01

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

[1.6.2]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.6.2
[1.6.1]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.6.1
[1.6.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.6.0
[1.3.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.3.0
[1.2.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.2.0
[1.1.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.1.0
[1.0.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.0.0
