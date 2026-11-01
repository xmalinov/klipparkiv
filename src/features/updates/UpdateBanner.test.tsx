import { render, screen } from "@testing-library/react";
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
});
