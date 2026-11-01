#!/usr/bin/env node
// Merges platform fragments into the signed update.json published on GitHub.
// Usage:
//   node scripts/merge-update-manifest.mjs --out release/update.json \
//     --version 1.3.0 [--min-version 1.3.0] [--notes "..."] \
//     release/update-windows.json release/update-macos-x64.json ...

import { readFileSync, writeFileSync } from "node:fs";

function arg(name, fallback = undefined) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1) return fallback;
  return process.argv[index + 1];
}

const out = arg("out");
const version = arg("version");
if (!out || !version) {
  console.error("usage: merge-update-manifest.mjs --out update.json --version 1.3.0 fragments...");
  process.exit(2);
}

const args = process.argv.slice(2);
const fragments = [];
for (let index = 0; index < args.length; index += 1) {
  const value = args[index];
  if (value.startsWith("--")) {
    index += 1;
    continue;
  }
  fragments.push(value);
}
if (fragments.length === 0) {
  console.error("no fragments");
  process.exit(2);
}

const platforms = {};
for (const file of fragments) {
  const fragment = JSON.parse(readFileSync(file, "utf8"));
  if (fragment.version !== version) {
    throw new Error(`${file}: version ${fragment.version}, expected ${version}`);
  }
  if (platforms[fragment.platform]) {
    throw new Error(`${file}: duplicate platform ${fragment.platform}`);
  }
  platforms[fragment.platform] = fragment.asset;
}

const required = (arg("require") ?? process.env.REQUIRE_PLATFORMS ?? "")
  .split(",")
  .map((value) => value.trim())
  .filter(Boolean);
for (const platform of required) {
  if (!platforms[platform]) {
    throw new Error(`missing platform ${platform}`);
  }
}

for (const [platform, asset] of Object.entries(platforms)) {
  if (!/^[a-f0-9]{64}$/.test(asset.sha256)) {
    throw new Error(`${platform}: invalid sha256`);
  }
  if (!Number.isInteger(asset.size) || asset.size <= 0) {
    throw new Error(`${platform}: invalid size`);
  }
  if (!asset.signature || !asset.url) {
    throw new Error(`${platform}: missing signature or url`);
  }
}

const manifest = {
  schema: 1,
  channel: "stable",
  version,
  pubDate: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  notes: arg("notes", process.env.RELEASE_NOTES ?? ""),
  ...(arg("min-version") ? { minVersion: arg("min-version") } : {}),
  platforms,
};

writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`manifest: ${out}`);
for (const [platform, asset] of Object.entries(platforms)) {
  console.log(`  ${platform}: ${asset.size} bytes`);
}
