import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  DEFAULT_PRACTICE_STATE,
  decodePeaks,
  onPracticeAnalysisDone,
  onPracticeAnalysisError,
  onPracticeAnalysisProgress,
  onPracticeError,
  onPracticePosition,
  practiceAnalysisCancel,
  practiceAnalysisGet,
  practiceAnalysisStart,
  practiceClose,
  practiceOpen,
  practicePause,
  practicePlay,
  practicePosition,
  practiceRecent,
  practiceRecentHide,
  practiceSeek,
  practiceSetGrid,
  practiceSetLoop,
  practiceSetPitch,
  practiceSetSpeed,
  practiceSetVolume,
  practiceStateGet,
  practiceStateSave,
  practiceWaveform,
} from "./practice";

const invoke = vi.hoisted(() => vi.fn());
const listen = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

describe("practice api", () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockReset();
    listen.mockResolvedValue(() => {});
  });

  it("decodes signed peak bytes", () => {
    const peaks = decodePeaks("9go=");
    expect(Array.from(peaks)).toEqual([-10, 10]);
  });

  it("passes loop arguments in camelCase", async () => {
    invoke.mockResolvedValue(undefined);
    await practiceSetLoop(true, 1.5, 3.5, true);
    expect(invoke).toHaveBeenCalledWith("practice_set_loop", {
      enabled: true,
      startSecs: 1.5,
      endSecs: 3.5,
      preRoll: true,
      preRollSecs: null,
    });
  });

  it("opens a track through the backend", async () => {
    invoke.mockResolvedValue({
      path: "C:\\audio\\a.mp3",
      durationSecs: 10,
      sampleRate: 48000,
      state: {},
    });
    const opened = await practiceOpen("C:\\audio\\a.mp3");
    expect(opened.durationSecs).toBe(10);
    expect(invoke).toHaveBeenCalledWith("practice_open", {
      path: "C:\\audio\\a.mp3",
    });
  });

  it("starts and reads beat-grid analysis", async () => {
    invoke.mockResolvedValue(null);
    await practiceAnalysisGet("C:\\audio\\a.mp3");
    expect(invoke).toHaveBeenCalledWith("practice_analysis_get", {
      path: "C:\\audio\\a.mp3",
    });
    invoke.mockResolvedValue(undefined);
    await practiceAnalysisStart("C:\\audio\\a.mp3");
    expect(invoke).toHaveBeenCalledWith("practice_analysis_start", {
      path: "C:\\audio\\a.mp3",
    });
  });

  it("sends the metronome grid in camelCase", async () => {
    invoke.mockResolvedValue(undefined);
    await practiceSetGrid({
      bpm: 120,
      beatsPerBar: 4,
      barOffset: 0,
      beats: [0, 0.5],
      downbeats: [0],
    });
    expect(invoke).toHaveBeenCalledWith("practice_set_grid", {
      grid: {
        bpm: 120,
        beatsPerBar: 4,
        barOffset: 0,
        beats: [0, 0.5],
        downbeats: [0],
      },
    });
  });

  it("subscribes to analysis done events", async () => {
    const handler = vi.fn();
    await onPracticeAnalysisDone(handler);
    expect(listen).toHaveBeenCalledWith(
      "analysis://done",
      expect.any(Function),
    );
  });

  it("subscribes to position events", async () => {
    const handler = vi.fn();
    await onPracticePosition(handler);
    expect(listen).toHaveBeenCalledWith(
      "practice://position",
      expect.any(Function),
    );
    const callback = listen.mock.calls[0][1] as (event: {
      payload: unknown;
    }) => void;
    callback({ payload: { positionSecs: 1 } });
    expect(handler).toHaveBeenCalledWith({ positionSecs: 1 });
  });

  it("forwards every transport command", async () => {
    invoke.mockResolvedValue(undefined);
    await practicePlay();
    expect(invoke).toHaveBeenLastCalledWith("practice_play");
    await practicePause();
    expect(invoke).toHaveBeenLastCalledWith("practice_pause");
    await practiceSeek(12.5);
    expect(invoke).toHaveBeenLastCalledWith("practice_seek", {
      positionSecs: 12.5,
    });
    await practiceSetSpeed(1.5);
    expect(invoke).toHaveBeenLastCalledWith("practice_set_speed", {
      speed: 1.5,
    });
    await practiceSetPitch(-2, 30);
    expect(invoke).toHaveBeenLastCalledWith("practice_set_pitch", {
      semitones: -2,
      cents: 30,
    });
    await practiceSetVolume(0.4);
    expect(invoke).toHaveBeenLastCalledWith("practice_set_volume", {
      volume: 0.4,
    });
    await practiceClose();
    expect(invoke).toHaveBeenLastCalledWith("practice_close");
  });

  it("reads the transport position, including the empty state", async () => {
    invoke.mockResolvedValue(null);
    await expect(practicePosition()).resolves.toBeNull();
    expect(invoke).toHaveBeenLastCalledWith("practice_position");
  });

  it("reads and saves the persisted practice state", async () => {
    invoke.mockResolvedValue(DEFAULT_PRACTICE_STATE);
    await expect(practiceStateGet("C:\\audio\\a.mp3")).resolves.toEqual(
      DEFAULT_PRACTICE_STATE,
    );
    expect(invoke).toHaveBeenLastCalledWith("practice_state_get", {
      path: "C:\\audio\\a.mp3",
    });

    invoke.mockResolvedValue(undefined);
    await practiceStateSave("C:\\audio\\a.mp3", DEFAULT_PRACTICE_STATE);
    expect(invoke).toHaveBeenLastCalledWith("practice_state_save", {
      path: "C:\\audio\\a.mp3",
      state: DEFAULT_PRACTICE_STATE,
    });
  });

  it("limits the recent list and hides entries", async () => {
    invoke.mockResolvedValue([]);
    await practiceRecent();
    expect(invoke).toHaveBeenLastCalledWith("practice_recent", { limit: 20 });
    await practiceRecent(5);
    expect(invoke).toHaveBeenLastCalledWith("practice_recent", { limit: 5 });
    await practiceRecentHide("C:\\audio\\a.mp3");
    expect(invoke).toHaveBeenLastCalledWith("practice_recent_hide", {
      path: "C:\\audio\\a.mp3",
    });
  });

  it("requests the waveform and cancels analysis", async () => {
    invoke.mockResolvedValue({ version: 1, durationSecs: 10, levels: [] });
    await expect(practiceWaveform("C:\\audio\\a.mp3")).resolves.toMatchObject({
      durationSecs: 10,
    });
    expect(invoke).toHaveBeenLastCalledWith("practice_waveform", {
      path: "C:\\audio\\a.mp3",
    });

    invoke.mockResolvedValue(undefined);
    await practiceAnalysisCancel();
    expect(invoke).toHaveBeenLastCalledWith("practice_analysis_cancel");
  });

  it("subscribes to every practice and analysis channel", async () => {
    const handlers = {
      error: vi.fn(),
      progress: vi.fn(),
      failedAnalysis: vi.fn(),
    };
    await onPracticeError(handlers.error);
    await onPracticeAnalysisProgress(handlers.progress);
    await onPracticeAnalysisError(handlers.failedAnalysis);

    expect(listen.mock.calls.map(([event]) => event)).toEqual(
      expect.arrayContaining([
        "practice://error",
        "analysis://progress",
        "analysis://error",
      ]),
    );

    const emit = (event: string, payload: unknown) => {
      for (const [name, callback] of listen.mock.calls) {
        if (name === event) (callback as (e: { payload: unknown }) => void)({ payload });
      }
    };
    emit("practice://error", { message: "device lost" });
    emit("analysis://progress", { path: "a", stage: "beat", progress: 0.5 });
    emit("analysis://error", { path: "a", message: "model missing" });

    expect(handlers.error).toHaveBeenCalledWith({ message: "device lost" });
    expect(handlers.progress).toHaveBeenCalledWith({
      path: "a",
      stage: "beat",
      progress: 0.5,
    });
    expect(handlers.failedAnalysis).toHaveBeenCalledWith({
      path: "a",
      message: "model missing",
    });
  });
});
