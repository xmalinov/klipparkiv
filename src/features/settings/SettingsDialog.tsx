import { useEffect, useState } from "react";
import type { AppInfo } from "../../api/appInfo";
import {
  applyTheme,
  getSettings,
  saveSettings,
  updateYtdlp,
  type Settings,
} from "../../api/settings";

type Tab = "general" | "library" | "updates" | "about";

const TABS: { id: Tab; label: string }[] = [
  { id: "general", label: "Общие" },
  { id: "library", label: "Библиотека" },
  { id: "updates", label: "Обновления" },
  { id: "about", label: "О программе" },
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
  const [tab, setTab] = useState<Tab>("general");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updateNote, setUpdateNote] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((value) => {
        if (alive) setSettings(value);
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
      setUpdateNote(`yt-dlp обновлён до ${version}`);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setUpdating(false);
    }
  }

  return (
    <div className="overlay" role="presentation" onClick={onClose}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Настройки"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="dialog-head">
          <span>Настройки</span>
          <button type="button" className="icon-btn" aria-label="Закрыть" onClick={onClose}>
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
              {item.label}
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
            <p className="queue-empty">Загрузка настроек…</p>
          ) : tab === "general" ? (
            <>
              <div className="row">
                <label className="label" htmlFor="audio-subdir">
                  Подпапка для аудио
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
                  Подпапка для видео
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
                  Одновременных загрузок
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
                <span className="label">Тема</span>
                <div className="mini-tabs" role="group" aria-label="Тема">
                  {[
                    { id: "system" as const, label: "Системная" },
                    { id: "dark" as const, label: "Тёмная" },
                    { id: "light" as const, label: "Светлая" },
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
                  Глубина сканирования
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
                  Сканировать при открытии
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
                <span className="label">yt-dlp</span>
                <span className="muted">{info?.ytdlp ?? "не найден"}</span>
              </div>
              <div className="row">
                <span className="label">ffmpeg</span>
                <span className="muted">{info?.ffmpeg ?? "не найден"}</span>
              </div>
              <div className="row">
                <span className="label">ffprobe</span>
                <span className="muted">{info?.ffprobe ?? "не найден"}</span>
              </div>
              <div className="row">
                <span className="label">Обновление yt-dlp</span>
                <button
                  type="button"
                  className="btn ghost"
                  onClick={runUpdate}
                  disabled={updating}
                >
                  {updating ? "Обновляем…" : "Обновить"}
                </button>
              </div>
              {updateNote && <p className="hint">{updateNote}</p>}
              <p className="hint">
                ffmpeg и ffprobe заменяются вручную: положите новые exe в bin\.
              </p>
            </>
          ) : (
            <>
              <div className="row">
                <span className="label">Версия приложения</span>
                <span className="muted">0.1.0</span>
              </div>
              <div className="row">
                <span className="label">Папка приложения</span>
                <span className="mono small-text" title={info?.root}>
                  {info?.root}
                </span>
              </div>
              <div className="row">
                <span className="label">Режим</span>
                <span className="muted">Портативный, без установки</span>
              </div>
              <p className="hint">
                Настройки и библиотека лежат в data\, ничего не пишется в реестр.
              </p>
            </>
          )}
        </div>

        <div className="dialog-foot">
          <button type="button" className="btn ghost" onClick={onClose}>
            Отмена
          </button>
          <button
            type="button"
            className="btn primary"
            onClick={save}
            disabled={!settings || saving}
          >
            Сохранить
          </button>
        </div>
      </div>
    </div>
  );
}
