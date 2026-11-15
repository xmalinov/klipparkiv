#!/usr/bin/env node
// Builds the release body from the bilingual CHANGELOG.md section.
// Usage: node scripts/release-notes.mjs <version> <out-file> [--summary <file>]
//
// --summary writes the one-line description that `update.json` carries into the
// in-app update banner. It is short on purpose: the banner renders it as a
// single line with an ellipsis.

import { readFileSync, writeFileSync } from "node:fs";

import { sectionsFor, summaryFor } from "./lib/changelog.mjs";
import { isPrerelease } from "./lib/version.mjs";

function arg(name) {
  const index = process.argv.indexOf(`--${name}`);
  return index === -1 ? undefined : process.argv[index + 1];
}

const positional = process.argv.slice(2).filter((value, index, all) => {
  if (value.startsWith("--")) return false;
  return all[index - 1] !== "--summary";
});
const [version, out] = positional;
if (!version || !out) {
  console.error(
    "usage: node scripts/release-notes.mjs <version> <out-file> [--summary <file>]",
  );
  process.exit(1);
}

const changelog = readFileSync("CHANGELOG.md", "utf8");
const sections = sectionsFor(changelog, version);
if (sections.length === 0) {
  console.error(`CHANGELOG.md has no "### [${version}]" section`);
  process.exit(1);
}

const downloads = `## What to download / Что скачать

| Platform | File |
|---|---|
| Windows 10/11 x64 | \`klipparkiv-${version}-win-x64.zip\` |
| macOS Apple Silicon | \`klipparkiv-${version}-mac-arm64.dmg\` |
| macOS Intel | \`klipparkiv-${version}-mac-x64.dmg\` |

The standalone \`.exe\`, \`.app.tar.gz\`, \`.sig\` and \`update.json\` are update
files, not first-install files. Не скачивайте отдельный \`.exe\`, \`.app.tar.gz\`,
\`.sig\` или \`update.json\` для первой установки.

---

`;

// English is first, Russian is last; keep both for the bilingual audience.
const notes =
  sections.length >= 2
    ? `## English\n\n${sections[0]}\n\n---\n\n## Русский\n\n${sections.at(-1)}`
    : sections[0];

// A test build is published as a prerelease and is never offered to stable
// users, so the release page has to say that out loud.
const banner = isPrerelease(version)
  ? `> **Test build / Тестовая сборка.** This is \`${version}\`, not a stable
> release. It is not offered through in-app updates. Для обычной работы
> скачайте последнюю стабильную версию со страницы Releases.

---

`
  : "";
const body = `${banner}${downloads}${notes}`;

writeFileSync(out, `${body}\n`);
console.log(`release notes for ${version}: ${out} (${body.length} chars)`);

const summaryPath = arg("summary");
if (summaryPath) {
  const summary = summaryFor(changelog, version);
  if (!summary) {
    console.error(`CHANGELOG.md has no summary paragraph for ${version}`);
    process.exit(1);
  }
  writeFileSync(summaryPath, `${summary}\n`);
  console.log(`update banner summary: ${summaryPath} (${summary.length} chars)`);
}
