import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import LibraryScreen from "./LibraryScreen";
import type {
  ImportReport,
  LibraryItem,
  LibraryPage,
} from "../../api/library";

const invoke = vi.hoisted(() => vi.fn());
const revealItemInDir = vi.hoisted(() => vi.fn());
const startImport = vi.hoisted(() => vi.fn());
const openDialog = vi.hoisted(() => vi.fn());
const dropState = vi.hoisted(() => ({
  handler: null as null | ((paths: string[]) => unknown),
  dragging: false,
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openDialog }));
vi.mock("./ImportProvider", () => ({
  useImport: () => ({
    progress: null,
    importing: false,
    cancelling: false,
    error: null,
    startImport,
    cancelImport: vi.fn(),
    clearError: vi.fn(),
  }),
}));
vi.mock("./useFileDrop", () => ({
  useFileDrop: ({ onDrop }: { onDrop: (paths: string[]) => unknown }) => {
    dropState.handler = onDrop;
    return { dragging: dropState.dragging };
  },
}));

const emptyReport: ImportReport = {
  imported: [],
  existing: [],
  skipped: [],
  failed: [],
  cancelled: false,
};

const audio: LibraryItem = {
  id: "C:\\root\\audio\\abc - Song.mp3",
  path: "C:\\root\\audio\\abc - Song.mp3",
  kind: "audio",
  title: "abc - Song",
  channel: null,
  durationSecs: 213,
  width: null,
  height: null,
  sizeBytes: 8_400_000,
  mtime: 1_760_000_000,
  thumbPath: "C:\\root\\data\\thumbs\\abc.jpg",
};

const video: LibraryItem = {
  ...audio,
  id: "C:\\root\\video\\v1 - Clip.mp4",
  path: "C:\\root\\video\\v1 - Clip.mp4",
  kind: "video",
  title: "v1 - Clip",
  durationSecs: 3723,
  width: 1920,
  height: 1080,
  sizeBytes: 240_000_000,
  thumbPath: null,
};

function setup(items: LibraryItem[] = [audio, video]) {
  const page: LibraryPage = { items, total: items.length };
  invoke.mockImplementation((command: string) => {
    if (command === "library_scan")
      return Promise.resolve({ scanned: items.length, added: 0, updated: 0, removed: 0 });
    if (command === "library_query") return Promise.resolve(page);
    if (command === "library_thumb") return Promise.resolve("data:image/jpeg;base64,AA==");
    return Promise.resolve(null);
  });
}

