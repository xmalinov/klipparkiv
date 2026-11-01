import { describe, expect, it } from "vitest";
import { formatBytes, formatFileCount, formatItemDate, formatResolution } from "./library";

describe("library formatters", () => {
  it("formats byte sizes", () => {
    expect(formatBytes(8_400_000)).toBe("8,0 МБ");
    expect(formatBytes(240_000_000)).toBe("228,9 МБ");
    expect(formatBytes(2_200_000_000)).toBe("2,0 ГБ");
    expect(formatBytes(500)).toBe("1 КБ");
  });

  it("pluralizes file counts", () => {
    expect(formatFileCount(1)).toBe("1 файл");
    expect(formatFileCount(2)).toBe("2 файла");
    expect(formatFileCount(5)).toBe("5 файлов");
    expect(formatFileCount(21)).toBe("21 файл");
    expect(formatFileCount(11)).toBe("11 файлов");
  });

  it("formats dates as day.month", () => {
    expect(formatItemDate(1_760_000_000)).toMatch(/^\d{2}\.\d{2}$/);
    expect(formatItemDate(0)).toBe("");
  });

  it("formats resolution", () => {
    expect(formatResolution(1920, 1080)).toBe("1080p");
    expect(formatResolution(3840, 2160)).toBe("4K");
    expect(formatResolution(null, null)).toBe("");
  });
});
