// Builds a throwaway copy of the six files the release scripts read, so the
// script tests never touch the real repository.

import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

export function makeReleaseTree(version = "1.8.0") {
  const root = mkdtempSync(path.join(tmpdir(), "klipparkiv-release-"));
  mkdirSync(path.join(root, "src-tauri"), { recursive: true });

  writeFileSync(
    path.join(root, "package.json"),
    `${JSON.stringify(
      {
        name: "klipparkiv",
        private: true,
        version,
        description: "Offline media library",
      },
      null,
      2,
    )}\n`,
  );

  writeFileSync(
    path.join(root, "package-lock.json"),
    `${JSON.stringify(
      {
        name: "klipparkiv",
        version,
        lockfileVersion: 3,
        requires: true,
        packages: {
          "": { name: "klipparkiv", version },
          "node_modules/react": { version: "19.1.0" },
        },
      },
      null,
      2,
    )}\n`,
  );

  writeFileSync(
    path.join(root, "src-tauri", "Cargo.toml"),
    `[package]\nname = "klipparkiv"\nversion = "${version}"\nedition = "2021"\n\n[dependencies]\nserde = "1"\n`,
  );

  // serde deliberately carries the same version as the app: the bump must leave
  // dependency entries alone.
  writeFileSync(
    path.join(root, "src-tauri", "Cargo.lock"),
    `version = 4\n\n[[package]]\nname = "klipparkiv"\nversion = "${version}"\ndependencies = [\n "serde",\n]\n\n[[package]]\nname = "serde"\nversion = "${version}"\n`,
  );

  writeFileSync(
    path.join(root, "src-tauri", "tauri.conf.json"),
    `${JSON.stringify({ productName: "Klipparkiv", version, identifier: "com.klipparkiv.app" }, null, 2)}\n`,
  );

  writeFileSync(
    path.join(root, "README.txt"),
    `Klipparkiv ${version} - portable build for Windows\n================================================\n\nEnglish\n-------\n`,
  );

  return root;
}

export const CHANGELOG_FIXTURE = `# Changelog

## English

### [1.8.0] — 2026-11-13

Stable notes.

### [1.9.0-beta.1] — 2026-12-01

Test build notes.

## Русский

### [1.8.0] — 2026-11-13

Стабильные изменения.

### [1.9.0-beta.1] — 2026-12-01

Изменения тестовой сборки.
`;
