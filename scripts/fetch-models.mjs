#!/usr/bin/env node
// Downloads the pinned Beat This! small model and mel front-end into
// src-tauri/resources/bin/models. These files are intentionally not tracked by git.
// Usage: node scripts/fetch-models.mjs

import { createHash } from "node:crypto";
import { createWriteStream, mkdirSync, readFileSync, renameSync, unlinkSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { pipeline } from "node:stream/promises";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const dest = join(root, "src-tauri", "resources", "bin", "models");
mkdirSync(dest, { recursive: true });

const files = [
  {
    name: "beat_this_small.onnx",
    url: "https://github.com/danigb/beat-this-rs/raw/main/models/beat_this_small.onnx",
    sha256: "a5f8d39d989f31859454ba27afe61c5317ca95e4d9373e6853e5361b8937172f",
  },
  {
    name: "mel_spectrogram.onnx",
    url: "https://github.com/danigb/beat-this-rs/raw/main/models/mel_spectrogram.onnx",
    sha256: "fdd59e65c515331308e4c8841edf99972deca646bdf6197744c2a5b7755e3de9",
  },
];

for (const file of files) {
  const target = join(dest, file.name);
  console.log(`download ${file.url}`);
  const response = await fetch(file.url, { redirect: "follow" });
  if (!response.ok || !response.body) {
    throw new Error(`${file.url}: HTTP ${response.status}`);
  }
  const temporary = `${target}.part`;
  await pipeline(response.body, createWriteStream(temporary));
  const hash = createHash("sha256").update(readFileSync(temporary)).digest("hex");
  if (hash !== file.sha256) {
    unlinkSync(temporary);
    throw new Error(`${file.url}: sha256 ${hash}, expected ${file.sha256}`);
  }
  renameSync(temporary, target);
  console.log(`ok ${file.name}`);
}

console.log(`Beat grid models ready in ${dest}`);
