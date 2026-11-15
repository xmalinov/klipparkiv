import type { PracticeAnalysis } from "../../api/practice";

export interface BarPlan {
  barStarts: number[];
  /** Time of the first beat when the track starts before bar 1. */
  pickupUntil: number | null;
  /** 0–1: how well the bars fit a single local meter. */
  confidence: number;
  /** Detected or requested beats per bar. */
  meter: number;
}

const MIN_CONFIDENCE = 0.55;
const RELATIVE_CONFIDENCE = 0.6;
const MIN_BAR_BEATS = 2;
const MAX_BAR_BEATS = 16;
const MAX_METER = 12;

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

export function lowerBound(values: number[], target: number): number {
  let low = 0;
  let high = values.length;
  while (low < high) {
    const middle = (low + high) >> 1;
    if (values[middle] < target) low = middle + 1;
    else high = middle;
  }
  return low;
}

export function nearestBeatIndex(values: number[], time: number): number | null {
  if (values.length === 0) return null;
  const index = lowerBound(values, time);
  if (index === 0) return 0;
  if (index >= values.length) return values.length - 1;
  return time - values[index - 1] <= values[index] - time
    ? index - 1
    : index;
}

function median(values: number[]): number | null {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  const middle = sorted.length >> 1;
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

export function phaseVote(
  indices: number[],
  weights: number[],
  meter: number,
): { offset: number; coverage: number } {
  const size = Math.max(1, Math.min(16, Math.round(meter)));
  const totals = new Array<number>(size).fill(0);
  let total = 0;
  for (let index = 0; index < indices.length; index += 1) {
    const weight = weights[index] ?? 1;
    if (!Number.isFinite(weight) || weight <= 0) continue;
    totals[((indices[index] % size) + size) % size] += weight;
    total += weight;
  }
  let best = 0;
  for (let offset = 1; offset < size; offset += 1) {
    if (totals[offset] > totals[best]) best = offset;
  }
  return {
    offset: best,
    coverage: total > 0 ? totals[best] / total : 0,
  };
}

export function fixedBarStarts(
  beats: number[],
  meter: number,
  barOffset: number,
): BarPlan {
  const bar = Math.max(1, Math.min(16, Math.round(meter)));
  const offset = ((Math.round(barOffset) % bar) + bar) % bar;
  const barStarts: number[] = [];
  for (let index = offset; index < beats.length; index += bar) {
    barStarts.push(beats[index]);
  }
  return {
    barStarts,
    pickupUntil:
      barStarts.length > 0 && beats.length > 0 && barStarts[0] > beats[0] + 1e-6
        ? beats[0]
        : null,
    confidence: 1,
    meter: bar,
  };
}

interface Candidate {
  index: number;
  weight: number;
}

export function autoBarStarts(
  beats: number[],
  downbeats: number[],
  confidence: number[] = [],
): BarPlan {
  if (beats.length < 2 || downbeats.length === 0) {
    return { barStarts: [], pickupUntil: null, confidence: 0, meter: 0 };
  }

  const candidates: Candidate[] = [];
  for (let index = 0; index < downbeats.length; index += 1) {
    const beatIndex = nearestBeatIndex(beats, downbeats[index]);
    if (beatIndex === null) continue;
    const raw = confidence[index] ?? 1;
    const weight = Number.isFinite(raw) ? clamp(raw, 0, 1) : 0;
    candidates.push({ index: beatIndex, weight });
  }
  candidates.sort((left, right) => left.index - right.index);

  const deduped: Candidate[] = [];
  for (const candidate of candidates) {
    const previous = deduped[deduped.length - 1];
    if (previous && previous.index === candidate.index) {
      previous.weight = Math.max(previous.weight, candidate.weight);
    } else {
      deduped.push({ ...candidate });
    }
  }
  if (deduped.length === 0) {
    return { barStarts: [], pickupUntil: null, confidence: 0, meter: 0 };
  }

  const maxWeight = deduped.reduce(
    (maximum, candidate) => Math.max(maximum, candidate.weight),
    0,
  );
  const threshold = Math.max(MIN_CONFIDENCE, maxWeight * RELATIVE_CONFIDENCE);
  const strong = deduped.filter((candidate) => candidate.weight >= threshold);
  const pool = strong.length >= 2 ? strong : deduped;

  const rawGaps: number[] = [];
  for (let index = 1; index < pool.length; index += 1) {
    rawGaps.push(pool[index].index - pool[index - 1].index);
  }
  const oneBeatGaps = rawGaps.filter((gap) => gap === 1).length;
  if (rawGaps.length >= 3 && oneBeatGaps * 2 >= rawGaps.length) {
    // A downbeat on almost every beat is a confused model or speech, not a
    // bar grid. Fall back to four beats from the first candidate.
    const start = pool[0].index;
    const barStarts: number[] = [];
    for (let index = start; index < beats.length; index += 4) {
      barStarts.push(beats[index]);
    }
    return {
      barStarts,
      pickupUntil: start > 0 ? beats[0] : null,
      confidence: 0.2,
      meter: 4,
    };
  }

  const gaps: number[] = [];
  for (let index = 1; index < pool.length; index += 1) {
    const gap = pool[index].index - pool[index - 1].index;
    if (gap >= MIN_BAR_BEATS && gap <= MAX_BAR_BEATS) gaps.push(gap);
  }
  const local = clamp(Math.round(median(gaps) ?? 4), 2, MAX_METER);

  const cleaned: Candidate[] = [];
  for (const candidate of pool) {
    const previous = cleaned[cleaned.length - 1];
    if (!previous) {
      cleaned.push(candidate);
      continue;
    }
    const gap = candidate.index - previous.index;
    if (gap < MIN_BAR_BEATS) {
      if (cleaned.length >= 2 && candidate.weight > previous.weight + 0.2) {
        cleaned[cleaned.length - 1] = candidate;
      }
      continue;
    }
    if (gap > local * 1.5) {
      const bars = Math.round(gap / local);
      if (
        bars >= 2 &&
        bars <= 8 &&
        Math.abs(gap - bars * local) <= 0.5
      ) {
        for (let step = 1; step < bars; step += 1) {
          const index = previous.index + step * local;
          if (index < candidate.index) {
            cleaned.push({ index, weight: 1 });
          }
        }
      }
    }
    cleaned.push(candidate);
  }

  if (cleaned.length < 2) {
    const start = cleaned[0]?.index ?? pool[0]?.index ?? 0;
    const barStarts: number[] = [];
    for (let index = start; index < beats.length; index += 4) {
      barStarts.push(beats[index]);
    }
    return {
      barStarts,
      pickupUntil: start > 0 ? beats[0] : null,
      confidence: 0.3,
      meter: 4,
    };
  }

  const bars = cleaned;
  const barStarts = bars.map((candidate) => beats[candidate.index]);
  const vote = phaseVote(
    bars.map((candidate) => candidate.index),
    bars.map((candidate) => candidate.weight),
    local,
  );
  return {
    barStarts,
    pickupUntil: barStarts[0] > beats[0] + 1e-6 ? beats[0] : null,
    confidence: vote.coverage,
    meter: local,
  };
}

/** A playable grid derived from the analysis, a manual BPM or both. */
export interface PracticeGrid {
  bpm: number;
  beats: number[];
  downbeats: number[];
  barStarts: number[];
  beatsPerBar: number;
  barOffset: number;
  pickupUntil: number | null;
  confidence: number;
}

export function nearestValue(values: number[], time: number): number | null {
  if (values.length === 0) return null;
  let low = 0;
  let high = values.length;
  while (low < high) {
    const middle = (low + high) >> 1;
    if (values[middle] < time) low = middle + 1;
    else high = middle;
  }
  if (low === 0) return values[0];
  if (low >= values.length) return values[values.length - 1];
  return time - values[low - 1] <= values[low] - time
    ? values[low - 1]
    : values[low];
}

/** `2` subdivides every beat, `0.5` keeps every other one. */
export function scaleBeats(beats: number[], scale: number): number[] {
  if (scale === 2) {
    const scaled: number[] = [];
    for (let index = 0; index < beats.length; index += 1) {
      scaled.push(beats[index]);
      if (index + 1 < beats.length) {
        scaled.push((beats[index] + beats[index + 1]) / 2);
      }
    }
    return scaled;
  }
  if (scale === 0.5) {
    return beats.filter((_, index) => index % 2 === 0);
  }
  return beats;
}

export function buildBarStarts(
  beats: number[],
  beatsPerBar: number,
  barOffset: number,
): number[] {
  return fixedBarStarts(beats, beatsPerBar, barOffset).barStarts;
}

export function buildGrid(
  analysis: PracticeAnalysis | null,
  manualBpm: number | null,
  duration: number,
  bpmScale: number,
  beatsPerBar: number,
  barOffset: number,
  manualGrid = false,
): PracticeGrid | null {
  let sourceBeats: number[] = [];
  let sourceBpm = 0;
  let autoPlan: BarPlan | null = null;

  if (manualBpm && manualBpm > 0 && duration > 0) {
    const interval = 60 / manualBpm;
    const count = Math.ceil(duration / interval) + 1;
    sourceBeats = Array.from({ length: count }, (_, index) => index * interval);
    sourceBpm = manualBpm;
  } else if (analysis && analysis.beats.length >= 2 && analysis.bpm > 0) {
    sourceBeats = analysis.beats;
    sourceBpm = analysis.bpm;
    if (!manualGrid && bpmScale === 1) {
      autoPlan = autoBarStarts(
        analysis.beats,
        analysis.downbeats,
        analysis.downbeatConfidence ?? [],
      );
    }
  }
  if (sourceBeats.length < 2) return null;

  const beats = scaleBeats(sourceBeats, bpmScale);
  if (beats.length < 2) return null;

  const effectiveMeter =
    manualBpm || manualGrid
      ? beatsPerBar
      : (analysis?.beatsPerBar || beatsPerBar);
  const effectiveOffset =
    manualBpm || manualGrid ? barOffset : (analysis?.barOffset ?? barOffset);
  const scaledOffset = Math.round(effectiveOffset * bpmScale);

  let plan = autoPlan;
  if (!plan || plan.barStarts.length < 2) {
    plan = fixedBarStarts(beats, effectiveMeter, scaledOffset);
  }
  if (plan.barStarts.length === 0) return null;

  const outputMeter = plan.meter || effectiveMeter;
  const outputOffset =
    outputMeter > 0
      ? ((effectiveOffset % outputMeter) + outputMeter) % outputMeter
      : 0;

  return {
    bpm: sourceBpm * bpmScale,
    beats,
    downbeats: plan.barStarts,
    barStarts: plan.barStarts,
    beatsPerBar: outputMeter,
    barOffset: outputOffset,
    pickupUntil: plan.pickupUntil,
    confidence: plan.confidence,
  };
}
