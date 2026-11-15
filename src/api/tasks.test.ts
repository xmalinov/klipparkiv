import { describe, expect, it } from "vitest";
import {
  applyProgress,
  formatDuration,
  formatEta,
  formatSpeed,
  formatTimecode,
  formatUploadDate,
  formatViews,
  parseTimecode,
  stateLabel,
  upsertTask,
  type TaskSnapshot,
} from "./tasks";

/**
 * The formatters and the timecode parser live in `api/tasks.ts` today and are
 * scheduled to move to `src/lib/`. These tests pin the contract so the move
 * stays behaviour-preserving.
 */
function makeTask(overrides: Partial<TaskSnapshot> = {}): TaskSnapshot {
  return {
    id: "1",
    url: "https://example.com",
    title: null,
    mode: "audio",
    clip: false,
    state: "queued",
    percent: 0,
    speedBps: null,
    etaSecs: null,
    error: null,
    playlistIndex: null,
    playlistCount: null,
    outputPath: null,
    ...overrides,
  };
}

describe("task list helpers", () => {
  it("appends a new task and replaces a known one in place", () => {
    const first = makeTask({ id: "1" });
    const second = makeTask({ id: "2" });

    const appended = upsertTask([first], second);
    expect(appended.map((task) => task.id)).toEqual(["1", "2"]);

    const replaced = upsertTask(appended, { ...first, percent: 30 });
    expect(replaced.map((task) => task.id)).toEqual(["1", "2"]);
    expect(replaced[0].percent).toBe(30);
    // The input array is never mutated: React state relies on new references.
    expect(appended[0].percent).toBe(0);
    expect(replaced).not.toBe(appended);
  });
});

describe("timecodes", () => {
  it("parses SS, MM:SS and HH:MM:SS", () => {
    expect(parseTimecode("90")).toBe(90);
    expect(parseTimecode("1:30")).toBe(90);
    expect(parseTimecode("00:01:30")).toBe(90);
    expect(parseTimecode("1:02:03")).toBe(3723);
    expect(parseTimecode("  1:30  ")).toBe(90);
  });

  it("rejects malformed timecodes instead of guessing", () => {
    expect(parseTimecode("")).toBeNull();
    expect(parseTimecode("   ")).toBeNull();
    expect(parseTimecode("1::30")).toBeNull();
    expect(parseTimecode("1:2:3:4")).toBeNull();
    expect(parseTimecode("1:60")).toBeNull();
    expect(parseTimecode("1:2:60")).toBeNull();
    expect(parseTimecode("1:99:00")).toBeNull();
    expect(parseTimecode("a:30")).toBeNull();
    expect(parseTimecode("-5")).toBeNull();
  });

  it("formats seconds back to HH:MM:SS", () => {
    expect(formatTimecode(0)).toBe("00:00:00");
    expect(formatTimecode(90)).toBe("00:01:30");
    expect(formatTimecode(3723)).toBe("01:02:03");
    expect(formatTimecode(-10)).toBe("00:00:00");
    // 59.6 rounds up into the next minute, not into "00:00:60".
    expect(formatTimecode(59.6)).toBe("00:01:00");
  });
});

describe("task labels and units", () => {
  it("labels every state and shows the percentage while running", () => {
    expect(stateLabel(makeTask({ state: "queued" }))).toBe("В очереди");
    expect(stateLabel(makeTask({ state: "running", percent: 42.4 }))).toBe("42%");
    expect(stateLabel(makeTask({ state: "done" }))).toBe("Готово");
    expect(stateLabel(makeTask({ state: "skipped" }))).toBe("Уже было");
    expect(stateLabel(makeTask({ state: "partial" }))).toBe("Частично");
    expect(stateLabel(makeTask({ state: "failed" }))).toBe("Ошибка");
    expect(stateLabel(makeTask({ state: "canceled" }))).toBe("Отменено");

    expect(stateLabel(makeTask({ state: "done" }), "en")).toBe("Done");
    expect(stateLabel(makeTask({ state: "skipped" }), "en")).toBe("Already there");
  });

  it("formats transfer speed with the locale separator", () => {
    expect(formatSpeed(null)).toBe("—");
    expect(formatSpeed(1_048_576)).toBe("1,0 МБ/с");
    expect(formatSpeed(2_048)).toBe("2 КБ/с");
    expect(formatSpeed(500)).toBe("500 Б/с");
    expect(formatSpeed(1_048_576, "en")).toBe("1.0 MB/s");
  });

  it("formats the remaining time", () => {
    expect(formatEta(null)).toBe("");
    expect(formatEta(-1)).toBe("");
    expect(formatEta(0)).toBe("0:00");
    expect(formatEta(90)).toBe("1:30");
    expect(formatEta(3600)).toBe("60:00");
  });
});

describe("task formatters", () => {
  it("keeps the last known speed when a progress event omits it", () => {
    const task: TaskSnapshot = {
      id: "1",
      url: "https://example.com",
      title: null,
      mode: "audio",
      clip: false,
      state: "running",
      percent: 10,
      speedBps: 2_000_000,
      etaSecs: 30,
      error: null,
      playlistIndex: null,
      playlistCount: null,
      outputPath: null,
    };
    const [next] = applyProgress([task], {
      id: "1",
      percent: 20,
      speedBps: null,
      etaSecs: null,
      playlistIndex: null,
      playlistCount: null,
    });
    expect(next.percent).toBe(20);
    expect(next.speedBps).toBe(2_000_000);
    expect(next.etaSecs).toBe(30);
  });

  it("formats durations", () => {
    expect(formatDuration(213)).toBe("3:33");
    expect(formatDuration(3723)).toBe("1:02:03");
    expect(formatDuration(null)).toBe("");
  });

  it("formats view counts", () => {
    expect(formatViews(1_200_000_000)).toBe("1,2 млрд просмотров");
    expect(formatViews(2_400_000)).toBe("2,4 млн просмотров");
    expect(formatViews(15_400)).toBe("15 тыс. просмотров");
    expect(formatViews(42)).toBe("42 просмотров");
    expect(formatViews(1_200_000_000, "en")).toBe("1.2B views");
    expect(formatViews(2_400_000, "en")).toBe("2.4M views");
    expect(formatViews(15_400, "en")).toBe("15K views");
    expect(formatViews(1, "en")).toBe("1 view");
    expect(formatViews(null)).toBe("");
  });

  it("formats upload dates", () => {
    expect(formatUploadDate("20091025")).toBe("25.10.2009");
    expect(formatUploadDate("20091025", "en")).toBe("10/25/2009");
    expect(formatUploadDate(null)).toBe("");
    expect(formatUploadDate("unknown")).toBe("unknown");
  });
});