describe("LibraryScreen", () => {
  beforeEach(() => {
    invoke.mockReset();
    revealItemInDir.mockReset();
    startImport.mockReset();
    openDialog.mockReset();
    dropState.handler = null;
    dropState.dragging = false;
  });

  it("renders scanned items", async () => {
    setup();
    render(<LibraryScreen />);

    expect(await screen.findByText("abc - Song")).toBeInTheDocument();
    expect(screen.getByText("v1 - Clip")).toBeInTheDocument();
    expect(screen.getByText("2 файла")).toBeInTheDocument();
    expect(screen.getByText("3:33")).toBeInTheDocument();
    expect(screen.getByText("1:02:03")).toBeInTheDocument();
  });

  it("searches with a debounce", async () => {
    const user = userEvent.setup();
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");

    await user.type(screen.getByLabelText("Поиск в библиотеке"), "rick");

    await waitFor(
      () =>
        expect(invoke).toHaveBeenCalledWith("library_query", {
          query: {
            search: "rick",
            kind: undefined,
            sort: "dateDesc",
            limit: 200,
            offset: 0,
          },
        }),
      { timeout: 2000 },
    );
  });

  it("filters by kind", async () => {
    const user = userEvent.setup();
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");

    await user.click(screen.getByRole("tab", { name: "Видео" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("library_query", {
        query: {
          search: undefined,
          kind: "video",
          sort: "dateDesc",
          limit: 200,
          offset: 0,
        },
      }),
    );
  });

  it("switches to the list view", async () => {
    const user = userEvent.setup();
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");

    await user.click(screen.getByRole("button", { name: "Список" }));

    expect(screen.getByText("Длительность")).toBeInTheDocument();
    expect(screen.getByText("Качество")).toBeInTheDocument();
    expect(screen.getByText("1080p")).toBeInTheDocument();
  });

  it("opens media with the system player on double click", async () => {
    const user = userEvent.setup();
    setup();
    render(<LibraryScreen />);
    const title = await screen.findByText("abc - Song");

    await user.dblClick(title);

    expect(invoke).toHaveBeenCalledWith("library_open", { path: audio.path });
  });

  it("shows an empty state", async () => {
    setup([]);
    render(<LibraryScreen />);

    expect(
      await screen.findByText("Здесь появятся скачанные файлы"),
    ).toBeInTheDocument();
  });

  it("rescans on refresh", async () => {
    const user = userEvent.setup();
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");
    invoke.mockClear();
    setup();

    await user.click(screen.getByRole("button", { name: "Сканировать сейчас" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("library_scan", { full: true }),
    );
  });

  it("loads the next page on scroll", async () => {
    invoke.mockImplementation((command: string, args: { query?: { offset?: number } }) => {
      if (command === "library_scan")
        return Promise.resolve({ scanned: 2, added: 0, updated: 0, removed: 0 });
      if (command === "library_query") {
        return Promise.resolve(
          args.query?.offset === 0
            ? { items: [audio], total: 2 }
            : { items: [video], total: 2 },
        );
      }
      if (command === "library_thumb")
        return Promise.resolve("data:image/jpeg;base64,AA==");
      return Promise.resolve(null);
    });
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");

    const grid = document.querySelector(".lib-grid");
    expect(grid).not.toBeNull();
    fireEvent.scroll(grid!);

    await screen.findByText("v1 - Clip");
    expect(invoke).toHaveBeenCalledWith(
      "library_query",
      expect.objectContaining({
        query: expect.objectContaining({ limit: 200, offset: 1 }),
      }),
    );
  });

  it("imports dropped files and refreshes without a full scan", async () => {
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");
    invoke.mockClear();
    startImport.mockResolvedValue({ ...emptyReport, imported: [audio] });

    await act(async () => {
      await dropState.handler!(["C:\\src\\new.mp3"]);
    });

    expect(startImport).toHaveBeenCalledWith(["C:\\src\\new.mp3"]);
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("library_query", expect.anything()),
    );
    expect(invoke).not.toHaveBeenCalledWith("library_scan", expect.anything());
  });

  it("summarizes imported, existing, skipped and failed files", async () => {
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");
    startImport.mockResolvedValue({
      ...emptyReport,
      imported: [audio],
      existing: [video],
      skipped: [
        { path: "C:\\src\\lesson", reason: "folder" },
        { path: "C:\\src\\notes.pdf", reason: "unsupported" },
      ],
      failed: [{ path: "C:\\src\\broken.mp3", error: "read error" }],
    });

    await act(async () => {
      await dropState.handler!(["C:\\src\\lesson", "C:\\src\\broken.mp3"]);
    });

    expect(screen.getByText("Добавлено: 1")).toBeInTheDocument();
    expect(screen.getByText("Уже в библиотеке: 1")).toBeInTheDocument();
    expect(screen.getByText("Ошибки: 1")).toBeInTheDocument();
    expect(
      screen.getByText("Папки пока не поддерживаются — перетащите файлы"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Поддерживаются только аудио и видеофайлы"),
    ).toBeInTheDocument();
  });

  it("shows a rejection when another import is already running", async () => {
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");
    startImport.mockRejectedValue(new Error("Импорт уже выполняется"));

    await act(async () => {
      await dropState.handler!(["C:\\src\\a.mp3"]);
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Не удалось добавить файлы: Импорт уже выполняется",
    );
  });

  it("reports a cancelled import", async () => {
    setup();
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");
    startImport.mockResolvedValue({
      ...emptyReport,
      cancelled: true,
      skipped: [{ path: "C:\\src\\b.mp3", reason: "cancelled" }],
    });

    await act(async () => {
      await dropState.handler!(["C:\\src\\a.mp3", "C:\\src\\b.mp3"]);
    });

    expect(screen.getByText("Импорт отменён")).toBeInTheDocument();
    expect(screen.getByText("Пропущено: 1")).toBeInTheDocument();
  });

  it("imports files chosen in the dialog", async () => {
    const user = userEvent.setup();
    setup();
    openDialog.mockResolvedValue(["C:\\src\\chosen.mp3"]);
    startImport.mockResolvedValue({ ...emptyReport, imported: [audio] });
    render(<LibraryScreen />);
    await screen.findByText("abc - Song");

    await user.click(screen.getByRole("button", { name: "Добавить файлы" }));

    await waitFor(() =>
      expect(startImport).toHaveBeenCalledWith(["C:\\src\\chosen.mp3"]),
    );
    expect(openDialog).toHaveBeenCalledWith(
      expect.objectContaining({ multiple: true }),
    );
  });

  it("shows the drop overlay while dragging files", async () => {
    dropState.dragging = true;
    setup();
    render(<LibraryScreen />);

    expect(
      await screen.findByText("Отпустите, чтобы добавить в библиотеку"),
    ).toBeInTheDocument();
    expect(
      screen.getByText("Аудио и видео копируются в библиотеку"),
    ).toBeInTheDocument();
  });

  it("offers import actions in the empty state", async () => {
    setup([]);
    render(<LibraryScreen />);

    expect(
      await screen.findByText("Аудио и видео копируются в библиотеку"),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", { name: "Добавить файлы" }),
    ).toHaveLength(2);
  });
});
