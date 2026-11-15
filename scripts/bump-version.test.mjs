import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";

import { bumpVersion } from "./bump-version.mjs";
import { makeReleaseTree } from "./test/fixtures.mjs";

const SCRIPT = path.resolve("scripts/bump-version.mjs");
const created = [];

function tree(version = "1.8.0") {
  const root = makeReleaseTree(version);
  created.push(root);
  return root;
}

function run(args, root) {
  return spawnSync(process.execPath, [SCRIPT, ...args], {
    cwd: root,
    encoding: "utf8",
  });
}

afterEach(() => {
  while (created.length > 0) rmSync(created.pop(), { recursive: true, force: true });
});

describe("bumpVersion", () => {
  it("moves every version file to a test build version", () => {
    const root = tree("1.8.0");

    const result = bumpVersion(root, "1.9.0-beta.1");

    expect(result.from).toBe("1.8.0");
    expect(result.changed).toHaveLength(6);
    expect(JSON.parse(result.contents["package.json"]).version).toBe("1.9.0-beta.1");
    expect(JSON.parse(result.contents["src-tauri/tauri.conf.json"]).version).toBe(
      "1.9.0-beta.1",
    );
    expect(result.contents["src-tauri/Cargo.toml"]).toContain(
      'name = "klipparkiv"\nversion = "1.9.0-beta.1"',
    );
    expect(result.contents["README.txt"].split("\n")[0]).toBe(
      "Klipparkiv 1.9.0-beta.1 - portable build for Windows",
    );

    const lock = JSON.parse(result.contents["package-lock.json"]);
    expect(lock.version).toBe("1.9.0-beta.1");
    expect(lock.packages[""].version).toBe("1.9.0-beta.1");
  });

  it("leaves dependency versions alone", () => {
    const root = tree("1.8.0");

    const result = bumpVersion(root, "1.9.0");
    const lock = JSON.parse(result.contents["package-lock.json"]);

    expect(lock.packages["node_modules/react"].version).toBe("19.1.0");
    // serde shares the old app version in the fixture; only the app entry moves.
    expect(result.contents["src-tauri/Cargo.lock"]).toContain(
      'name = "serde"\nversion = "1.8.0"',
    );
    expect(result.contents["src-tauri/Cargo.lock"]).toContain(
      'name = "klipparkiv"\nversion = "1.9.0"',
    );
  });

  it("promotes a test build back to a stable version", () => {
    const root = tree("1.9.0-beta.1");

    const result = bumpVersion(root, "1.9.0");

    expect(result.from).toBe("1.9.0-beta.1");
    expect(JSON.parse(result.contents["package.json"]).version).toBe("1.9.0");
    expect(result.contents["README.txt"].split("\n")[0]).toBe(
      "Klipparkiv 1.9.0 - portable build for Windows",
    );
  });

  it("is a no-op when the tree already has the requested version", () => {
    const root = tree("1.8.0");

    expect(bumpVersion(root, "1.8.0").changed).toEqual([]);
  });

  it("refuses a tree whose version files disagree", () => {
    const root = tree("1.8.0");
    writeFileSync(
      path.join(root, "README.txt"),
      "Klipparkiv 1.7.0 - portable build for Windows\n",
    );

    expect(() => bumpVersion(root, "1.9.0")).toThrow(/version files disagree/);
  });

  it("refuses a version that is not semantic", () => {
    const root = tree("1.8.0");

    expect(() => bumpVersion(root, "v1.9.0")).toThrow(/not a semantic version/);
  });
});

describe("bump-version CLI", () => {
  it("writes the files and prints the tag command", () => {
    const root = tree("1.8.0");

    const result = run(["1.9.0-beta.1"], root);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("updated 1.8.0 -> 1.9.0-beta.1");
    expect(result.stdout).toContain("git tag -a v1.9.0-beta.1");
    const readme = readFileSync(path.join(root, "README.txt"), "utf8");
    expect(readme.startsWith("Klipparkiv 1.9.0-beta.1")).toBe(true);
  });

  it("does not touch the files with --dry-run", () => {
    const root = tree("1.8.0");

    const result = run(["1.9.0", "--dry-run"], root);

    expect(result.status).toBe(0);
    expect(result.stdout).toContain("would update 1.8.0 -> 1.9.0");
    expect(readFileSync(path.join(root, "README.txt"), "utf8")).toContain("Klipparkiv 1.8.0");
  });

  it("fails with a usage message when the version is missing", () => {
    const root = tree("1.8.0");

    const result = run([], root);

    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain("usage: node scripts/bump-version.mjs");
  });
});
