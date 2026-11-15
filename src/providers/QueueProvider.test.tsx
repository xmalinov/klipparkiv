import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  createCommandRouter,
  createEventBus,
  flush,
  type RoutedCommands,
} from "../test/tauri";
import { QueueProvider, useQueue } from "./QueueProvider";
import type { TaskSnapshot } from "../api/tasks";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

function makeTask(overrides: Partial<TaskSnapshot> = {}): TaskSnapshot {
  return {
    id: "t1",
    url: "https://example.com/a",
    title: null,
    mode: "audio",
    clip: false,
    state: "queued",
    percent: 0,
    speedBps: null,
    etaSecs: null,
    error: null,
    playlistIndex: null,
    playlistCount: null,
    outputPath: null,
    ...overrides,
  };
}

function Probe() {
  const { tasks, queue, addTasks, cancel, remove, togglePause } = useQueue();
  return (
    <div>
      <span data-testid="count">{tasks.length}</span>
      <span data-testid="paused">{String(queue.paused)}</span>
      <ul>
        {tasks.map((task) => (
          <li key={task.id} data-testid={`task-${task.id}`}>
            {Math.round(task.percent)}
          </li>
        ))}
      </ul>
      <button type="button" onClick={() => addTasks([makeTask({ id: "t9" })])}>
        add
      </button>
      <button type="button" onClick={() => void cancel("t1")}>
        cancel
      </button>
      <button
        type="button"
        onClick={() => {
          void remove("t1").catch(() => {});
        }}
      >
        remove
      </button>
      <button type="button" onClick={() => void togglePause()}>
        toggle
      </button>
    </div>
  );
}

function renderProvider() {
  return render(
    <QueueProvider>
      <Probe />
    </QueueProvider>,
  );
}

let router: RoutedCommands;
let bus: ReturnType<typeof createEventBus>;

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  router = createCommandRouter(invoke);
  bus = createEventBus(listen);
});

