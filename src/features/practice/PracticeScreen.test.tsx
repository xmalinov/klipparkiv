import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PracticeScreen from "./PracticeScreen";
import { DEFAULT_PRACTICE_STATE } from "../../api/practice";
import type { ImportReport } from "../../api/library";
import { REGION_COLORS } from "./PracticeProvider";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

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

function setup({
  withTrack = true,
  ...overrides
}: { withTrack?: boolean } & Record<string, unknown> = {}) {
  const setSpeed = vi.fn();
  const setVolume = vi.fn();
  const playPause = vi.fn();
  const selectRegion = vi.fn();
  const selectRegionByIndex = vi.fn();
  const openTrack = vi.fn();
  const onBack = vi.fn();
  const startAnalysis = vi.fn();
  const loopBars = vi.fn();
  const setManualBpm = vi.fn();
  const setBpmScale = vi.fn();
  const setBeatsPerBar = vi.fn();
  const setBarOffset = vi.fn();
  const setAutoBars = vi.fn();
  const setBarOneAtPlayhead = vi.fn();
  const setSnapMode = vi.fn();
  const setMetronome = vi.fn();
  const setCountIn = vi.fn();
  const setGridEnabled = vi.fn();
  const seek = vi.fn();
  const setPitch = vi.fn();
  const addRegion = vi.fn();
  const saveSelectionAsRegion = vi.fn();
  const updateRegion = vi.fn();
  const deleteRegion = vi.fn();
  const setLoopEnabled = vi.fn();
  const loopRegion = vi.fn();
  const setLoopBoundary = vi.fn();
  const clearLoop = vi.fn();
  const nudgeLoop = vi.fn();
  const hideRecent = vi.fn();
  const close = vi.fn();
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
    analysis: null,
    analysisStatus: "idle",
    analysisProgress: 0,
    analysisError: null,
    grid: null,
    openTrack,
    hideRecent,
    close,
    playPause,
    seek,
    setSpeed,
    setPitch,
    setVolume,
    setSelection: vi.fn(),
    addRegion,
    saveSelectionAsRegion,
    updateRegion,
    deleteRegion,
    setLoopEnabled,
    loopRegion,
    setLoopBoundary,
    clearLoop,
    nudgeLoop,
    selectRegion,
    selectRegionByIndex,
    setPreRoll: vi.fn(),
    startAnalysis,
    loopBars,
    setManualBpm,
    setBpmScale,
    setBeatsPerBar,
    setBarOffset,
    setAutoBars,
    setBarOneAtPlayhead,
    setSnapMode,
    setMetronome,
    setCountIn,
    setGridEnabled,
    ...overrides,
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
    startAnalysis,
    loopBars,
    setManualBpm,
    setBpmScale,
    setBeatsPerBar,
    setBarOffset,
    setAutoBars,
    setBarOneAtPlayhead,
    setSnapMode,
    setMetronome,
    setCountIn,
    setGridEnabled,
    seek,
    setPitch,
    addRegion,
    saveSelectionAsRegion,
    updateRegion,
    deleteRegion,
    setLoopEnabled,
    loopRegion,
    setLoopBoundary,
    clearLoop,
    nudgeLoop,
    hideRecent,
    close,
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
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
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

  it("applies a manual BPM on blur and Enter", () => {
    const { setManualBpm } = setup();
    const input = screen.getByLabelText("BPM вручную");

    fireEvent.change(input, { target: { value: "120.5" } });
    fireEvent.blur(input);
    expect(setManualBpm).toHaveBeenLastCalledWith(120.5);

    fireEvent.change(input, { target: { value: "90" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(setManualBpm).toHaveBeenLastCalledWith(90);
  });

  it("clears the manual BPM for a non-positive value", () => {
    const { setManualBpm } = setup();
    const input = screen.getByLabelText("BPM вручную");

    fireEvent.change(input, { target: { value: "-5" } });
    fireEvent.blur(input);

    expect(setManualBpm).toHaveBeenLastCalledWith(null);
  });

  it("derives the BPM from tap tempo and ignores very short intervals", () => {
    const now = vi.spyOn(performance, "now");
    const { setManualBpm } = setup();
    const tap = screen.getByRole("button", { name: "Тап" });

    now.mockReturnValue(0);
    fireEvent.click(tap);
    now.mockReturnValue(50);
    fireEvent.click(tap);
    expect(setManualBpm).not.toHaveBeenCalled();

    now.mockReturnValue(500);
    fireEvent.click(tap);
    expect(setManualBpm).toHaveBeenLastCalledWith(60000 / 450);
    now.mockRestore();
  });

  it("shows analysis progress while the grid is built", () => {
    setup({ analysisStatus: "loading", analysisProgress: 0.42 });
    expect(screen.getByRole("status")).toHaveTextContent("Строю сетку 42%");
  });

  it("offers to build the grid when there is none", async () => {
    const { startAnalysis } = setup();
    await userEvent.click(
      screen.getByRole("button", { name: "Построить сетку" }),
    );
    expect(startAnalysis).toHaveBeenCalled();
  });

  it("offers to retry a failed analysis", async () => {
    const { startAnalysis } = setup({
      analysisStatus: "error",
      analysisError: "model missing",
    });
    expect(screen.getByText("Сетка недоступна")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Повторить" }));
    expect(startAnalysis).toHaveBeenCalled();
  });

  it("disables grid controls until a grid exists", () => {
    setup();
    expect(screen.getByRole("button", { name: "2" })).toBeDisabled();
    expect(screen.getByLabelText("Клик")).toBeDisabled();
    expect(screen.getByLabelText("Отсчёт")).toBeDisabled();
  });

  it("loops bars and toggles the metronome when the grid is ready", async () => {
    const grid = {
      bpm: 120,
      beats: [0, 1, 2, 3],
      downbeats: [0],
      barStarts: [0, 2],
      beatsPerBar: 4,
      barOffset: 0,
      pickupUntil: null,
      confidence: 1,
    };
    const { loopBars, setMetronome, setCountIn } = setup({ grid });

    await userEvent.click(screen.getByRole("button", { name: "2" }));
    expect(loopBars).toHaveBeenCalledWith(2);

    await userEvent.click(screen.getByLabelText("Клик"));
    expect(setMetronome).toHaveBeenCalledWith(
      true,
      DEFAULT_PRACTICE_STATE.metronomeVolume,
    );

    await userEvent.click(screen.getByLabelText("Отсчёт"));
    expect(setCountIn).toHaveBeenCalledWith(false);
  });

  it("changes the tempo scale, meter and snap mode", async () => {
    const { setBpmScale, setBeatsPerBar, setAutoBars, setSnapMode } = setup();

    await userEvent.click(screen.getByRole("button", { name: "÷2" }));
    expect(setBpmScale).toHaveBeenCalledWith(0.5);
    await userEvent.click(screen.getByRole("button", { name: "×2" }));
    expect(setBpmScale).toHaveBeenCalledWith(2);

    const meter = screen.getByLabelText("Доли");
    expect(within(meter).getByRole("option", { name: "5" })).toBeInTheDocument();
    expect(within(meter).getByRole("option", { name: "7" })).toBeInTheDocument();
    expect(within(meter).getByRole("option", { name: "16" })).toBeInTheDocument();
    fireEvent.change(meter, { target: { value: "7" } });
    expect(setBeatsPerBar).toHaveBeenCalledWith(7);

    fireEvent.change(meter, { target: { value: "0" } });
    expect(setAutoBars).toHaveBeenCalled();

    fireEvent.change(screen.getByLabelText("Привязка"), {
      target: { value: "bar" },
    });
    expect(setSnapMode).toHaveBeenCalledWith("bar");
  });

  it("shows automatic bars without a numeric offset", () => {
    const grid = {
      bpm: 120,
      beats: [0, 1, 2, 3],
      downbeats: [0],
      barStarts: [0, 2],
      beatsPerBar: 4,
      barOffset: 0,
      pickupUntil: null,
      confidence: 0.42,
    };
    setup({ grid });

    expect(screen.getByLabelText("Доли")).toHaveValue("0");
    expect(screen.getByText("—", { selector: ".grid-value" })).toBeInTheDocument();
    expect(
      screen.getByTitle("Уверенность сетки: 42%"),
    ).toBeInTheDocument();
  });

  it("sets bar 1 at the playhead", async () => {
    const { setBarOneAtPlayhead } = setup();
    await userEvent.click(
      screen.getByRole("button", { name: "Бар 1 здесь" }),
    );
    expect(setBarOneAtPlayhead).toHaveBeenCalled();
  });

  it("shifts the bar start and shows the scaled BPM", async () => {
    const { setBarOffset } = setup({
      state: { ...DEFAULT_PRACTICE_STATE, manualBpm: 100, bpmScale: 2 },
    });

    expect(screen.getByText("200.0")).toBeInTheDocument();
    expect(screen.getByLabelText("BPM вручную")).toHaveValue(100);

    await userEvent.click(
      screen.getByRole("button", { name: "Сдвинуть такт вперёд" }),
    );
    expect(setBarOffset).toHaveBeenCalledWith(1);

    await userEvent.click(
      screen.getByRole("button", { name: "Сдвинуть такт назад" }),
    );
    expect(setBarOffset).toHaveBeenCalledWith(-1);
  });

  it("toggles the automatic grid", async () => {
    const { setGridEnabled } = setup();
    const toggle = screen.getByLabelText("Сетка авто");
    await userEvent.click(toggle);
    expect(setGridEnabled).toHaveBeenCalledWith(false);
  });

  it("handles the playback and transport keyboard shortcuts", () => {
    const {
      playPause,
      seek,
      setPitch,
      setLoopBoundary,
      setLoopEnabled,
      clearLoop,
      nudgeLoop,
      deleteRegion,
      selectRegionByIndex,
      setSpeed,
    } = setup();

    fireEvent.keyDown(window, { key: " " });
    expect(playPause).toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "ArrowLeft" });
    expect(seek).toHaveBeenLastCalledWith(0);
    fireEvent.keyDown(window, { key: "ArrowRight", shiftKey: true });
    expect(seek).toHaveBeenLastCalledWith(10);

    fireEvent.keyDown(window, { key: "ArrowUp", shiftKey: true });
    expect(setPitch).toHaveBeenLastCalledWith(1, 0);
    fireEvent.keyDown(window, { key: "ArrowDown", shiftKey: true });
    expect(setPitch).toHaveBeenLastCalledWith(-1, 0);

    fireEvent.keyDown(window, { key: "a" });
    expect(setLoopBoundary).toHaveBeenLastCalledWith("start");
    fireEvent.keyDown(window, { key: "b" });
    expect(setLoopBoundary).toHaveBeenLastCalledWith("end");

    fireEvent.keyDown(window, { key: "l" });
    expect(setLoopEnabled).toHaveBeenCalledWith(false);
    fireEvent.keyDown(window, { key: "L" });
    expect(clearLoop).toHaveBeenCalled();

    fireEvent.keyDown(window, { key: "[" });
    expect(nudgeLoop).toHaveBeenLastCalledWith("start", -0.05);
    fireEvent.keyDown(window, { key: "]" });
    expect(nudgeLoop).toHaveBeenLastCalledWith("end", 0.05);

    fireEvent.keyDown(window, { key: "+" });
    expect(setSpeed).toHaveBeenLastCalledWith(1.25);
    fireEvent.keyDown(window, { key: "-" });
    expect(setSpeed).toHaveBeenLastCalledWith(0.75);

    fireEvent.keyDown(window, { key: "2" });
    expect(selectRegionByIndex).toHaveBeenCalledWith(1);
    fireEvent.keyDown(window, { key: "Delete" });
    expect(deleteRegion).toHaveBeenCalledWith("r1");
  });

  it("ignores shortcuts typed into form fields", () => {
    const { setLoopBoundary } = setup();
    const input = screen.getByLabelText("Своя скорость");

    fireEvent.keyDown(input, { key: "a" });
    expect(setLoopBoundary).not.toHaveBeenCalled();
  });

  it("opens the region dialogs and saves drafts", async () => {
    const user = userEvent.setup();
    const { saveSelectionAsRegion, updateRegion } = setup();

    fireEvent.keyDown(window, { key: "m" });
    let dialog = screen.getByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("Название региона"), {
      target: { value: "Intro" },
    });
    await user.click(within(dialog).getByRole("button", { name: "Сохранить" }));
    expect(saveSelectionAsRegion).toHaveBeenCalledWith(
      "Intro",
      "custom",
      REGION_COLORS.custom,
    );

    fireEvent.keyDown(window, { key: "s" });
    dialog = screen.getByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Сохранить" }));
    expect(saveSelectionAsRegion).toHaveBeenLastCalledWith(
      "A-B",
      "custom",
      REGION_COLORS.custom,
    );

    await user.click(screen.getByRole("button", { name: "Изменить регион" }));
    dialog = screen.getByRole("dialog");
    fireEvent.change(within(dialog).getByLabelText("Название региона"), {
      target: { value: "Chorus" },
    });
    await user.click(within(dialog).getByRole("button", { name: "Сохранить" }));
    expect(updateRegion).toHaveBeenCalledWith("r1", {
      name: "Chorus",
      kind: "verse",
      color: "#4f8cff",
    });
  });

  it("cancels a region dialog without saving", async () => {
    const user = userEvent.setup();
    const { saveSelectionAsRegion, updateRegion } = setup();

    fireEvent.keyDown(window, { key: "m" });
    await user.click(screen.getByRole("button", { name: "Отмена" }));

    expect(screen.queryByRole("dialog")).toBeNull();
    expect(saveSelectionAsRegion).not.toHaveBeenCalled();
    expect(updateRegion).not.toHaveBeenCalled();
  });

  it("loops, edits and deletes a region from the list", async () => {
    const user = userEvent.setup();
    const { loopRegion, deleteRegion } = setup();

    await user.click(screen.getByRole("button", { name: "Зациклить регион" }));
    expect(loopRegion).toHaveBeenCalledWith("r1");

    await user.click(screen.getByRole("button", { name: "Изменить регион" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Отмена" }));

    await user.click(screen.getByRole("button", { name: "Удалить регион" }));
    expect(deleteRegion).toHaveBeenCalledWith("r1");
  });

  it("steps speed, pitch and seeks five seconds", async () => {
    const user = userEvent.setup();
    const { setSpeed, setPitch, seek } = setup();

    await user.click(screen.getByRole("button", { name: "Скорость +0,05" }));
    expect(setSpeed).toHaveBeenLastCalledWith(1.05);
    await user.click(screen.getByRole("button", { name: "Скорость −0,05" }));
    expect(setSpeed).toHaveBeenLastCalledWith(0.95);

    await user.click(screen.getByRole("button", { name: "Понизить на полутон" }));
    expect(setPitch).toHaveBeenLastCalledWith(-1, 0);
    await user.click(screen.getByRole("button", { name: "Повысить на полутон" }));
    expect(setPitch).toHaveBeenLastCalledWith(1, 0);

    await user.click(screen.getByRole("button", { name: "-5 seconds" }));
    expect(seek).toHaveBeenLastCalledWith(0);
    await user.click(screen.getByRole("button", { name: "+5 seconds" }));
    expect(seek).toHaveBeenLastCalledWith(6.5);

    await user.click(screen.getByRole("button", { name: "К началу лупа" }));
    expect(seek).toHaveBeenLastCalledWith(1);
  });

  it("opens the system player for the current track", async () => {
    const user = userEvent.setup();
    setup();

    await user.click(screen.getByRole("button", { name: "Системный плеер" }));
    expect(invoke).toHaveBeenCalledWith("library_open", { path: track.path });
  });

  it("shows the help overlay and hides it with Escape", async () => {
    setup();

    fireEvent.keyDown(window, { key: "?" });
    expect(
      screen.getByRole("dialog", { name: "Горячие клавиши" }),
    ).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows a playback error from the provider", () => {
    setup({ error: "device lost" });
    expect(screen.getByRole("alert")).toHaveTextContent("device lost");
  });

  it("opens a recent track and hides it", async () => {
    const user = userEvent.setup();
    const { openTrack, hideRecent } = setup({
      recent: [track, { ...track, id: "b", path: "b.mp3", title: "Other" }],
    });

    await user.click(screen.getByRole("button", { name: "Other" }));
    expect(openTrack).toHaveBeenCalled();

    await user.click(
      screen.getAllByRole("button", { name: "Убрать из недавних" })[1],
    );
    expect(hideRecent).toHaveBeenCalledWith("b.mp3");
  });

  it("imports files chosen from the empty state", async () => {
    const user = userEvent.setup();
    const { openTrack } = setup({ withTrack: false });
    openDialog.mockResolvedValue(["C:\\src\\song.mp3"]);
    startImport.mockResolvedValue({ ...emptyReport, imported: [track] });

    await user.click(screen.getByRole("button", { name: "Добавить файлы" }));

    await waitFor(() =>
      expect(startImport).toHaveBeenCalledWith(["C:\\src\\song.mp3"]),
    );
    expect(openTrack).toHaveBeenCalledWith(track);
  });
});
