import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateStatus } from "../../api/updates";
import SettingsDialog from "./SettingsDialog";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
const updates = vi.hoisted(() => ({
  getUpdateStatus: vi.fn(),
  subscribeUpdate: vi.fn(),
  checkAppUpdate: vi.fn(),
  downloadAppUpdate: vi.fn(),
  installAppUpdate: vi.fn(),
  cancelPendingUpdate: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("../../api/updates", () => updates);

const updateStatus = (patch: Partial<UpdateStatus>): UpdateStatus => ({
  state: "idle",
  currentVersion: "1.7.1",
  version: null,
  notes: null,
  size: null,
  downloaded: 0,
  percent: 0,
  error: null,
  ...patch,
});

const settings = {
  audioSubdir: "audio",
  videoSubdir: "video",
  concurrency: 2,
  theme: "dark",
  language: "ru",
  scanDepth: 3,
  autoscan: true,
  embedThumbnail: true,
};

const info = {
  root: "C:\\apps\\klipparkiv",
  supportRoot: "C:\\apps\\klipparkiv\\data",
  installKind: "portable" as const,
  writable: true,
  version: "1.1.0",
  updateConfigured: true,
  binDir: "C:\\apps\\klipparkiv\\bin",
  ytdlp: "2026.10.26",
  ffmpeg: "2026-10-29",
  ffprobe: "2026-10-29",
  binaries: [
    {
      name: "yt-dlp",
      path: "C:\\apps\\klipparkiv\\bin\\yt-dlp.exe",
      version: "2026.10.26",
      error: null,
    },
    {
      name: "ffmpeg",
      path: "C:\\apps\\klipparkiv\\bin\\ffmpeg.exe",
      version: "2026-10-29",
      error: null,
    },
    {
      name: "ffprobe",
      path: "C:\\apps\\klipparkiv\\bin\\ffprobe.exe",
      version: "2026-10-29",
      error: null,
    },
  ],
};

describe("SettingsDialog", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    listen.mockResolvedValue(() => {});
    updates.getUpdateStatus.mockReset().mockResolvedValue(null);
    updates.subscribeUpdate.mockReset().mockResolvedValue(() => {});
    updates.checkAppUpdate.mockReset();
    updates.downloadAppUpdate.mockReset();
    updates.installAppUpdate.mockReset();
    updates.cancelPendingUpdate.mockReset();
    invoke.mockImplementation((command: string) => {
      if (command === "get_settings") return Promise.resolve(settings);
      if (command === "save_settings")
        return Promise.resolve({ ...settings, concurrency: 4 });
      if (command === "update_ytdlp") return Promise.resolve("2026.11.01");
      return Promise.resolve(null);
    });
  });

  it("loads current settings and saves changes", async () => {
    const user = userEvent.setup();
    const onSaved = vi.fn();
    const onClose = vi.fn();
    render(<SettingsDialog info={info} onClose={onClose} onSaved={onSaved} />);

    const select = await screen.findByLabelText("Одновременных загрузок");
    await user.selectOptions(select, "4");
    await user.click(screen.getByRole("button", { name: "Сохранить" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("save_settings", {
        settings: { ...settings, concurrency: 4 },
      }),
    );
    expect(onSaved).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("applies the selected theme", async () => {
    const user = userEvent.setup();
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("button", { name: "Светлая" }));
    await user.click(screen.getByRole("button", { name: "Сохранить" }));

    await waitFor(() =>
      expect(document.documentElement.dataset.theme).toBeDefined(),
    );
  });

  it("updates yt-dlp from the updates tab", async () => {
    const user = userEvent.setup();
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    await user.click(screen.getByRole("button", { name: "Обновить" }));

    expect(await screen.findByText("yt-dlp обновлён до 2026.11.01")).toBeInTheDocument();
  });

  it("shows the portable root in about", async () => {
    const user = userEvent.setup();
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "О программе" }));
    expect(screen.getByText("Портативный, без установки")).toBeInTheDocument();
  });

  it("checks for app updates and offers the download", async () => {
    const user = userEvent.setup();
    updates.checkAppUpdate.mockResolvedValue(
      updateStatus({ state: "available", version: "1.8.0" }),
    );
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    await user.click(screen.getByRole("button", { name: "Проверить обновления" }));

    expect(updates.checkAppUpdate).toHaveBeenCalledWith(false);
    expect(await screen.findByText("Доступна версия 1.8.0")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Скачать обновление" }),
    ).toBeEnabled();
  });

  it("downloads and installs an available update", async () => {
    const user = userEvent.setup();
    updates.getUpdateStatus.mockResolvedValue(
      updateStatus({ state: "available", version: "1.8.0" }),
    );
    updates.downloadAppUpdate.mockResolvedValue(
      updateStatus({ state: "ready", version: "1.8.0" }),
    );
    updates.installAppUpdate.mockResolvedValue(
      updateStatus({ state: "installing", version: "1.8.0" }),
    );
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    await user.click(screen.getByRole("button", { name: "Скачать обновление" }));

    expect(updates.downloadAppUpdate).toHaveBeenCalled();
    expect(
      await screen.findByText("Версия 1.8.0 готова к установке"),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Перезапустить и обновить" }),
    );
    expect(updates.installAppUpdate).toHaveBeenCalled();
    expect(
      await screen.findByRole("button", { name: "Перезапускаем…" }),
    ).toBeDisabled();
  });

  it("cancels a deferred update", async () => {
    const user = userEvent.setup();
    updates.getUpdateStatus.mockResolvedValue(
      updateStatus({ state: "deferred", version: "1.8.0" }),
    );
    updates.cancelPendingUpdate.mockResolvedValue(
      updateStatus({ state: "ready", version: "1.8.0" }),
    );
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    expect(
      screen.getByText("Установится после завершения загрузок"),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Отменить обновление" }),
    );
    expect(updates.cancelPendingUpdate).toHaveBeenCalled();
    expect(
      await screen.findByText("Версия 1.8.0 готова к установке"),
    ).toBeInTheDocument();
  });

  it("reports a failed update check", async () => {
    const user = userEvent.setup();
    updates.checkAppUpdate.mockRejectedValue("network down");
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    await user.click(screen.getByRole("button", { name: "Проверить обновления" }));

    expect(await screen.findByText("network down")).toBeInTheDocument();
  });

  it("shows download progress and disables the button", async () => {
    const user = userEvent.setup();
    updates.getUpdateStatus.mockResolvedValue(
      updateStatus({
        state: "downloading",
        version: "1.8.0",
        percent: 42.6,
      }),
    );
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    expect(
      screen.getByText("Скачиваем обновление… 43%"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Скачиваем…" })).toBeDisabled();
  });

  it("reports that the installed version is current", async () => {
    const user = userEvent.setup();
    updates.getUpdateStatus.mockResolvedValue(updateStatus({ state: "current" }));
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    expect(
      screen.getByText("Установлена последняя версия"),
    ).toBeInTheDocument();
  });

  it("reacts to pushed update statuses", async () => {
    const user = userEvent.setup();
    let push: ((status: UpdateStatus) => void) | undefined;
    updates.subscribeUpdate.mockImplementation((handler) => {
      push = handler;
      return Promise.resolve(() => {});
    });
    render(<SettingsDialog info={info} onClose={() => {}} onSaved={() => {}} />);
    await screen.findByLabelText("Одновременных загрузок");

    await act(async () => {
      push?.(updateStatus({ state: "available", version: "2.0.0" }));
    });
    await user.click(screen.getByRole("tab", { name: "Обновления" }));

    expect(screen.getByText("Доступна версия 2.0.0")).toBeInTheDocument();
  });

  it("warns when self-updates are not configured", async () => {
    const user = userEvent.setup();
    render(
      <SettingsDialog
        info={{ ...info, updateConfigured: false }}
        onClose={() => {}}
        onSaved={() => {}}
      />,
    );
    await screen.findByLabelText("Одновременных загрузок");

    await user.click(screen.getByRole("tab", { name: "Обновления" }));
    expect(
      screen.getByText(
        "Автообновление не настроено в этой сборке. Скачайте новую версию со страницы релизов.",
      ),
    ).toBeInTheDocument();
  });
});
