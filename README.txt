Klipparkiv 1.7.0 - portable build for Windows
================================================

English
-------

What this is
  A desktop library for backing tracks, lessons and timecode clips. It runs
  yt-dlp and FFmpeg locally. It does not host or distribute media.

How to run
  1. Unpack the whole folder (do not run the exe from inside the archive).
  2. Run klipparkiv.exe.
  3. Paste a link to media you own or are allowed to save.
  4. Choose Audio, Video or Clip, pick a folder, then press Download.

What is inside
  klipparkiv.exe          the application
  klipparkiv-updater.exe  applies downloaded updates on restart
  bin\                    yt-dlp.exe, ffmpeg.exe, ffprobe.exe (do not delete)
  data\                   settings, library index, covers, logs
  audio\                  default audio folder
  video\                  default video folder
  LICENSE                 MIT license for Klipparkiv
  THIRD_PARTY_NOTICES.md  licenses and sources for bundled components
  LICENSES\               full license texts
  PRIVACY.md              network and local-data behaviour
  SECURITY.md             how to report a vulnerability

Data and portability
  The app keeps its own settings, library index and media in this folder:
  data\settings.json, data\library.db, audio\ and video\. To move it, copy the
  whole folder. To remove it, delete the folder.

  WebView2, the system component that renders the interface, may keep its own
  cache in %LOCALAPPDATA%. That cache is not part of your library.

Permissions
  The folder must be writable. On CD/DVD or in a protected directory the app
  reports an error instead of writing to the system profile.

WebView2
  Microsoft Edge WebView2 Runtime is required. It is usually present on
  Windows 10/11, but not guaranteed on every edition. If it is missing:
  https://developer.microsoft.com/microsoft-edge/webview2/

Updating the binaries
  yt-dlp changes often. Replace bin\yt-dlp.exe with a fresh build from
  https://github.com/yt-dlp/yt-dlp/releases, or use Settings -> Update yt-dlp.
  Settings and the library are preserved.

Updating the app
  When a new release is published, a banner appears in the window. Click
  "Update", wait for the download, then "Restart and update". Only the program
  is replaced: data\, audio\ and video\ stay in place.

Limitations
  - This build is for Windows 10/11 x64.
  - FAT32 drives cannot store files over 4 GB; use exFAT or NTFS.
  - The unsigned exe may trigger SmartScreen: More info -> Run anyway.
  - Video is MP4 H.264/AAC when the source provides it. If H.264 is missing,
    the best available stream is used; some players may not open that fallback.

Legal
  Use Klipparkiv only for media you own, have permission to save, or that is
  offered under a license that allows downloading. Respect the source site's
  terms of service and copyright law. Klipparkiv does not host content and is
  not affiliated with YouTube, Google, yt-dlp or FFmpeg.

  Klipparkiv is MIT-licensed. Bundled yt-dlp and FFmpeg keep their own licenses;
  see THIRD_PARTY_NOTICES.md and LICENSES\. There is no telemetry and there are
  no analytics or cookies.

Security
  Report vulnerabilities privately through the repository's Security tab. See
  SECURITY.md. Do not include cookies or tokens in a report.

Русский
-------

Что это
  Настольная библиотека для бэкинг-треков, уроков и клипов по таймкоду.
  Приложение локально запускает yt-dlp и FFmpeg. Оно не хранит и не
  распространяет медиа.

Как запустить
  1. Распакуйте папку целиком (не запускайте exe из архива).
  2. Запустите klipparkiv.exe.
  3. Вставьте ссылку на медиа, которое вам принадлежит или которое разрешено
     сохранять.
  4. Выберите «Аудио», «Видео» или «Клип», укажите папку и нажмите «Скачать».

Что внутри
  klipparkiv.exe          приложение
  klipparkiv-updater.exe  применяет скачанные обновления при перезапуске
  bin\                    yt-dlp.exe, ffmpeg.exe, ffprobe.exe (не удалять)
  data\                   настройки, индекс библиотеки, обложки, логи
  audio\                  аудио по умолчанию
  video\                  видео по умолчанию
  LICENSE                 лицензия MIT для Klipparkiv
  THIRD_PARTY_NOTICES.md  лицензии и исходники встроенных компонентов
  LICENSES\               полные тексты лицензий
  PRIVACY.md              сеть и локальные данные
  SECURITY.md             как сообщить об уязвимости

Данные и портативность
  Приложение хранит свои настройки, индекс библиотеки и медиа в этой папке:
  data\settings.json, data\library.db, audio\ и video\. Чтобы перенести —
  скопируйте папку целиком. Чтобы удалить — удалите папку.

  WebView2, системный компонент отрисовки интерфейса, может держать свой кэш в
  %LOCALAPPDATA%. Этот кэш не является вашей библиотекой.

Права
  Папка должна быть доступна для записи. На CD/DVD или в защищённом каталоге
  приложение сообщит об ошибке, а не будет писать в системный профиль.

WebView2
  Нужен Microsoft Edge WebView2 Runtime. Обычно он есть в Windows 10/11, но не
  гарантирован в каждой редакции. Если его нет:
  https://developer.microsoft.com/microsoft-edge/webview2/

Обновление бинарников
  yt-dlp обновляется часто. Замените bin\yt-dlp.exe на свежий с
  https://github.com/yt-dlp/yt-dlp/releases или используйте
  «Настройки» -> «Обновить yt-dlp». Настройки и библиотека сохраняются.

Обновление приложения
  Когда выходит новая версия, в окне появляется баннер. Нажмите «Обновить»,
  дождитесь загрузки, затем «Перезапустить и обновить». Заменяется только
  программа: data\, audio\ и video\ остаются на месте.

Ограничения
  - Эта сборка для Windows 10/11 x64.
  - На FAT32 файлы больше 4 ГБ не поместятся; используйте exFAT или NTFS.
  - Неподписанный exe может вызвать SmartScreen: «Подробнее» -> «Выполнить».
  - Видео — MP4 H.264/AAC, если источник его отдаёт. Если H.264 нет, берётся
    лучший доступный поток; некоторые плееры могут его не открыть.

Правовое
  Используйте Klipparkiv только для медиа, которое вам принадлежит, которое
  разрешено сохранять или которое распространяется по лицензии, допускающей
  скачивание. Соблюдайте условия сервисов и авторские права. Klipparkiv не
  хранит контент и не связан с YouTube, Google, yt-dlp или FFmpeg.

  Klipparkiv распространяется по MIT. Встроенные yt-dlp и FFmpeg сохраняют свои
  лицензии; см. THIRD_PARTY_NOTICES.md и LICENSES\. Телеметрии, аналитики и
  cookies нет.

Безопасность
  Об уязвимостях сообщайте приватно через вкладку Security репозитория. См.
  SECURITY.md. Не прикладывайте cookies и токены.
