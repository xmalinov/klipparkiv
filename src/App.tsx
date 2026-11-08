import { useEffect, useState } from "react";
import { formatVersion, loadAppInfo, type AppInfo } from "./api/appInfo";
import { applyTheme, getSettings } from "./api/settings";
import DownloadScreen from "./features/download/DownloadScreen";
import LibraryScreen from "./features/library/LibraryScreen";
import SettingsDialog from "./features/settings/SettingsDialog";
import UpdateBanner from "./features/updates/UpdateBanner";
import { useI18n, type Translate } from "./i18n";
import "./styles/tokens.css";
import "./App.css";

type Screen = "download" | "library";

const NAV: {
  id: Screen;
  labelKey: "nav.download" | "nav.library";
  icon: React.ReactNode;
}[] = [
  {
    id: "download",
    labelKey: "nav.download",
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
    labelKey: "nav.library",
    icon: (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="M8 4v16" />
      </svg>
    ),
  },
];

function missingBinaries(info: AppInfo) {
  return info.binaries.filter((binary) => binary.version === null);
}

function environmentWarnings(info: AppInfo, t: Translate): string[] {
  return info.writable ? [] : [t("app.notWritable")];
}

export default function App() {
  const { t } = useI18n();
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

  const warnings = info ? environmentWarnings(info, t) : [];

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
            <span>{t(item.labelKey)}</span>
          </button>
        ))}
      </aside>

      <div className="main">
        <header className="header">
          <h1>{screen === "download" ? t("nav.download") : t("nav.library")}</h1>
          <div className="header-right">
            <span>{t(info?.installKind === "installed" ? "app.installedMode" : "app.portableMode")}</span>
            <button
              type="button"
              className="icon-btn"
              aria-label={t("app.settings")}
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
          <UpdateBanner />
          {error && (
            <p className="banner" role="alert">
              {t("app.environmentError", { error })}
            </p>
          )}
          {warnings.map((warning) => (
            <p className="banner" role="alert" key={warning}>
              {warning}
            </p>
          ))}
          {info && missingBinaries(info).length > 0 && (
            <div className="banner" role="alert">
              <p>
                {t("app.missingBinaries", {
                  names: missingBinaries(info)
                    .map((binary) => binary.name)
                    .join(", "),
                  dir: info.binDir,
                })}
              </p>
              <p className="banner-hint">{t("app.missingBinariesHint")}</p>
              <ul className="binary-details">
                {missingBinaries(info).map((binary) => (
                  <li key={binary.name}>
                    <code>{binary.name}</code>
                    <span className="mono small-text">{binary.path}</span>
                    <span>{binary.error ?? t("common.notFound")}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

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
