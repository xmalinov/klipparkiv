import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LibraryItem } from "../../api/library";
import {
  DEFAULT_PRACTICE_STATE,
  type PracticeAnalysis,
  type PracticeRegion,
  type PracticeState,
  type Waveform,
} from "../../api/practice";
import { renderPractice, resetPracticeProbe } from "../../test/practice";
import {
  createCommandRouter,
  createEventBus,
  flush,
  type PracticeEventBus,
  type RoutedCommands,
} from "../../test/tauri";
import { usePractice } from "./PracticeProvider";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

const AUDIO: LibraryItem = {
  id: "track-a",
  path: "C:\\audio\\song.mp3",
  kind: "audio",
  title: "Song A",
  channel: "Artist",
  durationSecs: 120,
  width: null,
  height: null,
  sizeBytes: 1024,
  mtime: 1,
  thumbPath: null,
};

const VIDEO: LibraryItem = {
  ...AUDIO,
  id: "track-v",
  path: "C:\\video\\clip.mp4",
  kind: "video",
  title: "Clip",
};

const WAVEFORM: Waveform = { version: 1, durationSecs: 120, levels: [] };

const ANALYSIS: PracticeAnalysis = {
  version: 1,
  path: AUDIO.path,
  durationSecs: 120,
  bpmRaw: 120,
  bpm: 120,
  octave: 0,
  confidence: 0.9,
  beats: [0, 0.5, 1, 1.5],
  downbeats: [0, 2],
  beatConfidence: [1, 1, 1, 1],
  downbeatConfidence: [1, 1],
  beatsPerBar: 3,
  barOffset: 1,
  model: "beat-this",
  analyzedAt: 1,
};

const IRREGULAR_ANALYSIS: PracticeAnalysis = {
  version: 2,
  path: AUDIO.path,
  durationSecs: 12,
  bpmRaw: 120,
  bpm: 120,
  octave: 1,
  confidence: 0.8,
  beats: Array.from({ length: 25 }, (_, index) => index * 0.5),
  downbeats: [0, 1.5, 3, 4.5, 6, 7.5],
  beatConfidence: Array.from({ length: 25 }, () => 1),
  downbeatConfidence: Array.from({ length: 6 }, () => 1),
  beatsPerBar: 3,
  barOffset: 0,
  model: "test",
  analyzedAt: 1,
};

const REGION_A: PracticeRegion = {
  id: "r1",
  name: "Verse",
  kind: "verse",
  color: "#111111",
  start: 5,
  end: 10,
};

const REGION_B: PracticeRegion = {
  id: "r2",
  name: "Chorus",
  kind: "chorus",
  color: "#222222",
  start: 15,
  end: 20,
};

function opened(state: Partial<PracticeState> = {}) {
  return {
    path: AUDIO.path,
    durationSecs: 120,
    sampleRate: 48000,
    state: { ...DEFAULT_PRACTICE_STATE, ...state },
  };
}

function last<T>(items: T[]): T | undefined {
  return items[items.length - 1];
}

function lastCall(command: string) {
  return last(router.args(command));
}

let router: RoutedCommands;
let bus: PracticeEventBus;

async function openTrack(
  view: ReturnType<typeof renderPractice>,
  state: Partial<PracticeState> = {},
  item: LibraryItem = AUDIO,
) {
  router.on("practice_open", () => opened(state));
  await act(async () => {
    await view.api().openTrack(item);
  });
}

afterEach(() => {
  vi.useRealTimers();
  resetPracticeProbe();
});

beforeEach(() => {
  invokeMock.mockReset();
  listenMock.mockReset();
  router = createCommandRouter(invokeMock);
  bus = createEventBus(listenMock);
  router.on("practice_recent", () => []);
  router.on("practice_waveform", () => WAVEFORM);
  router.on("practice_analysis_get", () => null);
});

