import { useEffect, useState } from "react";
import { formatVersion, loadAppInfo, type AppInfo } from "./api/appInfo";
import { applyTheme, getSettings } from "./api/settings";
import DownloadScreen from "./features/download/DownloadScreen";
import LibraryScreen from "./features/library/LibraryScreen";
import SettingsDialog from "./features/settings/SettingsDialog";
import "./styles/tokens.css";
import "./App.css";

type Screen = "download" | "library";

const NAV: { id: Screen; label: string; icon: React.ReactNode }[] = [
  {
    id: "download",
    label: "Загрузка",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12 3v12" />
        <path d="m7 10 5 5 5-5" />
        <path d="M4 20h16" />
      </svg>
    ),
  },
  {
    id: "library",
    label: "Библиотека",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M8 4v16" />
      </svg>
    ),
  },
];

function environmentWarnings(info: AppInfo): string[] {
  const warnings: string[] = [];
  if (!info.writable) warnings.push("Нет прав на запись в папку приложения");
  const missing = [
    ["yt-dlp", info.ytdlp],
    ["ffmpeg", info.ffmpeg],
    ["ffprobe", info.ffprobe],
  ]
    .filter(([, version]) => version === null)
    .map(([name]) => name);
  if (missing.length > 0) {
    warnings.push(`Не найдены в bin\\: ${missing.join(", ")}`);
  }
  return warnings;
}

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<Screen>("download");
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((settings) => {
        if (alive) applyTheme(settings.theme);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    loadAppInfo()
      .then((value) => {
        if (alive) setInfo(value);
      })
      .catch((cause: unknown) => {
        if (alive) setError(cause instanceof Error ? cause.message : String(cause));
      });
    return () => {
      alive = false;
    };
  }, []);

  const warnings = info ? environmentWarnings(info) : [];

  return (
    <div className="app">
      <aside className="sidebar">
        {NAV.map((item) => (
          <button
            key={item.id}
            type="button"
            className={screen === item.id ? "nav active" : "nav"}
            aria-current={screen === item.id ? "page" : undefined}
            onClick={() => setScreen(item.id)}
          >
            {item.icon}
            <span>{item.label}</span>
          </button>
        ))}
      </aside>

      <div className="main">
        <header className="header">
          <h1>{screen === "download" ? "Загрузка" : "Библиотека"}</h1>
          <div className="header-right">
            <span>портативный режим</span>
            <button
              type="button"
              className="icon-btn"
              aria-label="Настройки"
              onClick={() => setSettingsOpen(true)}
            >
              <svg viewBox="0 0 24 24">
                <circle cx="12" cy="12" r="3" />
                <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
              </svg>
            </button>
          </div>
        </header>

        <main className="content">
          {error && (
            <p className="banner" role="alert">
              Не удалось прочитать окружение: {error}
            </p>
          )}
          {warnings.map((warning) => (
            <p className="banner" role="alert" key={warning}>
              {warning}
            </p>
          ))}

          {screen === "download" ? (
            <DownloadScreen root={info?.root} />
          ) : (
            <LibraryScreen />
          )}
        </main>

        <footer className="statusbar">
          <span className="root" title={info?.root}>
            {info?.root ?? ""}
          </span>
          <span>
            {info
              ? `yt-dlp ${formatVersion(info.ytdlp)} · ffmpeg ${formatVersion(info.ffmpeg)}`
              : ""}
          </span>
        </footer>
      </div>

      {settingsOpen && (
        <SettingsDialog
          info={info}
          onClose={() => setSettingsOpen(false)}
          onSaved={() => undefined}
        />
      )}
    </div>
  );
}
