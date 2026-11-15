import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { CHANGELOG_FIXTURE, makeReleaseTree } from "./test/fixtures.mjs";

const created = [];

function tree(version = "1.8.0") {
  const root = makeReleaseTree(version);
  created.push(root);
  return root;
}

function run(script, args, root) {
  return spawnSync(process.execPath, [path.resolve("scripts", script), ...args], {
    cwd: root,
    encoding: "utf8",
  });
}

afterEach(() => {
  while (created.length > 0) rmSync(created.pop(), { recursive: true, force: true });
});

describe("check-version", () => {
  it("accepts a consistent stable tree", () => {
    const result = run("check-version.mjs", [], tree("1.8.0"));

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("version 1.8.0 is consistent (stable channel)");
  });

  it("accepts a consistent test build", () => {
    const result = run("check-version.mjs", ["--tag", "v1.9.0-beta.1"], tree("1.9.0-beta.1"));

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("version 1.9.0-beta.1 is consistent (beta channel)");
  });

  it("rejects a README.txt that lost the prerelease suffix", () => {
    const root = tree("1.9.0-beta.1");
    writeFileSync(path.join(root, "README.txt"), "Klipparkiv 1.9.0 - portable build\n");

    const result = run("check-version.mjs", [], root);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("version mismatch");
  });

  it("rejects a tag that does not match the version", () => {
    const result = run("check-version.mjs", ["--tag", "v1.9.0"], tree("1.9.0-beta.1"));

    expect(result.status).toBe(1);
    expect(result.stderr).toContain("does not match version 1.9.0-beta.1");
  });
});

describe("release-notes", () => {
  function changelogTree() {
    const root = tree("1.8.0");
    writeFileSync(path.join(root, "CHANGELOG.md"), CHANGELOG_FIXTURE);
    return root;
  }

  it("marks a test build as a prerelease", () => {
    const root = changelogTree();

    const result = run("release-notes.mjs", ["1.9.0-beta.1", "notes.md"], root);

    expect(result.status).toBe(0);
    const notes = readFileSync(path.join(root, "notes.md"), "utf8");
    expect(notes).toContain("Test build");
    expect(notes).toContain("not a stable");
    expect(notes).toContain("Test build notes.");
    expect(notes).toContain("Изменения тестовой сборки.");
  });

  it("keeps the stable notes free of the test-build banner", () => {
    const root = changelogTree();

    const result = run("release-notes.mjs", ["1.8.0", "notes.md"], root);

    expect(result.status).toBe(0);
    const notes = readFileSync(path.join(root, "notes.md"), "utf8");
    expect(notes).not.toContain("Test build");
    expect(notes).toContain("Stable notes.");
  });

  it("fails when the changelog has no section for the version", () => {
    const root = changelogTree();

    const result = run("release-notes.mjs", ["2.0.0", "notes.md"], root);

    expect(result.status).toBe(1);
    expect(result.stderr).toContain('no "### [2.0.0]" section');
  });
});

describe("merge-update-manifest", () => {
  function fragment(version, platform) {
    return {
      schema: 1,
      version,
      platform,
      asset: {
        kind: "exe",
        url: `https://example.com/${platform}`,
        size: 1024,
        sha256: "a".repeat(64),
        signature: "signature",
      },
    };
  }

  function writeFragments(root, version) {
    writeFileSync(
      path.join(root, "windows.json"),
      `${JSON.stringify(fragment(version, "windows-x86_64"))}\n`,
    );
    writeFileSync(
      path.join(root, "mac.json"),
      `${JSON.stringify(fragment(version, "darwin-aarch64"))}\n`,
    );
  }

  it("marks a test build manifest as beta", () => {
    const root = tree("1.9.0-beta.1");
    writeFragments(root, "1.9.0-beta.1");

    const result = run(
      "merge-update-manifest.mjs",
      [
        "--out",
        "update.json",
        "--version",
        "1.9.0-beta.1",
        "--require",
        "windows-x86_64,darwin-aarch64",
        "windows.json",
        "mac.json",
      ],
      root,
    );

    expect(result.status).toBe(0);
    const manifest = JSON.parse(readFileSync(path.join(root, "update.json"), "utf8"));
    expect(manifest.channel).toBe("beta");
    expect(manifest.version).toBe("1.9.0-beta.1");
    expect(Object.keys(manifest.platforms).sort()).toEqual([
      "darwin-aarch64",
      "windows-x86_64",
    ]);
  });

  it("keeps a stable version on the stable channel", () => {
    const root = tree("1.9.0");
    writeFragments(root, "1.9.0");

    const result = run(
      "merge-update-manifest.mjs",
      ["--out", "update.json", "--version", "1.9.0", "windows.json", "mac.json"],
      root,
    );

    expect(result.status).toBe(0);
    expect(JSON.parse(readFileSync(path.join(root, "update.json"), "utf8")).channel).toBe(
      "stable",
    );
  });

  it("fails when a required platform is missing", () => {
    const root = tree("1.9.0");
    writeFragments(root, "1.9.0");

    const result = run(
      "merge-update-manifest.mjs",
      [
        "--out",
        "update.json",
        "--version",
        "1.9.0",
        "--require",
        "windows-x86_64,darwin-x86_64",
        "windows.json",
      ],
      root,
    );

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("missing platform darwin-x86_64");
  });
});
