#!/usr/bin/env node
/**
 * Guards the Rust <-> TypeScript boundary while `commands.rs`, `library.rs` and
 * `updater.rs` are being split into modules.
 *
 * Moving a `#[tauri::command]` into a new module keeps working only while the
 * function is still listed in `tauri::generate_handler!` under the same name.
 * A typo there fails at runtime with "command not found" and no compile error,
 * so this check fails the build instead.
 *
 * Exit code 1 only when the UI invokes a command the backend does not register.
 * Registered but unused commands are reported as warnings: they are dead API
 * surface, not a broken build.
 */
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const libPath = join(root, "src-tauri", "src", "lib.rs");
const apiDir = join(root, "src", "api");

function fail(message) {
  console.error(`check-contracts: ${message}`);
  process.exit(1);
}

const lib = readFileSync(libPath, "utf8");
const start = lib.indexOf("tauri::generate_handler![");
if (start === -1) {
  fail("cannot find tauri::generate_handler![ in src-tauri/src/lib.rs");
}
const end = lib.indexOf("\n        ])", start);
if (end === -1) {
  fail("cannot find the end of the generate_handler! list in lib.rs");
}

const registered = new Set(
  lib
    .slice(start, end)
    .split(/\r?\n/)
    .map((line) => line.trim().replace(/,$/, ""))
    // `commands::enqueue_task` -> `enqueue_task`
    .map((line) => line.split("::").pop())
    .filter((name) => /^[a-z_][a-z0-9_]*$/.test(name)),
);
if (registered.size === 0) {
  fail("parsed zero commands from generate_handler![ — the format changed");
}

/** `invoke<Foo>("name", ...)`, `invoke("name")` and multi-line variants. */
const invokePattern = /\binvoke(?:<[^>]*>)?\(\s*"([a-z0-9_]+)"/g;
const invoked = new Map();
for (const file of readdirSync(apiDir)) {
  if (!file.endsWith(".ts") || file.endsWith(".test.ts")) continue;
  const text = readFileSync(join(apiDir, file), "utf8");
  for (const match of text.matchAll(invokePattern)) {
    invoked.set(match[1], file);
  }
}

const missing = [...invoked.keys()].filter((name) => !registered.has(name));
const unused = [...registered].filter((name) => !invoked.has(name));

if (missing.length > 0) {
  for (const name of missing) {
    console.error(
      `check-contracts: ${invoked.get(name)} invokes "${name}", but it is not registered in generate_handler!`,
    );
  }
  fail(`${missing.length} command(s) are invoked by the UI but missing on the backend`);
}

for (const name of unused) {
  console.warn(
    `check-contracts: warning: "${name}" is registered but never invoked by src/api`,
  );
}

console.log(
  `check-contracts: ${registered.size} commands registered, ${invoked.size} invoked, ${unused.length} unused`,
);
