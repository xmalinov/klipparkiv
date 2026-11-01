import { invoke } from "@tauri-apps/api/core";

export type AppInfo = {
  root: string;
  supportRoot: string;
  installKind: "portable" | "installed";
  writable: boolean;
  version: string;
  updateConfigured: boolean;
  ytdlp: string | null;
  ffmpeg: string | null;
  ffprobe: string | null;
};

export async function loadAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function formatVersion(version: string | null): string {
  return version ?? "не найден";
}
