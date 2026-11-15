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
  applyProgress,
  cancelTask as cancelTaskCommand,
  getQueueState,
  listTasks,
  pauseQueue,
  removeTask as removeTaskCommand,
  resumeQueue,
  subscribeTasks,
  upsertTask,
  type QueueState,
  type TaskSnapshot,
} from "../api/tasks";

export const EMPTY_QUEUE: QueueState = { paused: false, running: 0, queued: 0 };

interface QueueContextValue {
  tasks: TaskSnapshot[];
  queue: QueueState;
  addTasks: (created: TaskSnapshot[]) => void;
  cancel: (id: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
  togglePause: () => Promise<void>;
}

const QueueContext = createContext<QueueContextValue | null>(null);

/** Task ids are `t1`, `t2`, … and grow with creation order. */
function taskOrder(id: string): number {
  const match = /^t(\d+)$/.exec(id);
  return match ? Number(match[1]) : Number.MAX_SAFE_INTEGER;
}

/**
 * `list_tasks` reads a hash map on the backend, so the order is arbitrary.
 * The numeric id is the only stable record of creation order.
 */
export function sortTasks(tasks: TaskSnapshot[]): TaskSnapshot[] {
  return [...tasks].sort(
    (left, right) =>
      taskOrder(left.id) - taskOrder(right.id) || left.id.localeCompare(right.id),
  );
}

function isTaskSnapshot(value: unknown): value is TaskSnapshot {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { id?: unknown }).id === "string" &&
    typeof (value as { state?: unknown }).state === "string"
  );
}

function normalizeTasks(value: unknown): TaskSnapshot[] {
  return Array.isArray(value) ? value.filter(isTaskSnapshot) : [];
}

function normalizeQueue(value: unknown): QueueState | null {
  if (typeof value !== "object" || value === null) return null;
  const candidate = value as Partial<QueueState>;
  if (typeof candidate.paused !== "boolean") return null;
  return {
    paused: candidate.paused,
    running: Number.isFinite(candidate.running) ? Number(candidate.running) : 0,
    queued: Number.isFinite(candidate.queued) ? Number(candidate.queued) : 0,
  };
}

/**
 * Owns the download queue for the whole shell.
 *
 * The backend keeps running when a screen unmounts, so the tasks must not live
 * in `DownloadScreen`: the list is restored from `list_tasks` and kept current
 * by `task://state` / `task://progress` / `queue://changed` events.
 */
export function QueueProvider({ children }: { children: ReactNode }) {
  const [tasks, setTasks] = useState<TaskSnapshot[]>([]);
  const [queue, setQueue] = useState<QueueState>(EMPTY_QUEUE);

  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    let queueFromEvent = false;

    subscribeTasks({
      onState: (task) =>
        setTasks((current) => sortTasks(upsertTask(current, task))),
      onProgress: (progress) =>
        setTasks((current) => applyProgress(current, progress)),
      onQueue: (state) => {
        queueFromEvent = true;
        setQueue(normalizeQueue(state) ?? EMPTY_QUEUE);
      },
    })
      .then((unlisten) => {
        if (!alive) {
          unlisten();
          return;
        }
        stop = unlisten;
        // Subscribe first, fetch second: an event that arrives while the
        // snapshot is loading is newer than the snapshot and must win.
        return Promise.all([listTasks(), getQueueState()]).then(
          ([restored, state]) => {
            if (!alive) return;
            setTasks((current) => {
              const byId = new Map<string, TaskSnapshot>();
              for (const task of normalizeTasks(restored)) {
                byId.set(task.id, task);
              }
              for (const task of current) byId.set(task.id, task);
              return sortTasks([...byId.values()]);
            });
            if (!queueFromEvent) {
              setQueue(normalizeQueue(state) ?? EMPTY_QUEUE);
            }
          },
        );
      })
      .catch(() => {
        // A failed restore must not hide the download form: the queue stays
        // empty and future events still populate it.
      });

    return () => {
      alive = false;
      stop?.();
    };
  }, []);

  const addTasks = useCallback((created: TaskSnapshot[]) => {
    if (created.length === 0) return;
    setTasks((current) => sortTasks(created.reduce(upsertTask, current)));
  }, []);

  const cancel = useCallback(async (id: string) => {
    await cancelTaskCommand(id);
  }, []);

  const remove = useCallback(async (id: string) => {
    await removeTaskCommand(id);
    // The backend has no "removed" event: the list is the only consumer.
    setTasks((current) => current.filter((task) => task.id !== id));
  }, []);

  const togglePause = useCallback(async () => {
    const next = queue.paused ? await resumeQueue() : await pauseQueue();
    setQueue(normalizeQueue(next) ?? EMPTY_QUEUE);
  }, [queue.paused]);

  const value = useMemo<QueueContextValue>(
    () => ({ tasks, queue, addTasks, cancel, remove, togglePause }),
    [addTasks, cancel, queue, remove, tasks, togglePause],
  );

  return <QueueContext.Provider value={value}>{children}</QueueContext.Provider>;
}

export function useQueue(): QueueContextValue {
  const context = useContext(QueueContext);
  if (!context) {
    throw new Error("useQueue must be used inside QueueProvider");
  }
  return context;
}
