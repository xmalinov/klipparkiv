import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { getSettings } from "../api/settings";

export type Language = "ru" | "en";

export const ru = {
  "nav.download": "Загрузка",
  "nav.library": "Библиотека",
  "app.portableMode": "портативный режим",
  "app.settings": "Настройки",
  "app.environmentError": "Не удалось прочитать окружение: {error}",
  "app.notWritable": "Нет прав на запись в папку приложения",
  "app.missingBinaries": "Не найдены в bin\\: {names}",

  "common.audio": "Аудио",
  "common.video": "Видео",
  "common.cancel": "Отмена",
  "common.save": "Сохранить",
  "common.close": "Закрыть",
  "common.notFound": "не найден",
  "common.default": "По умолчанию",
  "common.change": "Изменить",
  "common.remove": "Убрать",

  "download.mode.audio": "Аудио",
  "download.mode.video": "Видео",
  "download.mode.both": "Аудио+Видео",
  "download.mode.clip": "Клип",
  "download.quality.best": "Лучшее",
  "download.bestQuality": "лучшее качество",
  "download.hint.audio": "MP3 {kbps} kbps · обложка и метаданные внутри файла",
  "download.hint.video": "MP4 {quality} · слияние в один файл",
  "download.hint.both":
    "MP3 {kbps} kbps + MP4 {quality} · две задачи, два архива",
  "download.hint.clipAudio": "Фрагмент · MP3 {kbps} kbps",
  "download.hint.clipVideo": "Фрагмент · MP4 {quality}",
  "download.task.audio": "Аудио",
  "download.task.video": "Видео",
  "download.task.clip": "{base} · клип",
  "download.placeholder": "Вставьте ссылку на YouTube или введите её вручную",
  "download.urlLabel": "Ссылка на видео",
  "download.paste": "Вставить",
  "download.clipboardError":
    "Буфер обмена недоступен — вставьте ссылку через Ctrl+V",
  "download.modeGroup": "Режим загрузки",
  "download.audioQuality": "Качество аудио",
  "download.videoQuality": "Качество видео",
  "download.batch": "Из файла со ссылками…",
  "download.batchFilter": "Списки ссылок",
  "download.clipKind": "Тип клипа",
  "download.clipStart": "Начало",
  "download.clipEnd": "Конец",
  "download.clipStartLabel": "Начало клипа",
  "download.clipEndLabel": "Конец клипа",
  "download.probing": "Проверяем ссылку…",
  "download.coverCached": "обложка в кэше",
  "download.folder": "Папка:",
  "download.outsideRoot":
    "Папка вне приложения — файл не попадёт в библиотеку",
  "download.submit": "Скачать",
  "download.queueLabel": "Очередь загрузок",
  "download.queue": "Очередь",
  "download.pause": "Пауза",
  "download.resume": "Продолжить",
  "download.queueEmpty": "Здесь появятся загрузки",
  "download.colName": "Название",
  "download.colStatus": "Статус",
  "download.colProgress": "Прогресс",
  "download.colSpeed": "Скорость",
  "download.cancelTask": "Отменить {url}",
  "download.cancel": "Отменить",
  "download.removeTask": "Убрать",

  "library.sort.dateDesc": "Сначала новые",
  "library.sort.dateAsc": "Сначала старые",
  "library.sort.title": "По названию",
  "library.sort.duration": "По длительности",
  "library.sort.size": "По размеру",
  "library.sort.channel": "По каналу",
  "library.openError": "Не удалось открыть файл: {error}",
  "library.searchPlaceholder": "Поиск по названию или каналу…",
  "library.searchLabel": "Поиск в библиотеке",
  "library.kindGroup": "Тип медиа",
  "library.all": "Все",
  "library.sortLabel": "Сортировка",
  "library.viewLabel": "Вид",
  "library.grid": "Плитка",
  "library.list": "Список",
  "library.scan": "Сканировать сейчас",
  "library.empty": "Здесь появятся скачанные файлы",
  "library.noResults": "Ничего не найдено. Измените запрос или фильтр.",
  "library.colTitle": "Название",
  "library.colType": "Тип",
  "library.colDuration": "Длительность",
  "library.colSize": "Размер",
  "library.colDate": "Дата",
  "library.colQuality": "Качество",
  "library.menu.play": "Воспроизвести",
  "library.menu.reveal": "Показать в проводнике",
  "library.menu.copyPath": "Копировать путь",
  "library.menu.remove": "Убрать из библиотеки",
  "library.scanning": "Сканирование…",
  "library.filesCount": "Файлов: {count}",

  "settings.title": "Настройки",
  "settings.tab.general": "Общие",
  "settings.tab.library": "Библиотека",
  "settings.tab.updates": "Обновления",
  "settings.tab.about": "О программе",
  "settings.loading": "Загрузка настроек…",
  "settings.audioSubdir": "Подпапка для аудио",
  "settings.videoSubdir": "Подпапка для видео",
  "settings.concurrency": "Одновременных загрузок",
  "settings.theme": "Тема",
  "settings.theme.system": "Системная",
  "settings.theme.dark": "Тёмная",
  "settings.theme.light": "Светлая",
  "settings.language": "Язык",
  "settings.language.ru": "Русский",
  "settings.language.en": "English",
  "settings.scanDepth": "Глубина сканирования",
  "settings.autoscan": "Сканировать при открытии",
  "settings.updateYtdlp": "Обновление yt-dlp",
  "settings.updating": "Обновляем…",
  "settings.update": "Обновить",
  "settings.updated": "yt-dlp обновлён до {version}",
  "settings.ffmpegHint":
    "ffmpeg и ffprobe заменяются вручную: положите новые exe в bin\\.",
  "settings.appVersion": "Версия приложения",
  "settings.appFolder": "Папка приложения",
  "settings.mode": "Режим",
  "settings.modePortable": "Портативный, без установки",
  "settings.aboutHint":
    "Настройки и библиотека лежат в data\\, ничего не пишется в реестр.",
} as const;

