import { describe, expect, it } from "vitest";
import { formatBytes, formatDuration, formatSpeed } from "./format";
import { formatTimecode, parseTimecode } from "./timecode";

/**
 * These cases used to be covered only through `api/*`, where three copies of
 * the formatters drifted. The values below are the contract every screen sees.
 */
describe("shared formatters", () => {
  it("formats bytes from bytes to gigabytes", () => {
    expect(formatBytes(512)).toBe("512 Б");
    expect(formatBytes(2048)).toBe("2 КБ");
    expect(formatBytes(1.5 * 1024 * 1024)).toBe("1,5 МБ");
    expect(formatBytes(2 * 1024 ** 3)).toBe("2,0 ГБ");
    expect(formatBytes(2 * 1024 ** 3, "en")).toBe("2.0 GB");
    expect(formatBytes(null)).toBe("");
  });

  it("formats durations and speeds the same way everywhere", () => {
    expect(formatDuration(213)).toBe("3:33");
    expect(formatDuration(3723)).toBe("1:02:03");
    expect(formatSpeed(1_048_576, "en")).toBe("1.0 MB/s");
  });

  it("round-trips timecodes", () => {
    expect(parseTimecode("1:30")).toBe(90);
    expect(formatTimecode(90)).toBe("00:01:30");
    expect(parseTimecode("1:60")).toBeNull();
  });
});
