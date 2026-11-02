#!/usr/bin/env node
// Builds the release body from the bilingual CHANGELOG.md section.
// Usage: node scripts/release-notes.mjs <version> <out-file>

import { readFileSync, writeFileSync } from "node:fs";

const [version, out] = process.argv.slice(2);
if (!version || !out) {
  console.error("usage: node scripts/release-notes.mjs <version> <out-file>");
  process.exit(1);
}

const lines = readFileSync("CHANGELOG.md", "utf8")
  .replace(/\r\n/g, "\n")
  .split("\n");
const marker = `### [${version}]`;
const sections = [];

for (let index = 0; index < lines.length; index += 1) {
  if (!lines[index].startsWith(marker)) continue;

  let end = index + 1;
  while (end < lines.length && !/^##\s|^###\s/.test(lines[end])) {
    end += 1;
  }
  sections.push(lines.slice(index + 1, end).join("\n").trim());
}

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
const body = `${downloads}${notes}`;

writeFileSync(out, `${body}\n`);
console.log(`release notes for ${version}: ${out} (${body.length} chars)`);
