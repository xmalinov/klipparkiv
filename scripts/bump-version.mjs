#!/usr/bin/env node
// Sets one release version in every file the release workflow checks:
// package.json, package-lock.json, src-tauri/Cargo.toml,
// src-tauri/tauri.conf.json, src-tauri/Cargo.lock and README.txt.
//
// Usage: node scripts/bump-version.mjs <version> [--root <dir>] [--dry-run]
//
// Every file is read and validated before anything is written, so a typo
// cannot leave a half-bumped tree.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { parseVersion, readmeVersion } from "./lib/version.mjs";

const CARGO_PACKAGE_VERSION = /^(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m;
const CARGO_PACKAGE_NAME = /^\s*name = "klipparkiv"\s*$/m;
const CARGO_LOCK_VERSION = /^\s*version = "([^"]+)"/m;
const CARGO_LOCK_LINE = /^(\s*version = ")[^"]+(")/m;
const JSON_FIRST_VERSION = /("version":\s*")[^"]*(")/;
const README_VERSION = /^(Klipparkiv\s+)\S+/m;
const LOCK_APP_NAME = /^\s*name = "klipparkiv"\s*$/m;

const FILES = {
  packageJson: "package.json",
  packageLock: "package-lock.json",
  cargoToml: "src-tauri/Cargo.toml",
  tauriConf: "src-tauri/tauri.conf.json",
  cargoLock: "src-tauri/Cargo.lock",
  readmeTxt: "README.txt",
};

function read(root, relative) {
  const file = path.join(root, relative);
  if (!existsSync(file)) throw new Error(`${relative} not found in ${root}`);
  return readFileSync(file, "utf8");
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function replaceOnce(value, pattern, replacement, label) {
  if (!pattern.test(value)) throw new Error(`cannot find the version in ${label}`);
  return value.replace(pattern, replacement);
}

function appLockVersion(cargoLock) {
  return cargoLock
    .split("[[package]]")
    .find((block) => LOCK_APP_NAME.test(block))
    ?.match(CARGO_LOCK_VERSION)?.[1];
}

function currentVersions(root) {
  const readmeTxt = read(root, FILES.readmeTxt);
  const packageLock = JSON.parse(read(root, FILES.packageLock));
  const cargoToml = read(root, FILES.cargoToml);
  const cargoLock = read(root, FILES.cargoLock);

  const versions = {
    [FILES.packageJson]: JSON.parse(read(root, FILES.packageJson)).version,
    [FILES.packageLock]: packageLock.version,
    [`${FILES.packageLock}#packages`]: packageLock.packages?.[""]?.version,
    [FILES.cargoToml]:
      /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(cargoToml)?.[1],
    [FILES.cargoLock]: appLockVersion(cargoLock),
    [FILES.tauriConf]: JSON.parse(read(root, FILES.tauriConf)).version,
    [FILES.readmeTxt]: readmeVersion(readmeTxt),
  };

  return {
    versions,
    files: {
      [FILES.packageJson]: read(root, FILES.packageJson),
      [FILES.packageLock]: read(root, FILES.packageLock),
      [FILES.cargoToml]: cargoToml,
      [FILES.tauriConf]: read(root, FILES.tauriConf),
      [FILES.cargoLock]: cargoLock,
      [FILES.readmeTxt]: readmeTxt,
    },
  };
}

/**
 * Rewrites every version file under `root` to `version`.
 * Returns `{ from, to, changed, contents }` without writing anything.
 */
export function bumpVersion(root, version) {
  if (!parseVersion(version)) {
    throw new Error(`not a semantic version: ${version}`);
  }

  const { versions, files } = currentVersions(root);
  const current = new Set(Object.values(versions));
  if (current.size !== 1) {
    throw new Error(
      `version files disagree: ${JSON.stringify(versions)}; run npm run check:version`,
    );
  }
  const from = [...current][0];

  const next = { ...files };

  next[FILES.packageJson] = replaceOnce(
    files[FILES.packageJson],
    JSON_FIRST_VERSION,
    `$1${version}$2`,
    FILES.packageJson,
  );
  next[FILES.tauriConf] = replaceOnce(
    files[FILES.tauriConf],
    JSON_FIRST_VERSION,
    `$1${version}$2`,
    FILES.tauriConf,
  );

  // package-lock.json carries the app version twice: at the root and in the
  // `packages[""]` entry. Anchoring on the previous version keeps unrelated
  // dependency entries untouched, and the count guard catches a surprise.
  const lockPattern = new RegExp(`("version":\\s*")${escapeRegExp(from)}(")`, "g");
  let lockHits = 0;
  next[FILES.packageLock] = files[FILES.packageLock].replace(
    lockPattern,
    (_, head, tail) => {
      lockHits += 1;
      return `${head}${version}${tail}`;
    },
  );
  if (lockHits !== 2) {
    throw new Error(
      `${FILES.packageLock}: expected 2 entries with version ${from}, found ${lockHits}`,
    );
  }

  next[FILES.cargoToml] = replaceOnce(
    files[FILES.cargoToml],
    CARGO_PACKAGE_VERSION,
    `$1${version}$2`,
    FILES.cargoToml,
  );

  let appBlock = false;
  next[FILES.cargoLock] = files[FILES.cargoLock]
    .split("[[package]]")
    .map((block) => {
      if (!CARGO_PACKAGE_NAME.test(block)) return block;
      appBlock = true;
      return block.replace(CARGO_LOCK_LINE, `$1${version}$2`);
    })
    .join("[[package]]");
  if (!appBlock) throw new Error(`${FILES.cargoLock}: no klipparkiv package entry`);

  next[FILES.readmeTxt] = replaceOnce(
    files[FILES.readmeTxt],
    README_VERSION,
    `$1${version}`,
    FILES.readmeTxt,
  );

  // Verify every rewritten file before returning, so a broken replacement
  // never reaches the disk.
  const written = {
    [FILES.packageJson]: JSON.parse(next[FILES.packageJson]).version,
    [FILES.packageLock]: JSON.parse(next[FILES.packageLock]).version,
    [`${FILES.packageLock}#packages`]: JSON.parse(next[FILES.packageLock]).packages?.[""]
      ?.version,
    [FILES.cargoToml]:
      /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(next[FILES.cargoToml])?.[1],
    [FILES.cargoLock]: appLockVersion(next[FILES.cargoLock]),
    [FILES.tauriConf]: JSON.parse(next[FILES.tauriConf]).version,
    [FILES.readmeTxt]: readmeVersion(next[FILES.readmeTxt]),
  };
  for (const [file, value] of Object.entries(written)) {
    if (value !== version) {
      throw new Error(`${file}: expected ${version} after the rewrite, found ${value}`);
    }
  }

  const changed = Object.keys(files).filter((file) => next[file] !== files[file]);
  return { from, to: version, changed, contents: next };
}

function parseArgs(argv) {
  const args = { version: undefined, root: process.cwd(), dryRun: false };
  const positional = [];
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--root") {
      const dir = argv[index + 1];
      if (!dir) throw new Error("--root needs a directory");
      args.root = path.resolve(dir);
      index += 1;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value.startsWith("--")) throw new Error(`unknown option: ${value}`);
    positional.push(value);
  }
  if (positional.length !== 1) {
    throw new Error("usage: node scripts/bump-version.mjs <version> [--root <dir>] [--dry-run]");
  }
  args.version = positional[0];
  return args;
}

function main(argv) {
  const { version, root, dryRun } = parseArgs(argv);
  const result = bumpVersion(root, version);

  if (result.changed.length === 0) {
    console.log(`already at ${result.to}`);
    return;
  }

  if (!dryRun) {
    for (const file of result.changed) {
      writeFileSync(path.join(root, file), result.contents[file]);
    }
  }

  console.log(`${dryRun ? "would update" : "updated"} ${result.from} -> ${result.to}:`);
  for (const file of result.changed) console.log(`  ${file}`);

  if (!dryRun) {
    const tag = `v${result.to}`;
    console.log("");
    console.log("next:");
    console.log(`  npm run check:version`);
    console.log(`  git commit -am "chore: release ${result.to}"`);
    console.log(`  git tag -a ${tag} -m "Klipparkiv ${result.to}"`);
    console.log(`  git push origin HEAD ${tag}`);
  }
}

const invokedDirectly =
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;

if (invokedDirectly) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
