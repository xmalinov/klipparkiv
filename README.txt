Klipparkiv 1.2 — portable build / портативная сборка
======================================================

English
-------

How to run
1. Unpack the whole folder (do not run the exe from inside the archive).
2. Run klipparkiv.exe.
3. Paste a YouTube link and click "Download".

What is inside
  klipparkiv.exe  the application
  klipparkiv-updater.exe  applies downloaded updates on restart
  bin\            yt-dlp.exe, ffmpeg.exe, ffprobe.exe (do not delete)
  data\           settings, library index, covers, logs
  audio\          default audio folder
  video\          default video folder

Portability
The app does not install anything and does not write to the registry. Everything
is stored in this folder: settings in data\settings.json, the library in
data\library.db. To move it, copy the whole folder. To remove it, delete the folder.

Permissions
The folder must be writable. On CD/DVD or in a protected directory the app reports
an error instead of writing to the system profile.

WebView2
Microsoft Edge WebView2 Runtime is required (preinstalled on Windows 10/11).
If it is missing: https://developer.microsoft.com/microsoft-edge/webview2/

Updating the binaries
yt-dlp is updated often (YouTube changes). Replace bin\yt-dlp.exe with a fresh
build from https://github.com/yt-dlp/yt-dlp/releases — settings and the library
are preserved. You can also update from the app: Settings → Update yt-dlp.

Updating the app
When a new release is published, a banner appears in the window. Click "Update",
wait for the download, then "Restart and update". Only the program is replaced:
data\, audio\ and video\ are preserved. If downloads are running, the update
waits for them to finish before restarting.

Limitations
- Windows 10/11 x64 only.
- FAT32 flash drive: files over 4 GB will not fit, use exFAT/NTFS.
- The unsigned exe may trigger a SmartScreen warning: More info → Run anyway.
  This is normal for an unsigned portable build.

License
MIT. Not affiliated with YouTube, Google, yt-dlp or FFmpeg. Download only content
you own or are allowed to save.

Русский
-------

Как запустить
1. Распакуйте папку целиком (не запускайте exe из архива).
2. Запустите klipparkiv.exe.
3. Вставьте ссылку на YouTube и нажмите «Скачать».

Что внутри
  klipparkiv.exe  приложение
  klipparkiv-updater.exe  применяет скачанные обновления при перезапуске
  bin\            yt-dlp.exe, ffmpeg.exe, ffprobe.exe (не удалять)
  data\           настройки, индекс библиотеки, обложки, логи
  audio\          аудио по умолчанию
  video\          видео по умолчанию

Портативность
Приложение не устанавливается и не пишет в реестр. Всё хранится в этой папке:
настройки — data\settings.json, библиотека — data\library.db.
Перенос на другой диск или флешку: скопируйте папку целиком.
Удаление: удалите папку.

Права
Папка должна быть доступна для записи. На CD/DVD или в защищённом каталоге
приложение сообщит об ошибке, а не будет писать в системный профиль.

WebView2
Нужен Microsoft Edge WebView2 Runtime (есть в Windows 10/11 по умолчанию).
Если его нет: https://developer.microsoft.com/microsoft-edge/webview2/

Обновление бинарников
yt-dlp обновляется часто (YouTube меняется). Замените bin\yt-dlp.exe на свежий
с https://github.com/yt-dlp/yt-dlp/releases — настройки и библиотека не пострадают.
Обновить можно и из приложения: «Настройки» → «Обновить yt-dlp».

Обновление приложения
Когда выходит новая версия, в окне появляется баннер. Нажмите «Обновить»,
дождитесь загрузки, затем «Перезапустить и обновить». Заменяется только
программа: data\, audio\ и video\ остаются на месте. Если идут загрузки,
обновление дождётся их завершения и только потом перезапустит приложение.

Ограничения
- Только Windows 10/11 x64.
- Флешка в FAT32: файл больше 4 ГБ не поместится, используйте exFAT/NTFS.
- Неподписанный exe может вызвать предупреждение SmartScreen: «Подробнее» →
  «Выполнить в любом случае». Это нормально для portable-сборки без подписи.

Лицензия
MIT. Проект не связан с YouTube, Google, yt-dlp или FFmpeg.
Скачивайте только тот контент, на который у вас есть права.
