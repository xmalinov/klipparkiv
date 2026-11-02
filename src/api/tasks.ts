import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TaskMode = "audio" | "video";
export type DownloadMode = TaskMode | "both" | "clip";
export type TaskState =
  | "queued"
  | "running"
  | "done"
  | "skipped"
  | "partial"
  | "failed"
  | "canceled";

export interface TaskSnapshot {
  id: string;
  url: string;
  /** Human title once known: preview card or yt-dlp output. */
  title: string | null;
  mode: TaskMode;
  clip: boolean;
  state: TaskState;
  percent: number;
  speedBps: number | null;
  etaSecs: number | null;
  error: string | null;
  playlistIndex: number | null;
  playlistCount: number | null;
  outputPath: string | null;
}

export interface PlaylistEntry {
  id: string;
  title: string;
}

export interface VideoInfo {
  /** `video` for a single item, `playlist` when the link refers to a list. */
  kind: "video" | "playlist";
  id: string;
  title: string;
  channel: string | null;
  durationSecs: number | null;
  thumbnailUrl: string | null;
  uploadDate: string | null;
  viewCount: number | null;
  playlistCount: number | null;
  /** First few playlist items, enough for the preview card. */
  entries: PlaylistEntry[];
}

export interface ProgressPayload {
  id: string;
  percent: number;
  speedBps: number | null;
  etaSecs: number | null;
  playlistIndex: number | null;
  playlistCount: number | null;
}

export interface LogPayload {
  id: string;
  line: string;
}

export interface QueueState {
  paused: boolean;
  running: number;
  queued: number;
}

export interface BatchRequest {
  path: string;
  mode: DownloadMode;
  quality?: string;
  audioKbps?: number;
  outDir?: string;
}

export interface LinksRequest {
  text: string;
  mode: DownloadMode;
  quality?: string;
  audioKbps?: number;
  outDir?: string;
}

export interface NewTask {
  url: string;
  mode: DownloadMode;
  quality?: string;
  audioKbps?: number;
  outDir?: string;
  clipStart?: string;
  clipEnd?: string;
  title?: string;
  playlist?: boolean;
  playlistStart?: number;
  playlistEnd?: number;
}

export function enqueueTask(task: NewTask): Promise<TaskSnapshot[]> {
  return invoke<TaskSnapshot[]>("enqueue_task", { task });
}

export function probeMetadata(url: string): Promise<VideoInfo> {
  return invoke<VideoInfo>("probe_metadata", { url });
}

export function enqueueBatch(request: BatchRequest): Promise<TaskSnapshot[]> {
  return invoke<TaskSnapshot[]>("enqueue_batch", { request });
}

export function enqueueLinks(request: LinksRequest): Promise<TaskSnapshot[]> {
  return invoke<TaskSnapshot[]>("enqueue_links", { request });
}

export function pauseQueue(): Promise<QueueState> {
  return invoke<QueueState>("pause_queue");
}

export function resumeQueue(): Promise<QueueState> {
  return invoke<QueueState>("resume_queue");
}

export function cancelTask(id: string): Promise<void> {
  return invoke<void>("cancel_task", { id });
}

type Handlers = {
  onState: (task: TaskSnapshot) => void;
  onProgress: (progress: ProgressPayload) => void;
  onLog?: (log: LogPayload) => void;
  onQueue?: (state: QueueState) => void;
};

export async function subscribeTasks(handlers: Handlers): Promise<UnlistenFn> {
  const unlisteners = await Promise.all([
    listen<TaskSnapshot>("task://state", (event) => handlers.onState(event.payload)),
    listen<ProgressPayload>("task://progress", (event) =>
      handlers.onProgress(event.payload),
    ),
    listen<LogPayload>("task://log", (event) => handlers.onLog?.(event.payload)),
    listen<QueueState>("queue://changed", (event) => handlers.onQueue?.(event.payload)),
  ]);

  return () => {
    for (const unlisten of unlisteners) unlisten();
  };
}

export function upsertTask(tasks: TaskSnapshot[], next: TaskSnapshot): TaskSnapshot[] {
  const index = tasks.findIndex((task) => task.id === next.id);
  if (index === -1) return [...tasks, next];
  const copy = tasks.slice();
  copy[index] = next;
  return copy;
}

export function applyProgress(
  tasks: TaskSnapshot[],
  progress: ProgressPayload,
): TaskSnapshot[] {
  return tasks.map((task) =>
    task.id === progress.id
      ? {
          ...task,
          percent: progress.percent,
          speedBps: progress.speedBps,
          etaSecs: progress.etaSecs,
          playlistIndex: progress.playlistIndex ?? task.playlistIndex,
          playlistCount: progress.playlistCount ?? task.playlistCount,
        }
      : task,
  );
}

