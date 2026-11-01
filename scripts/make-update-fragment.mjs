#!/usr/bin/env node
// Creates one platform fragment for update.json.
// Usage:
//   node scripts/make-update-fragment.mjs \
//     --platform darwin-aarch64 --version 1.3.0 \
//     --file release/app.tar.gz --sig release/app.tar.gz.sig \
//     --url https://github.com/.../app.tar.gz \
//     [--manual-file release/app.dmg --manual-sig release/app.dmg.sig \
//      --manual-url https://github.com/.../app.dmg] \
//     --out release/update-macos-arm64.json

import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";

function arg(name, fallback = undefined) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1) return fallback;
  return process.argv[index + 1];
}

function required(name) {
  const value = arg(name);
  if (!value) {
    console.error(`missing --${name}`);
    process.exit(2);
  }
  return value;
}

function hash(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

function asset(file, signatureFile, url) {
  const size = statSync(file).size;
  const sha256 = hash(file);
  const signature = readFileSync(signatureFile, "utf8").trim();
  if (!signature) throw new Error(`empty signature: ${signatureFile}`);
  return { size, sha256, signature, url };
}

const platform = required("platform");
const version = required("version");
const file = required("file");
const sig = required("sig");
const url = required("url");
const kind = platform.startsWith("windows-") ? "exe" : "app-tar-gz";

const entry = {
  kind,
  ...asset(file, sig, url),
};

const manualFile = arg("manual-file");
const manualSig = arg("manual-sig");
const manualUrl = arg("manual-url");
if (manualFile || manualSig || manualUrl) {
  if (!manualFile || !manualSig || !manualUrl) {
    console.error("manual-file, manual-sig and manual-url must be provided together");
    process.exit(2);
  }
  entry.manual = {
    kind: "dmg",
    ...asset(manualFile, manualSig, manualUrl),
  };
}

const out = required("out");
writeFileSync(
  out,
  `${JSON.stringify({ schema: 1, version, platform, asset: entry }, null, 2)}\n`,
);
console.log(`fragment ${platform}: ${entry.size} bytes, sha256 ${entry.sha256}`);
