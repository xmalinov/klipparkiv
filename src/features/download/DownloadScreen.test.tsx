import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  title: null,
  mode: "audio",
  clip: false,
  state: "queued",
  percent: 0,
  speedBps: null,
  etaSecs: null,
  error: null,
  playlistIndex: null,
  playlistCount: null,
  outputPath: null,
};

const info = {
  kind: "video",
  id: "abc",
  title: "Rick Astley - Never Gonna Give You Up",
  channel: "Rick Astley",
  durationSecs: 213,
  thumbnailUrl: "https://i.ytimg.com/vi/abc/hqdefault.jpg",
  uploadDate: "20091025",
  viewCount: 1_200_000_000,
  playlistCount: null,
  entries: [],
};

const playlist = {
  kind: "playlist",
  id: "PL123",
  title: "Best of Rick",
  channel: "Rick Astley",
  durationSecs: null,
  thumbnailUrl: null,
  uploadDate: null,
  viewCount: null,
  playlistCount: 42,
  entries: [
    { id: "a", title: "One" },
    { id: "b", title: "Two" },
  ],
};

const EXPECTED_EMPTY_CLIP = {
  title: undefined,
  playlist: undefined,
  playlistStart: undefined,
  playlistEnd: undefined,
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
          ...EXPECTED_EMPTY_CLIP,
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
          ...EXPECTED_EMPTY_CLIP,
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
          ...EXPECTED_EMPTY_CLIP,
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
          ...EXPECTED_EMPTY_CLIP,
        },
      }),
    );
    expect(await screen.findByText("Видео · клип")).toBeInTheDocument();
  });

  it("allows an open clip end", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([{ ...task, mode: "video", clip: true }]);
    render(<DownloadScreen />);

    await user.click(screen.getByRole("tab", { name: "Клип" }));
    await user.type(screen.getByLabelText("Начало клипа"), "1:30");
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/x");

    expect(screen.getByRole("button", { name: "Скачать" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: expect.objectContaining({
          clipStart: "1:30",
          clipEnd: undefined,
        }),
      }),
    );
  });

  it("rejects a clip whose start is after its end", async () => {
    const user = userEvent.setup();
    render(<DownloadScreen />);

    await user.click(screen.getByRole("tab", { name: "Клип" }));
    await user.type(screen.getByLabelText("Начало клипа"), "2:00");
    await user.type(screen.getByLabelText("Конец клипа"), "1:30");
    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/x");

    expect(
      screen.getByText("Начало должно быть раньше конца"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Скачать" })).toBeDisabled();
  });

  it("clears the clip with the whole-video chip", async () => {
    const user = userEvent.setup();
    render(<DownloadScreen />);

    await user.click(screen.getByRole("tab", { name: "Клип" }));
    await user.type(screen.getByLabelText("Начало клипа"), "1:30");
    await user.click(screen.getByRole("button", { name: "Весь ролик" }));

    expect(screen.getByLabelText("Начало клипа")).toHaveValue("");
  });

  it("syncs the clip sliders with the video duration", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.resolve(info)
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await screen.findByText(info.title, {}, { timeout: 2000 });
    await user.click(screen.getByRole("tab", { name: "Клип" }));

    const start = screen.getByRole("slider", {
      name: "Ползунок начала клипа",
    });
    const end = screen.getByRole("slider", {
      name: "Ползунок конца клипа",
    });

    fireEvent.change(start, { target: { value: "30" } });
    expect(screen.getByLabelText("Начало клипа")).toHaveValue("00:00:30");

    fireEvent.change(end, { target: { value: "60" } });
    expect(screen.getByLabelText("Конец клипа")).toHaveValue("00:01:00");
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

  it("offers the playlist choice for a playlist link", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.resolve(playlist)
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(
      screen.getByLabelText("Ссылка на видео"),
      "https://www.youtube.com/playlist?list=PL123",
    );

    expect(
      await screen.findByText("Best of Rick", {}, { timeout: 2000 }),
    ).toBeInTheDocument();
    expect(screen.getByText(/42 видео/)).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Только это видео" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Весь плейлист" }),
    ).toBeInTheDocument();
  });

  it("downloads a whole playlist with a range", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.resolve(playlist)
        : Promise.resolve([{ ...task, playlistCount: 42 }]),
    );
    render(<DownloadScreen />);

    await user.type(
      screen.getByLabelText("Ссылка на видео"),
      "https://www.youtube.com/playlist?list=PL123",
    );
    await screen.findByText("Best of Rick", {}, { timeout: 2000 });
    await user.type(screen.getByLabelText("С №"), "2");
    await user.type(screen.getByLabelText("по №"), "10");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: expect.objectContaining({
          playlist: true,
          playlistStart: 2,
          playlistEnd: 10,
          title: "Best of Rick",
        }),
      }),
    );
  });

  it("downloads a watch link on a single video by default", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.resolve(playlist)
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(
      screen.getByLabelText("Ссылка на видео"),
      "https://www.youtube.com/watch?v=abc&list=PL123",
    );
    await screen.findByText("Best of Rick", {}, { timeout: 2000 });
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: expect.objectContaining({
          playlist: undefined,
          playlistStart: undefined,
          playlistEnd: undefined,
        }),
      }),
    );
  });

  it("treats a playlist link as a playlist before the probe finishes", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? new Promise(() => {})
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(
      screen.getByLabelText("Ссылка на видео"),
      "https://www.youtube.com/playlist?list=PL123",
    );
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_task", {
        task: expect.objectContaining({ playlist: true }),
      }),
    );
  });

  it("keeps the playlist choice when the probe fails", async () => {
    const user = userEvent.setup();
    invoke.mockImplementation((command: string) =>
      command === "probe_metadata"
        ? Promise.reject("ERROR: playlist unavailable")
        : Promise.resolve([task]),
    );
    render(<DownloadScreen />);

    await user.type(
      screen.getByLabelText("Ссылка на видео"),
      "https://www.youtube.com/playlist?list=PL123",
    );

    expect(
      await screen.findByText("ERROR: playlist unavailable", {}, { timeout: 2000 }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Только это видео" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Весь плейлист" }),
    ).toBeInTheDocument();
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
      payload: {
        id: "t1",
        percent: 45.4,
        speedBps: 2_200_000,
        etaSecs: 80,
        playlistIndex: null,
        playlistCount: null,
      },
    });

    expect(await screen.findByText("45%")).toBeInTheDocument();
    expect(screen.getByText("2,1 МБ/с · 1:20")).toBeInTheDocument();
  });

  it("shows the title and playlist counter in the queue", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([
      {
        ...task,
        title: "Best of Rick",
        playlistIndex: 2,
        playlistCount: 42,
      },
    ]);
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await user.click(screen.getByRole("button", { name: "Скачать" }));

    expect(await screen.findByText("Best of Rick")).toBeInTheDocument();
    expect(screen.getByText(/плейлист · 2\/42/)).toBeInTheDocument();
  });

  it("marks an already downloaded task", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task]);
    render(<DownloadScreen />);

    await user.type(screen.getByLabelText("Ссылка на видео"), "https://youtu.be/abc");
    await user.click(screen.getByRole("button", { name: "Скачать" }));
    await screen.findByText("В очереди");

    handlers["task://state"]({
      payload: { ...task, state: "skipped", percent: 100 },
    });

    expect(await screen.findByText("Уже было")).toBeInTheDocument();
    expect(screen.getByText("Файл уже есть")).toBeInTheDocument();
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
          audioKbps: 320,
          outDir: undefined,
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

  it("adds a pasted list of links", async () => {
    const user = userEvent.setup();
    invoke.mockResolvedValue([task, { ...task, id: "t2" }]);
    render(<DownloadScreen />);

    await user.click(screen.getByRole("button", { name: "Список ссылок" }));
    await user.type(
      screen.getByLabelText("Список ссылок"),
      "https://youtu.be/a{enter}https://youtu.be/b",
    );
    await user.click(screen.getByRole("button", { name: "Добавить все" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_links", {
        request: {
          text: "https://youtu.be/a\nhttps://youtu.be/b",
          mode: "audio",
          quality: undefined,
          audioKbps: 320,
          outDir: undefined,
        },
      }),
    );
    expect(await screen.findAllByText("В очереди")).toHaveLength(2);
  });

  it("keeps only the first link from the list", async () => {
    const user = userEvent.setup();
    render(<DownloadScreen />);

    await user.click(screen.getByRole("button", { name: "Список ссылок" }));
    await user.type(
      screen.getByLabelText("Список ссылок"),
      "https://youtu.be/a\nhttps://youtu.be/b",
    );
    await user.click(screen.getByRole("button", { name: "Оставить первую" }));

    expect(screen.getByLabelText("Ссылка на видео")).toHaveValue(
      "https://youtu.be/a",
    );
  });

  it("accepts dropped links", async () => {
    const { container } = render(<DownloadScreen />);
    invoke.mockResolvedValue([task]);

    const dropZone = container.querySelector(".download");
    expect(dropZone).not.toBeNull();
    fireEvent.drop(dropZone!, {
      dataTransfer: {
        files: [],
        getData: () => "https://youtu.be/a\nhttps://youtu.be/b",
      },
    });

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_links", {
        request: expect.objectContaining({
          text: "https://youtu.be/a\nhttps://youtu.be/b",
        }),
      }),
    );
  });

  it("reads dropped .txt link lists", async () => {
    const { container } = render(<DownloadScreen />);
    invoke.mockResolvedValue([task]);

    const dropZone = container.querySelector(".download");
    expect(dropZone).not.toBeNull();
    const file = new File(["https://youtu.be/a"], "links.txt", {
      type: "text/plain",
    });
    fireEvent.drop(dropZone!, {
      dataTransfer: { files: [file], getData: () => "" },
    });

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("enqueue_links", {
        request: expect.objectContaining({ text: "https://youtu.be/a" }),
      }),
    );
  });
});
