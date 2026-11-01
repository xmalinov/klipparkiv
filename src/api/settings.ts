import { invoke } from "@tauri-apps/api/core";

export interface Settings {
  audioSubdir: string;
  videoSubdir: string;
  concurrency: number;
  theme: "dark" | "light" | "system";
  language: "ru" | "en";
  scanDepth: number;
  autoscan: boolean;
  embedThumbnail: boolean;
}

export const DEFAULT_SETTINGS: Settings = {
  audioSubdir: "audio",
  videoSubdir: "video",
  concurrency: 2,
  theme: "dark",
  language: "ru",
  scanDepth: 3,
  autoscan: true,
  embedThumbnail: true,
};

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function saveSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("save_settings", { settings });
}

export function updateYtdlp(): Promise<string> {
  return invoke<string>("update_ytdlp");
}

export function applyTheme(theme: Settings["theme"]): "dark" | "light" {
  const systemDark =
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
  const resolved = theme === "system" ? (systemDark ? "dark" : "light") : theme;
  document.documentElement.dataset.theme = resolved;
  return resolved;
}
