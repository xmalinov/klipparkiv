import { describe, expect, it } from "vitest";
import {
  buildBarStarts,
  buildGrid,
  nearestValue,
  scaleBeats,
} from "./PracticeProvider";

describe("practice grid helpers", () => {
  it("finds the nearest value", () => {
    expect(nearestValue([0, 1, 2], 0.6)).toBe(1);
    expect(nearestValue([0, 1, 2], 0.4)).toBe(0);
    expect(nearestValue([], 1)).toBeNull();
  });

  it("subdivides and halves beats", () => {
    expect(scaleBeats([0, 1, 2], 2)).toEqual([0, 0.5, 1, 1.5, 2]);
    expect(scaleBeats([0, 1, 2, 3], 0.5)).toEqual([0, 2]);
  });

  it("builds bar starts with an offset", () => {
    const beats = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    expect(buildBarStarts(beats, 4, 0)).toEqual([0, 4, 8]);
    expect(buildBarStarts(beats, 4, 1)).toEqual([0, 1, 5]);
  });

  it("builds a manual grid from BPM", () => {
    const grid = buildGrid(null, 120, 4, 1, 4, 0);
    expect(grid).not.toBeNull();
    expect(grid!.bpm).toBe(120);
    expect(grid!.beats[1] - grid!.beats[0]).toBeCloseTo(0.5);
    expect(grid!.barStarts).toEqual([0, 2, 4]);
  });

  it("uses a cached analysis grid", () => {
    const grid = buildGrid(
      {
        version: 1,
        path: "a.mp3",
        durationSecs: 4,
        bpmRaw: 120,
        bpm: 120,
        octave: 1,
        confidence: 0.9,
        beats: [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4],
        downbeats: [0, 2, 4],
        beatsPerBar: 4,
        barOffset: 0,
        model: "test",
        analyzedAt: 0,
      },
      120,
      4,
      1,
      4,
      0,
    );
    expect(grid!.bpm).toBe(120);
    expect(grid!.barStarts).toEqual([0, 2, 4]);
  });

  it("scales the displayed BPM without moving beats", () => {
    const base = buildGrid(null, 120, 4, 1, 4, 0)!;
    const doubled = buildGrid(null, 120, 4, 2, 4, 0)!;
    expect(doubled.bpm).toBe(240);
    expect(doubled.beats[1]).toBeCloseTo(0.25);
    expect(base.beats[1]).toBeCloseTo(0.5);
  });
});
