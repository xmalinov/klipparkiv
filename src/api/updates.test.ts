import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  cancelPendingUpdate,
  checkAppUpdate,
  downloadAppUpdate,
  formatBytes,
  getUpdateStatus,
  installAppUpdate,
  openManualUpdate,
  subscribeUpdate,
  type UpdateStatus,
} from "./updates";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const STATUS: UpdateStatus = {
  state: "available",
  currentVersion: "1.7.1",
  version: "1.8.0",
  notes: "Fixes",
  size: 2048,
  downloaded: 0,
  percent: 0,
  error: null,
};

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  listen.mockResolvedValue(() => {});
});

describe("updates api", () => {
  it("reads the current update status", async () => {
    invoke.mockResolvedValue(STATUS);
    await expect(getUpdateStatus()).resolves.toEqual(STATUS);
    expect(invoke).toHaveBeenCalledWith("get_update_status");
  });

  it("passes the silent flag when checking", async () => {
    invoke.mockResolvedValue(STATUS);
    await checkAppUpdate();
    expect(invoke).toHaveBeenLastCalledWith("check_app_update", { silent: false });
    await checkAppUpdate(true);
    expect(invoke).toHaveBeenLastCalledWith("check_app_update", { silent: true });
  });

  it("downloads, installs, opens manual updates and cancels", async () => {
    invoke.mockResolvedValue(STATUS);
    await downloadAppUpdate();
    expect(invoke).toHaveBeenLastCalledWith("download_app_update");
    await installAppUpdate();
    expect(invoke).toHaveBeenLastCalledWith("install_app_update");
    await openManualUpdate();
    expect(invoke).toHaveBeenLastCalledWith("open_manual_update");
    await cancelPendingUpdate();
    expect(invoke).toHaveBeenLastCalledWith("cancel_pending_update");
  });

  it("forwards pushed update states", async () => {
    const handler = vi.fn();
    await subscribeUpdate(handler);
    expect(listen).toHaveBeenCalledWith("update://state", expect.any(Function));

    const callback = listen.mock.calls[0][1] as (event: {
      payload: UpdateStatus;
    }) => void;
    callback({ payload: STATUS });
    expect(handler).toHaveBeenCalledWith(STATUS);
  });

  it("formats bytes for both languages", () => {
    expect(formatBytes(null)).toBe("");
    expect(formatBytes(Number.NaN)).toBe("");
    expect(formatBytes(512)).toBe("512 Б");
    expect(formatBytes(512, "en")).toBe("512 B");
    expect(formatBytes(2048)).toBe("2 КБ");
    expect(formatBytes(2048, "en")).toBe("2 KB");
    expect(formatBytes(1024 * 1024 * 1.5)).toBe("1,5 МБ");
    expect(formatBytes(1024 * 1024 * 1.5, "en")).toBe("1.5 MB");
  });
});
