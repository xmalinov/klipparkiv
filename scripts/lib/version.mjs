// Shared version helpers for the release scripts.
//
// A release version is either `X.Y.Z` (stable) or `X.Y.Z-<prerelease>`
// (a test build, e.g. `1.9.0-beta.1`). The prerelease suffix is what keeps a
// test build out of the stable update channel: GitHub resolves
// `releases/latest` to the newest non-prerelease release only, and the app
// ignores any version that is not newer than the running one.

export const VERSION_PATTERN = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;

/** Parses `1.9.0` or `1.9.0-beta.1`; returns null for anything else. */
export function parseVersion(value) {
  const match = VERSION_PATTERN.exec(String(value ?? "").trim());
  if (!match) return null;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4] ?? null,
  };
}

/** True for `1.9.0-beta.1`, `2.0.0-rc.1` and similar test builds. */
export function isPrerelease(value) {
  const parsed = parseVersion(value);
  return parsed !== null && parsed.prerelease !== null;
}

/** Update channel a version belongs to. The app reads only the stable channel. */
export function channelFor(value) {
  return isPrerelease(value) ? "beta" : "stable";
}

/**
 * Reads the version from the `Klipparkiv <version> - portable build ...` header
 * of README.txt. Returns null when the line is missing or malformed.
 */
export function readmeVersion(text) {
  const match = /^Klipparkiv\s+(\S+)/m.exec(text);
  if (!match) return null;
  return parseVersion(match[1]) ? match[1] : null;
}
