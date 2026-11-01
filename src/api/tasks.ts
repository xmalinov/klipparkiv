import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TaskMode = "audio" | "video";
export type DownloadMode = TaskMode | "both" | "clip";
export type TaskState = "queued" | "running" | "done" | "failed" | "canceled";

export interface TaskSnapshot {
  id: string;
  url: string;
  mode: TaskMode;
  clip: boolean;
  state: TaskState;
  percent: number;
  speedBps: number | null;
  etaSecs: number | null;
  error: string | null;
}

export interface VideoInfo {
  id: string;
  title: string;
  channel: string | null;
  durationSecs: number | null;
  thumbnailUrl: string | null;
  uploadDate: string | null;
  viewCount: number | null;
}

export interface ProgressPayload {
  id: string;
  percent: number;
  speedBps: number | null;
  etaSecs: number | null;
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
}

export interface NewTask {
  url: string;
  mode: DownloadMode;
  quality?: string;
  audioKbps?: number;
  outDir?: string;
  clipStart?: string;
  clipEnd?: string;
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
        }
      : task,
  );
}

export function formatPercent(percent: number): string {
  return `${Math.round(percent)}%`;
}

export function formatSpeed(bytesPerSecond: number | null): string {
  if (bytesPerSecond === null || !Number.isFinite(bytesPerSecond)) return "—";
  if (bytesPerSecond >= 1024 * 1024) {
    return `${(bytesPerSecond / (1024 * 1024)).toFixed(1).replace(".", ",")} МБ/с`;
  }
  if (bytesPerSecond >= 1024) {
    return `${Math.round(bytesPerSecond / 1024)} КБ/с`;
  }
  return `${Math.round(bytesPerSecond)} Б/с`;
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

export function formatViews(count: number | null): string {
  if (count === null || !Number.isFinite(count)) return "";
  if (count >= 1_000_000_000) {
    return `${(count / 1_000_000_000).toFixed(1).replace(".", ",")} млрд просмотров`;
  }
  if (count >= 1_000_000) {
    return `${(count / 1_000_000).toFixed(1).replace(".", ",")} млн просмотров`;
  }
  if (count >= 1_000) {
    return `${Math.round(count / 1_000)} тыс. просмотров`;
  }
  return `${count} просмотров`;
}

export function formatUploadDate(raw: string | null): string {
  if (!raw || !/^\d{8}$/.test(raw)) return raw ?? "";
  return `${raw.slice(6, 8)}.${raw.slice(4, 6)}.${raw.slice(0, 4)}`;
}

export function stateLabel(task: TaskSnapshot): string {
  switch (task.state) {
    case "queued":
      return "В очереди";
    case "running":
      return formatPercent(task.percent);
    case "done":
      return "Готово";
    case "failed":
      return "Ошибка";
    case "canceled":
      return "Отменено";
  }
}
