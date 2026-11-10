import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PracticeScreen from "./PracticeScreen";
import { DEFAULT_PRACTICE_STATE } from "../../api/practice";

const context = vi.hoisted(() => ({
  value: {} as Record<string, unknown>,
}));

vi.mock("./PracticeProvider", () => ({
  usePractice: () => context.value,
  REGION_COLORS: {
    intro: "#8b9bb4",
    verse: "#4f8cff",
    chorus: "#ff8a4f",
    solo: "#e05c8a",
    bridge: "#9b7bff",
    outro: "#5fb98a",
    custom: "#c9a227",
  },
  REGION_KINDS: [
    "intro",
    "verse",
    "chorus",
    "solo",
    "bridge",
    "outro",
    "custom",
  ],
}));

const track = {
  id: "C:\\root\\audio\\song.mp3",
  path: "C:\\root\\audio\\song.mp3",
  kind: "audio" as const,
  title: "Track",
  channel: null,
  durationSecs: 10,
  width: null,
  height: null,
  sizeBytes: 1000,
  mtime: 1,
  thumbPath: null,
};

function setup() {
  const setSpeed = vi.fn();
  const setVolume = vi.fn();
  const playPause = vi.fn();
  const selectRegion = vi.fn();
  const selectRegionByIndex = vi.fn();
  const onBack = vi.fn();
  context.value = {
    track,
    state: {
      ...DEFAULT_PRACTICE_STATE,
      loopEnabled: true,
      activeRegionId: "r1",
      regions: [
        {
          id: "r1",
          name: "Verse",
          kind: "verse",
          color: "#4f8cff",
          start: 1,
          end: 2,
        },
      ],
    },
    snapshot: {
      positionSecs: 1.5,
      durationSecs: 10,
      playing: false,
      ended: false,
      underruns: 0,
    },
    waveform: null,
    waveformLoading: false,
    error: null,
    recent: [],
    selection: null,
    duration: 10,
    openTrack: vi.fn(),
    hideRecent: vi.fn(),
    close: vi.fn(),
    playPause,
    seek: vi.fn(),
    setSpeed,
    setPitch: vi.fn(),
    setVolume,
    setSelection: vi.fn(),
    addRegion: vi.fn(),
    saveSelectionAsRegion: vi.fn(),
    updateRegion: vi.fn(),
    deleteRegion: vi.fn(),
    setLoopEnabled: vi.fn(),
    loopRegion: vi.fn(),
    setLoopBoundary: vi.fn(),
    clearLoop: vi.fn(),
    nudgeLoop: vi.fn(),
    selectRegion,
    selectRegionByIndex,
    setPreRoll: vi.fn(),
  };
  render(<PracticeScreen onBack={onBack} />);
  return { setSpeed, setVolume, playPause, selectRegion, selectRegionByIndex, onBack };
}

describe("PracticeScreen", () => {
  beforeEach(() => {
    context.value = {};
  });

  it("renders the track, regions and transport", () => {
    setup();
    expect(screen.getByText("Track")).toBeInTheDocument();
    expect(screen.getByText("Verse")).toBeInTheDocument();
    expect(screen.getByText("0:02 / 0:10")).toBeInTheDocument();
  });

  it("changes speed and toggles playback", async () => {
    const { setSpeed, playPause } = setup();
    await userEvent.click(screen.getByRole("button", { name: "0.5×" }));
    expect(setSpeed).toHaveBeenCalledWith(0.5);
    await userEvent.click(screen.getByRole("button", { name: "Играть" }));
    expect(playPause).toHaveBeenCalled();
  });

  it("applies a custom speed", async () => {
    const { setSpeed } = setup();
    const input = screen.getByLabelText("Своя скорость");
    await userEvent.clear(input);
    await userEvent.type(input, "0.7{Enter}");
    expect(setSpeed).toHaveBeenCalledWith(0.7);
  });

  it("changes volume", () => {
    const { setVolume } = setup();
    fireEvent.change(screen.getByLabelText("Громкость"), {
      target: { value: "40" },
    });
    expect(setVolume).toHaveBeenCalledWith(0.4);
  });

  it("handles keyboard shortcuts", () => {
    const { playPause, selectRegionByIndex } = setup();
    fireEvent.keyDown(window, { key: " " });
    expect(playPause).toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "1" });
    expect(selectRegionByIndex).toHaveBeenCalledWith(0);
  });

  it("selects a region and goes back", async () => {
    const { selectRegion, onBack } = setup();
    await userEvent.click(screen.getByText("Verse"));
    expect(selectRegion).toHaveBeenCalledWith("r1");
    await userEvent.click(screen.getByRole("button", { name: /Библиотека/ }));
    expect(onBack).toHaveBeenCalled();
  });
});
