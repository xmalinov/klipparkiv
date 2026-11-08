import { invoke } from "@tauri-apps/api/core";

export type BinaryStatus = {
  name: string;
  path: string;
  version: string | null;
  error: string | null;
};

export type AppInfo = {
  root: string;
  supportRoot: string;
  installKind: "portable" | "installed";
  writable: boolean;
  version: string;
  updateConfigured: boolean;
  binDir: string;
  ytdlp: string | null;
  ffmpeg: string | null;
  ffprobe: string | null;
  binaries: BinaryStatus[];
};

export async function loadAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function formatVersion(version: string | null): string {
  return version ?? "не найден";
}
