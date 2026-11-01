import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import LibraryScreen from "./LibraryScreen";
import type { LibraryItem, LibraryPage } from "../../api/library";

const invoke = vi.hoisted(() => vi.fn());
const revealItemInDir = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/plugin-opener", () => ({ revealItemInDir }));

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
          query: { search: "rick", kind: undefined, sort: "dateDesc" },
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
        query: { search: undefined, kind: "video", sort: "dateDesc" },
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
});
