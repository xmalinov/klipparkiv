import { useEffect, useState } from "react";
import type { AppInfo } from "../../api/appInfo";
import {
  applyTheme,
  getSettings,
  saveSettings,
  updateYtdlp,
  type Settings,
} from "../../api/settings";
import {
  cancelPendingUpdate,
  checkAppUpdate,
  downloadAppUpdate,
  getUpdateStatus,
  installAppUpdate,
  subscribeUpdate,
  type UpdateStatus,
} from "../../api/updates";
import { useI18n } from "../../i18n";

type Tab = "general" | "library" | "updates" | "about";

const TABS: {
  id: Tab;
  labelKey:
    | "settings.tab.general"
    | "settings.tab.library"
    | "settings.tab.updates"
    | "settings.tab.about";
}[] = [
  { id: "general", labelKey: "settings.tab.general" },
  { id: "library", labelKey: "settings.tab.library" },
  { id: "updates", labelKey: "settings.tab.updates" },
  { id: "about", labelKey: "settings.tab.about" },
];

export default function SettingsDialog({
  info,
  onClose,
  onSaved,
}: {
  info: AppInfo | null;
  onClose: () => void;
  onSaved: (settings: Settings) => void;
}) {
  const { setLanguage, t } = useI18n();
  const [tab, setTab] = useState<Tab>("general");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updateNote, setUpdateNote] = useState<string | null>(null);
  const [appUpdate, setAppUpdate] = useState<UpdateStatus | null>(null);
  const [appBusy, setAppBusy] = useState(false);
  const [appUpdateError, setAppUpdateError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    let dispose: (() => void) | undefined;
    getUpdateStatus()
      .then((value) => {
        if (alive && value && typeof value.state === "string") setAppUpdate(value);
      })
      .catch(() => {});
    subscribeUpdate((value) => {
      if (alive) setAppUpdate(value);
    }).then((unlisten) => {
      if (alive) dispose = unlisten;
      else unlisten();
    });
    return () => {
      alive = false;
      dispose?.();
    };
  }, []);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((value) => {
        if (alive) {
          setSettings({
            ...value,
            language: value.language === "en" ? "en" : "ru",
          });
        }
      })
      .catch((cause: unknown) => {
        if (alive) setError(String(cause));
      });
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  function update<K extends keyof Settings>(key: K, value: Settings[K]) {
    setSettings((current) => (current ? { ...current, [key]: value } : current));
  }

  async function save() {
    if (!settings || saving) return;
    setSaving(true);
    setError(null);
    try {
      const saved = await saveSettings(settings);
      applyTheme(saved.theme);
      setLanguage(saved.language === "en" ? "en" : "ru");
      onSaved(saved);
      onClose();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function runUpdate() {
    setUpdating(true);
    setUpdateNote(null);
    setError(null);
    try {
      const version = await updateYtdlp();
      setUpdateNote(t("settings.updated", { version }));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setUpdating(false);
    }
  }

  async function runAppAction(action: () => Promise<UpdateStatus>) {
    setAppBusy(true);
    setAppUpdateError(null);
    try {
      const next = await action();
      if (next && typeof next.state === "string") setAppUpdate(next);
    } catch (cause) {
      setAppUpdateError(String(cause));
    } finally {
      setAppBusy(false);
    }
  }

  function appUpdateStatusText(status: UpdateStatus): string {
    switch (status.state) {
      case "checking":
        return t("settings.checkingUpdates");
      case "current":
        return t("settings.currentVersion");
      case "available":
        return t("settings.updateAvailable", { version: status.version ?? "" });
      case "downloading":
        return t("settings.updateDownloading", { percent: Math.round(status.percent) });
      case "ready":
        return t("settings.updateReady", { version: status.version ?? "" });
      case "deferred":
        return t("settings.updateDeferred");
      case "installing":
        return t("settings.updateInstalling");
      case "error":
        return status.error ?? "";
      default:
        return "";
    }
  }

  const appUpdateLabel =
    appUpdate?.state === "available"
      ? t("settings.downloadApp")
      : appUpdate?.state === "ready"
        ? t("settings.restartApp")
        : appUpdate?.state === "deferred"
          ? t("settings.cancelAppUpdate")
          : appUpdate?.state === "downloading"
            ? t("settings.downloadingApp")
            : appUpdate?.state === "installing"
              ? t("settings.updateInstalling")
              : appUpdate?.state === "checking"
                ? t("settings.checkingUpdates")
                : t("settings.checkUpdates");

  const appUpdateDisabled =
    appBusy ||
    appUpdate?.state === "downloading" ||
    appUpdate?.state === "installing" ||
    appUpdate?.state === "checking";

  function runAppUpdateButton() {
    if (!appUpdate || appUpdate.state === "idle" || appUpdate.state === "current") {
      return runAppAction(() => checkAppUpdate(false));
    }
    if (appUpdate.state === "available") {
      return runAppAction(downloadAppUpdate);
    }
    if (appUpdate.state === "ready") {
      return runAppAction(installAppUpdate);
    }
    if (appUpdate.state === "deferred") {
      return runAppAction(cancelPendingUpdate);
    }
    return runAppAction(() => checkAppUpdate(false));
  }

  return (
    <div className="overlay" role="presentation" onClick={onClose}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={t("settings.title")}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="dialog-head">
          <span>{t("settings.title")}</span>
          <button
            type="button"
            className="icon-btn"
            aria-label={t("common.close")}
            onClick={onClose}
          >
            ×
          </button>
        </div>

        <div className="dialog-tabs" role="tablist">
          {TABS.map((item) => (
            <button
              key={item.id}
              type="button"
              role="tab"
              aria-selected={tab === item.id}
              className={tab === item.id ? "tab active" : "tab"}
              onClick={() => setTab(item.id)}
            >
              {t(item.labelKey)}
            </button>
          ))}
        </div>

        <div className="dialog-body">
          {error && (
            <p className="banner" role="alert">
              {error}
            </p>
          )}

          {!settings ? (
            <p className="queue-empty">{t("settings.loading")}</p>
          ) : tab === "general" ? (
            <>
              <div className="row">
                <label className="label" htmlFor="audio-subdir">
                  {t("settings.audioSubdir")}
                </label>
                <input
                  id="audio-subdir"
                  className="field small"
                  value={settings.audioSubdir}
                  onChange={(event) => update("audioSubdir", event.currentTarget.value)}
                />
              </div>
              <div className="row">
                <label className="label" htmlFor="video-subdir">
                  {t("settings.videoSubdir")}
                </label>
                <input
                  id="video-subdir"
                  className="field small"
                  value={settings.videoSubdir}
                  onChange={(event) => update("videoSubdir", event.currentTarget.value)}
                />
              </div>
              <div className="row">
                <label className="label" htmlFor="concurrency">
                  {t("settings.concurrency")}
                </label>
                <select
                  id="concurrency"
                  className="field small"
                  value={settings.concurrency}
                  onChange={(event) => update("concurrency", Number(event.currentTarget.value))}
                >
                  {[1, 2, 3, 4].map((value) => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ))}
                </select>
              </div>
              <div className="row">
                <span className="label">{t("settings.language")}</span>
                <div
                  className="mini-tabs"
                  role="group"
                  aria-label={t("settings.language")}
                >
                  {[
                    { id: "ru" as const, label: t("settings.language.ru") },
                    { id: "en" as const, label: t("settings.language.en") },
                  ].map((option) => (
                    <button
                      key={option.id}
                      type="button"
                      className={settings.language === option.id ? "on" : ""}
                      onClick={() => update("language", option.id)}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>
              <div className="row">
                <span className="label">{t("settings.theme")}</span>
                <div
                  className="mini-tabs"
                  role="group"
                  aria-label={t("settings.theme")}
                >
                  {[
                    { id: "system" as const, label: t("settings.theme.system") },
                    { id: "dark" as const, label: t("settings.theme.dark") },
                    { id: "light" as const, label: t("settings.theme.light") },
                  ].map((option) => (
                    <button
                      key={option.id}
                      type="button"
                      className={settings.theme === option.id ? "on" : ""}
                      onClick={() => update("theme", option.id)}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>
            </>
          ) : tab === "library" ? (
            <>
              <div className="row">
                <label className="label" htmlFor="scan-depth">
                  {t("settings.scanDepth")}
                </label>
                <select
                  id="scan-depth"
                  className="field small"
                  value={settings.scanDepth}
                  onChange={(event) => update("scanDepth", Number(event.currentTarget.value))}
                >
                  {[1, 2, 3, 4, 5].map((value) => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ))}
                </select>
              </div>
              <div className="row">
                <label className="label" htmlFor="autoscan">
                  {t("settings.autoscan")}
                </label>
                <input
                  id="autoscan"
                  type="checkbox"
                  checked={settings.autoscan}
                  onChange={(event) => update("autoscan", event.currentTarget.checked)}
                />
              </div>
            </>
          ) : tab === "updates" ? (
            <>
              <div className="row">
                <span className="label">{t("settings.appVersion")}</span>
                <span className="muted">{info?.version ?? "—"}</span>
              </div>
              <div className="row">
                <span className="label">{t("settings.appUpdate")}</span>
                <button
                  type="button"
                  className="btn ghost"
                  onClick={runAppUpdateButton}
                  disabled={appUpdateDisabled}
                >
                  {appUpdateLabel}
                </button>
              </div>
              {(appUpdateError || (appUpdate && appUpdateStatusText(appUpdate))) && (
                <p className={appUpdate?.state === "error" ? "hint warning" : "hint"}>
                  {appUpdateError ?? (appUpdate ? appUpdateStatusText(appUpdate) : "")}
                </p>
              )}
              {info && !info.updateConfigured && (
                <p className="hint warning">{t("settings.updatesNotConfigured")}</p>
              )}
              <p className="hint">{t("settings.appUpdateHint")}</p>
              <div className="row">
                <span className="label">yt-dlp</span>
                <span className="muted">
                  {info?.ytdlp ?? t("common.notFound")}
                </span>
              </div>
              <div className="row">
                <span className="label">ffmpeg</span>
                <span className="muted">
                  {info?.ffmpeg ?? t("common.notFound")}
                </span>
              </div>
              <div className="row">
                <span className="label">ffprobe</span>
                <span className="muted">
                  {info?.ffprobe ?? t("common.notFound")}
                </span>
              </div>
              <div className="row">
                <span className="label">{t("settings.updateYtdlp")}</span>
                <button
                  type="button"
                  className="btn ghost"
                  onClick={runUpdate}
                  disabled={updating}
                >
                  {updating ? t("settings.updating") : t("settings.update")}
                </button>
              </div>
              {updateNote && <p className="hint">{updateNote}</p>}
              <p className="hint">{t("settings.ffmpegHint")}</p>
            </>
          ) : (
            <>
              <div className="row">
                <span className="label">{t("settings.appVersion")}</span>
                <span className="muted">{info?.version ?? "—"}</span>
              </div>
              <div className="row">
                <span className="label">{t("settings.appFolder")}</span>
                <span className="mono small-text" title={info?.root}>
                  {info?.root}
                </span>
              </div>
              <div className="row">
                <span className="label">{t("settings.mode")}</span>
                <span className="muted">{t("settings.modePortable")}</span>
              </div>
              <p className="hint">{t("settings.aboutHint")}</p>
            </>
          )}
        </div>

        <div className="dialog-foot">
          <button type="button" className="btn ghost" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button
            type="button"
            className="btn primary"
            onClick={save}
            disabled={!settings || saving}
          >
            {t("common.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
