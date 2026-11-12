import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { flushSync } from "react-dom";
import type { LibraryItem } from "../../api/library";
import {
  DEFAULT_PRACTICE_STATE,
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
  practiceRecent,
  practiceRecentHide,
  practiceSeek,
  practiceSetGrid,
  practiceSetLoop,
  practiceSetMetronome,
  practiceSetPitch,
  practiceSetSpeed,
  practiceSetVolume,
  practiceStateSave,
  practiceWaveform,
  type GridPayload,
  type PracticeAnalysis,
  type PracticeRegion,
  type PracticeSnapshot,
  type PracticeState,
  type RegionKind,
  type SnapMode,
  type Waveform,
} from "../../api/practice";
import {
  autoBarStarts,
  fixedBarStarts,
  nearestBeatIndex,
  type BarPlan,
} from "./gridMath";

export const REGION_COLORS: Record<RegionKind, string> = {
  intro: "#8b9bb4",
  verse: "#4f8cff",
  chorus: "#ff8a4f",
  solo: "#e05c8a",
  bridge: "#9b7bff",
  outro: "#5fb98a",
  custom: "#c9a227",
};

export const REGION_KINDS: RegionKind[] = [
  "intro",
  "verse",
  "chorus",
  "solo",
  "bridge",
  "outro",
  "custom",
];

export interface Selection {
  start: number;
  end: number;
}

export type AnalysisStatus = "idle" | "loading" | "ready" | "error";

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

interface PracticeContextValue {
  track: LibraryItem | null;
  state: PracticeState;
  snapshot: PracticeSnapshot;
  waveform: Waveform | null;
  waveformLoading: boolean;
  opening: boolean;
  error: string | null;
  analysis: PracticeAnalysis | null;
  analysisStatus: AnalysisStatus;
  analysisProgress: number;
  analysisError: string | null;
  grid: PracticeGrid | null;
  recent: LibraryItem[];
  selection: Selection | null;
  duration: number;
  openTrack: (item: LibraryItem) => Promise<void>;
  hideRecent: (path: string) => Promise<void>;
  close: () => Promise<void>;
  playPause: () => Promise<void>;
  seek: (position: number) => Promise<void>;
  setSpeed: (speed: number) => Promise<void>;
  setPitch: (semitones: number, cents: number) => Promise<void>;
  setVolume: (volume: number) => Promise<void>;
  setSelection: (selection: Selection | null) => void;
  addRegion: () => Promise<void>;
  saveSelectionAsRegion: (
    name: string,
    kind: RegionKind,
    color: string,
  ) => Promise<void>;
  updateRegion: (id: string, patch: Partial<PracticeRegion>) => Promise<void>;
  deleteRegion: (id: string) => Promise<void>;
  setLoopEnabled: (enabled: boolean) => Promise<void>;
  loopRegion: (id: string) => Promise<void>;
  setLoopBoundary: (which: "start" | "end") => Promise<void>;
  clearLoop: () => Promise<void>;
  nudgeLoop: (which: "start" | "end", delta: number) => Promise<void>;
  selectRegion: (id: string) => Promise<void>;
  selectRegionByIndex: (index: number) => Promise<void>;
  setPreRoll: (enabled: boolean) => Promise<void>;
  snapTime: (time: number, mode?: SnapMode) => number;
  loopBars: (bars: number) => Promise<void>;
  setBpmScale: (scale: number) => Promise<void>;
  setBeatsPerBar: (beats: number) => Promise<void>;
  setBarOffset: (offset: number) => Promise<void>;
  setAutoBars: () => Promise<void>;
  setBarOneAtPlayhead: () => Promise<void>;
  setSnapMode: (mode: SnapMode) => Promise<void>;
  setMetronome: (enabled: boolean, volume?: number) => Promise<void>;
  setCountIn: (enabled: boolean) => Promise<void>;
  setManualBpm: (bpm: number | null) => Promise<void>;
  setGridEnabled: (enabled: boolean) => Promise<void>;
  startAnalysis: () => Promise<void>;
}

const EMPTY_SNAPSHOT: PracticeSnapshot = {
  positionSecs: 0,
  durationSecs: null,
  playing: false,
  ended: false,
  underruns: 0,
};

const PracticeContext = createContext<PracticeContextValue | null>(null);

