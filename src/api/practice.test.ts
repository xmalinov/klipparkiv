import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  decodePeaks,
  onPracticeAnalysisDone,
  onPracticePosition,
  practiceAnalysisGet,
  practiceAnalysisStart,
  practiceOpen,
  practiceSetGrid,
  practiceSetLoop,
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
});
