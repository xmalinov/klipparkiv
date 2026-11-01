import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DownloadScreen from "./DownloadScreen";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());
const open = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open }));

type Handler = (event: { payload: unknown }) => void;
const handlers: Record<string, Handler> = {};

const task = {
  id: "t1",
  url: "https://youtu.be/abc",
  mode: "audio",
  clip: false,
  state: "queued",
  percent: 0,
  speedBps: null,
  etaSecs: null,
  error: null,
};

const info = {
  id: "abc",
  title: "Rick Astley - Never Gonna Give You Up",
  channel: "Rick Astley",
  durationSecs: 213,
  thumbnailUrl: "https://i.ytimg.com/vi/abc/hqdefault.jpg",
  uploadDate: "20091025",
  viewCount: 1_200_000_000,
};

describe("DownloadScreen", () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue([]);
    listen.mockReset();
    open.mockReset();
    Object.keys(handlers).forEach((key) => delete handlers[key]);

    listen.mockImplementation(async (event: string, handler: Handler) => {
      handlers[event] = handler;
      return () => {};
    });
  });

  it("enqueues an audio download", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen root="C:\\apps\\klipparkiv" />);

    await user.type(
      screen.getByLabelText("Ссылка на видео"),
      "https://youtu.be/abc",
    );
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: {
          url: "https://youtu.be/abc",
          mode: "audio",
          quality: undefined,
          audioKbps: 320,
          outDir: undefined,
          clipStart: undefined,
          clipEnd: undefined,
        },
      }),
    );
    expect(await screen.findByText("В очереди")).toBeInTheDocument();
  });

  it("sends the selected quality for video", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([{ ...task, mode: "video" }]);
    render(<DownloadScreen root="C:\\apps" />);

    await user.click(screen.getByRole("tab", { name: "Видео" }));
    await user.selectOptions(screen.getByRole("combobox"), "720");
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/x");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: {
          url: "https://youtu.be/x",
          mode: "video",
          quality: "720",
          audioKbps: undefined,
          outDir: undefined,
          clipStart: undefined,
          clipEnd: undefined,
        },
      }),
    );
  });

  it("enqueues both audio and video as one request", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task, { ...task, id: "t2", mode: "video" }]);
    render(<DownloadScreen />);

    await user.click(screen.getByRole("tab", { name: "Аудио+Видео" }));
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/x");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: {
          url: "https://youtu.be/x",
          mode: "both",
          quality: "1080",
          audioKbps: 320,
          outDir: undefined,
          clipStart: undefined,
          clipEnd: undefined,
        },
      }),
    );
    expect(await screen.findAllByText("В очереди")).toHaveLength(2);
  });

  it("sends clip range for a video clip", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([{ ...task, mode: "video", clip: true }]);
    render(<DownloadScreen />);

    await user.click(screen.getByRole("tab", { name: "Клип" }));
    await user.type(screen.getByLabelText("Начало клипа"), "1:30");
    await user.type(screen.getByLabelText("Конец клипа"), "2:00");
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/x");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: {
          url: "https://youtu.be/x",
          mode: "video",
          quality: "1080",
          audioKbps: undefined,
          outDir: undefined,
          clipStart: "1:30",
          clipEnd: "2:00",
        },
      }),
    );
    expect(await screen.findByText("Видео · клип")).toBeInTheDocument();
  });

  it("requires both clip boundaries", async () => {
    const user = userEvent.setup();
    render(<DownloadScreen />);

    await user.click(screen.getByRole("tab", { name: "Клип" }));
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/x");
    await user.type(screen.getByLabelText("Начало клипа"), "1:30");

    expect(screen.getByRole("button", { name: "Скачать" })).toBeDisabled();
  });

  it("shows probed metadata", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.resolve(info)
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");

    expect(await screen.findByText(info.title, {}, { timeout: 2000 })).toBeInTheDocument();
    expect(screen.getByText(/1,2 млрд просмотров/)).toBeInTheDocument();
    expect(screen.getByText(/3:33/)).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("probe_metadata", {
      url: "https://youtu.be/abc",
    });
  });

  it("keeps the download available when probing fails", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.reject("ERROR: video unavailable")
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");

    expect(await screen.findByText("ERROR: video unavailable", {}, { timeout: 2000 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Скачать" })).toBeEnabled();
  });

  it("shows backend validation errors", async () => {
    const user = userEvent.setup();
    invoke.mockRejectedValue("Ссылка должна начинаться с http:// или https://");
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "not a link");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Ссылка должна начинаться с http://",
    );
  });

  it("updates progress from events", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await user.click(screen.getByRole("button", { name: "Скачать" }));
    await screen.findByText("В очереди");

    handlers["task://state"]({ payload: { ...task, state: "running" } });
    handlers["task://progress"]({
      payload: { id: "t1", percent: 45.4, speedBps: 2_200_000, etaSecs: 80 },
    });

    expect(await screen.findByText("45%")).toBeInTheDocument();
    expect(screen.getByText("2,1 МБ/с · 1:20")).toBeInTheDocument();
  });

  it("marks a finished download", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await user.click(screen.getByRole("button", { name: "Скачать" }));
    await screen.findByText("В очереди");

    handlers["task://state"]({ payload: { ...task, state: "done", percent: 100 } });

    expect(await screen.findByText("Готово")).toBeInTheDocument();
  });

  it("cancels a task", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await user.click(screen.getByRole("button", { name: "Скачать" }));
    await screen.findByText("В очереди");

    await user.click(screen.getByRole("button", { name: /Отменить/ }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("cancel_task", { id: "t1" }),
    );
  });

  it("disables the button without a url", () => {
    render(<DownloadScreen />);
    expect(screen.getByRole("button", { name: "Скачать" })).toBeDisabled();
  });

  it("sends the selected audio bitrate", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen />);

    await user.selectOptions(screen.getByLabelText("Качество аудио"), "128");
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/a");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: expect.objectContaining({ audioKbps: 128 }),
      }),
    );
  });

  it("downloads into a chosen folder", async () => {
    const user = userEvent.setup();
    open.mockResolvedValue("D:\\Downloads");
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen root="C:\\apps\\klipparkiv" />);

    await user.click(screen.getByRole("button", { name: "Изменить" }));
    expect(await screen.findByText(/папка вне приложения/i)).toBeInTheDocument();

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/a");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: expect.objectContaining({ outDir: "D:\\Downloads" }),
      }),
    );
  });

  it("returns to the app folder", async () => {
    const user = userEvent.setup();
    open.mockResolvedValue("D:\\Downloads");
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen root="C:\\apps\\klipparkiv" />);

    await user.click(screen.getByRole("button", { name: "Изменить" }));
    await screen.findByText(/папка вне приложения/i);
    await user.click(screen.getByRole("button", { name: "По умолчанию" }));

    expect(screen.queryByText(/папка вне приложения/i)).not.toBeInTheDocument();
  });

  it("pauses and resumes the queue", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) => {
      if (command === "enqueue_task") return Promise.resolve([task]);
      if (command === "pause_queue")
        return Promise.resolve({ paused: true, running: 1, queued: 0 });
      if (command === "resume_queue")
        return Promise.resolve({ paused: false, running: 1, queued: 0 });
      return Promise.resolve(null);
    });
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await user.click(screen.getByRole("button", { name: "Скачать" }));
    await screen.findByText("В очереди");

    await user.click(screen.getByRole("button", { name: "Пауза" }));
    expect(await screen.findByRole("button", { name: "Продолжить" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("pause_queue");

    await user.click(screen.getByRole("button", { name: "Продолжить" }));
    expect(await screen.findByRole("button", { name: "Пауза" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("resume_queue");
  });

  it("enqueues a batch file through the dialog", async () => {
    const user = userEvent.setup();
    open.mockResolvedValue("C:\\lists\\urls.txt");
    invoke.mockResolvedValue([task, { ...task, id: "t2" }]);
    render(<DownloadScreen />);

    await user.click(
      screen.getByRole("button", { name: "Из файла со ссылками…" }),
    );

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_batch", {
        request: {
          path: "C:\\lists\\urls.txt",
          mode: "audio",
          quality: undefined,
        },
      }),
    );
    expect(await screen.findAllByText("В очереди")).toHaveLength(2);
  });

  it("does nothing when the batch dialog is canceled", async () => {
    const user = userEvent.setup();
    open.mockResolvedValue(null);
    render(<DownloadScreen />);

    await user.click(
      screen.getByRole("button", { name: "Из файла со ссылками…" }),
    );

    expect(invoke).not.toHaveBeenCalledWith(
      "enqueue_batch",
      expect.anything(),
    );
  });
});
