import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { formatPercent } from "../lib/format";

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

/** Snapshot of the queue after a page reload or a screen switch. */
export function listTasks(): Promise<TaskSnapshot[]> {
  return invoke<TaskSnapshot[]>("list_tasks");
}

export function getQueueState(): Promise<QueueState> {
  return invoke<QueueState>("queue_state");
}

export function resumeQueue(): Promise<QueueState> {
  return invoke<QueueState>("resume_queue");
}

export function cancelTask(id: string): Promise<void> {
  return invoke<void>("cancel_task", { id });
}

/** Drops a finished task from the history; a running one must be cancelled. */
export function removeTask(id: string): Promise<void> {
  return invoke<void>("remove_task", { id });
}

const TERMINAL_STATES: readonly TaskState[] = [
  "done",
  "skipped",
  "partial",
  "failed",
  "canceled",
];

export function isTerminalState(state: TaskState): boolean {
  return TERMINAL_STATES.includes(state);
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
          speedBps: progress.speedBps ?? task.speedBps,
          etaSecs: progress.etaSecs ?? task.etaSecs,
          playlistIndex: progress.playlistIndex ?? task.playlistIndex,
          playlistCount: progress.playlistCount ?? task.playlistCount,
        }
      : task,
  );
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
export {
  formatDuration,
  formatEta,
  formatPercent,
  formatSpeed,
  formatUploadDate,
  formatViews,
} from "../lib/format";
export { formatTimecode, parseTimecode } from "../lib/timecode";
