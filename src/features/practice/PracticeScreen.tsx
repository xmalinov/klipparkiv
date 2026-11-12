import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openMedia } from "../../api/library";
import { formatDuration } from "../../api/tasks";
import type { PracticeRegion, RegionKind } from "../../api/practice";
import { useI18n, type TranslationKey } from "../../i18n";
import DropOverlay from "../library/DropOverlay";
import { useImport } from "../library/ImportProvider";
import { MEDIA_EXTENSIONS } from "../library/mediaExtensions";
import { useFileDrop } from "../library/useFileDrop";
import RegionDialog, { type RegionDraft } from "./RegionDialog";
import { REGION_COLORS, usePractice } from "./PracticeProvider";
import Waveform from "./Waveform";

const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2];

const KIND_KEYS: Record<RegionKind, TranslationKey> = {
  intro: "practice.regionKinds.intro",
  verse: "practice.regionKinds.verse",
  chorus: "practice.regionKinds.chorus",
  solo: "practice.regionKinds.solo",
  bridge: "practice.regionKinds.bridge",
  outro: "practice.regionKinds.outro",
  custom: "practice.regionKinds.custom",
};

const HELP_ROWS: { keys: string; label: TranslationKey }[] = [
  { keys: "Space", label: "practice.help.space" },
  { keys: "← / →", label: "practice.help.seek" },
  { keys: "Shift + ← / →", label: "practice.help.seekLong" },
  { keys: "A / B", label: "practice.help.loopPoints" },
  { keys: "L", label: "practice.help.loopToggle" },
  { keys: "Shift + L", label: "practice.help.loopClear" },
  { keys: "[ / ]", label: "practice.help.nudge" },
  { keys: "1…9", label: "practice.help.regions" },
  { keys: "M", label: "practice.help.addRegion" },
  { keys: "S", label: "practice.help.saveRegion" },
  { keys: "Delete", label: "practice.help.deleteRegion" },
  { keys: "+ / −", label: "practice.help.speed" },
  { keys: "Shift + ↑ / ↓", label: "practice.help.pitch" },
  { keys: "?", label: "practice.help.help" },
];

