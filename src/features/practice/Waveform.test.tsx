import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PracticeRegion } from "../../api/practice";
import Waveform from "./Waveform";

const practice = vi.hoisted(() => ({
  value: {} as Record<string, unknown>,
}));

vi.mock("../../i18n", () => ({
  useI18n: () => ({
    language: "en",
    setLanguage: vi.fn(),
    t: (key: string) =>
      (
        {
          "practice.selection": "Selection",
          "practice.waveformLoading": "Loading waveform",
          "practice.zoomIn": "Zoom in",
          "practice.zoomOut": "Zoom out",
          "practice.fit": "Fit",
        } as Record<string, string>
      )[key] ?? key,
  }),
}));

vi.mock("./PracticeProvider", () => ({
  usePractice: () => practice.value,
}));

const REGION: PracticeRegion = {
  id: "r1",
  name: "Verse",
  kind: "verse",
  color: "#4f8cff",
  start: 4,
  end: 6,
};

/** 100 stereo buckets encoded the way the backend sends them. */
function peakLevel() {
  const bytes = new Uint8Array(200);
  for (let index = 0; index < 100; index += 1) {
    bytes[index * 2] = 256 - (index % 120);
    bytes[index * 2 + 1] = index % 120;
  }
  return btoa(String.fromCharCode(...bytes));
}

function createDrawing() {
  const texts: string[] = [];
  const context = {
    setTransform: vi.fn(),
    clearRect: vi.fn(),
    fillRect: vi.fn(),
    strokeRect: vi.fn(),
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    stroke: vi.fn(),
    setLineDash: vi.fn(),
    fillText: vi.fn((text: string) => {
      texts.push(text);
    }),
    fillStyle: "",
    strokeStyle: "",
    lineWidth: 1,
    font: "",
  };
  return { context, texts };
}

let drawing: ReturnType<typeof createDrawing>;

beforeEach(() => {
  drawing = createDrawing();
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(
    () => drawing.context as unknown as CanvasRenderingContext2D,
  );
  Object.defineProperty(HTMLCanvasElement.prototype, "clientWidth", {
    configurable: true,
    get: () => 500,
  });
  HTMLCanvasElement.prototype.setPointerCapture = vi.fn();
  HTMLCanvasElement.prototype.releasePointerCapture = vi.fn();
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(private readonly callback: ResizeObserverCallback) {}
      observe() {
        this.callback(
          [{ contentRect: { width: 500 } }] as ResizeObserverEntry[],
          this as unknown as ResizeObserver,
        );
      }
      disconnect() {}
    },
  );
});

interface Harness {
  selection?: { start: number; end: number } | null;
  seek?: ReturnType<typeof vi.fn>;
  setSelection?: ReturnType<typeof vi.fn>;
  selectRegion?: ReturnType<typeof vi.fn>;
  updateRegion?: ReturnType<typeof vi.fn>;
  loopRegion?: ReturnType<typeof vi.fn>;
  snapTime?: ReturnType<typeof vi.fn>;
  grid?: Record<string, unknown>;
  snapshot?: Record<string, unknown>;
}

function renderWaveform(harness: Harness = {}) {
  practice.value = {
    waveform: {
      version: 1,
      durationSecs: 10,
      levels: [{ stepSecs: 0.01, peaks: peakLevel() }],
    },
    waveformLoading: false,
    snapshot: {
      positionSecs: 2,
      durationSecs: 10,
      playing: false,
      ended: false,
      underruns: 0,
    },
    state: { regions: [REGION] },
    duration: 10,
    selection: null,
    seek: vi.fn(),
    setSelection: vi.fn(),
    selectRegion: vi.fn(),
    updateRegion: vi.fn(),
    loopRegion: vi.fn(),
    track: { id: "track-1", path: "C:\\audio\\a.mp3" },
    grid: {
      bpm: 120,
      beats: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
      downbeats: [0, 4, 8],
      barStarts: [0, 4, 8],
      beatsPerBar: 4,
      barOffset: 0,
      pickupUntil: null,
      confidence: 1,
    },
    snapTime: (time: number) => Math.round(time),
    ...harness,
  };
  const view = render(<Waveform height={190} />);
  const canvas = screen.getByLabelText("Selection") as HTMLCanvasElement;
  Object.defineProperty(canvas, "getBoundingClientRect", {
    configurable: true,
    value: () => ({
      left: 0,
      top: 0,
      right: 500,
      bottom: 190,
      width: 500,
      height: 190,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    }),
  });
  return { ...view, canvas };
}

