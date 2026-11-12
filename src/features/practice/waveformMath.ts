import type { WaveformLevel } from "../../api/practice";

export const MIN_VIEW_SECS = 0.5;

export interface ViewRange {
  start: number;
  end: number;
}

export interface TimeSelection {
  start: number;
  end: number;
}

export interface RegionLike {
  id: string;
  start: number;
  end: number;
}

/** Formats a timeline label; `precision` also fixes the seconds width. */
export function formatClock(seconds: number, precision = 1): string {
  const safe = Math.max(0, seconds);
  const minutes = Math.floor(safe / 60);
  const rest = safe - minutes * 60;
  return `${minutes}:${rest.toFixed(precision).padStart(precision > 0 ? 3 + precision : 2, "0")}`;
}

/**
 * Picks the most detailed peak level that still has at least two buckets per
 * pixel at the current zoom. Levels are sorted defensively — the backend does
 * not guarantee ordering.
 */
export function pickLevel(
  levels: WaveformLevel[],
  viewSeconds: number,
  width: number,
): WaveformLevel | null {
  if (levels.length === 0 || width <= 0) return null;
  const target = viewSeconds / Math.max(1, width);
  const sorted = [...levels].sort(
    (left, right) => left.stepSecs - right.stepSecs,
  );
  let chosen = sorted[0];
  for (const level of sorted) {
    if (level.stepSecs <= target * 2) chosen = level;
  }
  return chosen;
}

export function timeToX(
  time: number,
  view: ViewRange,
  viewDuration: number,
  width: number,
): number {
  return ((time - view.start) / viewDuration) * width;
}

export function xToTime(
  x: number,
  view: ViewRange,
  viewDuration: number,
  width: number,
): number {
  return view.start + (x / Math.max(1, width)) * viewDuration;
}

export function fitView(duration: number): ViewRange {
  return { start: 0, end: Math.max(MIN_VIEW_SECS, duration) };
}

/** Clamps a requested window into `[0, duration]` with a minimum span. */
export function clampViewRange(
  start: number,
  end: number,
  duration: number,
): ViewRange {
  const total = Math.max(MIN_VIEW_SECS, duration);
  const span = Math.min(total, Math.max(MIN_VIEW_SECS, end - start));
  let nextStart = Math.max(0, Math.min(start, total - span));
  if (nextStart < 0) nextStart = 0;
  return { start: nextStart, end: nextStart + span };
}

/** Zooms around a time anchor, keeping the anchor under the same pixel. */
export function zoomViewRange(
  view: ViewRange,
  factor: number,
  center: number,
  duration: number,
): ViewRange {
  const total = Math.max(MIN_VIEW_SECS, duration);
  const currentSpan = Math.max(MIN_VIEW_SECS, view.end - view.start);
  const nextSpan = Math.min(total, Math.max(MIN_VIEW_SECS, currentSpan * factor));
  const ratio = (center - view.start) / currentSpan;
  let start = center - ratio * nextSpan;
  start = Math.max(0, Math.min(start, total - nextSpan));
  return { start, end: start + nextSpan };
}

/** Seconds covered by the 7px grab zone around a draggable edge. */
export function edgeTolerance(width: number, viewDuration: number): number {
  return width > 0 ? (7 / width) * viewDuration : 0.02;
}

export function regionAt<T extends RegionLike>(
  time: number,
  regions: T[],
): T | null {
  return (
    regions.find((region) => time >= region.start && time <= region.end) ?? null
  );
}

export function selectionEdgeAt(
  time: number,
  selection: TimeSelection | null,
  tolerance: number,
): "start" | "end" | null {
  if (!selection) return null;
  if (Math.abs(time - selection.start) <= tolerance) return "start";
  if (Math.abs(time - selection.end) <= tolerance) return "end";
  return null;
}

export function regionEdgeAt<T extends RegionLike>(
  time: number,
  regions: T[],
  tolerance: number,
): { region: T; edge: "start" | "end" } | null {
  for (const region of regions) {
    if (Math.abs(time - region.start) <= tolerance) {
      return { region, edge: "start" };
    }
    if (Math.abs(time - region.end) <= tolerance) {
      return { region, edge: "end" };
    }
  }
  return null;
}

export function createSelection(
  anchor: number,
  current: number,
  total: number,
): TimeSelection {
  return {
    start: Math.max(0, Math.min(anchor, current)),
    end: Math.min(total, Math.max(anchor, current)),
  };
}

/** Moves a fixed-width selection, clamping the start to the track bounds. */
export function moveSelection(
  start: number,
  span: number,
  total: number,
): TimeSelection {
  const clamped = Math.max(0, Math.min(start, total - span));
  return { start: clamped, end: clamped + span };
}

export function resizeSelectionStart(
  snapped: number,
  fixedEnd: number,
): TimeSelection {
  return {
    start: Math.max(0, Math.min(snapped, fixedEnd - 0.05)),
    end: fixedEnd,
  };
}

export function resizeSelectionEnd(
  fixedStart: number,
  snapped: number,
  total: number,
): TimeSelection {
  return {
    start: fixedStart,
    end: Math.min(total, Math.max(snapped, fixedStart + 0.05)),
  };
}

export const resizeRegionStart = resizeSelectionStart;
export const resizeRegionEnd = resizeSelectionEnd;