function GridBar() {
  const { t } = useI18n();
  const {
    state,
    analysis,
    analysisStatus,
    analysisProgress,
    analysisError,
    grid,
    setBpmScale,
    setBeatsPerBar,
    setBarOffset,
    setSnapMode,
    setMetronome,
    setCountIn,
    setManualBpm,
    setGridEnabled,
    loopBars,
    startAnalysis,
  } = usePractice();
  const [manual, setManual] = useState(
    state.manualBpm === null ? "" : String(state.manualBpm),
  );
  const [taps, setTaps] = useState<number[]>([]);

  useEffect(() => {
    setManual(state.manualBpm === null ? "" : String(state.manualBpm));
  }, [state.manualBpm]);

  const bpm = (state.manualBpm ?? analysis?.bpm ?? 0) * state.bpmScale;
  const hasGrid = Boolean(grid);
  const busy = analysisStatus === "loading";

  function applyManual() {
    const value = Number(manual.replace(",", "."));
    setManualBpm(Number.isFinite(value) && value > 0 ? value : null);
  }

  function tapTempo() {
    const now = performance.now();
    const recent = [...taps.filter((time) => now - time < 3000), now].slice(-8);
    setTaps(recent);
    if (recent.length < 2) return;
    const intervals = recent
      .slice(1)
      .map((time, index) => time - recent[index])
      .filter((interval) => interval > 100);
    if (intervals.length === 0) return;
    intervals.sort((left, right) => left - right);
    const median = intervals[Math.floor(intervals.length / 2)];
    if (median > 0) setManualBpm(60000 / median);
  }

  return (
    <div className="practice-grid-bar">
      <div className="grid-bpm">
        <span className="label">{t("practice.bpm")}</span>
        <strong>{bpm > 0 ? bpm.toFixed(1) : "—"}</strong>
        <button
          type="button"
          className={state.bpmScale === 0.5 ? "chip active" : "chip"}
          onClick={() => setBpmScale(0.5)}
          title={t("practice.bpmHalf")}
        >
          ÷2
        </button>
        <button
          type="button"
          className={state.bpmScale === 2 ? "chip active" : "chip"}
          onClick={() => setBpmScale(2)}
          title={t("practice.bpmDouble")}
        >
          ×2
        </button>
      </div>

      <label className="check">
        <input
          type="checkbox"
          checked={state.gridEnabled}
          onChange={(event) => setGridEnabled(event.currentTarget.checked)}
        />
        {t("practice.gridAuto")}
      </label>

      <label className="grid-field">
        <span>{t("practice.meter")}</span>
        <select
          className="field"
          value={state.beatsPerBar}
          onChange={(event) => setBeatsPerBar(Number(event.currentTarget.value))}
        >
          {[2, 3, 4, 6, 8].map((beats) => (
            <option key={beats} value={beats}>
              {beats}
            </option>
          ))}
        </select>
      </label>

      <div className="grid-field">
        <span>{t("practice.barStart")}</span>
        <button
          type="button"
          className="icon-btn"
          onClick={() => setBarOffset(state.barOffset - 1)}
          aria-label={t("practice.barPrevious")}
        >
          ‹
        </button>
        <span className="grid-value">{state.barOffset + 1}</span>
        <button
          type="button"
          className="icon-btn"
          onClick={() => setBarOffset(state.barOffset + 1)}
          aria-label={t("practice.barNext")}
        >
          ›
        </button>
      </div>

      <label className="grid-field">
        <span>{t("practice.snap")}</span>
        <select
          className="field"
          value={state.snapMode}
          onChange={(event) =>
            setSnapMode(event.currentTarget.value as "bar" | "beat" | "free")
          }
        >
          <option value="bar">{t("practice.snapBar")}</option>
          <option value="beat">{t("practice.snapBeat")}</option>
          <option value="free">{t("practice.snapFree")}</option>
        </select>
      </label>

      <div className="grid-field">
        <span>{t("practice.loopLength")}</span>
        {[1, 2, 4, 8].map((bars) => (
          <button
            key={bars}
            type="button"
            className="chip"
            disabled={!hasGrid}
            onClick={() => loopBars(bars)}
          >
            {bars}
          </button>
        ))}
      </div>

      <label className="check">
        <input
          type="checkbox"
          checked={state.metronomeEnabled}
          disabled={!hasGrid}
          onChange={(event) =>
            setMetronome(event.currentTarget.checked, state.metronomeVolume)
          }
        />
        {t("practice.metronome")}
      </label>

      <label className="check">
        <input
          type="checkbox"
          checked={state.countIn}
          disabled={!hasGrid}
          onChange={(event) => setCountIn(event.currentTarget.checked)}
        />
        {t("practice.countIn")}
      </label>

      <label className="grid-field grid-manual">
        <span>{t("practice.manualBpm")}</span>
        <input
          className="field speed-input"
          type="number"
          min={20}
          max={400}
          step={0.1}
          value={manual}
          placeholder={t("practice.auto")}
          onChange={(event) => setManual(event.currentTarget.value)}
          onBlur={applyManual}
          onKeyDown={(event) => {
            if (event.key === "Enter") applyManual();
          }}
        />
        <button type="button" className="chip" onClick={tapTempo}>
          {t("practice.tap")}
        </button>
      </label>

      <div className="grid-status" role="status">
        {busy ? (
          <span className="grid-building">
            <span className="spinner small" aria-hidden="true" />
            {t("practice.gridBuilding")} {Math.round(analysisProgress * 100)}%
          </span>
        ) : analysisStatus === "error" ? (
          <button type="button" className="chip" onClick={() => startAnalysis()}>
            {t("practice.gridRetry")}
          </button>
        ) : hasGrid ? (
          <span className="ok">●</span>
        ) : (
          <button type="button" className="chip" onClick={() => startAnalysis()}>
            {t("practice.gridAnalyze")}
          </button>
        )}
        {analysisError && analysisStatus === "error" && (
          <span className="grid-error" title={analysisError}>
            {t("practice.gridUnavailable")}
          </span>
        )}
      </div>
    </div>
  );
}

