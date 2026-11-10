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
import type { LibraryItem } from "../../api/library";
import {
  DEFAULT_PRACTICE_STATE,
  onPracticeError,
  onPracticePosition,
  practiceClose,
  practiceOpen,
  practicePause,
  practicePlay,
  practiceRecent,
  practiceRecentHide,
  practiceSeek,
  practiceSetLoop,
  practiceSetPitch,
  practiceSetSpeed,
  practiceSetVolume,
  practiceStateSave,
  practiceWaveform,
  type PracticeRegion,
  type PracticeSnapshot,
  type PracticeState,
  type RegionKind,
  type Waveform,
} from "../../api/practice";

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

interface PracticeContextValue {
  track: LibraryItem | null;
  state: PracticeState;
  snapshot: PracticeSnapshot;
  waveform: Waveform | null;
  waveformLoading: boolean;
  error: string | null;
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

export function PracticeProvider({ children }: { children: ReactNode }) {
  const [track, setTrack] = useState<LibraryItem | null>(null);
  const [state, setState] = useState<PracticeState>(DEFAULT_PRACTICE_STATE);
  const [snapshot, setSnapshot] = useState<PracticeSnapshot>(EMPTY_SNAPSHOT);
  const [waveform, setWaveform] = useState<Waveform | null>(null);
  const [waveformLoading, setWaveformLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [recent, setRecent] = useState<LibraryItem[]>([]);
  const [selection, setSelection] = useState<Selection | null>(null);
  const positionRef = useRef(0);
  const selectionRef = useRef<Selection | null>(null);
  const stateRef = useRef(state);
  const trackRef = useRef<LibraryItem | null>(null);
  const hydratedRef = useRef(false);
  const saveTimer = useRef<number | undefined>(undefined);

  stateRef.current = state;
  trackRef.current = track;

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
      setSnapshot(next);
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
      if (trackRef.current) await saveNow();
      hydratedRef.current = false;
      setError(null);
      updateSelection(null);
      setWaveform(null);
      try {
        const opened = await practiceOpen(item.path);
        const nextState = opened.state;
        setTrack(item);
        setState(nextState);
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
        setWaveformLoading(true);
        practiceWaveform(item.path)
          .then((value) => setWaveform(value))
          .catch((cause: unknown) => setError(String(cause)))
          .finally(() => setWaveformLoading(false));
        refreshRecent();
      } catch (cause) {
        setError(String(cause));
      }
    },
    [refreshRecent, saveNow, updateSelection],
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
    setState(DEFAULT_PRACTICE_STATE);
    setSnapshot(EMPTY_SNAPSHOT);
    setWaveform(null);
    updateSelection(null);
    positionRef.current = 0;
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

  const applyLoop = useCallback(
    async (enabled: boolean, region: PracticeRegion | null, preRoll: boolean) => {
      try {
        if (enabled && region) {
          await practiceSetLoop(true, region.start, region.end, preRoll);
        } else {
          await practiceSetLoop(false, null, null, preRoll);
        }
      } catch (cause) {
        setError(String(cause));
      }
    },
    [],
  );

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
      setState((current) => ({ ...current, activeRegionId: id }));
      if (state.loopEnabled && region) {
        await applyLoop(true, region, state.preRoll);
      }
    },
    [applyLoop, state.loopEnabled, state.preRoll, state.regions],
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
      const position = positionRef.current;
      const base = selectionRef.current ?? { start: position, end: position };
      const next =
        which === "start"
          ? { start: position, end: Math.max(position, base.end) }
          : { start: Math.min(base.start, position), end: position };
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
    [applyLoop, updateSelection],
  );

  const clearLoop = useCallback(async () => {
    setState((current) => ({
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

  const duration =
    snapshot.durationSecs ?? track?.durationSecs ?? 0;

  const value = useMemo<PracticeContextValue>(
    () => ({
      track,
      state,
      snapshot,
      waveform,
      waveformLoading,
      error,
      recent,
      selection,
      duration,
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
    }),
    [
      addRegion,
      close,
      deleteRegion,
      duration,
      error,
      openTrack,
      hideRecent,
      playPause,
      recent,
      saveSelectionAsRegion,
      seek,
      selectRegion,
      selectRegionByIndex,
      selection,
      setLoopBoundary,
      setLoopEnabled,
      loopRegion,
      clearLoop,
      nudgeLoop,
      setPitch,
      setPreRoll,
      setSpeed,
      setVolume,
      snapshot,
      state,
      track,
      updateRegion,
      updateSelection,
      waveform,
      waveformLoading,
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
