import { describe, expect, it } from "vitest";
import {
  autoBarStarts,
  fixedBarStarts,
  nearestBeatIndex,
  phaseVote,
} from "./gridMath";

function beats(count: number, step = 0.5): number[] {
  return Array.from({ length: count }, (_, index) => index * step);
}

describe("grid math", () => {
  it("finds the nearest beat index", () => {
    const values = [0, 1, 2];
    expect(nearestBeatIndex(values, 0.6)).toBe(1);
    expect(nearestBeatIndex(values, 0.4)).toBe(0);
    expect(nearestBeatIndex(values, 9)).toBe(2);
    expect(nearestBeatIndex([], 1)).toBeNull();
  });

  it("votes for a phase across all confident downbeats", () => {
    const vote = phaseVote([1, 4, 8, 12], [0.1, 0.9, 0.9, 0.9], 4);
    expect(vote.offset).toBe(0);
    expect(vote.coverage).toBeGreaterThan(0.9);
  });

  it("does not invent a bar at zero for an offset grid", () => {
    const plan = fixedBarStarts(beats(9), 4, 1);
    expect(plan.barStarts).toEqual([0.5, 2.5]);
    expect(plan.pickupUntil).toBe(0);
  });

  it("uses model downbeats as bars and ignores weak or too-close candidates", () => {
    const plan = autoBarStarts(
      beats(24),
      [0, 0.5, 2, 4, 6, 8],
      [1, 0.1, 1, 1, 1, 1],
    );
    expect(plan.barStarts).toEqual([0, 2, 4, 6, 8]);
    expect(plan.pickupUntil).toBeNull();
    expect(plan.confidence).toBe(1);
  });

  it("marks an intro before the first confident downbeat as a pickup", () => {
    const plan = autoBarStarts(beats(24), [2, 4, 6, 8], [1, 1, 1, 1]);
    expect(plan.barStarts).toEqual([2, 4, 6, 8]);
    expect(plan.pickupUntil).toBe(0);
  });

  it("fills a missing downbeat when the gap is a whole number of bars", () => {
    const plan = autoBarStarts(beats(24), [0, 10], [1, 1]);
    expect(plan.barStarts).toEqual([0, 2, 4, 6, 8, 10]);
  });

  it("fills a shorter missing downbeat inside the normal range", () => {
    const plan = autoBarStarts(beats(24), [0, 4, 6, 8], [1, 1, 1, 1]);
    expect(plan.barStarts).toEqual([0, 2, 4, 6, 8]);
  });

  it("drops downbeats below the adaptive confidence threshold", () => {
    const plan = autoBarStarts(
      beats(24),
      [0, 2, 4, 6, 8],
      [1, 0.5, 0.9, 0.9, 0.9],
    );
    expect(plan.barStarts).toEqual([0, 2, 4, 6, 8]);
  });

  it("falls back to a fixed grid when there are too few downbeats", () => {
    const plan = autoBarStarts(beats(9), [0.5], [1]);
    expect(plan.barStarts.length).toBeGreaterThan(0);
    expect(plan.barStarts[0]).toBe(0.5);
  });

  it("does not draw a bar on every beat when downbeats are noisy", () => {
    const plan = autoBarStarts(
      beats(9),
      [0, 0.5, 1, 1.5, 2],
      [1, 1, 1, 1, 1],
    );
    expect(plan.barStarts).toEqual([0, 2, 4]);
    expect(plan.confidence).toBe(0.2);
  });

  it("clamps meters and normalizes offsets", () => {
    const values = beats(9);
    expect(fixedBarStarts([], 4, 0).barStarts).toEqual([]);
    expect(fixedBarStarts(values, 0, 0).meter).toBe(1);
    expect(fixedBarStarts(values, 17, 0).meter).toBe(16);
    expect(fixedBarStarts(values, 4, -1).barStarts).toEqual([
      values[3],
      values[7],
    ]);
  });

  it("keeps long grids sorted and inside the track", () => {
    const count = 12_000;
    const beatTimes = beats(count);
    const downbeats: number[] = [];
    const confidence: number[] = [];
    for (let index = 0; index < count; index += 4) {
      downbeats.push(beatTimes[index]);
      confidence.push(0.8);
      if (index % 40 === 0 && index + 1 < count) {
        downbeats.push(beatTimes[index + 1]);
        confidence.push(0.3);
      }
    }

    const started = performance.now();
    const plan = autoBarStarts(beatTimes, downbeats, confidence);
    expect(performance.now() - started).toBeLessThan(2_000);
    expect(plan.barStarts.length).toBeGreaterThan(1_000);
    for (let index = 1; index < plan.barStarts.length; index += 1) {
      expect(plan.barStarts[index]).toBeGreaterThan(plan.barStarts[index - 1]);
    }
    expect(plan.barStarts[plan.barStarts.length - 1]).toBeLessThanOrEqual(
      beatTimes[beatTimes.length - 1],
    );
  });

  it("keeps invariants for randomized downbeats", () => {
    let state = 0x12345678;
    const random = () => {
      state = (Math.imul(state, 1_664_525) + 1_013_904_223) >>> 0;
      return state / 2 ** 32;
    };

    for (let run = 0; run < 100; run += 1) {
      const count = 40 + Math.floor(random() * 200);
      const beatTimes = [0];
      for (let index = 1; index < count; index += 1) {
        beatTimes.push(beatTimes[index - 1] + 0.25 + random() * 0.6);
      }

      const downbeats: number[] = [];
      const confidence: number[] = [];
      let index = 0;
      while (index < count) {
        downbeats.push(beatTimes[index]);
        confidence.push(0.2 + random() * 0.8);
        index +=
          random() < 0.15 ? 1 : 2 + Math.floor(random() * 6);
        if (random() < 0.1 && index < count) {
          downbeats.push(beatTimes[index]);
          confidence.push(random() * 0.3);
          index += 1;
        }
      }

      const plan = autoBarStarts(beatTimes, downbeats, confidence);
      expect(plan.barStarts.length).toBeGreaterThan(0);
      expect(plan.confidence).toBeGreaterThanOrEqual(0);
      expect(plan.confidence).toBeLessThanOrEqual(1);
      expect(plan.meter).toBeGreaterThanOrEqual(0);
      expect(plan.meter).toBeLessThanOrEqual(12);
      for (let bar = 0; bar < plan.barStarts.length; bar += 1) {
        const start = plan.barStarts[bar];
        expect(Number.isFinite(start)).toBe(true);
        expect(start).toBeGreaterThanOrEqual(beatTimes[0]);
        expect(start).toBeLessThanOrEqual(beatTimes[beatTimes.length - 1]);
        if (bar > 0) {
          expect(start).toBeGreaterThan(plan.barStarts[bar - 1]);
        }
      }
      if (plan.pickupUntil !== null) {
        expect(plan.pickupUntil).toBe(beatTimes[0]);
      }
    }
  });
});
