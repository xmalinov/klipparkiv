import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

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
});
