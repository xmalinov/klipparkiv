import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type UpdateState =
  | "idle"
  | "checking"
  | "current"
  | "available"
  | "downloading"
  | "ready"
  | "deferred"
  | "installing"
  | "manual"
  | "error";

export interface UpdateStatus {
  state: UpdateState;
  currentVersion: string;
  version: string | null;
  notes: string | null;
  size: number | null;
  downloaded: number;
  percent: number;
  error: string | null;
}

export function getUpdateStatus(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("get_update_status");
}

export function checkAppUpdate(silent = false): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("check_app_update", { silent });
}

export function downloadAppUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("download_app_update");
}

export function installAppUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("install_app_update");
}

export function openManualUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("open_manual_update");
}

export function cancelPendingUpdate(): Promise<UpdateStatus> {
  return invoke<UpdateStatus>("cancel_pending_update");
}

export function subscribeUpdate(
  handler: (status: UpdateStatus) => void,
): Promise<UnlistenFn> {
  return listen<UpdateStatus>("update://state", (event) => handler(event.payload));
}

export function formatBytes(
  bytes: number | null,
  language: "ru" | "en" = "ru",
): string {
  if (bytes === null || !Number.isFinite(bytes)) return "";
  const separator = language === "ru" ? "," : ".";
  if (bytes >= 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1).replace(".", separator)} ${
      language === "ru" ? "МБ" : "MB"
    }`;
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)} ${language === "ru" ? "КБ" : "KB"}`;
  }
  return `${bytes} ${language === "ru" ? "Б" : "B"}`;
}