export function formatPercent(percent: number): string {
  return `${Math.round(percent)}%`;
}

export function formatSpeed(
  bytesPerSecond: number | null,
  language: "ru" | "en" = "ru",
): string {
  if (bytesPerSecond === null || !Number.isFinite(bytesPerSecond)) return "—";
  const separator = language === "ru" ? "," : ".";
  if (bytesPerSecond >= 1024 * 1024) {
    return `${(bytesPerSecond / (1024 * 1024)).toFixed(1).replace(".", separator)} ${
      language === "ru" ? "МБ/с" : "MB/s"
    }`;
  }
  if (bytesPerSecond >= 1024) {
    return `${Math.round(bytesPerSecond / 1024)} ${
      language === "ru" ? "КБ/с" : "KB/s"
    }`;
  }
  return `${Math.round(bytesPerSecond)} ${language === "ru" ? "Б/с" : "B/s"}`;
}

export function formatEta(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return "";
  const total = Math.round(seconds);
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return "";
  const total = Math.round(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const rest = total % 60;
  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${rest.toString().padStart(2, "0")}`;
  }
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

/**
 * Mirrors the backend parser: `SS`, `MM:SS` or `HH:MM:SS`.
 * Returns `null` for anything else.
 */
export function parseTimecode(raw: string): number | null {
  const text = raw.trim();
  if (!text) return null;
  const parts = text.split(":");
  if (parts.length > 3 || parts.some((part) => part === "")) return null;
  let seconds = 0;
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    if (!/^\d+$/.test(part)) return null;
    const value = Number(part);
    const isHours = parts.length === 3 && index === 0;
    if (parts.length > 1 && !isHours && value >= 60) return null;
    seconds = seconds * 60 + value;
  }
  return seconds;
}

/** `90` → `00:01:30`, matching the strings sent to the backend. */
export function formatTimecode(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const rest = total % 60;
  return [hours, minutes, rest]
    .map((value) => value.toString().padStart(2, "0"))
    .join(":");
}

export function formatViews(
  count: number | null,
  language: "ru" | "en" = "ru",
): string {
  if (count === null || !Number.isFinite(count)) return "";
  const separator = language === "ru" ? "," : ".";
  const decimal = (value: number) => value.toFixed(1).replace(".", separator);
  if (language === "en") {
    if (count >= 1_000_000_000) return `${decimal(count / 1_000_000_000)}B views`;
    if (count >= 1_000_000) return `${decimal(count / 1_000_000)}M views`;
    if (count >= 1_000) return `${Math.round(count / 1_000)}K views`;
    return `${count} ${count === 1 ? "view" : "views"}`;
  }
  if (count >= 1_000_000_000) {
    return `${decimal(count / 1_000_000_000)} млрд просмотров`;
  }
  if (count >= 1_000_000) {
    return `${decimal(count / 1_000_000)} млн просмотров`;
  }
  if (count >= 1_000) {
    return `${Math.round(count / 1_000)} тыс. просмотров`;
  }
  return `${count} просмотров`;
}

export function formatUploadDate(
  raw: string | null,
  language: "ru" | "en" = "ru",
): string {
  if (!raw || !/^\d{8}$/.test(raw)) return raw ?? "";
  const day = raw.slice(6, 8);
  const month = raw.slice(4, 6);
  const year = raw.slice(0, 4);
  return language === "ru"
    ? `${day}.${month}.${year}`
    : `${month}/${day}/${year}`;
}

export function stateLabel(
  task: TaskSnapshot,
  language: "ru" | "en" = "ru",
): string {
  const labels =
    language === "ru"
      ? {
          queued: "В очереди",
          done: "Готово",
          skipped: "Уже было",
          partial: "Частично",
          failed: "Ошибка",
          canceled: "Отменено",
        }
      : {
          queued: "Queued",
          done: "Done",
          skipped: "Already there",
          partial: "Partial",
          failed: "Error",
          canceled: "Canceled",
        };
  switch (task.state) {
    case "queued":
      return labels.queued;
    case "running":
      return formatPercent(task.percent);
    case "done":
      return labels.done;
    case "skipped":
      return labels.skipped;
    case "partial":
      return labels.partial;
    case "failed":
      return labels.failed;
    case "canceled":
      return labels.canceled;
  }
}
