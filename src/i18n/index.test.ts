import { act, render, screen, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getSettings, type Settings } from "../api/settings";
import { I18nProvider, en, ru, translate, useI18n } from "./index";

vi.mock("../api/settings", () => ({ getSettings: vi.fn() }));

const mockedGetSettings = vi.mocked(getSettings);

function settings(language: "ru" | "en"): Settings {
  return {
    audioSubdir: "audio",
    videoSubdir: "video",
    concurrency: 2,
    theme: "dark",
    language,
    scanDepth: 3,
    autoscan: true,
    embedThumbnail: true,
  };
}

function renderProbe(initialLanguage?: "ru" | "en") {
  let api!: ReturnType<typeof useI18n>;
  function Probe() {
    api = useI18n();
    return createElement("span", { "data-testid": "text" }, api.t("download.submit"));
  }
  render(
    createElement(I18nProvider, {
      children: createElement(Probe),
      ...(initialLanguage ? { initialLanguage } : {}),
    }),
  );
  return {
    api: () => api,
  };
}

describe("i18n", () => {
  beforeEach(() => {
    mockedGetSettings.mockReset();
    mockedGetSettings.mockResolvedValue(settings("ru"));
    document.documentElement.lang = "";
  });

  it("has the same keys in both languages", () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(ru).sort());
  });

  it("translates and interpolates messages", () => {
    expect(translate("ru", "download.submit")).toBe("Скачать");
    expect(translate("en", "download.submit")).toBe("Download");
    expect(
      translate("en", "app.missingBinaries", {
        names: "yt-dlp, ffmpeg",
        dir: "C:\\apps\\klipparkiv\\bin",
      }),
    ).toBe("Not found in C:\\apps\\klipparkiv\\bin: yt-dlp, ffmpeg");
  });

  it("falls back to the Russian template for a missing parameter", () => {
    expect(translate("en", "settings.updated")).toBe("yt-dlp updated to {version}");
  });

  it("defaults to Russian and syncs the document language", async () => {
    const probe = renderProbe();

    expect(screen.getByTestId("text")).toHaveTextContent("Скачать");
    expect(probe.api().language).toBe("ru");
    await waitFor(() => expect(document.documentElement.lang).toBe("ru"));
  });

  it("adopts the saved language from the settings", async () => {
    mockedGetSettings.mockResolvedValue(settings("en"));
    const probe = renderProbe();

    await waitFor(() =>
      expect(screen.getByTestId("text")).toHaveTextContent("Download"),
    );
    expect(probe.api().language).toBe("en");
    expect(document.documentElement.lang).toBe("en");
  });

  it("switches language at runtime", async () => {
    const probe = renderProbe();
    await waitFor(() => expect(mockedGetSettings).toHaveBeenCalled());
    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      probe.api().setLanguage("en");
    });

    expect(screen.getByTestId("text")).toHaveTextContent("Download");
    expect(document.documentElement.lang).toBe("en");
  });

  it("keeps Russian when settings fail or hold an unknown language", async () => {
    mockedGetSettings.mockRejectedValueOnce(new Error("offline"));
    const failed = renderProbe();
    expect(failed.api().language).toBe("ru");

    mockedGetSettings.mockResolvedValueOnce({
      ...settings("ru"),
      language: "de" as "ru",
    });
    const invalid = renderProbe();
    await waitFor(() => expect(mockedGetSettings).toHaveBeenCalledTimes(2));
    expect(invalid.api().language).toBe("ru");
  });

  it("honours an explicit initial language until settings load", () => {
    mockedGetSettings.mockReturnValue(new Promise<Settings>(() => {}));
    const probe = renderProbe("en");

    expect(probe.api().language).toBe("en");
    expect(screen.getByTestId("text")).toHaveTextContent("Download");
  });
});