export default function PracticeScreen({ onBack }: { onBack: () => void }) {
  const { t } = useI18n();
  const {
    track,
    state,
    snapshot,
    error,
    opening,
    recent,
    selection,
    duration,
    openTrack,
    hideRecent,
    playPause,
    seek,
    setSpeed,
    setPitch,
    setVolume,
    setLoopEnabled,
    setLoopBoundary,
    clearLoop,
    nudgeLoop,
    selectRegion,
    selectRegionByIndex,
    loopRegion,
    deleteRegion,
    saveSelectionAsRegion,
    updateRegion,
    setPreRoll,
  } = usePractice();
  const [dialog, setDialog] = useState<{
    mode: "add" | "save" | "edit";
    initial: RegionDraft;
    regionId?: string;
  } | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);
  const [speedText, setSpeedText] = useState(String(state.speed));
  const [importNotice, setImportNotice] = useState<string | null>(null);
  const importer = useImport();
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const importPaths = useCallback(
    async (paths: string[]) => {
      if (!importer) return;
      setImportNotice(null);
      try {
        const report = await importer.startImport(paths);
        if (report.cancelled) {
          setImportNotice(t("library.importCancelled"));
          return;
        }
        const items = [...report.imported, ...report.existing];
        const firstAudio = items.find((item) => item.kind === "audio");
        const hasVideo = items.some((item) => item.kind === "video");
        if (firstAudio) {
          // Opens paused; grid analysis starts inside openTrack.
          if (mountedRef.current) await openTrack(firstAudio);
          if (hasVideo) setImportNotice(t("practice.videoNotPractice"));
          return;
        }
        if (hasVideo) {
          setImportNotice(t("practice.videoNotPractice"));
        } else if (report.skipped.some((skip) => skip.reason === "folder")) {
          setImportNotice(t("library.importFolderHint"));
        } else if (report.failed.length > 0) {
          setImportNotice(
            report.failed[0]?.error ?? t("practice.importUnsupported"),
          );
        } else if (items.length === 0) {
          setImportNotice(t("practice.importUnsupported"));
        }
      } catch (cause) {
        setImportNotice(cause instanceof Error ? cause.message : String(cause));
      }
    },
    [importer, openTrack, t],
  );

  const drop = useFileDrop({ onDrop: importPaths });

  const addFiles = useCallback(async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [
          { name: t("library.mediaFilter"), extensions: MEDIA_EXTENSIONS },
        ],
      });
      const paths = Array.isArray(selected)
        ? selected
        : selected
          ? [selected]
          : [];
      if (paths.length > 0) await importPaths(paths);
    } catch (cause) {
      setImportNotice(String(cause));
    }
  }, [importPaths, t]);

  useEffect(() => {
    setSpeedText(String(state.speed));
  }, [state.speed]);

  function applyCustomSpeed() {
    const parsed = Number(speedText.replace(",", "."));
    if (!Number.isFinite(parsed)) {
      setSpeedText(String(state.speed));
      return;
    }
    const next = Math.min(2, Math.max(0.25, Math.round(parsed * 100) / 100));
    setSpeed(next);
    setSpeedText(String(next));
  }

  const activeRegion =
    state.regions.find((region) => region.id === state.activeRegionId) ?? null;

  const openAddDialog = useCallback(() => {
    const index = state.regions.length + 1;
    setDialog({
      mode: "add",
      initial: {
        name: `${t("practice.addRegion")} ${index}`,
        kind: "custom",
        color: REGION_COLORS.custom,
      },
    });
  }, [state.regions.length, t]);

  const openSaveDialog = useCallback(() => {
    setDialog({
      mode: "save",
      initial: {
        name: "A-B",
        kind: "custom",
        color: REGION_COLORS.custom,
      },
    });
  }, []);

  const openEditDialog = useCallback((region: PracticeRegion) => {
    setDialog({
      mode: "edit",
      regionId: region.id,
      initial: {
        name: region.name,
        kind: region.kind,
        color: region.color,
      },
    });
  }, []);

  const cycleSpeed = useCallback(
    (direction: number) => {
      const current = SPEEDS.indexOf(state.speed);
      const next = Math.min(
        SPEEDS.length - 1,
        Math.max(0, (current < 0 ? 3 : current) + direction),
      );
      setSpeed(SPEEDS[next]);
    },
    [setSpeed, state.speed],
  );

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.tagName === "SELECT")
      ) {
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey) return;
      if (dialog) {
        if (event.key === "Escape") setDialog(null);
        return;
      }
      if (helpOpen) {
        if (event.key === "Escape" || event.key === "?") setHelpOpen(false);
        return;
      }

      switch (event.key) {
        case " ":
          event.preventDefault();
          playPause();
          break;
        case "ArrowLeft":
          event.preventDefault();
          seek(Math.max(0, snapshot.positionSecs - (event.shiftKey ? 10 : 2)));
          break;
        case "ArrowRight":
          event.preventDefault();
          seek(
            Math.min(duration || Number.POSITIVE_INFINITY, snapshot.positionSecs + (event.shiftKey ? 10 : 2)),
          );
          break;
        case "ArrowUp":
          if (event.shiftKey) {
            event.preventDefault();
            setPitch(Math.min(12, state.pitchSemitones + 1), state.cents);
          }
          break;
        case "ArrowDown":
          if (event.shiftKey) {
            event.preventDefault();
            setPitch(Math.max(-12, state.pitchSemitones - 1), state.cents);
          }
          break;
        case "a":
        case "A":
          setLoopBoundary("start");
          break;
        case "b":
        case "B":
          setLoopBoundary("end");
          break;
        case "l":
          setLoopEnabled(!state.loopEnabled);
          break;
        case "L":
          clearLoop();
          break;
        case "[":
          nudgeLoop("start", -0.05);
          break;
        case "]":
          nudgeLoop("end", 0.05);
          break;
        case "m":
        case "M":
          openAddDialog();
          break;
        case "s":
        case "S":
          openSaveDialog();
          break;
        case "Delete":
          if (activeRegion) deleteRegion(activeRegion.id);
          break;
        case "+":
        case "=":
          cycleSpeed(1);
          break;
        case "-":
          cycleSpeed(-1);
          break;
        case "?":
          setHelpOpen(true);
          break;
        default:
          if (/^[1-9]$/.test(event.key)) {
            selectRegionByIndex(Number(event.key) - 1);
          }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [
    activeRegion,
    clearLoop,
    cycleSpeed,
    deleteRegion,
    dialog,
    duration,
    helpOpen,
    nudgeLoop,
    openAddDialog,
    openSaveDialog,
    playPause,
    seek,
    selectRegionByIndex,
    setLoopBoundary,
    setLoopEnabled,
    setPitch,
    snapshot.positionSecs,
    state.cents,
    state.loopEnabled,
    state.pitchSemitones,
  ]);

  if (opening) {
    return (
      <div className="practice practice-opening" role="status">
        <span className="spinner" aria-hidden="true" />
        <p>{t("practice.opening")}</p>
        {drop.dragging && (
          <DropOverlay
            title={t("practice.dropActive")}
            hint={t("practice.dropHint")}
          />
        )}
      </div>
    );
  }

  if (!track) {
    return (
      <div className="practice practice-empty">
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M9 18V5l12-2v13" />
          <circle cx="6" cy="18" r="3" />
          <circle cx="18" cy="16" r="3" />
        </svg>
        <p>{t("practice.empty")}</p>
        <p className="practice-empty-hint">{t("practice.emptyHint")}</p>
        <button
          type="button"
          className="btn primary"
          onClick={addFiles}
          disabled={importer?.importing}
        >
          {t("practice.addFiles")}
        </button>
        {importNotice && (
          <p className="banner" role="status">
            {importNotice}
          </p>
        )}
        {drop.dragging && (
          <DropOverlay
            title={t("practice.dropActive")}
            hint={t("practice.dropHint")}
          />
        )}
      </div>
    );
  }

  return (
    <div className="practice">
      <div className="practice-head">
        <button type="button" className="btn ghost" onClick={onBack}>
          ← {t("practice.back")}
        </button>
        <div className="practice-title" title={track.title}>
          {track.title}
        </div>
        <button
          type="button"
          className="btn ghost"
          onClick={addFiles}
          disabled={importer?.importing}
        >
          {t("practice.addFiles")}
        </button>
        <button
          type="button"
          className="btn ghost"
          onClick={() => openMedia(track.path)}
        >
          {t("practice.systemPlayer")}
        </button>
      </div>

      {error && (
        <p className="banner" role="alert">
          {t("practice.error", { error })}
        </p>
      )}

      {importNotice && (
        <p className="banner" role="status">
          {importNotice}
        </p>
      )}

      <GridBar />

      <Waveform height={220} />
      <div className="practice-main">
        <div className="practice-left">
          <div className="transport">
            <button
              type="button"
              className="transport-play"
              aria-label={snapshot.playing ? t("practice.pause") : t("practice.play")}
              onClick={playPause}
            >
              {snapshot.playing ? "❚❚" : "▶"}
            </button>
            <button
              type="button"
              className="icon-btn"
              aria-label="-5 seconds"
              onClick={() => seek(Math.max(0, snapshot.positionSecs - 5))}
            >
              −5
            </button>
            <button
              type="button"
              className="icon-btn"
              aria-label="+5 seconds"
              onClick={() => seek(Math.min(duration, snapshot.positionSecs + 5))}
            >
              +5
            </button>
            {state.loopEnabled && activeRegion && (
              <button
                type="button"
                className="icon-btn"
                aria-label={t("practice.toLoopStart")}
                title={t("practice.toLoopStart")}
                onClick={() => seek(activeRegion.start)}
              >
                ⏮
              </button>
            )}
            <span className="transport-time">
              {formatDuration(snapshot.positionSecs)} / {formatDuration(duration)}
            </span>

            <div className="transport-group">
              <span className="label">{t("practice.speed")}</span>
              {SPEEDS.map((speed) => (
                <button
                  key={speed}
                  type="button"
                  className={state.speed === speed ? "chip active" : "chip"}
                  onClick={() => setSpeed(speed)}
                >
                  {speed}×
                </button>
              ))}
              <input
                className="field speed-input"
                type="number"
                min={0.25}
                max={2}
                step={0.05}
                value={speedText}
                aria-label={t("practice.speedCustom")}
                title={t("practice.speedCustom")}
                onChange={(event) => setSpeedText(event.currentTarget.value)}
                onBlur={applyCustomSpeed}
                onKeyDown={(event) => {
                  if (event.key === "Enter") applyCustomSpeed();
                }}
              />
              <button
                type="button"
                className="icon-btn"
                aria-label={t("practice.speedDown")}
                title={t("practice.speedDown")}
                onClick={() =>
                  setSpeed(Math.max(0.25, Math.round((state.speed - 0.05) * 100) / 100))
                }
              >
                −
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label={t("practice.speedUp")}
                title={t("practice.speedUp")}
                onClick={() =>
                  setSpeed(Math.min(2, Math.round((state.speed + 0.05) * 100) / 100))
                }
              >
                +
              </button>
            </div>

            <div className="transport-group">
              <span className="label">{t("practice.pitch")}</span>
              <button
                type="button"
                className="icon-btn pitch-symbol"
                aria-label={t("practice.pitchDown")}
                title={t("practice.pitchDown")}
                onClick={() => setPitch(Math.max(-12, state.pitchSemitones - 1), state.cents)}
              >
                ♭
              </button>
              <span className="pitch-value" title={t("practice.semitones")}>
                {state.pitchSemitones > 0 ? "+" : ""}
                {state.pitchSemitones}
              </span>
              <button
                type="button"
                className="icon-btn pitch-symbol"
                aria-label={t("practice.pitchUp")}
                title={t("practice.pitchUp")}
                onClick={() => setPitch(Math.min(12, state.pitchSemitones + 1), state.cents)}
              >
                ♯
              </button>
              <button
                type="button"
                className="icon-btn"
                aria-label={t("practice.pitchReset")}
                title={t("practice.pitchReset")}
                onClick={() => setPitch(0, 0)}
              >
                ↺
              </button>
            </div>
            <div className="transport-group">
              <span className="label">{t("practice.volume")}</span>
              <input
                className="volume-slider"
                type="range"
                min={0}
                max={100}
                step={1}
                value={Math.round(state.volume * 100)}
                aria-label={t("practice.volume")}
                onChange={(event) =>
                  setVolume(Number(event.currentTarget.value) / 100)
                }
              />
              <span className="volume-value">{Math.round(state.volume * 100)}%</span>
            </div>
          </div>

          <div className="practice-actions">
            <button
              type="button"
              className="chip"
              aria-label={t("practice.setA")}
              title={t("practice.setA")}
              onClick={() => setLoopBoundary("start")}
            >
              A
            </button>
            <button
              type="button"
              className="chip"
              aria-label={t("practice.setB")}
              title={t("practice.setB")}
              onClick={() => setLoopBoundary("end")}
            >
              B
            </button>
            <button
              type="button"
              className={state.loopEnabled ? "chip active" : "chip"}
              aria-pressed={state.loopEnabled}
              disabled={state.regions.length === 0}
              onClick={() => setLoopEnabled(!state.loopEnabled)}
            >
              {t("practice.loop")}
              {activeRegion ? `: ${activeRegion.name}` : ""}
            </button>
            <label className="check">
              <input
                type="checkbox"
                checked={state.preRoll}
                onChange={(event) => setPreRoll(event.currentTarget.checked)}
              />
              {t("practice.preRoll")}
            </label>
            <button type="button" className="btn ghost" onClick={openSaveDialog}>
              {t("practice.saveLoop")}
            </button>
            <button type="button" className="btn ghost" onClick={openAddDialog}>
              {t("practice.addRegion")}
            </button>
            {selection && (
              <span className="selection-hint">
                {t("practice.selection")}: {formatDuration(selection.start)}–
                {formatDuration(selection.end)}
              </span>
            )}
          </div>
        </div>

        <aside className="practice-side">
          <section className="practice-panel">
            <div className="panel-head">
              <span title={t("practice.regionsHint")}>{t("practice.regions")}</span>
              <button type="button" className="icon-btn" onClick={openAddDialog}>
                +
              </button>
            </div>
            {state.regions.length === 0 ? (
              <p className="queue-empty">{t("practice.noRegions")}</p>
            ) : (
              <div className="region-list">
                {state.regions.map((region, index) => (
                  <div
                    key={region.id}
                    className={
                      state.activeRegionId === region.id
                        ? "region-row active"
                        : "region-row"
                    }
                  >
                    <button
                      type="button"
                      className="region-main"
                      onClick={() => selectRegion(region.id)}
                      onDoubleClick={() => openEditDialog(region)}
                      aria-current={
                        state.activeRegionId === region.id ? "true" : undefined
                      }
                      title={region.name}
                    >
                      <span
                        className="region-dot"
                        style={{ background: region.color }}
                      />
                      <span className="region-name">{region.name}</span>
                      <span className="region-kind">
                        {t(KIND_KEYS[region.kind])}
                      </span>
                      <span className="region-time">
                        {formatDuration(region.start)}–{formatDuration(region.end)}
                      </span>
                      <span className="region-index">{index + 1}</span>
                    </button>
                    <button
                      type="button"
                      className={
                        state.loopEnabled && state.activeRegionId === region.id
                          ? "icon-btn on"
                          : "icon-btn"
                      }
                      aria-label={t("practice.loopRegion")}
                      aria-pressed={
                        state.loopEnabled && state.activeRegionId === region.id
                      }
                      title={t("practice.loopRegion")}
                      onClick={() => loopRegion(region.id)}
                    >
                      ⟳
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={t("practice.editRegion")}
                      title={t("practice.editRegion")}
                      onClick={() => openEditDialog(region)}
                    >
                      ✎
                    </button>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={t("practice.deleteRegion")}
                      title={t("practice.deleteRegion")}
                      onClick={() => deleteRegion(region.id)}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>

          <section className="practice-panel">
            <div className="panel-head">
              <span>{t("practice.recent")}</span>
            </div>
            {recent.length === 0 ? (
              <p className="queue-empty">{t("practice.noRecent")}</p>
            ) : (
              <div className="recent-list">
                {recent.map((item) => (
                  <div key={item.id} className="recent-row-wrap">
                    <button
                      type="button"
                      className={
                        item.id === track.id ? "recent-row active" : "recent-row"
                      }
                      onClick={() => openTrack(item)}
                      title={item.title}
                    >
                      {item.title}
                    </button>
                    <button
                      type="button"
                      className="icon-btn recent-remove"
                      aria-label={t("practice.recent.remove")}
                      title={t("practice.recent.remove")}
                      onClick={() => hideRecent(item.path)}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            )}
          </section>
        </aside>
      </div>

      {dialog && (
        <RegionDialog
          initial={dialog.initial}
          onCancel={() => setDialog(null)}
          onSave={(draft) => {
            if (dialog.mode === "edit" && dialog.regionId) {
              updateRegion(dialog.regionId, draft);
            } else {
              saveSelectionAsRegion(draft.name, draft.kind, draft.color);
            }
            setDialog(null);
          }}
        />
      )}

      {helpOpen && (
        <div className="overlay" role="presentation" onClick={() => setHelpOpen(false)}>
          <div
            className="dialog practice-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={t("practice.helpTitle")}
            onClick={(event) => event.stopPropagation()}
          >
            <div className="dialog-head">
              <span>{t("practice.helpTitle")}</span>
              <button
                type="button"
                className="icon-btn"
                aria-label={t("common.close")}
                onClick={() => setHelpOpen(false)}
              >
                ×
              </button>
            </div>
            <div className="dialog-body">
              <table className="hotkeys">
                <tbody>
                  {HELP_ROWS.map((row) => (
                    <tr key={row.keys}>
                      <td>
                        <kbd>{row.keys}</kbd>
                      </td>
                      <td>{t(row.label)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {drop.dragging && (
        <DropOverlay
          title={t("practice.dropActive")}
          hint={t("practice.dropHint")}
        />
      )}
    </div>
  );
}
