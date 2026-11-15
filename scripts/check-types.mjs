#!/usr/bin/env node
/**
 * Keeps the hand-written TypeScript unions in sync with the Rust enums that
 * cross the IPC boundary.
 *
 * The plan named `ts-rs` for this. A generated file would be nicer, but it adds
 * a build dependency and rewrites `src/api/*.ts` imports. The failure mode we
 * actually care about is drift: a variant added on one side and forgotten on the
 * other. This check compares the variant sets and fails the build on a mismatch.
 *
 * Exit code 1 when any pair diverges.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

const PAIRS = [
  {
    rust: "src-tauri/src/queue/store.rs",
    types: [
      { name: "TaskState", ts: "src/api/tasks.ts" },
      { name: "TaskMode", ts: "src/api/tasks.ts" },
    ],
  },
  {
    rust: "src-tauri/src/library/model.rs",
    types: [
      { name: "MediaKind", ts: "src/api/library.ts" },
      { name: "SortKey", ts: "src/api/library.ts" },
      { name: "ImportSkipReason", ts: "src/api/library.ts" },
    ],
  },
];

function fail(message) {
  console.error(`check-types: ${message}`);
  process.exitCode = 1;
}

/** `rename_all` on the nearest attribute block above the enum. */
function renameRule(source, enumIndex) {
  const before = source.slice(Math.max(0, enumIndex - 400), enumIndex);
  const match = before.match(/rename_all\s*=\s*"([A-Za-z-]+)"/g);
  if (!match) return "lowercase";
  return match[match.length - 1].match(/"([A-Za-z-]+)"/)[1];
}

function applyRename(variant, rule) {
  if (rule === "lowercase") return variant.toLowerCase();
  if (rule === "camelCase") return variant[0].toLowerCase() + variant.slice(1);
  if (rule === "kebab-case") {
    return variant.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
  }
  return variant;
}

function rustVariants(source, name) {
  const index = source.search(new RegExp(`pub(?:\\(crate\\))?\\s+enum\\s+${name}\\b`));
  if (index === -1) return null;
  const open = source.indexOf("{", index);
  const close = source.indexOf("}", open);
  if (open === -1 || close === -1) return null;
  const rule = renameRule(source, index);
  const variants = source
    .slice(open + 1, close)
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => /^[A-Z][A-Za-z0-9]*(,)?$/.test(line))
    .map((line) => line.replace(/,$/, ""))
    .map((variant) => applyRename(variant, rule));
  return variants.length > 0 ? variants : null;
}

function tsVariants(source, name) {
  const match = source.match(new RegExp(`export type ${name}\\s*=([^;]+);`));
  if (!match) return null;
  return [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

for (const pair of PAIRS) {
  const rustSource = readFileSync(join(root, pair.rust), "utf8");
  for (const type of pair.types) {
    const rust = rustVariants(rustSource, type.name);
    const tsSource = readFileSync(join(root, type.ts), "utf8");
    const ts = tsVariants(tsSource, type.name);

    if (!rust) {
      fail(`cannot find enum ${type.name} in ${pair.rust}`);
      continue;
    }
    if (!ts) {
      fail(`cannot find "export type ${type.name}" in ${type.ts}`);
      continue;
    }

    const rustSet = [...rust].sort();
    const tsSet = [...ts].sort();
    if (rustSet.join("|") !== tsSet.join("|")) {
      fail(
        `${type.name}: Rust [${rustSet.join(", ")}] != TypeScript [${tsSet.join(", ")}]`,
      );
    }
  }
}

if (!process.exitCode) {
  console.log(
    "check-types: every checked enum matches its TypeScript union" +
      ` (${PAIRS.reduce((sum, pair) => sum + pair.types.length, 0)} types)`,
  );
}
