import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsDialog from "./SettingsDialog";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

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
  writable: true,
  version: "1.1.0",
  updateConfigured: true,
  ytdlp: "2026.10.26",
  ffmpeg: "2026-10-29",
  ffprobe: "2026-10-29",
};

describe("SettingsDialog", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    listen.mockResolvedValue(() => {});
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
});
