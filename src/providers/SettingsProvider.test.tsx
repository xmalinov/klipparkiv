import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings } from "../api/settings";
import { createCommandRouter, type RoutedCommands } from "../test/tauri";
import {
  SettingsProvider,
  normalizeSettings,
  useSettings,
} from "./SettingsProvider";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const stored: Settings = {
  audioSubdir: "audio",
  videoSubdir: "video",
  concurrency: 2,
  theme: "dark",
  language: "ru",
  scanDepth: 3,
  autoscan: true,
  embedThumbnail: true,
};

let router: RoutedCommands;

function Probe() {
  const { settings, error, save, update } = useSettings();
  return (
    <div>
      <span data-testid="language">{settings?.language ?? "none"}</span>
      <span data-testid="concurrency">{settings?.concurrency ?? 0}</span>
      <span data-testid="theme">{settings?.theme ?? "none"}</span>
      <span data-testid="error">{error ?? ""}</span>
      <button
        type="button"
        onClick={() => update({ ...(settings ?? stored), theme: "light" })}
      >
        light
      </button>
      <button
        type="button"
        onClick={() => {
          void save({ ...(settings ?? stored), concurrency: 4 }).catch(() => {});
        }}
      >
        save
      </button>
    </div>
  );
}

function renderProvider() {
  return render(
    <SettingsProvider>
      <Probe />
    </SettingsProvider>,
  );
}

beforeEach(() => {
  invoke.mockReset();
  router = createCommandRouter(invoke);
});

describe("SettingsProvider", () => {
  it("reads the settings once for every consumer", async () => {
    router.on("get_settings", () => stored);

    render(
      <SettingsProvider>
        <Probe />
        <Probe />
      </SettingsProvider>,
    );

    await waitFor(() =>
      expect(screen.getAllByTestId("language")[0]).toHaveTextContent("ru"),
    );
    expect(router.count("get_settings")).toBe(1);
  });

  it("exposes null until the first load finishes", async () => {
    router.on("get_settings", () => new Promise<Settings>(() => {}));

    renderProvider();

    expect(screen.getByTestId("language")).toHaveTextContent("none");
  });

  it("reports a failed load and does not pretend the settings exist", async () => {
    router.fail("get_settings", "offline");

    renderProvider();

    await waitFor(() =>
      expect(screen.getByTestId("error")).toHaveTextContent("offline"),
    );
    expect(screen.getByTestId("language")).toHaveTextContent("none");
  });

  it("publishes saved settings without a refetch", async () => {
    const user = userEvent.setup();
    router.on("get_settings", () => stored);
    router.on("save_settings", () => ({ ...stored, concurrency: 4 }));

    renderProvider();
    await waitFor(() =>
      expect(screen.getByTestId("concurrency")).toHaveTextContent("2"),
    );

    await user.click(screen.getByRole("button", { name: "save" }));

    await waitFor(() =>
      expect(screen.getByTestId("concurrency")).toHaveTextContent("4"),
    );
    expect(router.count("get_settings")).toBe(1);
  });

  it("keeps the previous settings when saving fails", async () => {
    const user = userEvent.setup();
    router.on("get_settings", () => stored);
    router.fail("save_settings", "read-only folder");

    renderProvider();
    await waitFor(() =>
      expect(screen.getByTestId("concurrency")).toHaveTextContent("2"),
    );

    await user.click(screen.getByRole("button", { name: "save" }));

    await waitFor(() => expect(router.count("save_settings")).toBe(1));
    expect(screen.getByTestId("concurrency")).toHaveTextContent("2");
  });

  it("updates consumers in memory without writing to disk", async () => {
    const user = userEvent.setup();
    router.on("get_settings", () => stored);
    renderProvider();
    await waitFor(() =>
      expect(screen.getByTestId("theme")).toHaveTextContent("dark"),
    );

    await user.click(screen.getByRole("button", { name: "light" }));

    expect(screen.getByTestId("theme")).toHaveTextContent("light");
    expect(router.count("save_settings")).toBe(0);
  });

  it("normalizes a hostile payload instead of spreading it", () => {
    expect(normalizeSettings(null)).toEqual(stored);
    expect(
      normalizeSettings({
        ...stored,
        language: "de",
        theme: "neon",
        concurrency: 99,
        scanDepth: 0,
        audioSubdir: "  ",
        embedThumbnail: undefined,
      }),
    ).toEqual({
      ...stored,
      language: "ru",
      theme: "dark",
      concurrency: 4,
      scanDepth: 1,
      audioSubdir: "audio",
      embedThumbnail: true,
    });
  });
});