describe("PracticeProvider", () => {
  describe("opening a track", () => {
    it("hydrates track, state and duration from the backend", async () => {
      const view = renderPractice();
      await openTrack(view, { speed: 1.25, positionSecs: 12 });

      expect(view.api().track?.path).toBe(AUDIO.path);
      expect(view.api().state.speed).toBe(1.25);
      expect(view.api().snapshot.positionSecs).toBe(12);
      expect(view.api().snapshot.durationSecs).toBe(120);
      expect(view.api().opening).toBe(false);
      expect(router.args("practice_open")).toEqual([{ path: AUDIO.path }]);
      expect(router.args("practice_state_save")[0]).toMatchObject({
        path: AUDIO.path,
        state: { speed: 1.25 },
      });
      expect(router.count("practice_waveform")).toBe(1);
      expect(router.count("practice_analysis_get")).toBe(1);
    });

    it("ignores items that are not audio", async () => {
      const view = renderPractice();
      await openTrack(view, {}, VIDEO);

      expect(view.api().track).toBeNull();
      expect(router.count("practice_open")).toBe(0);
    });

    it("reports an open failure and leaves the screen empty", async () => {
      router.fail("practice_open", "unsupported codec");
      const view = renderPractice();

      await act(async () => {
        await view.api().openTrack(AUDIO);
      });

      expect(view.api().track).toBeNull();
      expect(view.api().opening).toBe(false);
      expect(view.api().error).toContain("unsupported codec");
    });

    it("sets waveformLoading until the waveform arrives", async () => {
      let resolveWaveform!: (value: Waveform) => void;
      router.on(
        "practice_waveform",
        () =>
          new Promise<Waveform>((resolve) => {
            resolveWaveform = resolve;
          }),
      );
      const view = renderPractice();
      await openTrack(view);

      expect(view.api().waveformLoading).toBe(true);

      await act(async () => {
        resolveWaveform(WAVEFORM);
        await Promise.resolve();
      });

      expect(view.api().waveform).toEqual(WAVEFORM);
      expect(view.api().waveformLoading).toBe(false);
    });
  });

  describe("transport", () => {
    it("plays, pauses and persists the position", async () => {
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await act(async () => {
        await view.api().playPause();
      });
      expect(router.args("practice_play")).toEqual([undefined]);
      expect(view.api().snapshot.playing).toBe(true);

      await act(async () => {
        await view.api().playPause();
      });
      expect(router.count("practice_pause")).toBe(1);
      expect(view.api().snapshot.playing).toBe(false);
      expect(router.count("practice_state_save")).toBe(1);
    });

    it("seeks and mirrors the new position in state and snapshot", async () => {
      const view = renderPractice();
      await openTrack(view);

      await act(async () => {
        await view.api().seek(42.5);
      });

      expect(router.args("practice_seek")).toEqual([{ positionSecs: 42.5 }]);
      expect(view.api().state.positionSecs).toBe(42.5);
      expect(view.api().snapshot.positionSecs).toBe(42.5);
    });

    it("clamps volume but forwards speed and pitch as-is", async () => {
      const view = renderPractice();
      await openTrack(view);

      await act(async () => {
        await view.api().setVolume(1.7);
      });
      expect(router.args("practice_set_volume")).toEqual([{ volume: 1 }]);
      expect(view.api().state.volume).toBe(1);

      await act(async () => {
        await view.api().setVolume(-0.5);
      });
      expect(lastCall("practice_set_volume")).toEqual({ volume: 0 });
      expect(view.api().state.volume).toBe(0);

      await act(async () => {
        await view.api().setSpeed(1.75);
      });
      expect(router.args("practice_set_speed")).toEqual([{ speed: 1.75 }]);
      expect(view.api().state.speed).toBe(1.75);

      await act(async () => {
        await view.api().setPitch(-3, 25);
      });
      expect(router.args("practice_set_pitch")).toEqual([
        { semitones: -3, cents: 25 },
      ]);
      expect(view.api().state.pitchSemitones).toBe(-3);
      expect(view.api().state.cents).toBe(25);
    });

    it("surfaces a backend error without changing optimistic state", async () => {
      const view = renderPractice();
      await openTrack(view);
      router.fail("practice_set_speed", "speed rejected");

      await act(async () => {
        await view.api().setSpeed(1.5);
      });

      expect(view.api().state.speed).toBe(1);
      expect(view.api().error).toContain("speed rejected");
    });

    it("takes the latest position from playback events when saving", async () => {
      vi.useFakeTimers();
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await bus.emit("practice://position", {
        positionSecs: 33,
        durationSecs: 120,
        playing: true,
        ended: false,
        underruns: 0,
      });
      await act(async () => {
        await view.api().setSpeed(1.25);
      });
      await act(async () => {
        vi.advanceTimersByTime(500);
        await Promise.resolve();
      });

      const saved = router.args("practice_state_save")[0] as {
        state: PracticeState;
      };
      expect(saved.state.positionSecs).toBe(33);
      expect(saved.state.speed).toBe(1.25);
    });

    it("autosaves every five seconds while playing", async () => {
      vi.useFakeTimers();
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await bus.emit("practice://position", {
        positionSecs: 25,
        durationSecs: 120,
        playing: true,
        ended: false,
        underruns: 0,
      });
      await act(async () => {
        vi.advanceTimersByTime(5000);
        await Promise.resolve();
      });

      const saves = router.args("practice_state_save") as {
        state: PracticeState;
      }[];
      expect(saves.length).toBeGreaterThan(0);
      expect(last(saves)?.state.positionSecs).toBe(25);
    });
  });

  describe("persistence", () => {
    it("debounces state saves by half a second", async () => {
      vi.useFakeTimers();
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await act(async () => {
        await view.api().setSpeed(1.25);
      });
      await act(async () => {
        vi.advanceTimersByTime(499);
        await Promise.resolve();
      });
      expect(router.count("practice_state_save")).toBe(0);

      await act(async () => {
        vi.advanceTimersByTime(1);
        await Promise.resolve();
      });
      expect(router.count("practice_state_save")).toBe(1);
    });

    it("closes the track and resets the practice session", async () => {
      const view = renderPractice();
      await openTrack(view, { activeRegionId: "r1", regions: [REGION_A] });
      router.reset();

      await act(async () => {
        await view.api().close();
      });

      expect(router.count("practice_state_save")).toBe(1);
      expect(router.count("practice_close")).toBe(1);
      expect(router.count("practice_analysis_cancel")).toBe(1);
      expect(view.api().track).toBeNull();
      expect(view.api().state).toEqual(DEFAULT_PRACTICE_STATE);
      expect(view.api().snapshot.positionSecs).toBe(0);
      expect(view.api().analysis).toBeNull();
      expect(view.api().analysisStatus).toBe("idle");
      expect(view.api().selection).toBeNull();
    });

    it("reports a failed state save", async () => {
      const view = renderPractice();
      await openTrack(view);
      router.fail("practice_state_save", "disk full");
      vi.useFakeTimers();

      await act(async () => {
        await view.api().setSpeed(1.25);
      });
      await act(async () => {
        vi.advanceTimersByTime(500);
        await Promise.resolve();
      });

      expect(view.api().error).toContain("disk full");
    });
  });

  describe("looping and regions", () => {
    const withRegions = () => ({
      regions: [REGION_A, REGION_B],
      activeRegionId: "r1",
    });

    it("enables the loop on the active region and clears it", async () => {
      const view = renderPractice();
      await openTrack(view, withRegions());
      router.reset();

      await act(async () => {
        await view.api().setLoopEnabled(true);
      });
      expect(router.args("practice_set_loop")[0]).toMatchObject({
        enabled: true,
        startSecs: 5,
        endSecs: 10,
      });
      expect(view.api().state.loopEnabled).toBe(true);

      await act(async () => {
        await view.api().setLoopEnabled(false);
      });
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: false,
        startSecs: null,
        endSecs: null,
      });
      expect(view.api().state.loopEnabled).toBe(false);
    });

    it("does not enable looping without regions", async () => {
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await act(async () => {
        await view.api().setLoopEnabled(true);
      });

      expect(view.api().state.loopEnabled).toBe(false);
      expect(router.args("practice_set_loop")[0]).toMatchObject({ enabled: false });
    });

    it("loops a specific region and follows the active region selection", async () => {
      const view = renderPractice();
      await openTrack(view, withRegions());

      await act(async () => {
        await view.api().loopRegion("r2");
      });
      expect(view.api().state.activeRegionId).toBe("r2");
      expect(view.api().state.loopEnabled).toBe(true);
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: true,
        startSecs: 15,
        endSecs: 20,
      });

      await act(async () => {
        await view.api().selectRegion("r1");
      });
      expect(lastCall("practice_set_loop")).toMatchObject({
        startSecs: 5,
        endSecs: 10,
      });
    });

    it("does not re-apply the loop when the active region is clicked again", async () => {
      const view = renderPractice();
      await openTrack(view, { ...withRegions(), loopEnabled: true });
      router.reset();

      await act(async () => {
        await view.api().selectRegion("r1");
      });

      expect(view.api().state.activeRegionId).toBe("r1");
      expect(router.count("practice_set_loop")).toBe(0);
    });

    it("leaves the loop when seeking outside the active region", async () => {
      const view = renderPractice();
      await openTrack(view, { ...withRegions(), loopEnabled: true });
      router.reset();

      await act(async () => {
        await view.api().seek(12);
      });

      expect(router.args("practice_set_loop")[0]).toMatchObject({
        enabled: false,
      });
      expect(lastCall("practice_seek")).toEqual({ positionSecs: 12 });
      expect(view.api().state.loopEnabled).toBe(false);
      expect(view.api().snapshot.positionSecs).toBe(12);
    });

    it("keeps the loop when seeking inside the active region", async () => {
      const view = renderPractice();
      await openTrack(view, { ...withRegions(), loopEnabled: true });
      router.reset();

      await act(async () => {
        await view.api().seek(6);
      });

      expect(router.count("practice_set_loop")).toBe(0);
      expect(view.api().state.loopEnabled).toBe(true);
      expect(lastCall("practice_seek")).toEqual({ positionSecs: 6 });
    });

    it("selects a region by its sorted index", async () => {
      const view = renderPractice();
      await openTrack(view, {
        regions: [REGION_B, REGION_A],
        activeRegionId: null,
      });

      await act(async () => {
        await view.api().selectRegionByIndex(0);
      });
      expect(view.api().state.activeRegionId).toBe("r1");
    });

    it("creates a region from the current selection", async () => {
      const view = renderPractice();
      await openTrack(view);

      view.api().setSelection({ start: 2, end: 6 });
      await flush();
      await act(async () => {
        await view.api().addRegion();
      });

      expect(view.api().state.regions).toHaveLength(1);
      expect(view.api().state.regions[0]).toMatchObject({
        name: "Region 1",
        kind: "custom",
        start: 2,
        end: 6,
      });
      expect(view.api().state.loopEnabled).toBe(true);
      expect(view.api().selection).toBeNull();
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: true,
        startSecs: 2,
        endSecs: 6,
      });
    });

    it("falls back to the track title and clamps the region to the duration", async () => {
      const view = renderPractice();
      await openTrack(view);

      view.api().setSelection({ start: 100, end: 500 });
      await flush();
      await act(async () => {
        await view.api().saveSelectionAsRegion("   ", "chorus", "#abcdef");
      });

      expect(view.api().state.regions[0]).toMatchObject({
        name: "Song A",
        kind: "chorus",
        color: "#abcdef",
        start: 100,
        end: 120,
      });
    });

    it("rejects an inverted region and keeps the existing one", async () => {
      const view = renderPractice();
      await openTrack(view, withRegions());

      await act(async () => {
        await view.api().updateRegion("r1", { start: 12 });
      });

      expect(view.api().state.regions.find((r) => r.id === "r1")).toMatchObject({
        start: 5,
        end: 10,
      });
    });

    it("resorts regions after an edit and refreshes an active loop", async () => {
      const view = renderPractice();
      await openTrack(view, { ...withRegions(), loopEnabled: true });
      router.reset();

      await act(async () => {
        await view.api().updateRegion("r1", { start: 16, end: 18 });
      });

      expect(view.api().state.regions.map((r) => r.id)).toEqual(["r2", "r1"]);
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: true,
        startSecs: 16,
        endSecs: 18,
      });
    });

    it("clears the loop when the active region is deleted", async () => {
      const view = renderPractice();
      await openTrack(view, {
        ...withRegions(),
        loopEnabled: true,
        regions: [
          REGION_A,
          REGION_B,
          { ...REGION_A, id: "ab", name: "A-B", start: 30, end: 40 },
        ],
      });
      router.reset();

      await act(async () => {
        await view.api().deleteRegion("r1");
      });
      expect(view.api().state.regions.map((r) => r.id)).toEqual(["r2", "ab"]);
      expect(view.api().state.activeRegionId).toBeNull();
      expect(view.api().state.loopEnabled).toBe(false);
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: false,
      });

      await act(async () => {
        await view.api().clearLoop();
      });
      expect(view.api().state.regions.map((r) => r.id)).toEqual(["r2"]);
    });

    it("sets A-B boundaries from the cursor position", async () => {
      const view = renderPractice();
      await openTrack(view);

      await act(async () => {
        await view.api().seek(6);
      });
      view.api().setSelection({ start: 3, end: 8 });
      await flush();
      await act(async () => {
        await view.api().setLoopBoundary("end");
      });

      const ab = view.api().state.regions.find((r) => r.id === "ab");
      expect(ab).toMatchObject({ name: "A-B", start: 3, end: 6 });
      expect(view.api().selection).toEqual({ start: 3, end: 6 });
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: true,
        startSecs: 3,
        endSecs: 6,
      });
    });

    it("nudges the loop start but refuses to cross it", async () => {
      const view = renderPractice();
      await openTrack(view, { ...withRegions(), loopEnabled: true });

      await act(async () => {
        await view.api().nudgeLoop("start", -10);
      });
      expect(view.api().state.regions.find((r) => r.id === "r1")?.start).toBe(0);

      await act(async () => {
        await view.api().nudgeLoop("end", -100);
      });
      expect(view.api().state.regions.find((r) => r.id === "r1")?.end).toBe(10);
    });

    it("refuses to loop bars before the grid is ready", async () => {
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await act(async () => {
        await view.api().loopBars(2);
      });

      expect(view.api().error).toBe("Beat grid is not ready");
      expect(router.count("practice_set_loop")).toBe(0);
    });

    it("loops the requested number of bars from the cursor", async () => {
      const view = renderPractice();
      await openTrack(view, { manualBpm: 120, positionSecs: 5 });

      await act(async () => {
        await view.api().loopBars(2);
      });

      const ab = view.api().state.regions.find((r) => r.id === "ab");
      expect(ab).toMatchObject({ name: "2 bars", start: 4, end: 8 });
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: true,
        startSecs: 4,
        endSecs: 8,
      });
    });
  });

  describe("beat grid and metronome", () => {
    it("clamps manual BPM and creates a grid from it", async () => {
      const view = renderPractice();
      await openTrack(view);

      await act(async () => {
        await view.api().setManualBpm(500);
      });
      expect(view.api().state.manualBpm).toBe(400);
      expect(view.api().grid?.bpm).toBe(400);

      await act(async () => {
        await view.api().setManualBpm(120);
      });
      expect(view.api().grid?.bpm).toBe(120);

      await act(async () => {
        await view.api().setManualBpm(0);
      });
      expect(view.api().state.manualBpm).toBeNull();
      expect(view.api().grid).toBeNull();
    });

    it("rounds beats per bar and wraps the bar offset", async () => {
      const view = renderPractice();
      await openTrack(view);

      await act(async () => {
        await view.api().setBeatsPerBar(99);
      });
      expect(view.api().state.beatsPerBar).toBe(16);
      expect(view.api().state.gridCustomized).toBe(true);

      await act(async () => {
        await view.api().setBarOffset(-1);
      });
      expect(view.api().state.barOffset).toBe(15);

      await act(async () => {
        await view.api().setBeatsPerBar(0);
      });
      expect(view.api().state.beatsPerBar).toBe(1);
      expect(view.api().state.barOffset).toBe(0);
    });

    it("returns to automatic bars and applies the analysis phase", async () => {
      router.on("practice_analysis_get", () => ANALYSIS);
      const view = renderPractice();
      await openTrack(view);
      expect(view.api().grid).not.toBeNull();

      await act(async () => {
        await view.api().setBeatsPerBar(7);
      });
      expect(view.api().state.gridCustomized).toBe(true);

      await act(async () => {
        await view.api().setAutoBars();
      });
      expect(view.api().state.gridCustomized).toBe(false);
      expect(view.api().state.manualBpm).toBeNull();
      expect(view.api().state.beatsPerBar).toBe(3);
      expect(view.api().state.barOffset).toBe(1);
    });

    it("snaps and loops with irregular model bars", async () => {
      router.on("practice_analysis_get", () => IRREGULAR_ANALYSIS);
      const view = renderPractice();
      await openTrack(view, { positionSecs: 3.6 });

      expect(view.api().grid?.barStarts).toEqual([0, 1.5, 3, 4.5, 6, 7.5]);
      expect(view.api().grid?.beatsPerBar).toBe(3);
      expect(view.api().snapTime(3.6, "bar")).toBe(3);

      await act(async () => {
        await view.api().loopBars(2);
      });
      expect(lastCall("practice_set_loop")).toMatchObject({
        enabled: true,
        startSecs: 3,
        endSecs: 6,
      });
    });

    it("sets bar 1 at the playhead using the current meter", async () => {
      const view = renderPractice();
      await openTrack(view, { manualBpm: 120, positionSecs: 5 });

      await act(async () => {
        await view.api().setBarOneAtPlayhead();
      });

      expect(view.api().state.gridCustomized).toBe(true);
      // Position 5 s is beat 10 at 120 BPM; 10 % 4 == 2.
      expect(view.api().state.barOffset).toBe(2);
    });

    it("snaps the grid scale to half, single or double beats", async () => {
      const view = renderPractice();
      await openTrack(view);

      for (const [input, expected] of [
        [0.4, 0.5],
        [1.2, 1],
        [1.6, 2],
      ] as const) {
        await act(async () => {
          await view.api().setBpmScale(input);
        });
        expect(view.api().state.bpmScale).toBe(expected);
      }
    });

    it("snaps times to beats, bars or the free position", async () => {
      const view = renderPractice();
      await openTrack(view, { manualBpm: 60 });

      expect(view.api().snapTime(0.3)).toBe(0);
      expect(view.api().snapTime(0.8)).toBe(1);
      expect(view.api().snapTime(3.8, "bar")).toBe(4);
      expect(view.api().snapTime(3.2, "free")).toBe(3.2);
      expect(view.api().snapTime(500)).toBe(120);
    });

    it("returns the clamped time when there is no grid", async () => {
      const view = renderPractice();
      await openTrack(view);
      expect(view.api().snapTime(-4)).toBe(0);
      expect(view.api().snapTime(10.5)).toBe(10.5);
    });

    it("disables the grid, cancels analysis and clears the backend grid", async () => {
      router.on("practice_analysis_get", () => ANALYSIS);
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await act(async () => {
        await view.api().setGridEnabled(false);
      });

      expect(router.count("practice_analysis_cancel")).toBe(1);
      expect(lastCall("practice_set_grid")).toEqual({ grid: null });
      expect(view.api().analysis).toBeNull();
      expect(view.api().analysisStatus).toBe("idle");
      expect(view.api().grid).toBeNull();
    });

    it("clamps the metronome volume and turns it on for count-in", async () => {
      const view = renderPractice();
      await openTrack(view);
      router.reset();

      await act(async () => {
        await view.api().setMetronome(true, 3);
      });
      expect(lastCall("practice_set_metronome")).toEqual({
        enabled: true,
        volume: 1,
      });
      expect(view.api().state.metronomeVolume).toBe(1);

      await act(async () => {
        await view.api().setMetronome(false);
      });
      const disabled = view.api().state.metronomeEnabled;
      expect(disabled).toBe(false);

      await act(async () => {
        await view.api().setCountIn(true);
      });
      expect(lastCall("practice_set_metronome")).toMatchObject({
        enabled: true,
      });
      expect(view.api().state.countIn).toBe(true);
    });
  });

  describe("beat analysis", () => {
    it("restores a cached analysis without starting a new job", async () => {
      router.on("practice_analysis_get", () => ANALYSIS);
      const view = renderPractice();
      await openTrack(view);

      expect(view.api().analysisStatus).toBe("ready");
      expect(view.api().analysisProgress).toBe(1);
      expect(view.api().state.beatsPerBar).toBe(3);
      expect(view.api().state.barOffset).toBe(1);
      expect(view.api().grid?.bpm).toBe(120);
      expect(router.count("practice_analysis_start")).toBe(0);
    });

    it("starts a job on a cache miss and applies progress and done events", async () => {
      const view = renderPractice();
      await openTrack(view);
      expect(router.count("practice_analysis_start")).toBe(1);

      await bus.emit("analysis://progress", {
        path: "C:\\audio\\other.mp3",
        stage: "beat",
        progress: 0.2,
      });
      expect(view.api().analysisProgress).toBe(0);

      await bus.emit("analysis://progress", {
        path: AUDIO.path,
        stage: "beat",
        progress: 0.4,
      });
      expect(view.api().analysisStatus).toBe("loading");
      expect(view.api().analysisProgress).toBe(0.4);

      await bus.emit("analysis://done", ANALYSIS);
      expect(view.api().analysisStatus).toBe("ready");
      expect(view.api().analysis?.bpm).toBe(120);
      expect(view.api().analysisProgress).toBe(1);
    });

    it("reports analysis failures only for the current track", async () => {
      const view = renderPractice();
      await openTrack(view);

      await bus.emit("analysis://error", {
        path: "C:\\audio\\other.mp3",
        message: "ignored",
      });
      expect(view.api().analysisError).toBeNull();

      await bus.emit("analysis://error", {
        path: AUDIO.path,
        message: "model missing",
      });
      expect(view.api().analysisStatus).toBe("error");
      expect(view.api().analysisError).toBe("model missing");
    });

    it("surfaces a failing cache lookup", async () => {
      router.fail("practice_analysis_get", "cache locked");
      const view = renderPractice();
      await openTrack(view);

      expect(view.api().analysisStatus).toBe("error");
      expect(view.api().analysisError).toContain("cache locked");
    });
  });

  describe("recent tracks and errors", () => {
    it("loads recent tracks and hides one", async () => {
      router.on("practice_recent", () => [AUDIO]);
      const view = renderPractice();
      await flush();
      expect(view.api().recent).toHaveLength(1);

      await act(async () => {
        await view.api().hideRecent(AUDIO.path);
      });
      expect(router.args("practice_recent_hide")).toEqual([{ path: AUDIO.path }]);
      expect(view.api().recent).toEqual([]);
    });

    it("reports a failed hide and falls back to an empty recent list", async () => {
      router.on("practice_recent", () => [AUDIO]);
      const view = renderPractice();
      await flush();
      router.fail("practice_recent_hide", "db locked");

      await act(async () => {
        await view.api().hideRecent(AUDIO.path);
      });
      expect(view.api().error).toContain("db locked");
      expect(view.api().recent).toHaveLength(1);
    });

    it("falls back to an empty list when recent cannot be loaded", async () => {
      router.fail("practice_recent", "db offline");
      const view = renderPractice();
      await flush();
      expect(view.api().recent).toEqual([]);
    });

    it("shows playback errors pushed by the backend", async () => {
      const view = renderPractice();
      await openTrack(view);

      await bus.emit("practice://error", { message: "device lost" });
      expect(view.api().error).toBe("device lost");
    });
  });

  describe("lifecycle", () => {
    it("removes every event listener when unmounted", async () => {
      const view = renderPractice();
      await flush();
      expect(bus.listenerCount()).toBe(5);

      view.unmount();
      await flush();
      expect(bus.listenerCount()).toBe(0);
    });

    it("requires a provider for usePractice", () => {
      const spy = vi.spyOn(console, "error").mockImplementation(() => {});
      function Bare() {
        usePractice();
        return null;
      }
      expect(() => render(<Bare />)).toThrow(
        "usePractice must be used inside PracticeProvider",
      );
      spy.mockRestore();
    });
  });
});
