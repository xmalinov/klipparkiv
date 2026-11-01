import { describe, expect, it } from "vitest";
import { formatVersion } from "./appInfo";

describe("formatVersion", () => {
  it("returns the version when present", () => {
    expect(formatVersion("2026.08.19")).toBe("2026.08.19");
  });

  it("returns a placeholder when missing", () => {
    expect(formatVersion(null)).toBe("не найден");
  });
});
