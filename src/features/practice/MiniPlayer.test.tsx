import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { LibraryItem } from "../../api/library";
import type { PracticeState } from "../../api/practice";
import { DEFAULT_PRACTICE_STATE } from "../../api/practice";
import MiniPlayer from "./MiniPlayer";

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
          "practice.title": "Practice",
          "practice.play": "Play",
          "practice.pause": "Pause",
          "practice.mini.open": "Open",
        } as Record<string, string>
      )[key] ?? key,
  }),
}));

vi.mock("./PracticeProvider", () => ({
  usePractice: () => practice.value,
}));

const TRACK: LibraryItem = {
  id: "t",
  path: "C:\\audio\\a.mp3",
  kind: "audio",
  title: "Song",
  channel: null,
  durationSecs: 200,
  width: null,
  height: null,
  sizeBytes: 1,
  mtime: 1,
  thumbPath: null,
};

function renderMini(overrides: Record<string, unknown> = {}) {
  const playPause = vi.fn();
  practice.value = {
    track: TRACK,
    state: {
      ...DEFAULT_PRACTICE_STATE,
      speed: 1.25,
      activeRegionId: "r1",
      regions: [
        { id: "r1", name: "Verse", kind: "verse", color: "#ff0000", start: 0, end: 8 },
      ],
    } satisfies Partial<PracticeState>,
    snapshot: {
      positionSecs: 65,
      durationSecs: 200,
      playing: false,
      ended: false,
      underruns: 0,
    },
    playPause,
    ...overrides,
  };
  const onOpen = vi.fn();
  const view = render(<MiniPlayer onOpen={onOpen} />);
  return { ...view, playPause, onOpen };
}

describe("MiniPlayer", () => {
  it("renders nothing without an open track", () => {
    practice.value = { track: null, playPause: vi.fn() };
    const { container } = render(<MiniPlayer onOpen={vi.fn()} />);
    expect(container.firstChild).toBeNull();
  });

  it("shows the title, time, speed and active region", () => {
    renderMini();
    expect(screen.getByRole("button", { name: "Song" })).toBeInTheDocument();
    expect(screen.getByText("1:05 / 3:20")).toBeInTheDocument();
    expect(screen.getByText("1.25×")).toBeInTheDocument();
    expect(screen.getByText("Verse")).toBeInTheDocument();
  });

  it("toggles playback and opens the player", () => {
    const { playPause, onOpen } = renderMini();

    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(playPause).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Song" }));
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(onOpen).toHaveBeenCalledTimes(2);
  });

  it("shows a pause button while playing", () => {
    renderMini({
      snapshot: {
        positionSecs: 10,
        durationSecs: 200,
        playing: true,
        ended: false,
        underruns: 0,
      },
    });
    expect(screen.getByRole("button", { name: "Pause" })).toBeInTheDocument();
  });
});