function newId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `r-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function sortRegions(regions: PracticeRegion[]): PracticeRegion[] {
  return [...regions].sort((left, right) => left.start - right.start);
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

export function PracticeProvider({ children }: { children: ReactNode }) {
  const [track, setTrack] = useState<LibraryItem | null>(null);
  const [state, setState] = useState<PracticeState>(DEFAULT_PRACTICE_STATE);
  const [snapshot, setSnapshot] = useState<PracticeSnapshot>(EMPTY_SNAPSHOT);
  const [waveform, setWaveform] = useState<Waveform | null>(null);
  const [waveformLoading, setWaveformLoading] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recent, setRecent] = useState<LibraryItem[]>([]);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [analysis, setAnalysis] = useState<PracticeAnalysis | null>(null);
  const [analysisStatus, setAnalysisStatus] = useState<AnalysisStatus>("idle");
  const [analysisProgress, setAnalysisProgress] = useState(0);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const positionRef = useRef(0);
  const selectionRef = useRef<Selection | null>(null);
  const stateRef = useRef(state);
  const trackRef = useRef<LibraryItem | null>(null);
  const analysisRef = useRef<PracticeAnalysis | null>(null);
  const gridRef = useRef<PracticeGrid | null>(null);
  const hydratedRef = useRef(false);
  const saveTimer = useRef<number | undefined>(undefined);

  stateRef.current = state;
  trackRef.current = track;
  analysisRef.current = analysis;

  const duration =
    snapshot.durationSecs ?? track?.durationSecs ?? 0;

  const grid = useMemo(
    () =>
      state.gridEnabled || state.manualBpm
        ? buildGrid(
            state.gridEnabled ? analysis : null,
            state.manualBpm,
            duration,
            state.bpmScale,
            state.beatsPerBar,
            state.barOffset,
            state.gridCustomized,
          )
        : null,
    [
      analysis,
      duration,
      state.barOffset,
      state.beatsPerBar,
      state.bpmScale,
      state.gridCustomized,
      state.gridEnabled,
      state.manualBpm,
    ],
  );
  gridRef.current = grid;
  const durationRef = useRef(0);
  durationRef.current = duration;

  useEffect(() => {
    if (!state.gridEnabled || !grid) {
      practiceSetGrid(null).catch(() => {});
      return;
    }
    const payload: GridPayload = {
      bpm: grid.bpm,
      beatsPerBar: grid.beatsPerBar,
      barOffset: grid.barOffset,
      beats: grid.beats,
      downbeats: grid.downbeats,
    };
    practiceSetGrid(payload).catch((cause) => setError(String(cause)));
  }, [grid, state.gridEnabled]);

  useEffect(() => {
    practiceSetMetronome(
      state.metronomeEnabled,
      state.metronomeVolume,
    ).catch((cause) => setError(String(cause)));
  }, [state.metronomeEnabled, state.metronomeVolume]);

  const updateSelection = useCallback((next: Selection | null) => {
    selectionRef.current = next;
    setSelection(next);
  }, []);

  const refreshRecent = useCallback(async () => {
    try {
      const items = await practiceRecent(20);
      setRecent(Array.isArray(items) ? items : []);
    } catch {
      setRecent([]);
    }
  }, []);

  const hideRecent = useCallback(async (path: string) => {
    try {
      await practiceRecentHide(path);
      setRecent((items) => items.filter((item) => item.path !== path));
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    refreshRecent();
  }, [refreshRecent]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onPracticePosition((next) => {
      positionRef.current = next.positionSecs;
      setSnapshot((current) =>
        current.positionSecs === next.positionSecs &&
        current.durationSecs === next.durationSecs &&
        current.playing === next.playing &&
        current.ended === next.ended &&
        current.underruns === next.underruns
          ? current
          : next,
      );
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onPracticeError((next) => setError(next.message)).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const applyAnalysis = useCallback((next: PracticeAnalysis) => {
    analysisRef.current = next;
    setAnalysis(next);
    setAnalysisProgress(1);
    setAnalysisError(null);
    setAnalysisStatus("ready");
    if (!stateRef.current.gridCustomized) {
      setState((current) => ({
        ...current,
        beatsPerBar: next.beatsPerBar || current.beatsPerBar,
        barOffset: next.barOffset ?? 0,
      }));
    }
  }, []);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenDone: (() => void) | undefined;
    let unlistenError: (() => void) | undefined;

    onPracticeAnalysisProgress((next) => {
      if (trackRef.current?.path !== next.path) return;
      setAnalysisProgress(next.progress);
      if (next.stage !== "done") setAnalysisStatus("loading");
    }).then((fn) => {
      unlistenProgress = fn;
    });
    onPracticeAnalysisDone((next) => {
      if (trackRef.current?.path !== next.path) return;
      applyAnalysis(next);
    }).then((fn) => {
      unlistenDone = fn;
    });
    onPracticeAnalysisError((next) => {
      if (trackRef.current?.path !== next.path) return;
      setAnalysisStatus("error");
      setAnalysisError(next.message);
    }).then((fn) => {
      unlistenError = fn;
    });

    return () => {
      unlistenProgress?.();
      unlistenDone?.();
      unlistenError?.();
    };
  }, [applyAnalysis]);

  const startAnalysis = useCallback(async () => {
    const currentTrack = trackRef.current;
    if (!currentTrack || !stateRef.current.gridEnabled) return;
    setAnalysisStatus("loading");
    setAnalysisProgress(0);
    setAnalysisError(null);
    try {
      const cached = await practiceAnalysisGet(currentTrack.path);
      if (cached) {
        applyAnalysis(cached);
        return;
      }
      await practiceAnalysisStart(currentTrack.path);
    } catch (cause) {
      setAnalysisStatus("error");
      setAnalysisError(String(cause));
    }
  }, [applyAnalysis]);

  const saveNow = useCallback(async () => {
    const currentTrack = trackRef.current;
    if (!currentTrack || !hydratedRef.current) return;
    const next = { ...stateRef.current, positionSecs: positionRef.current };
    try {
      await practiceStateSave(currentTrack.path, next);
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  useEffect(() => {
    if (!track || !hydratedRef.current) return;
    window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      saveNow();
    }, 500);
    return () => window.clearTimeout(saveTimer.current);
  }, [state, track, saveNow]);

  useEffect(() => {
    if (!snapshot.playing) return;
    const timer = window.setInterval(() => {
      saveNow();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [snapshot.playing, saveNow]);

  const openTrack = useCallback(
    async (item: LibraryItem) => {
      if (item.kind !== "audio") return;
      flushSync(() => setOpening(true));
      if (trackRef.current) {
        await practicePause().catch(() => {});
        await saveNow();
      }
      hydratedRef.current = false;
      setError(null);
      updateSelection(null);
      setWaveform(null);
      setAnalysis(null);
      analysisRef.current = null;
      setAnalysisStatus("idle");
      setAnalysisProgress(0);
      setAnalysisError(null);
      try {
        const opened = await practiceOpen(item.path);
        const nextState = opened.state;
        setTrack(item);
        trackRef.current = item;
        setState(nextState);
        stateRef.current = nextState;
        setSnapshot({
          positionSecs: nextState.positionSecs,
          durationSecs: opened.durationSecs ?? item.durationSecs,
          playing: false,
          ended: false,
          underruns: 0,
        });
        positionRef.current = nextState.positionSecs;
        hydratedRef.current = true;
        practiceStateSave(item.path, nextState).catch(() => {});
        if (nextState.gridEnabled) {
          startAnalysis();
        }
        setWaveformLoading(true);
        practiceWaveform(item.path)
          .then((value) => setWaveform(value))
          .catch((cause: unknown) => setError(String(cause)))
          .finally(() => setWaveformLoading(false));
        refreshRecent();
      } catch (cause) {
        setError(String(cause));
      } finally {
        setOpening(false);
      }
    },
    [refreshRecent, saveNow, startAnalysis, updateSelection],
  );

  const close = useCallback(async () => {
    await saveNow();
    try {
      await practiceClose();
    } catch (cause) {
      setError(String(cause));
    }
    hydratedRef.current = false;
    setTrack(null);
    trackRef.current = null;
    setState(DEFAULT_PRACTICE_STATE);
    setSnapshot(EMPTY_SNAPSHOT);
    setWaveform(null);
    setOpening(false);
    setAnalysis(null);
    analysisRef.current = null;
    setAnalysisStatus("idle");
    setAnalysisProgress(0);
    setAnalysisError(null);
    updateSelection(null);
    positionRef.current = 0;
    practiceAnalysisCancel().catch(() => {});
    refreshRecent();
  }, [refreshRecent, saveNow]);

  const playPause = useCallback(async () => {
    if (!trackRef.current) return;
    try {
      if (snapshot.playing) {
        await practicePause();
        setSnapshot((current) => ({ ...current, playing: false }));
        await saveNow();
      } else {
        await practicePlay();
        setSnapshot((current) => ({ ...current, playing: true }));
      }
    } catch (cause) {
      setError(String(cause));
    }
  }, [saveNow, snapshot.playing]);

  const seek = useCallback(async (position: number) => {
    try {
      const current = stateRef.current;
      const active =
        current.regions.find(
          (region) => region.id === current.activeRegionId,
        ) ?? null;
      const outsideLoop =
        current.loopEnabled &&
        active !== null &&
        (position < active.start || position > active.end);
      if (outsideLoop) {
        // The audio engine cannot play outside an active loop, so leaving the
        // loop is the only way to put the playhead where the user clicked.
        await practiceSetLoop(
          false,
          null,
          null,
          current.preRoll,
          current.preRollSecs,
        );
        setState((state) => ({ ...state, loopEnabled: false }));
      }
      await practiceSeek(position);
      positionRef.current = position;
      setSnapshot((current) => ({ ...current, positionSecs: position }));
      setState((current) => ({ ...current, positionSecs: position }));
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  const setSpeed = useCallback(async (speed: number) => {
    try {
      await practiceSetSpeed(speed);
      setState((current) => ({ ...current, speed }));
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  const setPitch = useCallback(async (semitones: number, cents: number) => {
    try {
      await practiceSetPitch(semitones, cents);
      setState((current) => ({ ...current, pitchSemitones: semitones, cents }));
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  const setVolume = useCallback(async (volume: number) => {
    const next = Math.min(1, Math.max(0, volume));
    try {
      await practiceSetVolume(next);
      setState((current) => ({ ...current, volume: next }));
    } catch (cause) {
      setError(String(cause));
    }
  }, []);

  const setBpmScale = useCallback(async (scale: number) => {
    const next = scale <= 0.75 ? 0.5 : scale >= 1.5 ? 2 : 1;
    setState((current) => ({ ...current, bpmScale: next }));
  }, []);

  const setBeatsPerBar = useCallback(async (beats: number) => {
    const next = Math.max(1, Math.min(16, Math.round(beats)));
    setState((current) => ({
      ...current,
      beatsPerBar: next,
      barOffset: current.barOffset % next,
      gridCustomized: true,
    }));
  }, []);

  const setBarOffset = useCallback(async (offset: number) => {
    setState((current) => ({
      ...current,
      barOffset:
        ((Math.round(offset) % current.beatsPerBar) + current.beatsPerBar) %
        current.beatsPerBar,
      gridCustomized: true,
    }));
  }, []);

  const setAutoBars = useCallback(async () => {
    const next = analysisRef.current;
    setState((current) => ({
      ...current,
      gridCustomized: false,
      manualBpm: null,
      beatsPerBar: next?.beatsPerBar || current.beatsPerBar,
      barOffset: next?.barOffset ?? 0,
    }));
  }, []);

  const setBarOneAtPlayhead = useCallback(async () => {
    const beats = gridRef.current?.beats ?? [];
    const index = nearestBeatIndex(beats, positionRef.current);
    if (index === null) return;
    const meter = stateRef.current.beatsPerBar;
    setState((current) => ({
      ...current,
      barOffset: index % meter,
      gridCustomized: true,
    }));
  }, []);

  const setSnapMode = useCallback(async (mode: SnapMode) => {
    setState((current) => ({ ...current, snapMode: mode }));
  }, []);

  const setMetronome = useCallback(
    async (enabled: boolean, volume?: number) => {
      const nextVolume = Math.min(
        1,
        Math.max(0, volume ?? stateRef.current.metronomeVolume),
      );
      try {
        await practiceSetMetronome(enabled, nextVolume);
        setState((current) => ({
          ...current,
          metronomeEnabled: enabled,
          metronomeVolume: nextVolume,
        }));
      } catch (cause) {
        setError(String(cause));
      }
    },
    [],
  );

  const setCountIn = useCallback(async (enabled: boolean) => {
    setState((current) => ({ ...current, countIn: enabled }));
    if (enabled && !stateRef.current.metronomeEnabled) {
      await setMetronome(true, stateRef.current.metronomeVolume);
    }
  }, [setMetronome]);

  const setManualBpm = useCallback(async (bpm: number | null) => {
    setState((current) => ({
      ...current,
      manualBpm:
        bpm === null || !Number.isFinite(bpm) || bpm <= 0
          ? null
          : Math.min(400, Math.max(20, bpm)),
      gridCustomized: true,
    }));
  }, []);

  const setGridEnabled = useCallback(async (enabled: boolean) => {
    setState((current) => ({ ...current, gridEnabled: enabled }));
    if (!enabled) {
      setAnalysis(null);
      analysisRef.current = null;
      setAnalysisStatus("idle");
      setAnalysisProgress(0);
      setAnalysisError(null);
      practiceAnalysisCancel().catch(() => {});
    }
  }, []);

  const applyLoop = useCallback(
    async (enabled: boolean, region: PracticeRegion | null, preRoll: boolean) => {
      try {
        const grid = gridRef.current;
        const wantsPreRoll = preRoll || stateRef.current.countIn;
        const preRollSecs = wantsPreRoll
          ? grid && stateRef.current.countIn && grid.bpm > 0
            ? (grid.beatsPerBar * 60) / grid.bpm
            : 2
          : 0;
        setState((current) =>
          current.preRollSecs === preRollSecs
            ? current
            : { ...current, preRollSecs },
        );
        if (enabled && region) {
          await practiceSetLoop(
            true,
            region.start,
            region.end,
            wantsPreRoll,
            preRollSecs,
          );
        } else {
          await practiceSetLoop(false, null, null, wantsPreRoll, preRollSecs);
        }
      } catch (cause) {
        setError(String(cause));
      }
    },
    [],
  );

  const snapTime = useCallback((time: number, mode?: SnapMode) => {
    const grid = gridRef.current;
    const current = stateRef.current;
    const snapMode = mode ?? current.snapMode;
    const maximum = durationRef.current > 0 ? durationRef.current : time;
    const clamped = Math.max(0, Math.min(maximum, time));
    if (!grid || snapMode === "free") return clamped;
    const values = snapMode === "beat" ? grid.beats : grid.barStarts;
    const snapped = nearestValue(values, clamped);
    if (snapped === null) return clamped;
    return Math.max(0, Math.min(maximum, snapped));
  }, []);
  const setLoopEnabled = useCallback(
    async (enabled: boolean) => {
      const active =
        state.regions.find((region) => region.id === state.activeRegionId) ??
        state.regions[0] ??
        null;
      const nextActive = active?.id ?? null;
      setState((current) => ({
        ...current,
        loopEnabled: enabled && Boolean(active),
        activeRegionId: nextActive,
      }));
      await applyLoop(enabled && Boolean(active), active, state.preRoll);
    },
    [applyLoop, state.activeRegionId, state.preRoll, state.regions],
  );

  const selectRegion = useCallback(
    async (id: string) => {
      const region = state.regions.find((entry) => entry.id === id) ?? null;
      const alreadyActive = state.activeRegionId === id;
      setState((current) => ({ ...current, activeRegionId: id }));
      if (!alreadyActive && state.loopEnabled && region) {
        await applyLoop(true, region, state.preRoll);
      }
    },
    [applyLoop, state.activeRegionId, state.loopEnabled, state.preRoll, state.regions],
  );

  const loopRegion = useCallback(
    async (id: string) => {
      const region = stateRef.current.regions.find((entry) => entry.id === id);
      if (!region) return;
      setState((current) => ({
        ...current,
        activeRegionId: id,
        loopEnabled: true,
      }));
      await applyLoop(true, region, stateRef.current.preRoll);
    },
    [applyLoop],
  );

  const setLoopBoundary = useCallback(
    async (which: "start" | "end") => {
      const position = snapTime(positionRef.current);
      const base = selectionRef.current ?? { start: position, end: position };
      const baseStart = snapTime(base.start);
      const baseEnd = snapTime(base.end);
      const next =
        which === "start"
          ? { start: position, end: Math.max(position, baseEnd) }
          : { start: Math.min(baseStart, position), end: position };
      updateSelection(next);
      if (next.end - next.start < 0.1) return;
      const region: PracticeRegion = {
        id: "ab",
        name: "A-B",
        kind: "custom",
        color: REGION_COLORS.custom,
        start: next.start,
        end: next.end,
      };
      setState((current) => ({
        ...current,
        regions: sortRegions([
          ...current.regions.filter((entry) => entry.id !== "ab"),
          region,
        ]),
        activeRegionId: "ab",
        loopEnabled: true,
      }));
      await applyLoop(true, region, stateRef.current.preRoll);
    },
    [applyLoop, snapTime, updateSelection],
  );

  const loopBars = useCallback(
    async (bars: number) => {
      const grid = gridRef.current;
      if (!grid || grid.barStarts.length === 0) {
        setError("Beat grid is not ready");
        return;
      }
      const requested = Math.max(1, Math.round(bars));
      const starts = grid.barStarts;
      const position = positionRef.current;
      let index = 0;
      for (let candidate = 0; candidate < starts.length; candidate += 1) {
        if (starts[candidate] <= position + 0.001) index = candidate;
        else break;
      }
      let start = starts[index];
      let end =
        index + requested < starts.length
          ? starts[index + requested]
          : durationRef.current;
      if (!(end > start)) {
        const fallback = Math.max(0, starts.length - requested);
        start = starts[fallback];
        end = durationRef.current;
      }
      if (!(end > start)) return;
      const region: PracticeRegion = {
        id: "ab",
        name: `${requested} bar${requested === 1 ? "" : "s"}`,
        kind: "custom",
        color: REGION_COLORS.custom,
        start,
        end,
      };
      setState((current) => ({
        ...current,
        regions: sortRegions([
          ...current.regions.filter((entry) => entry.id !== "ab"),
          region,
        ]),
        activeRegionId: "ab",
        loopEnabled: true,
      }));
      updateSelection({ start, end });
      await applyLoop(true, region, stateRef.current.countIn);
    },
    [applyLoop, updateSelection],
  );

  const clearLoop = useCallback(async () => {    setState((current) => ({
      ...current,
      regions: current.regions.filter((region) => region.id !== "ab"),
      activeRegionId:
        current.activeRegionId === "ab" ? null : current.activeRegionId,
      loopEnabled: current.activeRegionId === "ab" ? false : current.loopEnabled,
    }));
    updateSelection(null);
    await applyLoop(false, null, stateRef.current.preRoll);
  }, [applyLoop]);

  const selectRegionByIndex = useCallback(
    async (index: number) => {
      const region = sortRegions(stateRef.current.regions)[index];
      if (region) await selectRegion(region.id);
    },
    [selectRegion],
  );

  const saveSelectionAsRegion = useCallback(
    async (name: string, kind: RegionKind, color: string) => {
      const currentTrack = trackRef.current;
      if (!currentTrack) return;
      const duration =
        snapshot.durationSecs ?? currentTrack.durationSecs ?? Number.POSITIVE_INFINITY;
      const currentSelection = selectionRef.current;
      const start = currentSelection?.start ?? Math.max(0, positionRef.current);
      const end = Math.min(
        currentSelection?.end ?? Math.min(start + 4, duration),
        Number.isFinite(duration) ? duration : start + 4,
      );
      if (!(end > start)) return;
      const existing = stateRef.current.regions.find(
        (region) => region.id === "ab",
      );
      const region: PracticeRegion = {
        id: existing?.id ?? newId(),
        name: name.trim() || currentTrack.title,
        kind,
        color,
        start,
        end,
      };
      setState((current) => ({
        ...current,
        regions: existing
          ? sortRegions(
              current.regions.map((entry) =>
                entry.id === "ab" ? region : entry,
              ),
            )
          : sortRegions([...current.regions, region]),
        activeRegionId: region.id,
        loopEnabled: true,
      }));
      updateSelection(null);
      await applyLoop(true, region, state.preRoll);
    },
    [applyLoop, snapshot.durationSecs, state.preRoll],
  );

  const addRegion = useCallback(async () => {
    const index = state.regions.length + 1;
    await saveSelectionAsRegion(
      `Region ${index}`,
      "custom",
      REGION_COLORS.custom,
    );
  }, [saveSelectionAsRegion, state.regions.length]);

  const updateRegion = useCallback(
    async (id: string, patch: Partial<PracticeRegion>) => {
      const current = stateRef.current.regions.find((region) => region.id === id);
      if (!current) return;
      const next = { ...current, ...patch };
      if (!(next.end > next.start) || next.start < 0) return;
      setState((value) => ({
        ...value,
        regions: sortRegions(
          value.regions.map((region) => (region.id === id ? next : region)),
        ),
      }));
      if (stateRef.current.loopEnabled && stateRef.current.activeRegionId === id) {
        await applyLoop(true, next, stateRef.current.preRoll);
      }
    },
    [applyLoop, updateSelection],
  );

  const nudgeLoop = useCallback(
    async (which: "start" | "end", delta: number) => {
      const active = stateRef.current.regions.find(
        (region) => region.id === stateRef.current.activeRegionId,
      );
      if (!active) return;
      const next =
        which === "start"
          ? { ...active, start: Math.max(0, active.start + delta) }
          : { ...active, end: active.end + delta };
      if (next.end <= next.start) return;
      await updateRegion(active.id, next);
    },
    [updateRegion],
  );

  const deleteRegion = useCallback(
    async (id: string) => {
      const wasActive = stateRef.current.activeRegionId === id;
      const wasLooping = stateRef.current.loopEnabled;
      setState((current) => ({
        ...current,
        regions: current.regions.filter((region) => region.id !== id),
        activeRegionId: wasActive ? null : current.activeRegionId,
        loopEnabled: wasActive ? false : current.loopEnabled,
      }));
      if (wasActive && wasLooping) {
        await applyLoop(false, null, stateRef.current.preRoll);
      }
    },
    [applyLoop, updateSelection],
  );

  const setPreRoll = useCallback(
    async (enabled: boolean) => {
      setState((current) => ({ ...current, preRoll: enabled }));
      const active = stateRef.current.regions.find(
        (region) => region.id === stateRef.current.activeRegionId,
      );
      if (stateRef.current.loopEnabled && active) {
        await applyLoop(true, active, enabled);
      }
    },
    [applyLoop, updateSelection],
  );

  const value = useMemo<PracticeContextValue>(
    () => ({
      track,
      state,
      snapshot,
      waveform,
      waveformLoading,
      opening,
      error,
      recent,
      selection,
      duration,
      analysis,
      analysisStatus,
      analysisProgress,
      analysisError,
      grid,
      openTrack,
      hideRecent,
      close,
      playPause,
      seek,
      setSpeed,
      setPitch,
      setVolume,
      setSelection: updateSelection,
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
      setPreRoll,
      snapTime,
      loopBars,
      setBpmScale,
      setBeatsPerBar,
      setBarOffset,
      setAutoBars,
      setBarOneAtPlayhead,
      setSnapMode,
      setMetronome,
      setCountIn,
      setManualBpm,
      setGridEnabled,
      startAnalysis,
    }),
    [
      addRegion,
      analysis,
      analysisError,
      analysisProgress,
      analysisStatus,
      close,
      deleteRegion,
      duration,
      error,
      grid,
      openTrack,
      hideRecent,
      loopBars,
      playPause,
      recent,
      saveSelectionAsRegion,
      seek,
      selectRegion,
      selectRegionByIndex,
      selection,
      setBarOffset,
      setAutoBars,
      setBarOneAtPlayhead,
      setBeatsPerBar,
      setBpmScale,
      setCountIn,
      setLoopBoundary,
      setLoopEnabled,
      loopRegion,
      clearLoop,
      setManualBpm,
      setGridEnabled,
      setMetronome,
      nudgeLoop,
      setPitch,
      setPreRoll,
      setSnapMode,
      setSpeed,
      setVolume,
      snapTime,
      snapshot,
      startAnalysis,
      state,
      track,
      updateRegion,
      updateSelection,
      waveform,
      waveformLoading,
      opening,
    ],
  );

  return (
    <PracticeContext.Provider value={value}>{children}</PracticeContext.Provider>
  );
}

export function usePractice(): PracticeContextValue {
  const context = useContext(PracticeContext);
  if (!context) {
    throw new Error("usePractice must be used inside PracticeProvider");
  }
  return context;
}
