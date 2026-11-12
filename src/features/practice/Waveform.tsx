import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { decodePeaks } from "../../api/practice";
import { useI18n } from "../../i18n";
import { usePractice } from "./PracticeProvider";
import {
  MIN_VIEW_SECS,
  clampViewRange,
  createSelection,
  edgeTolerance as edgeToleranceFor,
  fitView,
  formatClock,
  moveSelection,
  pickLevel,
  regionAt as regionAtTime,
  regionEdgeAt as findRegionEdge,
  resizeRegionEnd,
  resizeRegionStart,
  resizeSelectionEnd,
  resizeSelectionStart,
  selectionEdgeAt as findSelectionEdge,
  timeToX as mapTimeToX,
  xToTime as mapXToTime,
  zoomViewRange,
} from "./waveformMath";

type DragState =
  | { kind: "create"; anchor: number; current: number }
  | {
      kind: "selection-move";
      anchor: number;
      start: number;
      end: number;
    }
  | { kind: "selection-start"; fixed: number }
  | { kind: "selection-end"; fixed: number }
  | { kind: "region-start" | "region-end"; regionId: string; fixed: number };

export default function Waveform({ height = 190 }: { height?: number }) {
  const { t } = useI18n();
  const {
    waveform,
    waveformLoading,
    snapshot,
    state,
    duration,
    selection,
    seek,
    setSelection,
    selectRegion,
    updateRegion,
    loopRegion,
    track,
    grid,
    snapTime,
  } = usePractice();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);
  const [view, setView] = useState({ start: 0, end: 1 });
  const [cursor, setCursor] = useState("crosshair");
  const [regionPreview, setRegionPreview] = useState<{
    id: string;
    start: number;
    end: number;
  } | null>(null);
  const dragRef = useRef<DragState | null>(null);
  const trackRef = useRef<string | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    if (typeof ResizeObserver === "undefined") {
      setWidth(canvas.clientWidth);
      return;
    }
    const observer = new ResizeObserver((entries) => {
      setWidth(entries[0]?.contentRect.width ?? 0);
    });
    observer.observe(canvas);
    setWidth(canvas.clientWidth);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const total = duration > 0 ? duration : 1;
    if (trackRef.current !== track?.id) {
      trackRef.current = track?.id ?? null;
      setView({ start: 0, end: total });
      return;
    }
    setView((current) =>
      current.end <= 1.001 ||
      current.end - current.start <= MIN_VIEW_SECS + 0.001
        ? { start: 0, end: total }
        : current,
    );
  }, [duration, track?.id]);

  const viewDuration = Math.max(MIN_VIEW_SECS, view.end - view.start);
  const level = useMemo(
    () => pickLevel(waveform?.levels ?? [], viewDuration, width),
    [viewDuration, waveform, width],
  );
  const peaks = useMemo(() => (level ? decodePeaks(level.peaks) : null), [level]);

  const timeToX = useCallback(
    (time: number) => mapTimeToX(time, view, viewDuration, width),
    [view, viewDuration, width],
  );
  const xToTime = useCallback(
    (x: number) => mapXToTime(x, view, viewDuration, width),
    [view, viewDuration, width],
  );

  const clampView = useCallback(
    (start: number, end: number) => {
      setView(clampViewRange(start, end, duration));
    },
    [duration],
  );

  const zoomAt = useCallback(
    (factor: number, center: number) => {
      setView((current) => zoomViewRange(current, factor, center, duration));
    },
    [duration],
  );

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || width <= 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.floor(width * dpr);
    canvas.height = Math.floor(height * dpr);
    const context = canvas.getContext("2d");
    if (!context) return;
    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);

    context.fillStyle = "rgba(127, 145, 170, 0.08)";
    context.fillRect(0, 0, width, height);
    context.strokeStyle = "rgba(127, 145, 170, 0.18)";
    context.beginPath();
    context.moveTo(0, height / 2);
    context.lineTo(width, height / 2);
    context.stroke();

    if (peaks) {
      const buckets = Math.floor(peaks.length / 2);
      const totalDuration = Math.max(0.1, waveform?.durationSecs || duration || 0.1);
      const fromBucket = Math.max(
        0,
        Math.min(buckets, (view.start / totalDuration) * buckets),
      );
      const toBucket = Math.max(
        fromBucket,
        Math.min(buckets, (view.end / totalDuration) * buckets),
      );
      context.strokeStyle = "rgba(127, 160, 205, 0.85)";
      context.lineWidth = 1;
      context.beginPath();
      for (let x = 0; x < width; x += 1) {
        const from = Math.floor(fromBucket + (x / width) * (toBucket - fromBucket));
        const to = Math.max(
          from + 1,
          Math.floor(fromBucket + ((x + 1) / width) * (toBucket - fromBucket)),
        );
        let min = 127;
        let max = -127;
        for (let bucket = from; bucket < Math.min(to, buckets); bucket += 1) {
          min = Math.min(min, peaks[bucket * 2]);
          max = Math.max(max, peaks[bucket * 2 + 1]);
        }
        if (min > max) continue;
        const top = height / 2 - (max / 127) * (height / 2 - 12);
        const bottom = height / 2 - (min / 127) * (height / 2 - 12);
        context.moveTo(x + 0.5, top);
        context.lineTo(x + 0.5, bottom);
      }
      context.stroke();
    }

    if (grid) {
      context.lineWidth = 1;
      for (const beat of grid.beats) {
        if (beat < view.start - 0.001 || beat > view.end + 0.001) continue;
        const x = timeToX(beat);
        context.strokeStyle = "rgba(120, 170, 255, 0.16)";
        context.beginPath();
        context.moveTo(x, 0);
        context.lineTo(x, height);
        context.stroke();
      }
      context.font = "10px sans-serif";
      for (let index = 0; index < grid.barStarts.length; index += 1) {
        const bar = grid.barStarts[index];
        if (bar < view.start - 0.001 || bar > view.end + 0.001) continue;
        const x = timeToX(bar);
        context.strokeStyle = "rgba(120, 170, 255, 0.38)";
        context.beginPath();
        context.moveTo(x, 0);
        context.lineTo(x, height);
        context.stroke();
        context.fillStyle = "rgba(160, 190, 240, 0.65)";
        context.fillText(String(index + 1), x + 3, height - 18);
      }
    }

    const regions = state.regions.map((region) =>
      regionPreview && regionPreview.id === region.id
        ? { ...region, start: regionPreview.start, end: regionPreview.end }
        : region,
    );
    for (const region of regions) {
      const x1 = Math.max(0, timeToX(region.start));
      const x2 = Math.min(width, timeToX(region.end));
      if (x2 <= 0 || x1 >= width || x2 <= x1) continue;
      context.fillStyle = `${region.color}33`;
      context.fillRect(x1, 0, x2 - x1, height);
      context.fillStyle = region.color;
      context.fillRect(x1, 0, 2, height);
      context.fillRect(x2 - 2, 0, 2, height);
      context.font = "11px sans-serif";
      context.fillStyle = region.color;
      context.fillText(region.name, x1 + 5, 15);
    }

    if (selection) {
      const x1 = Math.max(0, timeToX(selection.start));
      const x2 = Math.min(width, timeToX(selection.end));
      if (x2 > x1) {
        context.fillStyle = "rgba(255, 255, 255, 0.14)";
        context.fillRect(x1, 0, x2 - x1, height);
        context.strokeStyle = "rgba(255, 255, 255, 0.55)";
        context.setLineDash([4, 3]);
        context.strokeRect(x1, 0.5, x2 - x1, height - 1);
        context.setLineDash([]);
        context.fillStyle = "rgba(255, 255, 255, 0.8)";
        context.fillRect(x1 - 1, 0, 2, height);
        context.fillRect(x2 - 1, 0, 2, height);
      }
    }

    const playhead = timeToX(snapshot.positionSecs);
    if (playhead >= 0 && playhead <= width) {
      context.strokeStyle = "#ff8a4f";
      context.lineWidth = 1.5;
      context.beginPath();
      context.moveTo(playhead, 0);
      context.lineTo(playhead, height);
      context.stroke();
    }

    context.font = "10px sans-serif";
    context.fillStyle = "rgba(200, 210, 225, 0.65)";
    const ticks = 8;
    const precision = viewDuration < 2 ? 2 : viewDuration < 10 ? 1 : 0;
    for (let index = 0; index <= ticks; index += 1) {
      const time = view.start + (viewDuration * index) / ticks;
      const x = (width * index) / ticks;
      context.fillText(
        formatClock(time, precision),
        Math.min(width - 34, x + 3),
        height - 5,
      );
    }
  }, [
    height,
    peaks,
    regionPreview,
    selection,
    snapshot.positionSecs,
    state.regions,
    timeToX,
    grid,
    view.start,
    viewDuration,
    waveform?.durationSecs,
    duration,
    width,
  ]);

  const total = Math.max(0.1, duration || 0.1);

  function regionAt(time: number) {
    return regionAtTime(time, state.regions);
  }

  function edgeTolerance() {
    return edgeToleranceFor(width, viewDuration);
  }

  function selectionEdgeAt(time: number) {
    return findSelectionEdge(time, selection, edgeTolerance());
  }

  function regionEdgeAt(time: number) {
    return findRegionEdge(time, state.regions, edgeTolerance());
  }

  function pointerDown(event: React.PointerEvent<HTMLCanvasElement>) {
    if (event.button !== 0) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const time = xToTime(event.clientX - bounds.left);
    const snapped = event.altKey
      ? Math.max(0, Math.min(total, time))
      : snapTime(time);
    const selectionEdge = selectionEdgeAt(time);
    if (selectionEdge) {
      event.currentTarget.setPointerCapture(event.pointerId);
      dragRef.current =
        selectionEdge === "start"
          ? { kind: "selection-start", fixed: selection!.end }
          : { kind: "selection-end", fixed: selection!.start };
      setCursor("ew-resize");
      return;
    }
    if (selection && time > selection.start && time < selection.end) {
      event.currentTarget.setPointerCapture(event.pointerId);
      dragRef.current = {
        kind: "selection-move",
        anchor: time,
        start: selection.start,
        end: selection.end,
      };
      setCursor("grabbing");
      return;
    }
    const regionEdge = regionEdgeAt(time);
    if (regionEdge) {
      selectRegion(regionEdge.region.id);
      event.currentTarget.setPointerCapture(event.pointerId);
      dragRef.current = {
        kind: regionEdge.edge === "start" ? "region-start" : "region-end",
        regionId: regionEdge.region.id,
        fixed:
          regionEdge.edge === "start"
            ? regionEdge.region.end
            : regionEdge.region.start,
      };
      setRegionPreview({
        id: regionEdge.region.id,
        start: regionEdge.region.start,
        end: regionEdge.region.end,
      });
      setCursor("ew-resize");
      return;
    }
    const region = regionAt(time);
    if (region) {
      selectRegion(region.id);
      setSelection(null);
      seek(time);
      setCursor("pointer");
      return;
    }
    event.currentTarget.setPointerCapture(event.pointerId);
    dragRef.current = { kind: "create", anchor: snapped, current: snapped };
    setSelection(null);
    setCursor("crosshair");
  }

  function pointerMove(event: React.PointerEvent<HTMLCanvasElement>) {
    const bounds = event.currentTarget.getBoundingClientRect();
    const time = xToTime(event.clientX - bounds.left);
    const snapped = event.altKey
      ? Math.max(0, Math.min(total, time))
      : snapTime(time);
    const drag = dragRef.current;
    if (!drag) {
      if (selectionEdgeAt(time) || regionEdgeAt(time)) {
        setCursor("ew-resize");
      } else if (
        selection &&
        time > selection.start &&
        time < selection.end
      ) {
        setCursor("grab");
      } else if (regionAt(time)) {
        setCursor("pointer");
      } else {
        setCursor("crosshair");
      }
      return;
    }

    switch (drag.kind) {
      case "create":
        drag.current = snapped;
        setSelection(createSelection(drag.anchor, snapped, total));
        break;
      case "selection-move": {
        const span = drag.end - drag.start;
        let start = drag.start + (time - drag.anchor);
        start = event.altKey
          ? Math.max(0, Math.min(start, total - span))
          : snapTime(start);
        setSelection(moveSelection(start, span, total));
        break;
      }
      case "selection-start": {
        setSelection(resizeSelectionStart(snapped, drag.fixed));
        break;
      }
      case "selection-end": {
        setSelection(resizeSelectionEnd(drag.fixed, snapped, total));
        break;
      }
      case "region-start": {
        const region = state.regions.find((entry) => entry.id === drag.regionId);
        if (!region) break;
        const preview = resizeRegionStart(snapped, drag.fixed);
        setRegionPreview({ id: region.id, ...preview });
        break;
      }
      case "region-end": {
        const region = state.regions.find((entry) => entry.id === drag.regionId);
        if (!region) break;
        const preview = resizeRegionEnd(drag.fixed, snapped, total);
        setRegionPreview({ id: region.id, ...preview });
        break;
      }
    }
  }

  function pointerUp(event: React.PointerEvent<HTMLCanvasElement>) {
    const drag = dragRef.current;
    dragRef.current = null;
    setRegionPreview(null);
    if (!drag) return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const time = xToTime(event.clientX - bounds.left);
    const snapped = event.altKey
      ? Math.max(0, Math.min(total, time))
      : snapTime(time);

    if (drag.kind === "create") {
      if (Math.abs(time - drag.anchor) < 0.05) {
        setSelection(null);
        seek(Math.max(0, Math.min(total, time)));
      }
      return;
    }

    if (drag.kind === "selection-move") {
      if (Math.abs(time - drag.anchor) < 0.01) {
        seek(Math.max(0, Math.min(total, time)));
      }
      return;
    }

    if (drag.kind === "region-start" || drag.kind === "region-end") {
      const region = state.regions.find((entry) => entry.id === drag.regionId);
      if (!region) return;
      const start =
        drag.kind === "region-start"
          ? resizeRegionStart(snapped, drag.fixed).start
          : drag.fixed;
      const end =
        drag.kind === "region-end"
          ? resizeRegionEnd(drag.fixed, snapped, total).end
          : drag.fixed;
      updateRegion(region.id, { start, end });
    }
  }

  function doubleClick(event: React.MouseEvent<HTMLCanvasElement>) {
    const bounds = event.currentTarget.getBoundingClientRect();
    const region = regionAtTime(
      xToTime(event.clientX - bounds.left),
      state.regions,
    );
    if (region) loopRegion(region.id);
  }

  function wheel(event: React.WheelEvent<HTMLCanvasElement>) {
    event.preventDefault();
    const bounds = event.currentTarget.getBoundingClientRect();
    const center = xToTime(event.clientX - bounds.left);
    if (event.shiftKey) {
      const delta = (event.deltaY / Math.max(1, width)) * viewDuration;
      clampView(view.start + delta, view.end + delta);
    } else {
      zoomAt(event.deltaY > 0 ? 1.25 : 0.8, center);
    }
  }

  return (
    <div className="waveform-wrap">
      <canvas
        ref={canvasRef}
        className="waveform-canvas"
        style={{ height, cursor }}
        aria-label={t("practice.selection")}
        onPointerDown={pointerDown}
        onPointerMove={pointerMove}
        onPointerUp={pointerUp}
        onPointerCancel={() => {
          dragRef.current = null;
          setRegionPreview(null);
        }}
        onDoubleClick={doubleClick}
        onWheel={wheel}
      />
      {waveformLoading && (
        <div className="waveform-overlay">
          <span className="spinner" aria-hidden="true" />
          {t("practice.waveformLoading")}
        </div>
      )}
      <div className="waveform-zoom">
        <button
          type="button"
          className="icon-btn"
          aria-label={t("practice.zoomOut")}
          title={t("practice.zoomOut")}
          onClick={() => zoomAt(1.5, view.start + viewDuration / 2)}
        >
          −
        </button>
        <button
          type="button"
          className="icon-btn"
          aria-label={t("practice.zoomIn")}
          title={t("practice.zoomIn")}
          onClick={() => zoomAt(0.66, view.start + viewDuration / 2)}
        >
          +
        </button>
        <button
          type="button"
          className="chip waveform-fit"
          aria-label={t("practice.fit")}
          title={t("practice.fit")}
          onClick={() => setView(fitView(duration))}
        >
          {t("practice.fit")}
        </button>
      </div>
    </div>
  );
}
