# Changelog / История изменений

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres
to [Semantic Versioning](https://semver.org/).

**[English](#english)** · **[Русский](#русский)**

---

## English

### [1.7.1] — 2026-11-12

Local media import release.

#### Added

- Drag audio and video files from Explorer/Finder into the library; they are
  copied into `audio/` or `video/` and indexed immediately.
- Drag into Practice to copy and open the first audio file paused; grid analysis
  starts as usual and playback stays manual.
- "Add files…" dialog in Library and Practice.
- Import progress with Cancel and `Esc`, a per-file summary, and a hint when a
  folder is dropped.
- Title and artist from ID3 tags, embedded cover art for audio, poster frames
  for imported videos.
- Duplicate detection by size and SHA-256, conflict-safe `Name (2).ext` names.
- An `origin` marker so a full rescan keeps imported titles, artists and covers.

#### Changed

- Drag and drop now uses Tauri's native handler, required to receive file paths;
  the Download screen reads dropped `.txt` lists through the backend.
- `ogg` is accepted as an audio extension.

#### Notes

- Dragging a URL as plain text into Download no longer works on Windows; use the
  Paste button or `Ctrl+V`.

### [1.7.0] — 2026-11-11

Practice player release with a local beat grid. Download and library behaviour is
largely unchanged.

#### Added

- Built-in audio practice player: waveform with zoom, click-to-seek and
  drag selection.
- Speed 0.25–2.0 without pitch change, and pitch ±12 semitones plus cents,
  independent of speed.
- A-B loops and named regions (intro, verse, chorus, solo, bridge, outro,
  custom) with a 2 s pre-roll or a one-bar count-in.
- Short loops play seamlessly from memory; loops longer than 2 minutes restart
  the decoder with a short gap.
- BPM and beat-grid analysis with the bundled Beat This! small model; results
  are cached in `data\analysis`.
- Bar grid with 2/3/4/6/8 beats per bar, bar-start correction, half/double
  tempo correction, and a manual BPM fallback.
- Beat/bar-snapped loop creation (1/2/4/8 bars), snap-to-beat/bar/free dragging,
  and a metronome with downbeat accents and count-in.
- Keyboard shortcuts, a help sheet, a mini-player and a recent-tracks list.
- Per-track state (position, speed, pitch, regions, active loop) in SQLite.
  State is reset when the file changes on disk.
- Volume control 0–100%, saved with the track.
- Explicit A/B buttons and per-region loop/edit/delete actions.
- Pitch uses the musician-friendly ♭/♯ buttons.
- Waveform peaks cached in `data\waveforms`.
- Russian and English UI for the practice screen.

#### Changed

- Double-clicking an audio item opens Practice. Video items and the system
  player behave as before.
- The library context menu has "Open in Practice" for audio.
- Library results load in pages of 200 and append on scroll, so libraries with
  thousands of items are fully reachable without rendering everything at once.

#### Fixed

- Practice uses a single centered column: controls, regions and recent tracks
  stack below the waveform instead of leaving a large empty area.
- Custom speed can be typed (0.25–2.0, step 0.05) next to the presets.
- Waveform zoom is anchored to the cursor and no longer jumps back.
- Region and selection edges can be dragged; the selection can be moved, and a
  click inside it seeks.
- Recent tracks can be removed from the recent list without deleting them from
  the library or losing their practice state.
- The portable-mode label is vertically centered with the settings button, and
  library cards are no longer clipped at the top or left edge.
- Clicking a region now also moves the playhead, so the position cursor can be
  placed anywhere even with an active region.
- The active region and its loop button are highlighted; switching works by
  clicking a row or pressing 1–9.
- Empty region and recent lists align with their panel headers.
- Region and recent lists no longer scroll internally: the whole Practice
  screen scrolls when the window is too short.
- Removed the redundant "st" suffix from the pitch value.
- Opening a track no longer leaves the previous waveform on screen: Practice
  shows an "Opening track…" preloader, pauses the previous session and hides
  old analysis until the new track is ready.
- ffmpeg/ffprobe child processes no longer flash console windows on Windows
  when opening audio, building a waveform or analysing the beat grid.
- The download queue keeps the last known speed and ETA when a progress update
  omits them, so the speed column does not go blank.
- Practice now uses the full content width instead of being capped at 1100 px.
- Snap mode defaults to beat instead of bar; saved v1/v2 tracks migrate to beat,
  while an explicit bar choice in v3 is preserved.
- Reopening a track while its beat-grid analysis is still running no longer
  resets the progress to 0; the existing job reports its current progress.

#### Notes

- Audio only in this release; video, chord and note analysis are not part of
  1.7.0. BPM/beat-grid analysis and the metronome are local, CPU-only features.
- The audio engine is native (ffmpeg → Signalsmith Stretch → cpal), so the
  practice player is not limited by WebView audio codecs.
- The bundled Beat This! small model adds about 11 MB to the portable build;
  the larger model is not included.

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

### [1.7.1] — 2026-11-12

Релиз импорта локальных файлов.

#### Добавлено

- Перетаскивание аудио и видео из проводника/Finder в библиотеку: файлы
  копируются в `audio/` или `video/` и сразу попадают в индекс.
- Перетаскивание в практику: файл копируется и открывается на паузе; анализ
  сетки запускается как обычно, воспроизведение остаётся ручным.
- Кнопка «Добавить файлы…» в библиотеке и практике.
- Прогресс импорта с отменой по кнопке и `Esc`, сводка по файлам, подсказка
  при перетаскивании папки.
- Название и исполнитель из тегов, встроенная обложка аудио, постер для
  импортированного видео.
- Поиск дубликатов по размеру и SHA-256, безопасные имена `Имя (2).ext`.
- Признак `origin`, чтобы полный скан не сбрасывал импортированные названия,
  исполнителей и обложки.

#### Изменено

- Drag and drop переведён на нативный обработчик Tauri (иначе недоступны пути
  файлов); раздел «Загрузки» читает перетащенные `.txt` через бэкенд.
- `ogg` добавлен как аудиоформат.

#### Примечания

- Перетаскивание URL текстом в «Загрузки» больше не работает на Windows:
  используйте кнопку «Вставить» или `Ctrl+V`.

### [1.7.0] — 2026-11-11

Релиз практического плеера с локальной сеткой долей. Поведение загрузок и
библиотеки в основном не изменилось.

#### Добавлено

- Встроенный практический аудиоплеер: волна с зумом, перемотка кликом и
  выделение фрагмента мышью.
- Скорость 0.25–2.0 без изменения тона и тон ±12 полутонов плюс центы,
  независимо от скорости.
- A-B лупы и именованные регионы (интро, куплет, припев, соло, бридж, аутро,
  своё) с предроллом 2 с или отсчётом на один такт.
- Короткие лупы играют бесшовно из памяти; лупы длиннее 2 минут перезапускают
  декодер с коротким разрывом.
- Анализ BPM и сетки долей на встроенной малой модели Beat This!; результат
  кэшируется в `data\analysis`.
- Сетка тактов 2/3/4/6/8 долей, сдвиг первого такта, коррекция половинного и
  двойного темпа, ручной BPM как запасной вариант.
- Лупы по тактам (1/2/4/8), привязка перетаскивания к доле/такту или свободно,
  метроном с акцентом на сильную долю и отсчёт.
- Горячие клавиши, шпаргалка, мини-плеер и список недавних треков.
- Состояние на трек (позиция, скорость, тон, регионы, активный луп) в SQLite.
  При изменении файла на диске состояние сбрасывается.
- Громкость 0–100%, сохраняется вместе с треком.
- Явные кнопки A/B и действия региона «луп / изменить / удалить».
- Тон через удобные музыканту кнопки ♭/♯.
- Кэш пиков волны в `data\waveforms`.
- Русский и английский интерфейс практики.

#### Изменено

- Двойной клик по аудио открывает Practice. Видео и системный плеер работают
  как раньше.
- В контекстном меню библиотеки появился пункт «Открыть в практике».
- Библиотека загружает результаты по 200 и добавляет следующие при прокрутке,
  поэтому тысячи элементов доступны без отрисовки всего списка сразу.

#### Исправлено

- Practice теперь одна центрированная колонка: контролы, регионы и недавние
  треки идут под волной, без большого пустого места.
- Свою скорость можно ввести вручную (0.25–2.0, шаг 0.05) рядом с пресетами.
- Зум волны привязан к курсору и больше не сбрасывается.
- Границы регионов и выделения можно перетаскивать; выделение можно двигать,
  клик внутри — перемотка.
- Трек можно убрать из недавних, не удаляя его из библиотеки и не теряя
  состояние практики.
- Подпись «портативный режим» выровнена по центру с шестерёнкой, а карточки
  библиотеки больше не обрезаются сверху и слева.
- Клик по региону теперь двигает оранжевый курсор, поэтому позицию можно
  поставить в любое место даже при активном регионе.
- Активный регион и его кнопка лупа подсвечиваются; переключение — клик по
  строке или клавишами 1–9.
- Пустые списки регионов и недавних выровнены по заголовкам панелей.
- Списки регионов и недавних больше не скроллятся внутри: при маленьком окне
  прокручивается весь экран Practice.
- Убрал лишний суффикс «пт» у значения тона.
- При открытии трека больше не остаётся волна предыдущего трека: Practice
  показывает прелоадер «Открываю трек…», ставит прошлую сессию на паузу и
  скрывает старый анализ до готовности нового.
- Дочерние ffmpeg/ffprobe больше не показывают окна терминала на Windows при
  открытии аудио, построении волны и анализе сетки.
- Очередь загрузок сохраняет последнюю известную скорость и ETA, если
  очередное обновление прогресса их не содержит; колонка скорости не пустеет.
- Practice теперь занимает всю ширину контента, а не ограничен 1100 px.
- Привязка по умолчанию — к доле, а не к такту; сохранённые треки v1/v2
  переводятся на доли, явный выбор «к такту» в v3 сохраняется.
- Повторное открытие трека во время построения сетки больше не сбрасывает
  прогресс в 0: уже запущенная задача сообщает текущий прогресс.

#### Примечания

- В этом релизе только аудио; видео, аккорды и ноты в 1.7.0 не входят.
  Анализ BPM и сетки долей, а также метроном работают локально на CPU.
- Аудиодвижок нативный (ffmpeg → Signalsmith Stretch → cpal), поэтому плеер
  не зависит от аудиокодеков WebView.
- Встроенная малая модель Beat This! добавляет около 11 МБ к портативной
  сборке; большая модель не входит.

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

[1.7.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.7.0
[1.6.2]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.6.2
[1.6.1]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.6.1
[1.6.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.6.0
[1.3.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.3.0
[1.2.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.2.0
[1.1.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.1.0
[1.0.0]: https://github.com/xmalinov/klipparkiv/releases/tag/v1.0.0
