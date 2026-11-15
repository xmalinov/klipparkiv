import { describe, expect, it } from "vitest";

import {
  channelFor,
  isPrerelease,
  parseVersion,
  readmeVersion,
} from "./version.mjs";

describe("parseVersion", () => {
  it("accepts a stable release version", () => {
    expect(parseVersion("1.8.0")).toEqual({
      major: 1,
      minor: 8,
      patch: 0,
      prerelease: null,
    });
  });

  it("accepts a test build version", () => {
    expect(parseVersion("1.9.0-beta.1")?.prerelease).toBe("beta.1");
  });

  it.each(["v1.9.0", "1.9", "1.9.0.1", "", "latest", "1.9.0-"])(
    "rejects %j",
    (value) => {
      expect(parseVersion(value)).toBeNull();
    },
  );
});

describe("isPrerelease", () => {
  it("separates test builds from stable releases", () => {
    expect(isPrerelease("1.9.0-beta.1")).toBe(true);
    expect(isPrerelease("2.0.0-rc.1")).toBe(true);
    expect(isPrerelease("1.9.0")).toBe(false);
    expect(isPrerelease("nonsense")).toBe(false);
  });
});

describe("channelFor", () => {
  it("maps test builds to the beta channel", () => {
    expect(channelFor("1.9.0-beta.1")).toBe("beta");
    expect(channelFor("1.9.0")).toBe("stable");
  });
});

describe("readmeVersion", () => {
  it("reads the header of a stable README.txt", () => {
    expect(readmeVersion("Klipparkiv 1.8.0 - portable build for Windows\n")).toBe("1.8.0");
  });

  it("keeps the prerelease suffix", () => {
    expect(
      readmeVersion("Klipparkiv 1.9.0-beta.1 - portable build for Windows\n"),
    ).toBe("1.9.0-beta.1");
  });

  it("returns null when the header is missing or malformed", () => {
    expect(readmeVersion("Klipparkiv - portable build\n")).toBeNull();
    expect(readmeVersion("Klipparkiv 1.8 - portable build\n")).toBeNull();
    expect(readmeVersion("Something else\n")).toBeNull();
  });
});
