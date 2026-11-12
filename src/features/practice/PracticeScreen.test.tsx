import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PracticeScreen from "./PracticeScreen";
import { DEFAULT_PRACTICE_STATE } from "../../api/practice";
import type { ImportReport } from "../../api/library";

const context = vi.hoisted(() => ({
  value: {} as Record<string, unknown>,
}));
const startImport = vi.hoisted(() => vi.fn());
const openDialog = vi.hoisted(() => vi.fn());
const dropState = vi.hoisted(() => ({
  handler: null as null | ((paths: string[]) => unknown),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openDialog }));
vi.mock("../library/ImportProvider", () => ({
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
vi.mock("../library/useFileDrop", () => ({
  useFileDrop: ({ onDrop }: { onDrop: (paths: string[]) => unknown }) => {
    dropState.handler = onDrop;
    return { dragging: false };
  },
}));

vi.mock("./PracticeProvider", () => ({
  usePractice: () => context.value,
  REGION_COLORS: {
    intro: "#8b9bb4",
    verse: "#4f8cff",
    chorus: "#ff8a4f",
    solo: "#e05c8a",
    bridge: "#9b7bff",
    outro: "#5fb98a",
    custom: "#c9a227",
  },
  REGION_KINDS: [
    "intro",
    "verse",
    "chorus",
    "solo",
    "bridge",
    "outro",
    "custom",
  ],
}));

const track = {
  id: "C:\\root\\audio\\song.mp3",
  path: "C:\\root\\audio\\song.mp3",
  kind: "audio" as const,
  title: "Track",
  channel: null,
  durationSecs: 10,
  width: null,
  height: null,
  sizeBytes: 1000,
  mtime: 1,
  thumbPath: null,
};

function setup({ withTrack = true } = {}) {
  const setSpeed = vi.fn();
  const setVolume = vi.fn();
  const playPause = vi.fn();
  const selectRegion = vi.fn();
  const selectRegionByIndex = vi.fn();
  const openTrack = vi.fn();
  const onBack = vi.fn();
  context.value = {
    track: withTrack ? track : null,
    state: {
      ...DEFAULT_PRACTICE_STATE,
      loopEnabled: true,
      activeRegionId: "r1",
      regions: [
        {
          id: "r1",
          name: "Verse",
          kind: "verse",
          color: "#4f8cff",
          start: 1,
          end: 2,
        },
      ],
    },
    snapshot: {
      positionSecs: 1.5,
      durationSecs: 10,
      playing: false,
      ended: false,
      underruns: 0,
    },
    waveform: null,
    waveformLoading: false,
    error: null,
    recent: [],
    selection: null,
    duration: 10,
    openTrack,
    hideRecent: vi.fn(),
    close: vi.fn(),
    playPause,
    seek: vi.fn(),
    setSpeed,
    setPitch: vi.fn(),
    setVolume,
    setSelection: vi.fn(),
    addRegion: vi.fn(),
    saveSelectionAsRegion: vi.fn(),
    updateRegion: vi.fn(),
    deleteRegion: vi.fn(),
    setLoopEnabled: vi.fn(),
    loopRegion: vi.fn(),
    setLoopBoundary: vi.fn(),
    clearLoop: vi.fn(),
    nudgeLoop: vi.fn(),
    selectRegion,
    selectRegionByIndex,
    setPreRoll: vi.fn(),
  };
  render(<PracticeScreen onBack={onBack} />);
  return {
    setSpeed,
    setVolume,
    playPause,
    selectRegion,
    selectRegionByIndex,
    openTrack,
    onBack,
  };
}

const emptyReport: ImportReport = {
  imported: [],
  existing: [],
  skipped: [],
  failed: [],
  cancelled: false,
};

describe("PracticeScreen", () => {
  beforeEach(() => {
    context.value = {};
    startImport.mockReset();
    openDialog.mockReset();
    dropState.handler = null;
  });

  it("renders the track, regions and transport", () => {
    setup();
    expect(screen.getByText("Track")).toBeInTheDocument();
    expect(screen.getByText("Verse")).toBeInTheDocument();
    expect(screen.getByText("0:02 / 0:10")).toBeInTheDocument();
  });

  it("changes speed and toggles playback", async () => {
    const { setSpeed, playPause } = setup();
    await userEvent.click(screen.getByRole("button", { name: "0.5×" }));
    expect(setSpeed).toHaveBeenCalledWith(0.5);
    await userEvent.click(screen.getByRole("button", { name: "Играть" }));
    expect(playPause).toHaveBeenCalled();
  });

  it("applies a custom speed", async () => {
    const { setSpeed } = setup();
    const input = screen.getByLabelText("Своя скорость");
    await userEvent.clear(input);
    await userEvent.type(input, "0.7{Enter}");
    expect(setSpeed).toHaveBeenCalledWith(0.7);
  });

  it("changes volume", () => {
    const { setVolume } = setup();
    fireEvent.change(screen.getByLabelText("Громкость"), {
      target: { value: "40" },
    });
    expect(setVolume).toHaveBeenCalledWith(0.4);
  });

  it("handles keyboard shortcuts", () => {
    const { playPause, selectRegionByIndex } = setup();
    fireEvent.keyDown(window, { key: " " });
    expect(playPause).toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "1" });
    expect(selectRegionByIndex).toHaveBeenCalledWith(0);
  });

  it("selects a region and goes back", async () => {
    const { selectRegion, onBack } = setup();
    await userEvent.click(screen.getByText("Verse"));
    expect(selectRegion).toHaveBeenCalledWith("r1");
    await userEvent.click(screen.getByRole("button", { name: /Библиотека/ }));
    expect(onBack).toHaveBeenCalled();
  });

  it("imports a dropped audio file, opens it paused and does not autoplay", async () => {
    const { openTrack, playPause } = setup();
    startImport.mockResolvedValue({ ...emptyReport, imported: [track] });

    await act(async () => {
      await dropState.handler!(["C:\\src\\song.mp3"]);
    });

    expect(startImport).toHaveBeenCalledWith(["C:\\src\\song.mp3"]);
    expect(openTrack).toHaveBeenCalledWith(track);
    expect(playPause).not.toHaveBeenCalled();
  });

  it("shows a rejection when another import is already running", async () => {
    const { openTrack } = setup();
    startImport.mockRejectedValue(new Error("Импорт уже выполняется"));

    await act(async () => {
      await dropState.handler!(["C:\\src\\a.mp3"]);
    });

    expect(openTrack).not.toHaveBeenCalled();
    expect(screen.getByText("Импорт уже выполняется")).toBeInTheDocument();
  });

  it("does not open anything when the import was cancelled", async () => {
    const { openTrack } = setup();
    startImport.mockResolvedValue({
      ...emptyReport,
      cancelled: true,
      imported: [track],
    });

    await act(async () => {
      await dropState.handler!(["C:\\src\\song.mp3"]);
    });

    expect(openTrack).not.toHaveBeenCalled();
    expect(screen.getByText("Импорт отменён")).toBeInTheDocument();
  });

  it("imports a dropped video without opening it in Practice", async () => {
    const { openTrack } = setup();
    const video = {
      ...track,
      id: "C:\\root\\video\\clip.mp4",
      path: "C:\\root\\video\\clip.mp4",
      kind: "video" as const,
      title: "clip",
    };
    startImport.mockResolvedValue({ ...emptyReport, imported: [video] });

    await act(async () => {
      await dropState.handler!(["C:\\src\\clip.mp4"]);
    });

    expect(openTrack).not.toHaveBeenCalled();
    expect(
      screen.getByText("Видео добавлено в библиотеку. Практика работает с аудио."),
    ).toBeInTheDocument();
  });

  it("imports the first audio chosen in the empty state", async () => {
    const { openTrack } = setup({ withTrack: false });
    startImport.mockResolvedValue({ ...emptyReport, imported: [track] });
    openDialog.mockResolvedValue(["C:\\src\\song.mp3"]);

    await userEvent.click(screen.getByRole("button", { name: "Добавить файлы" }));

    await waitFor(() =>
      expect(startImport).toHaveBeenCalledWith(["C:\\src\\song.mp3"]),
    );
    expect(openTrack).toHaveBeenCalledWith(track);
  });
});