describe("QueueProvider", () => {
  it("restores the queue and the paused flag from the backend", async () => {
    router.on("list_tasks", () => [
      makeTask({ id: "t1", state: "done", percent: 100 }),
    ]);
    router.on("queue_state", () => ({ paused: true, running: 0, queued: 0 }));

    renderProvider();

    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));
    expect(screen.getByTestId("paused")).toHaveTextContent("true");
    expect(router.count("list_tasks")).toBe(1);
    expect(router.count("queue_state")).toBe(1);
  });

  it("restores tasks in creation order, not hash-map order", async () => {
    router.on("list_tasks", () => [
      makeTask({ id: "t10" }),
      makeTask({ id: "t2" }),
      makeTask({ id: "t1" }),
    ]);

    renderProvider();

    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("3"));
    expect(
      [...screen.getAllByTestId(/^task-t\d+$/u)].map((node) =>
        node.getAttribute("data-testid"),
      ),
    ).toEqual(["task-t1", "task-t2", "task-t10"]);
  });

  it("keeps the tasks when the consumer unmounts and mounts again", async () => {
    router.on("list_tasks", () => [makeTask({ id: "t1", percent: 40 })]);

    const first = renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));
    first.unmount();

    renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));
    expect(screen.getByTestId("task-t1")).toHaveTextContent("40");
  });

  it("applies progress events to a restored task", async () => {
    router.on("list_tasks", () => [makeTask({ id: "t1", state: "running" })]);
    renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));

    await bus.emit("task://progress", {
      id: "t1",
      percent: 63.2,
      speedBps: 1_000_000,
      etaSecs: 10,
      playlistIndex: null,
      playlistCount: null,
    });

    expect(screen.getByTestId("task-t1")).toHaveTextContent("63");
  });

  it("does not lose an event that arrives while the snapshot is loading", async () => {
    let resolveSnapshot: (value: TaskSnapshot[]) => void = () => {};
    router.on(
      "list_tasks",
      () =>
        new Promise<TaskSnapshot[]>((resolve) => {
          resolveSnapshot = resolve;
        }),
    );
    renderProvider();
    await flush();

    // A full state event can create a task that the (still pending) snapshot
    // does not know about yet.
    await bus.emit(
      "task://state",
      makeTask({ id: "t1", state: "running", percent: 55 }),
    );
    await act(async () =>
      resolveSnapshot([makeTask({ id: "t1", state: "queued", percent: 5 })]),
    );

    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));
    expect(screen.getByTestId("task-t1")).toHaveTextContent("55");
  });

  it("adds the tasks returned by an enqueue command", async () => {
    const user = userEvent.setup();
    renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("0"));

    await user.click(screen.getByRole("button", { name: "add" }));

    expect(screen.getByTestId("count")).toHaveTextContent("1");
    expect(screen.getByTestId("task-t9")).toBeInTheDocument();
  });

  it("cancels a task through the backend command", async () => {
    const user = userEvent.setup();
    router.on("list_tasks", () => [makeTask({ id: "t1" })]);
    renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));

    await user.click(screen.getByRole("button", { name: "cancel" }));

    await waitFor(() =>
      expect(router.args("cancel_task")).toEqual([{ id: "t1" }]),
    );
  });

  it("toggles pause and exposes the new state", async () => {
    const user = userEvent.setup();
    router.on("pause_queue", () => ({
      paused: true,
      running: 0,
      queued: 0,
    }));
    router.on("resume_queue", () => ({
      paused: false,
      running: 0,
      queued: 0,
    }));
    renderProvider();

    await user.click(screen.getByRole("button", { name: "toggle" }));
    await waitFor(() =>
      expect(screen.getByTestId("paused")).toHaveTextContent("true"),
    );

    await user.click(screen.getByRole("button", { name: "toggle" }));
    await waitFor(() =>
      expect(screen.getByTestId("paused")).toHaveTextContent("false"),
    );
    expect(router.count("pause_queue")).toBe(1);
    expect(router.count("resume_queue")).toBe(1);
  });

  it("removes a finished task after the backend confirms it", async () => {
    const user = userEvent.setup();
    router.on("list_tasks", () => [makeTask({ id: "t1", state: "done" })]);
    renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));

    await user.click(screen.getByRole("button", { name: "remove" }));

    await waitFor(() => expect(router.args("remove_task")).toEqual([{ id: "t1" }]));
    expect(screen.getByTestId("count")).toHaveTextContent("0");
  });

  it("keeps the task when the backend refuses to remove it", async () => {
    const user = userEvent.setup();
    router.on("list_tasks", () => [makeTask({ id: "t1", state: "running" })]);
    router.fail("remove_task", "cancel it first");
    renderProvider();
    await waitFor(() => expect(screen.getByTestId("count")).toHaveTextContent("1"));

    await user.click(screen.getByRole("button", { name: "remove" }));

    await waitFor(() => expect(router.count("remove_task")).toBe(1));
    expect(screen.getByTestId("count")).toHaveTextContent("1");
  });

  it("survives a malformed snapshot instead of blanking the shell", async () => {
    router.on("list_tasks", () => ({ not: "an array" }));
    router.on("queue_state", () => "garbage");

    renderProvider();

    await waitFor(() => expect(router.count("list_tasks")).toBe(1));
    expect(screen.getByTestId("count")).toHaveTextContent("0");
    expect(screen.getByTestId("paused")).toHaveTextContent("false");
  });

  it("keeps working when the restore request fails", async () => {
    router.fail("list_tasks", "backend is down");
    router.fail("queue_state", "backend is down");

    renderProvider();
    await flush();

    expect(screen.getByTestId("count")).toHaveTextContent("0");
    await bus.emit("task://state", makeTask({ id: "t3", state: "running" }));
    expect(screen.getByTestId("count")).toHaveTextContent("1");
  });
});
