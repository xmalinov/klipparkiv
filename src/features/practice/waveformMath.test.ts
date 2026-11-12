import { describe, expect, it } from "vitest";
import type { WaveformLevel } from "../../api/practice";
import {
  MIN_VIEW_SECS,
  clampViewRange,
  createSelection,
  edgeTolerance,
  fitView,
  formatClock,
  moveSelection,
  pickLevel,
  regionAt,
  regionEdgeAt,
  resizeRegionEnd,
  resizeRegionStart,
  resizeSelectionEnd,
  resizeSelectionStart,
  selectionEdgeAt,
  timeToX,
  xToTime,
  zoomViewRange,
} from "./waveformMath";

const level = (stepSecs: number): WaveformLevel => ({
  stepSecs,
  peaks: "",
});

describe("waveform math", () => {
  it("formats clock labels with a matching precision", () => {
    expect(formatClock(0)).toBe("0:00.0");
    expect(formatClock(65.25)).toBe("1:05.3");
    expect(formatClock(65.6, 0)).toBe("1:06");
    expect(formatClock(-4)).toBe("0:00.0");
  });

  it("picks the most detailed level for the current zoom", () => {
    const levels = [level(0.1), level(0.01), level(0.05)];
    expect(pickLevel(levels, 10, 1000)).toBe(levels[1]);
    expect(pickLevel(levels, 1, 10)).toBe(levels[0]);
    expect(pickLevel([], 10, 100)).toBeNull();
    expect(pickLevel(levels, 10, 0)).toBeNull();
  });

  it("maps time to pixels and back", () => {
    const view = { start: 10, end: 20 };
    expect(timeToX(15, view, 10, 400)).toBe(200);
    expect(xToTime(200, view, 10, 400)).toBe(15);
    expect(xToTime(1, view, 10, 1)).toBe(20);
    expect(xToTime(0, { start: 0, end: 10 }, 10, 400)).toBe(0);
  });

  it("fits the whole track with a minimum span", () => {
    expect(fitView(120)).toEqual({ start: 0, end: 120 });
    expect(fitView(0)).toEqual({ start: 0, end: MIN_VIEW_SECS });
  });

  it("clamps a view into the track and keeps a minimum span", () => {
    expect(clampViewRange(-5, 4, 100)).toEqual({ start: 0, end: 9 });
    expect(clampViewRange(95, 120, 100)).toEqual({ start: 75, end: 100 });
    expect(clampViewRange(3, 3.2, 100)).toEqual({ start: 3, end: 3.5 });
    expect(clampViewRange(0, 500, 100)).toEqual({ start: 0, end: 100 });
  });

  it("zooms around the anchor and stays inside the track", () => {
    const zoomed = zoomViewRange({ start: 0, end: 100 }, 0.5, 50, 100);
    expect(zoomed).toEqual({ start: 25, end: 75 });

    const clamped = zoomViewRange({ start: 0, end: 20 }, 0.5, 0, 100);
    expect(clamped.start).toBe(0);
    expect(clamped.end).toBe(10);

    const minimum = zoomViewRange({ start: 0, end: 100 }, 0.0001, 50, 100);
    expect(minimum.end - minimum.start).toBeCloseTo(MIN_VIEW_SECS);
  });

  it("derives the grab tolerance from the zoom", () => {
    expect(edgeTolerance(0, 10)).toBe(0.02);
    expect(edgeTolerance(350, 7)).toBeCloseTo(0.14);
  });

  it("finds regions and their edges inclusively", () => {
    const regions = [
      { id: "a", start: 1, end: 2 },
      { id: "b", start: 4, end: 6 },
    ];
    expect(regionAt(1, regions)?.id).toBe("a");
    expect(regionAt(2, regions)?.id).toBe("a");
    expect(regionAt(3, regions)).toBeNull();

    expect(regionEdgeAt(4.05, regions, 0.1)).toEqual({
      region: regions[1],
      edge: "start",
    });
    expect(regionEdgeAt(5.95, regions, 0.1)).toEqual({
      region: regions[1],
      edge: "end",
    });
    expect(regionEdgeAt(5, regions, 0.1)).toBeNull();
  });

  it("detects selection edges and ignores the interior", () => {
    const selection = { start: 2, end: 6 };
    expect(selectionEdgeAt(2.05, selection, 0.1)).toBe("start");
    expect(selectionEdgeAt(5.95, selection, 0.1)).toBe("end");
    expect(selectionEdgeAt(4, selection, 0.1)).toBeNull();
    expect(selectionEdgeAt(4, null, 0.1)).toBeNull();
  });

  it("normalises a drag-created selection", () => {
    expect(createSelection(5, 2, 10)).toEqual({ start: 2, end: 5 });
    expect(createSelection(-3, 15, 10)).toEqual({ start: 0, end: 10 });
  });

  it("moves a fixed-width selection inside the track", () => {
    expect(moveSelection(5, 2, 10)).toEqual({ start: 5, end: 7 });
    expect(moveSelection(-4, 2, 10)).toEqual({ start: 0, end: 2 });
    expect(moveSelection(9, 2, 10)).toEqual({ start: 8, end: 10 });
  });

  it("resizes selection and region edges with a minimum width", () => {
    expect(resizeSelectionStart(6, 6)).toEqual({ start: 5.95, end: 6 });
    expect(resizeSelectionStart(-1, 6)).toEqual({ start: 0, end: 6 });
    expect(resizeSelectionEnd(2, 1, 10)).toEqual({ start: 2, end: 2.05 });
    expect(resizeSelectionEnd(2, 20, 10)).toEqual({ start: 2, end: 10 });
    expect(resizeRegionStart(3.2, 8)).toEqual({ start: 3.2, end: 8 });
    expect(resizeRegionEnd(3, 9.5, 10)).toEqual({ start: 3, end: 9.5 });
  });
});
