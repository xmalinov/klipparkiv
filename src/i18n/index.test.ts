import { describe, expect, it } from "vitest";
import { en, ru, translate } from "./index";

describe("i18n", () => {
  it("has the same keys in both languages", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(ru).sort());
  });

  it("translates and interpolates messages", () => {
    expect(translate("ru", "download.submit")).toBe("Скачать");
    expect(translate("en", "download.submit")).toBe("Download");
    expect(
      translate("en", "app.missingBinaries", { names: "yt-dlp, ffmpeg" }),
    ).toBe("Not found in bin\\: yt-dlp, ffmpeg");
  });

  it("falls back to the Russian template for a missing parameter", () => {
    expect(translate("en", "settings.updated")).toBe("yt-dlp updated to {version}");
  });
});
