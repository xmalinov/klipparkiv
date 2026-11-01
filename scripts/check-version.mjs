#!/usr/bin/env node
// Ensures package.json, Cargo.toml, tauri.conf.json and the release tag agree.
// Usage: node scripts/check-version.mjs [--tag v1.3.0]

import { readFileSync } from "node:fs";

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
const cargo = readFileSync("src-tauri/Cargo.toml", "utf8");
const tauri = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const lock = readFileSync("src-tauri/Cargo.lock", "utf8");

const match = cargo.match(/^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
if (!match) {
  console.error("cannot find [package].version in Cargo.toml");
  process.exit(1);
}

// Cargo records the app package in the lockfile as a regular package entry.
// Reading it directly catches a Cargo.toml bump that was not followed by
// `cargo update -p klipparkiv`, which CI would otherwise fix silently.
const appLock = lock
  .split("[[package]]")
  .find((block) => /^\s*name = "klipparkiv"\s*$/m.test(block));
const lockVersion = appLock?.match(/^\s*version = "([^"]+)"/m)?.[1];
if (!lockVersion) {
  console.error("cannot find the klipparkiv package in src-tauri/Cargo.lock");
  process.exit(1);
}

const versions = {
  "package.json": pkg.version,
  "Cargo.toml": match[1],
  "Cargo.lock": lockVersion,
  "tauri.conf.json": tauri.version,
};
const unique = new Set(Object.values(versions));
if (unique.size !== 1) {
  console.error("version mismatch:", versions);
  process.exit(1);
}

const version = pkg.version;
const tagIndex = process.argv.indexOf("--tag");
const tag = tagIndex === -1 ? process.env.GITHUB_REF_NAME : process.argv[tagIndex + 1];
if (tag && tag.startsWith("v") && tag !== `v${version}`) {
  console.error(`tag ${tag} does not match version ${version}`);
  process.exit(1);
}

console.log(`version ${version} is consistent${tag ? ` with tag ${tag}` : ""}`);
