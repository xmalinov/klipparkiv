import { describe, expect, it } from "vitest";
import { formatDuration, formatUploadDate, formatViews } from "./tasks";

describe("task formatters", () => {
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