export type TranslationKey = keyof typeof ru;

export const en: Record<TranslationKey, string> = {
  "nav.download": "Download",
  "nav.library": "Library",
  "app.portableMode": "portable mode",
  "app.settings": "Settings",
  "app.environmentError": "Failed to read the environment: {error}",
  "app.notWritable": "The app folder is not writable",
  "app.missingBinaries": "Not found in bin\\: {names}",

  "common.audio": "Audio",
  "common.video": "Video",
  "common.cancel": "Cancel",
  "common.save": "Save",
  "common.close": "Close",
  "common.notFound": "not found",
  "common.default": "Default",
  "common.change": "Change",
  "common.remove": "Remove",

  "download.mode.audio": "Audio",
  "download.mode.video": "Video",
  "download.mode.both": "Audio+Video",
  "download.mode.clip": "Clip",
  "download.quality.best": "Best",
  "download.bestQuality": "best quality",
  "download.hint.audio": "MP3 {kbps} kbps · cover art and tags inside the file",
  "download.hint.video": "MP4 {quality} · merged into one file",
  "download.hint.both":
    "MP3 {kbps} kbps + MP4 {quality} · two tasks, two archives",
  "download.hint.clipAudio": "Clip · MP3 {kbps} kbps",
  "download.hint.clipVideo": "Clip · MP4 {quality}",
  "download.task.audio": "Audio",
  "download.task.video": "Video",
  "download.task.clip": "{base} · clip",
  "download.placeholder": "Paste a YouTube link or type it manually",
  "download.urlLabel": "Video link",
  "download.paste": "Paste",
  "download.clipboardError":
    "Clipboard is unavailable — paste the link with Ctrl+V",
  "download.modeGroup": "Download mode",
  "download.audioQuality": "Audio quality",
  "download.videoQuality": "Video quality",
  "download.batch": "From a link list…",
  "download.batchFilter": "Link lists",
  "download.clipKind": "Clip type",
  "download.clipStart": "Start",
  "download.clipEnd": "End",
  "download.clipStartLabel": "Clip start",
  "download.clipEndLabel": "Clip end",
  "download.probing": "Checking the link…",
  "download.coverCached": "cover cached",
  "download.folder": "Folder:",
  "download.outsideRoot":
    "Folder is outside the app — the file will not appear in the library",
  "download.submit": "Download",
  "download.queueLabel": "Download queue",
  "download.queue": "Queue",
  "download.pause": "Pause",
  "download.resume": "Resume",
  "download.queueEmpty": "Downloads will appear here",
  "download.colName": "Name",
  "download.colStatus": "Status",
  "download.colProgress": "Progress",
  "download.colSpeed": "Speed",
  "download.cancelTask": "Cancel {url}",
  "download.cancel": "Cancel",
  "download.removeTask": "Remove",

  "library.sort.dateDesc": "Newest first",
  "library.sort.dateAsc": "Oldest first",
  "library.sort.title": "By title",
  "library.sort.duration": "By duration",
  "library.sort.size": "By size",
  "library.sort.channel": "By channel",
  "library.openError": "Could not open the file: {error}",
  "library.searchPlaceholder": "Search by title or channel…",
  "library.searchLabel": "Search the library",
  "library.kindGroup": "Media type",
  "library.all": "All",
  "library.sortLabel": "Sort",
  "library.viewLabel": "View",
  "library.grid": "Grid",
  "library.list": "List",
  "library.scan": "Scan now",
  "library.empty": "Downloaded files will appear here",
  "library.noResults": "Nothing found. Change the query or filter.",
  "library.colTitle": "Title",
  "library.colType": "Type",
  "library.colDuration": "Duration",
  "library.colSize": "Size",
  "library.colDate": "Date",
  "library.colQuality": "Quality",
  "library.menu.play": "Play",
  "library.menu.reveal": "Show in File Explorer",
  "library.menu.copyPath": "Copy path",
  "library.menu.remove": "Remove from library",
  "library.scanning": "Scanning…",
  "library.filesCount": "Files: {count}",

  "settings.title": "Settings",
  "settings.tab.general": "General",
  "settings.tab.library": "Library",
  "settings.tab.updates": "Updates",
  "settings.tab.about": "About",
  "settings.loading": "Loading settings…",
  "settings.audioSubdir": "Audio subfolder",
  "settings.videoSubdir": "Video subfolder",
  "settings.concurrency": "Simultaneous downloads",
  "settings.theme": "Theme",
  "settings.theme.system": "System",
  "settings.theme.dark": "Dark",
  "settings.theme.light": "Light",
  "settings.language": "Language",
  "settings.language.ru": "Русский",
  "settings.language.en": "English",
  "settings.scanDepth": "Scan depth",
  "settings.autoscan": "Scan on startup",
  "settings.updateYtdlp": "yt-dlp update",
  "settings.updating": "Updating…",
  "settings.update": "Update",
  "settings.updated": "yt-dlp updated to {version}",
  "settings.ffmpegHint":
    "ffmpeg and ffprobe are replaced manually: put the new exe files in bin\\.",
  "settings.appVersion": "App version",
  "settings.appFolder": "App folder",
  "settings.mode": "Mode",
  "settings.modePortable": "Portable, no installation",
  "settings.aboutHint":
    "Settings and the library live in data\\; nothing is written to the registry.",
};

