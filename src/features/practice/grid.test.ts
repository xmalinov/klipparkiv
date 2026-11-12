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
    expect(buildBarStarts(beats, 4, 1)).toEqual([1, 5]);
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
        beatConfidence: [1, 1, 1, 1, 1, 1, 1, 1, 1],
        downbeatConfidence: [1, 1, 1],
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

  it("uses model downbeats for automatic bars", () => {
    const grid = buildGrid(
      {
        version: 2,
        path: "a.mp3",
        durationSecs: 4,
        bpmRaw: 120,
        bpm: 120,
        octave: 1,
        confidence: 0.9,
        beats: [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4],
        downbeats: [0, 1.5, 3],
        beatConfidence: [1, 1, 1, 1, 1, 1, 1, 1, 1],
        downbeatConfidence: [1, 1, 1],
        beatsPerBar: 4,
        barOffset: 0,
        model: "test",
        analyzedAt: 0,
      },
      null,
      4,
      1,
      4,
      0,
    );
    expect(grid!.barStarts).toEqual([0, 1.5, 3]);
  });

  it("forces a fixed meter when the grid is manual", () => {
    const grid = buildGrid(
      {
        version: 2,
        path: "a.mp3",
        durationSecs: 4,
        bpmRaw: 120,
        bpm: 120,
        octave: 1,
        confidence: 0.9,
        beats: [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4],
        downbeats: [0, 1.5, 3],
        beatConfidence: [1, 1, 1, 1, 1, 1, 1, 1, 1],
        downbeatConfidence: [1, 1, 1],
        beatsPerBar: 4,
        barOffset: 0,
        model: "test",
        analyzedAt: 0,
      },
      null,
      4,
      1,
      7,
      0,
      true,
    );
    expect(grid!.barStarts).toEqual([0, 3.5]);
  });

  it("keeps the reported bar offset inside the reported meter", () => {
    const grid = buildGrid(
      {
        version: 2,
        path: "a.mp3",
        durationSecs: 8,
        bpmRaw: 120,
        bpm: 120,
        octave: 1,
        confidence: 0.9,
        beats: [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5, 5.5, 6, 6.5, 7, 7.5],
        downbeats: [0, 1.5, 3, 4.5, 6],
        beatConfidence: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        downbeatConfidence: [1, 1, 1, 1, 1],
        beatsPerBar: 4,
        barOffset: 3,
        model: "test",
        analyzedAt: 0,
      },
      null,
      8,
      1,
      4,
      3,
    );
    expect(grid!.barOffset).toBeLessThan(grid!.beatsPerBar);
  });

  it("keeps buildGrid invariants across meters and scales", () => {
    const baseBeats = Array.from({ length: 40 }, (_, index) => index * 0.5);
    for (let meter = 2; meter <= 16; meter += 1) {
      for (let offset = 0; offset < meter; offset += 1) {
        const downbeats = baseBeats.filter((_, index) => index % meter === 0);
        const analysis = {
          version: 2,
          path: "a.mp3",
          durationSecs: 20,
          bpmRaw: 120,
          bpm: 120,
          octave: 1,
          confidence: 0.8,
          beats: baseBeats,
          downbeats,
          beatConfidence: baseBeats.map(() => 1),
          downbeatConfidence: downbeats.map(() => 1),
          beatsPerBar: meter,
          barOffset: offset,
          model: "test",
          analyzedAt: 0,
        };

        for (const scale of [0.5, 1, 2]) {
          for (const manualGrid of [false, true]) {
            const grid = buildGrid(
              analysis,
              null,
              20,
              scale,
              meter,
              offset,
              manualGrid,
            );
            expect(grid).not.toBeNull();
            if (!grid) continue;

            expect(grid.beatsPerBar).toBeGreaterThanOrEqual(1);
            expect(grid.beatsPerBar).toBeLessThanOrEqual(16);
            expect(grid.barOffset).toBeLessThan(grid.beatsPerBar);
            expect(Number.isFinite(grid.bpm)).toBe(true);
            expect(grid.bpm).toBeGreaterThan(0);
            expect(grid.confidence).toBeGreaterThanOrEqual(0);
            expect(grid.confidence).toBeLessThanOrEqual(1);
            expect(grid.barStarts.length).toBeGreaterThan(0);

            for (let index = 1; index < grid.beats.length; index += 1) {
              expect(grid.beats[index]).toBeGreaterThan(grid.beats[index - 1]);
            }
            for (let index = 1; index < grid.barStarts.length; index += 1) {
              expect(grid.barStarts[index]).toBeGreaterThan(
                grid.barStarts[index - 1],
              );
            }
            for (const bar of grid.barStarts) {
              expect(bar).toBeGreaterThanOrEqual(grid.beats[0]);
              expect(bar).toBeLessThanOrEqual(
                grid.beats[grid.beats.length - 1],
              );
            }
          }
        }
      }
    }
  });

  it("falls back to a fixed grid when the analysis has no downbeats", () => {
    const grid = buildGrid(
      {
        version: 2,
        path: "a.mp3",
        durationSecs: 4,
        bpmRaw: 120,
        bpm: 120,
        octave: 1,
        confidence: 0.2,
        beats: [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4],
        downbeats: [],
        beatConfidence: [1, 1, 1, 1, 1, 1, 1, 1, 1],
        downbeatConfidence: [],
        beatsPerBar: 4,
        barOffset: 0,
        model: "test",
        analyzedAt: 0,
      },
      null,
      4,
      1,
      4,
      0,
    );
    expect(grid!.barStarts).toEqual([0, 2, 4]);
  });

  it("needs usable beats and a duration for a manual grid", () => {
    expect(buildGrid(null, 120, 0, 1, 4, 0)).toBeNull();
    expect(
      buildGrid(
        {
          version: 2,
          path: "a.mp3",
          durationSecs: 4,
          bpmRaw: 120,
          bpm: 120,
          octave: 1,
          confidence: 0.2,
          beats: [0],
          downbeats: [],
          beatConfidence: [1],
          downbeatConfidence: [],
          beatsPerBar: 4,
          barOffset: 0,
          model: "test",
          analyzedAt: 0,
        },
        null,
        4,
        1,
        4,
        0,
      ),
    ).toBeNull();
  });

  it("scales the displayed BPM without moving beats", () => {
    const base = buildGrid(null, 120, 4, 1, 4, 0)!;
    const doubled = buildGrid(null, 120, 4, 2, 4, 0)!;
    expect(doubled.bpm).toBe(240);
    expect(doubled.beats[1]).toBeCloseTo(0.25);
    expect(base.beats[1]).toBeCloseTo(0.5);
  });
});