function pointer(
  element: HTMLElement,
  type: "pointerDown" | "pointerMove" | "pointerUp",
  clientX: number,
  init: Record<string, unknown> = {},
) {
  fireEvent[type](element, { clientX, button: 0, pointerId: 1, ...init });
}

describe("Waveform", () => {
  it("draws the time ruler, bars and region name, and positions the playhead", () => {
    renderWaveform();

    expect(drawing.texts).toContain("0:00");
    expect(drawing.texts).toContain("Verse");
    expect(drawing.texts).toContain("1");
    expect(drawing.context.stroke).toHaveBeenCalled();
    expect(drawing.context.moveTo).toHaveBeenCalled();
    expect(screen.getByTestId("waveform-playhead")).toHaveStyle({
      transform: "translateX(100px)",
    });
  });

  it("does not redraw the canvas when only the playhead moves", () => {
    const { rerender } = renderWaveform();
    const drawCalls = drawing.context.clearRect.mock.calls.length;
    const snapshot = practice.value.snapshot as Record<string, unknown>;
    practice.value = {
      ...practice.value,
      snapshot: { ...snapshot, positionSecs: 5 },
    };

    rerender(<Waveform height={190} />);

    expect(drawing.context.clearRect).toHaveBeenCalledTimes(drawCalls);
    expect(screen.getByTestId("waveform-playhead")).toHaveStyle({
      transform: "translateX(250px)",
    });
  });

  it("batches grid lines instead of stroking each one", () => {
    renderWaveform();
    // Background, waveform, beats and bars: a handful of strokes for 11 beats
    // and 3 bars. Per-line stroking would need more than 16.
    expect(drawing.context.stroke.mock.calls.length).toBeLessThan(10);
  });

  it("culls bar numbers when bars are too close", () => {
    const dense = Array.from({ length: 100 }, (_, index) => index * 0.1);
    renderWaveform({
      grid: {
        bpm: 120,
        beats: dense,
        downbeats: dense,
        barStarts: dense,
        beatsPerBar: 4,
        barOffset: 0,
        pickupUntil: null,
        confidence: 1,
      },
    });

    const numbers = drawing.texts.filter((text) => /^\d+$/.test(text));
    expect(numbers.length).toBeLessThan(30);
  });

  it("hides the playhead when it is outside the visible range", () => {
    renderWaveform({
      snapshot: {
        positionSecs: 20,
        durationSecs: 10,
        playing: false,
        ended: false,
        underruns: 0,
      },
    });

    expect(screen.getByTestId("waveform-playhead")).toHaveStyle({
      opacity: "0",
    });
  });

  it("zooms and prevents page scroll on wheel over the waveform", () => {
    const { canvas } = renderWaveform();
    const draws = drawing.context.clearRect.mock.calls.length;
    const event = new WheelEvent("wheel", {
      deltaY: 100,
      clientX: 250,
      bubbles: true,
      cancelable: true,
    });

    act(() => {
      canvas.dispatchEvent(event);
    });

    expect(event.defaultPrevented).toBe(true);
    expect(drawing.context.clearRect.mock.calls.length).toBeGreaterThan(draws);
  });

  it("shows the loading overlay while peaks are fetched", () => {
    practice.value = {
      waveform: null,
      waveformLoading: true,
      snapshot: {
        positionSecs: 0,
        durationSecs: 10,
        playing: false,
        ended: false,
        underruns: 0,
      },
      state: { regions: [] },
      duration: 10,
      selection: null,
      seek: vi.fn(),
      setSelection: vi.fn(),
      selectRegion: vi.fn(),
      updateRegion: vi.fn(),
      loopRegion: vi.fn(),
      track: { id: "t", path: "p" },
      grid: null,
      snapTime: (time: number) => time,
    };
    render(<Waveform />);
    expect(screen.getByText("Loading waveform")).toBeInTheDocument();
  });

  it("seeks on a click without dragging", () => {
    const seek = vi.fn();
    const { canvas } = renderWaveform({ seek });

    pointer(canvas, "pointerDown", 50);
    pointer(canvas, "pointerUp", 50);

    expect(seek).toHaveBeenCalledWith(1);
  });

  it("seeks to a click even when it is far from a snap point", () => {
    const seek = vi.fn();
    const { canvas } = renderWaveform({ seek });

    // 60 px of 500 px over 10 s is 1.2 s; the default snap rounds it to 1 s.
    pointer(canvas, "pointerDown", 60);
    pointer(canvas, "pointerUp", 60);

    expect(seek).toHaveBeenCalledWith(1.2);
  });

  it("seeks with a small pointer jitter", () => {
    const seek = vi.fn();
    const { canvas } = renderWaveform({ seek });

    pointer(canvas, "pointerDown", 60);
    pointer(canvas, "pointerUp", 63);

    expect(seek).toHaveBeenCalledWith(1.26);
  });

  it("creates a selection by dragging", () => {
    const setSelection = vi.fn();
    const { canvas } = renderWaveform({ setSelection });

    pointer(canvas, "pointerDown", 50);
    pointer(canvas, "pointerMove", 150);
    pointer(canvas, "pointerUp", 150);

    expect(setSelection).toHaveBeenLastCalledWith({ start: 1, end: 3 });
  });

  it("selects a region and seeks into it instead of starting a selection", () => {
    const seek = vi.fn();
    const selectRegion = vi.fn();
    const setSelection = vi.fn();
    const { canvas } = renderWaveform({ seek, selectRegion, setSelection });

    pointer(canvas, "pointerDown", 250);
    pointer(canvas, "pointerUp", 250);

    expect(selectRegion).toHaveBeenCalledWith("r1");
    expect(seek).toHaveBeenCalledWith(5);
    expect(setSelection).toHaveBeenCalledWith(null);
  });

  it("loops a region on double click", () => {
    const loopRegion = vi.fn();
    const { canvas } = renderWaveform({ loopRegion });

    fireEvent.doubleClick(canvas, { clientX: 250 });
    expect(loopRegion).toHaveBeenCalledWith("r1");
  });

  it("resizes a region from its edge", () => {
    const updateRegion = vi.fn();
    const selectRegion = vi.fn();
    const { canvas } = renderWaveform({ updateRegion, selectRegion });

    pointer(canvas, "pointerDown", 200);
    pointer(canvas, "pointerMove", 150);
    pointer(canvas, "pointerUp", 150);

    expect(selectRegion).toHaveBeenCalledWith("r1");
    expect(updateRegion).toHaveBeenCalledWith("r1", { start: 3, end: 6 });
  });

  it("moves an existing selection by dragging inside it", () => {
    const setSelection = vi.fn();
    const { canvas } = renderWaveform({
      setSelection,
      selection: { start: 2, end: 6 },
    });

    pointer(canvas, "pointerDown", 200);
    pointer(canvas, "pointerMove", 250);
    pointer(canvas, "pointerUp", 250);

    expect(setSelection).toHaveBeenLastCalledWith({ start: 3, end: 7 });
  });

  it("resizes a selection from its edge", () => {
    const setSelection = vi.fn();
    const { canvas } = renderWaveform({
      setSelection,
      selection: { start: 2, end: 6 },
    });

    pointer(canvas, "pointerDown", 100);
    pointer(canvas, "pointerMove", 150);

    expect(setSelection).toHaveBeenLastCalledWith({ start: 3, end: 6 });
  });

  it("bypasses snapping when Alt is held", () => {
    const snapTime = vi.fn((time: number) => Math.round(time));
    const { canvas } = renderWaveform({ snapTime });

    pointer(canvas, "pointerDown", 70);
    expect(snapTime.mock.calls[0][0]).toBeCloseTo(1.4);

    snapTime.mockClear();
    pointer(canvas, "pointerDown", 70, { altKey: true });
    expect(snapTime).not.toHaveBeenCalled();
  });

  it("zooms with the buttons and returns to the fit view", () => {
    const { canvas } = renderWaveform();

    drawing.texts.length = 0;
    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    expect(drawing.texts).toContain("0:01.7");

    drawing.texts.length = 0;
    fireEvent.click(screen.getByRole("button", { name: "Zoom out" }));
    expect(drawing.texts.join(" ")).not.toContain("0:01.7");

    drawing.texts.length = 0;
    fireEvent.click(screen.getByRole("button", { name: "Fit" }));
    expect(drawing.texts).toContain("0:00");
    expect(canvas).toBeInTheDocument();
  });

  it("zooms with the wheel around the cursor", () => {
    const { canvas } = renderWaveform();

    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    drawing.texts.length = 0;
    fireEvent.wheel(canvas, { deltaY: 100, clientX: 250 });
    expect(drawing.texts[0]).not.toBe("0:01.7");
  });

  it("pans with Shift+wheel", () => {
    const { canvas } = renderWaveform();

    fireEvent.click(screen.getByRole("button", { name: "Zoom in" }));
    drawing.texts.length = 0;
    fireEvent.wheel(canvas, { deltaY: 100, clientX: 250, shiftKey: true });
    expect(drawing.texts).toContain("0:03.0");
  });

  it("keeps a crosshair cursor over empty space", () => {
    const { canvas } = renderWaveform();

    pointer(canvas, "pointerMove", 20);
    expect(canvas.style.cursor).toBe("crosshair");
  });
});