const dictionaries: Record<Language, Record<TranslationKey, string>> = {
  ru,
  en,
};

export type Translate = (
  key: TranslationKey,
  params?: Record<string, string | number>,
) => string;

export function translate(
  language: Language,
  key: TranslationKey,
  params?: Record<string, string | number>,
): string {
  const template = dictionaries[language][key] ?? ru[key];
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    const value = params?.[name];
    return value === undefined ? match : String(value);
  });
}

const fallback = {
  language: "ru" as Language,
  setLanguage: (_language: Language) => {},
  t: ((key, params) => translate("ru", key, params)) as Translate,
};

const I18nContext = createContext(fallback);

export function I18nProvider({
  children,
  initialLanguage = "ru",
}: {
  children: ReactNode;
  initialLanguage?: Language;
}) {
  const [language, setLanguageState] = useState<Language>(initialLanguage);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((settings) => {
        if (!alive) return;
        if (settings.language === "ru" || settings.language === "en") {
          setLanguageState(settings.language);
        }
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);

  const setLanguage = useCallback((next: Language) => {
    setLanguageState(next);
  }, []);

  const t = useMemo<Translate>(
    () => (key, params) => translate(language, key, params),
    [language],
  );

  const value = useMemo(
    () => ({ language, setLanguage, t }),
    [language, setLanguage, t],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  return useContext(I18nContext);
}
