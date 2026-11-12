import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import UpdateBanner from "./UpdateBanner";
import type { UpdateStatus } from "../../api/updates";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const available: UpdateStatus = {
  state: "available",
  currentVersion: "1.1.0",
  version: "1.2.0",
  notes: "Test release",
  size: 7_000_000,
  downloaded: 0,
  percent: 0,
  error: null,
};

const ready: UpdateStatus = {
  ...available,
  state: "ready",
  downloaded: 7_000_000,
  percent: 100,
};

describe("UpdateBanner", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    listen.mockResolvedValue(() => {});
  });

  it("shows an available update and downloads it", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) => {
      if (command === "get_update_status") return Promise.resolve(available);
      if (command === "download_app_update") return Promise.resolve(ready);
      return Promise.resolve(null);
    });

    render(<UpdateBanner />);

    expect(await screen.findByText("Доступно обновление 1.2.0")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Обновить" }));

    expect(
      await screen.findByText("Обновление 1.2.0 готово к установке"),
    ).toBeInTheDocument();
  });

  it("shows a deferred update after clicking restart", async () => {
    const user = userEvent.setup();
    const deferred: UpdateStatus = { ...ready, state: "deferred" };
    invoke.mockImplementation((command: string) => {
      if (command === "get_update_status") return Promise.resolve(ready);
      if (command === "install_app_update") return Promise.resolve(deferred);
      return Promise.resolve(null);
    });

    render(<UpdateBanner />);

    await user.click(
      await screen.findByRole("button", { name: "Перезапустить и обновить" }),
    );

    expect(
      await screen.findByText("Обновление установится после завершения загрузок"),
    ).toBeInTheDocument();
  });

  it("hides the banner when the update is postponed", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue(available);
    render(<UpdateBanner />);
    await screen.findByText("Доступно обновление 1.2.0");

    await user.click(screen.getByRole("button", { name: "Позже" }));

    expect(screen.queryByRole("status")).toBeNull();
  });

  it("shows download progress and hides the postpone action", async () => {
    invoke.mockResolvedValue({
      ...available,
      state: "downloading",
      percent: 42.6,
      downloaded: 3_000_000,
    });
    render(<UpdateBanner />);

    expect(
      await screen.findByText("Скачиваем обновление… 43%"),
    ).toBeInTheDocument();
    expect(screen.getByRole("progressbar")).toHaveAttribute(
      "aria-valuenow",
      "43",
    );
    expect(screen.queryByRole("button", { name: "Позже" })).toBeNull();
  });

  it("offers a manual DMG download", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) => {
      if (command === "get_update_status")
        return Promise.resolve({ ...available, state: "manual" });
      if (command === "open_manual_update")
        return Promise.resolve({ ...available, state: "manual" });
      return Promise.resolve(null);
    });
    render(<UpdateBanner />);

    expect(
      await screen.findByText(
        "Обновление 1.2.0 скачано. Установите вручную.",
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Открыть DMG" }));
    expect(invoke).toHaveBeenCalledWith("open_manual_update");
  });

  it("shows an error and retries the check", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) => {
      if (command === "get_update_status")
        return Promise.resolve({
          ...available,
          state: "error",
          version: null,
          error: "network down",
        });
      if (command === "check_app_update") return Promise.resolve(available);
      return Promise.resolve(null);
    });
    render(<UpdateBanner />);

    expect(
      await screen.findByText("Не удалось обновить: network down"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Повторить" }));

    expect(invoke).toHaveBeenCalledWith("check_app_update", { silent: false });
    expect(
      await screen.findByText("Доступно обновление 1.2.0"),
    ).toBeInTheDocument();
  });

  it("renders nothing for an idle status", async () => {
    invoke.mockResolvedValue({ ...available, state: "idle" });
    const { container } = render(<UpdateBanner />);

    await waitFor(() => expect(invoke).toHaveBeenCalled());
    expect(container.firstChild).toBeNull();
  });

  it("follows pushed update states", async () => {
    let push: ((event: { payload: UpdateStatus }) => void) | undefined;
    listen.mockImplementation(
      (event: string, handler: (event: { payload: UpdateStatus }) => void) => {
        if (event === "update://state") push = handler;
        return Promise.resolve(() => {});
      },
    );
    invoke.mockResolvedValue(available);
    render(<UpdateBanner />);
    await screen.findByText("Доступно обновление 1.2.0");

    await act(async () => {
      push?.({
        payload: { ...available, state: "downloading", percent: 10 },
      });
    });

    expect(screen.getByText("Скачиваем обновление… 10%")).toBeInTheDocument();
  });

  it("unsubscribes when unmounted before the listener resolves", async () => {
    let resolveListen: ((unlisten: () => void) => void) | undefined;
    listen.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveListen = resolve;
        }),
    );
    invoke.mockResolvedValue(available);
    const { unmount } = render(<UpdateBanner />);
    await screen.findByText("Доступно обновление 1.2.0");

    const unlisten = vi.fn();
    unmount();
    await act(async () => {
      resolveListen?.(unlisten);
    });

    expect(unlisten).toHaveBeenCalled();
  });
});
