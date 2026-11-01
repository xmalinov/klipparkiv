import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const info = {
  root: "C:\\apps\\klipparkiv",
  writable: true,
  ytdlp: "2026.10.26",
  ffmpeg: "2026-10-29",
  ffprobe: "2026-10-29",
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

  it("warns about missing binaries", async () => {
    invoke.mockResolvedValue({ ...info, ytdlp: null });

    render(<App />);

    expect(
      await screen.findByText("Не найдены в bin\\: yt-dlp"),
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
