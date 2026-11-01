import { invoke } from "@tauri-apps/api/core";

export type MediaKind = "audio" | "video";
export type SortKey = "dateDesc" | "dateAsc" | "title" | "duration" | "size" | "channel";

export interface LibraryItem {
  id: string;
  path: string;
  kind: MediaKind;
  title: string;
  channel: string | null;
  durationSecs: number | null;
  width: number | null;
  height: number | null;
  sizeBytes: number;
  mtime: number;
  thumbPath: string | null;
}

export interface LibraryPage {
  items: LibraryItem[];
  total: number;
}

export interface LibraryQuery {
  search?: string;
  kind?: MediaKind;
  sort: SortKey;
  limit?: number;
  offset?: number;
}

export interface ScanStats {
  scanned: number;
  added: number;
  updated: number;
  removed: number;
}

export function queryLibrary(query: LibraryQuery): Promise<LibraryPage> {
  return invoke<LibraryPage>("library_query", { query });
}

export function scanLibrary(full = false): Promise<ScanStats> {
  return invoke<ScanStats>("library_scan", { full });
}

export function removeFromLibrary(id: string): Promise<void> {
  return invoke<void>("library_remove", { id });
}

export function loadThumb(path: string): Promise<string> {
  return invoke<string>("library_thumb", { path });
}

export function formatBytes(
  bytes: number,
  language: "ru" | "en" = "ru",
): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "";
  const separator = language === "ru" ? "," : ".";
  const units =
    language === "ru"
      ? { gb: "ГБ", mb: "МБ", kb: "КБ" }
      : { gb: "GB", mb: "MB", kb: "KB" };
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) {
    return `${(mb / 1024).toFixed(1).replace(".", separator)} ${units.gb}`;
  }
  if (mb >= 1) {
    return `${mb.toFixed(1).replace(".", separator)} ${units.mb}`;
  }
  return `${Math.max(1, Math.round(bytes / 1024))} ${units.kb}`;
}

export function formatItemDate(
  mtime: number,
  language: "ru" | "en" = "ru",
): string {
  if (!mtime) return "";
  const date = new Date(mtime * 1000);
  const day = date.getDate().toString().padStart(2, "0");
  const month = (date.getMonth() + 1).toString().padStart(2, "0");
  return language === "ru" ? `${day}.${month}` : `${month}/${day}`;
}

export function formatResolution(width: number | null, height: number | null): string {
  if (!width || !height) return "";
  return height >= 2160 ? "4K" : `${height}p`;
}

export function formatFileCount(
  count: number,
  language: "ru" | "en" = "ru",
): string {
  if (language === "en") {
    return `${count} ${count === 1 ? "file" : "files"}`;
  }
  const mod10 = count % 10;
  const mod100 = count % 100;
  if (mod10 === 1 && mod100 !== 11) return `${count} файл`;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) {
    return `${count} файла`;
  }
  return `${count} файлов`;
}
