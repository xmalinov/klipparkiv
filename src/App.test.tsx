import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: () => Promise.resolve(() => {}),
  }),
}));

const binaries = [
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
];

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
  binaries,
};

function routeCommands() {
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case "app_info":
        return Promise.resolve(info);
      case "get_settings":
        return Promise.resolve({
          audioSubdir: "audio",
          videoSubdir: "video",
          concurrency: 2,
          theme: "dark",
          language: "ru",
          scanDepth: 3,
          autoscan: true,
          embedThumbnail: true,
        });
      case "library_scan":
        return Promise.resolve({ scanned: 0, added: 0, updated: 0, removed: 0 });
      case "library_query":
        return Promise.resolve({ items: [], total: 0 });
      case "practice_recent":
        return Promise.resolve([]);
      case "practice_analysis_get":
        return Promise.resolve(null);
      case "get_update_status":
        return Promise.resolve({
          state: "idle",
          currentVersion: "1.7.1",
          version: null,
          notes: null,
          size: null,
          downloaded: 0,
          percent: 0,
          error: null,
        });
      default:
        return Promise.resolve(null);
    }
  });
}

describe("App", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    listen.mockResolvedValue(() => {});
  });

  it("renders the shell and the download screen", async () => {
    invoke.mockResolvedValue(info);

    render(<App />);

    expect(screen.getByRole("heading", { name: "Загрузка" })).toBeInTheDocument();
    expect(screen.getByText("Библиотека")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Вставьте ссылку на YouTube или введите её вручную"),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("yt-dlp 2026.10.26 · ffmpeg 2026-10-29"),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("app_info");
  });

  it("warns about missing binaries with the path and the reason", async () => {
    const error = "файл не найден: C:\\apps\\klipparkiv\\bin\\yt-dlp.exe";
    invoke.mockResolvedValue({
      ...info,
      ytdlp: null,
      binaries: info.binaries.map((binary) =>
        binary.name === "yt-dlp"
          ? { ...binary, version: null, error }
          : binary,
      ),
    });

    render(<App />);

    expect(
      await screen.findByText(
        "В папке C:\\apps\\klipparkiv\\bin не найдены: yt-dlp",
      ),
    ).toBeInTheDocument();
    expect(screen.getByText(error)).toBeInTheDocument();
    expect(
      screen.getByText(/Распакуйте архив целиком/),
    ).toBeInTheDocument();
  });

  it("warns about a read-only root", async () => {
    invoke.mockResolvedValue({ ...info, writable: false });

    render(<App />);

    expect(
      await screen.findByText("Нет прав на запись в папку приложения"),
    ).toBeInTheDocument();
  });

  it("shows an alert when the backend is unreachable", async () => {
    invoke.mockRejectedValue("backend is down");

    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Не удалось прочитать окружение: backend is down",
    );
  });

  it("navigates between download, library and practice", async () => {
    const user = userEvent.setup();
    routeCommands();
    render(<App />);
    await screen.findByText(/yt-dlp 2026\.10\.26/);

    await user.click(screen.getByRole("button", { name: "Библиотека" }));
    expect(
      screen.getByRole("heading", { name: "Библиотека" }),
    ).toBeInTheDocument();
    expect(await screen.findByText("Здесь появятся скачанные файлы")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Практика" }));
    expect(
      screen.getByRole("heading", { name: "Практика" }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Загрузка" }));
    expect(
      screen.getByRole("heading", { name: "Загрузка" }),
    ).toBeInTheDocument();
  });

  it("opens and closes the settings dialog", async () => {
    const user = userEvent.setup();
    routeCommands();
    render(<App />);
    await screen.findByText(/yt-dlp 2026\.10\.26/);

    await user.click(screen.getByRole("button", { name: "Настройки" }));
    expect(screen.getByRole("dialog", { name: "Настройки" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Закрыть" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
