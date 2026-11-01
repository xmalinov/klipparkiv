#!/usr/bin/env node
// Downloads pinned macOS binaries into src-tauri/resources/bin.
// Usage: node scripts/fetch-binaries.mjs mac-x64|mac-arm64

import { createHash } from "node:crypto";
import { chmodSync, createWriteStream, mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { pipeline } from "node:stream/promises";
import { execFileSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const platform = process.argv[2];

if (platform !== "mac-x64" && platform !== "mac-arm64") {
  console.error("usage: node scripts/fetch-binaries.mjs mac-x64|mac-arm64");
  process.exit(2);
}

const arch = platform === "mac-x64" ? "x64" : "arm64";
const config = JSON.parse(readFileSync(join(here, "binaries.json"), "utf8"));
const dest = join(root, "src-tauri", "resources", "bin");
mkdirSync(dest, { recursive: true });

async function download(url, target, expectedSha) {
  console.log(`download ${url}`);
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok || !response.body) {
    throw new Error(`${url}: HTTP ${response.status}`);
  }
  const temporary = `${target}.part`;
  await pipeline(response.body, createWriteStream(temporary));
  const hash = createHash("sha256").update(readFileSync(temporary)).digest("hex");
  if (hash !== expectedSha) {
    unlinkSync(temporary);
    throw new Error(`${url}: sha256 ${hash}, expected ${expectedSha}`);
  }
  const { renameSync } = await import("node:fs");
  renameSync(temporary, target);
  chmodSync(target, 0o755);
}

const jobs = [
  {
    name: "yt-dlp",
    url: config["yt-dlp"].macos.url,
    sha256: config["yt-dlp"].macos.sha256,
  },
  {
    name: "ffmpeg",
    url: config["ffmpeg-static"].macos[arch].ffmpeg.url,
    sha256: config["ffmpeg-static"].macos[arch].ffmpeg.sha256,
  },
  {
    name: "ffprobe",
    url: config["ffmpeg-static"].macos[arch].ffprobe.url,
    sha256: config["ffmpeg-static"].macos[arch].ffprobe.sha256,
  },
];

for (const job of jobs) {
  await download(job.url, join(dest, job.name), job.sha256);
}

// Fail early if a downloaded binary does not match the runner architecture.
for (const job of jobs) {
  const output = execFileSync("file", [join(dest, job.name)], { encoding: "utf8" });
  if (!output.includes(arch === "x64" ? "x86_64" : "arm64")) {
    throw new Error(`${job.name}: unexpected architecture: ${output.trim()}`);
  }
}

writeFileSync(join(dest, "yt-dlp.version"), `${config["yt-dlp"].version}\n`);
console.log(`macOS binaries ready in ${dest}`);
