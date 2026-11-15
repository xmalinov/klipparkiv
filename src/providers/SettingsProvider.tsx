import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  DEFAULT_SETTINGS,
  getSettings,
  saveSettings,
  type Settings,
} from "../api/settings";

interface SettingsContextValue {
  /** `null` until the first load finishes. */
  settings: Settings | null;
  error: string | null;
  /** Saves to disk and publishes the result to every consumer. */
  save: (next: Settings) => Promise<Settings>;
  /** Publishes settings that were already saved elsewhere. */
  update: (next: Settings) => void;
}

const SettingsContext = createContext<SettingsContextValue | null>(null);

function clamp(value: number, min: number, max: number): number {
  if (!Number.isFinite(value)) return min;
  return Math.min(max, Math.max(min, Math.round(value)));
}

/**
 * Brings a payload from the backend (or from a fixture) into the exact shape
 * the rest of the app expects, so one bad field cannot break a whole screen.
 */
export function normalizeSettings(value: unknown): Settings {
  if (typeof value !== "object" || value === null) return DEFAULT_SETTINGS;
  const raw = value as Partial<Settings>;
  const subdir = (candidate: unknown, fallback: string) =>
    typeof candidate === "string" && candidate.trim() ? candidate : fallback;
  return {
    audioSubdir: subdir(raw.audioSubdir, DEFAULT_SETTINGS.audioSubdir),
    videoSubdir: subdir(raw.videoSubdir, DEFAULT_SETTINGS.videoSubdir),
    concurrency: clamp(Number(raw.concurrency), 1, 4),
    theme:
      raw.theme === "light" || raw.theme === "system" ? raw.theme : "dark",
    language: raw.language === "en" ? "en" : "ru",
    scanDepth: clamp(Number(raw.scanDepth), 1, 5),
    autoscan: raw.autoscan !== false,
    embedThumbnail: raw.embedThumbnail !== false,
  };
}

/**
 * Single owner of `settings.json` on the UI side.
 *
 * Before this provider the shell, the i18n bootstrap, the download screen and
 * the settings dialog each called `get_settings`, so a saved value reached only
 * the dialog that wrote it. Now there is one read, one write and one update
 * path.
 */
export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getSettings()
      .then((value) => {
        if (alive) setSettings(normalizeSettings(value));
      })
      .catch((cause: unknown) => {
        if (alive) setError(String(cause));
      });
    return () => {
      alive = false;
    };
  }, []);

  const update = useCallback((next: Settings) => {
    setSettings(normalizeSettings(next));
    setError(null);
  }, []);

  const save = useCallback(async (next: Settings) => {
    const saved = normalizeSettings(await saveSettings(next));
    setSettings(saved);
    setError(null);
    return saved;
  }, []);

  const value = useMemo<SettingsContextValue>(
    () => ({ settings, error, save, update }),
    [error, save, settings, update],
  );

  return (
    <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>
  );
}

export function useSettings(): SettingsContextValue {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error("useSettings must be used inside SettingsProvider");
  }
  return context;
}
