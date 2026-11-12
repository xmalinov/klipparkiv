import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ImportProgress, ImportReport } from "../../api/library";
import { ImportProvider, useImport } from "./ImportProvider";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
const handlers = vi.hoisted(
  () => ({}) as Record<string, (event: { payload: unknown }) => void>,
);

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const emptyReport: ImportReport = {
  imported: [],
  existing: [],
  skipped: [],
  failed: [],
  cancelled: false,
};

function Consumer() {
  const importer = useImport()!;
  return (
    <div>
      <span>{importer.importing ? "importing" : "idle"}</span>
      {importer.progress && (
        <span>{`${importer.progress.index}/${importer.progress.total} ${importer.progress.name}`}</span>
      )}
      {importer.error && <span role="alert">{importer.error}</span>}
      <button
        type="button"
        onClick={() => void importer.startImport(["C:\\a.mp3"]).catch(() => {})}
      >
        start
      </button>
      <button type="button" onClick={importer.cancelImport}>
        cancel
      </button>
    </div>
  );
}

function renderProvider() {
  render(
    <ImportProvider>
      <Consumer />
    </ImportProvider>,
  );
}

describe("ImportProvider", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    Object.keys(handlers).forEach((key) => delete handlers[key]);
    listen.mockImplementation(
      async (event: string, handler: (payload: { payload: unknown }) => void) => {
        handlers[event] = handler;
        return () => {};
      },
    );
  });

  it("runs one job, exposes progress and returns the report", async () => {
    let finish: (report: ImportReport) => void = () => {};
    invoke.mockImplementation((command: string) => {
      if (command === "library_import") {
        return new Promise<ImportReport>((resolve) => {
          finish = resolve;
        });
      }
      return Promise.resolve(true);
    });
    renderProvider();

    await userEvent.click(screen.getByRole("button", { name: "start" }));
    expect(screen.getByText("importing")).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("library_import", {
      paths: ["C:\\a.mp3"],
    });

    const progress: ImportProgress = {
      jobId: 1,
      index: 1,
      total: 2,
      name: "a.mp3",
      copiedBytes: 10,
      totalBytes: 20,
      phase: "copying",
    };
    act(() =>
      handlers["library://import-progress"]({ payload: progress }),
    );
    expect(screen.getByText("1/2 a.mp3")).toBeInTheDocument();

    await act(async () => finish(emptyReport));
    await waitFor(() => expect(screen.getByText("idle")).toBeInTheDocument());
    expect(screen.queryByText("1/2 a.mp3")).not.toBeInTheDocument();
  });

  it("cancels through the command and Esc while a job is active", async () => {
    invoke.mockImplementation((command: string) =>
      command === "library_import"
        ? new Promise<ImportReport>(() => {})
        : Promise.resolve(true),
    );
    renderProvider();
    await userEvent.click(screen.getByRole("button", { name: "start" }));

    await userEvent.click(screen.getByRole("button", { name: "cancel" }));
    expect(invoke).toHaveBeenCalledWith("library_import_cancel");

    fireEvent.keyDown(window, { key: "Escape" });
    expect(
      invoke.mock.calls.filter(([command]) => command === "library_import_cancel"),
    ).toHaveLength(2);
  });

  it("shows the backend error and resets the job", async () => {
    invoke.mockImplementation((command: string) =>
      command === "library_import"
        ? Promise.reject("disk is full")
        : Promise.resolve(true),
    );
    renderProvider();

    await userEvent.click(screen.getByRole("button", { name: "start" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("disk is full");
    expect(screen.getByText("idle")).toBeInTheDocument();
  });

  it("rejects a second start while the first job is running", async () => {
    invoke.mockImplementation((command: string) =>
      command === "library_import"
        ? new Promise<ImportReport>(() => {})
        : Promise.resolve(true),
    );
    renderProvider();

    await userEvent.click(screen.getByRole("button", { name: "start" }));
    await userEvent.click(screen.getByRole("button", { name: "start" }));

    expect(invoke.mock.calls.filter(([command]) => command === "library_import"))
      .toHaveLength(1);
  });
});
