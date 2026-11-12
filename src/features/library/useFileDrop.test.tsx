import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useFileDrop } from "./useFileDrop";

const webview = vi.hoisted(() => ({
  handlers: [] as ((event: { payload: unknown }) => void)[],
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (handler: (event: { payload: unknown }) => void) => {
      webview.handlers.push(handler);
      return Promise.resolve(webview.unlisten);
    },
  }),
}));

function lastHandler() {
  return webview.handlers[webview.handlers.length - 1];
}

function enter(paths: string[]) {
  lastHandler()({ payload: { type: "enter", paths } });
}

function leave() {
  lastHandler()({ payload: { type: "leave" } });
}

function drop(paths: string[]) {
  lastHandler()({ payload: { type: "drop", paths } });
}

describe("useFileDrop", () => {
  beforeEach(() => {
    webview.handlers = [];
    webview.unlisten.mockReset();
  });

  it("tracks the drag state and reports dropped paths", async () => {
    const onDrop = vi.fn();
    const { result } = renderHook(() => useFileDrop({ onDrop }));
    await waitFor(() => expect(webview.handlers).toHaveLength(1));

    act(() => enter(["C:\\music\\a.mp3"]));
    expect(result.current.dragging).toBe(true);

    act(() => leave());
    expect(result.current.dragging).toBe(false);

    act(() => enter(["C:\\music\\a.mp3"]));
    act(() => drop(["C:\\music\\a.mp3"]));
    expect(result.current.dragging).toBe(false);
    expect(onDrop).toHaveBeenCalledWith(["C:\\music\\a.mp3"]);
  });

  it("ignores drops while disabled", async () => {
    const onDrop = vi.fn();
    const { result, rerender } = renderHook(
      ({ disabled }) => useFileDrop({ onDrop, disabled }),
      { initialProps: { disabled: false } },
    );
    await waitFor(() => expect(webview.handlers).toHaveLength(1));

    act(() => enter(["C:\\music\\a.mp3"]));
    expect(result.current.dragging).toBe(true);

    rerender({ disabled: true });
    expect(result.current.dragging).toBe(false);
    act(() => drop(["C:\\music\\a.mp3"]));
    expect(onDrop).not.toHaveBeenCalled();
  });

  it("unsubscribes on unmount", async () => {
    const { unmount } = renderHook(() => useFileDrop({ onDrop: vi.fn() }));
    await waitFor(() => expect(webview.handlers).toHaveLength(1));

    unmount();
    await waitFor(() => expect(webview.unlisten).toHaveBeenCalled());
  });
});
