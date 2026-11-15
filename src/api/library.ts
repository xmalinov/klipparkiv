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

export type ImportSkipReason =
  | "unsupported"
  | "folder"
  | "duplicateInBatch"
  | "alreadyInLibrary"
  | "cancelled";

export interface ImportSkip {
  path: string;
  reason: ImportSkipReason;
}

export interface ImportFailure {
  path: string;
  error: string;
}

export interface ImportReport {
  imported: LibraryItem[];
  existing: LibraryItem[];
  skipped: ImportSkip[];
  failed: ImportFailure[];
  cancelled: boolean;
}

export interface ImportProgress {
  jobId: number;
  index: number;
  total: number;
  name: string;
  copiedBytes: number;
  totalBytes: number;
  phase: "copying" | "probing" | "thumbnailing" | "done";
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

export function openMedia(path: string): Promise<void> {
  return invoke<void>("library_open", { path });
}

export function importMedia(paths: string[]): Promise<ImportReport> {
  return invoke<ImportReport>("library_import", { paths });
}

export function cancelImport(): Promise<boolean> {
  return invoke<boolean>("library_import_cancel");
}

export function readTextFile(path: string): Promise<string> {
  return invoke<string>("read_text_file", { path });
}


export {
  formatBytes,
  formatFileCount,
  formatItemDate,
  formatResolution,
} from "../lib/format";
