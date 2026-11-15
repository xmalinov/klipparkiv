import { describe, expect, it } from "vitest";

import { sectionsFor, summaryFor } from "./changelog.mjs";

const CHANGELOG = `# Changelog

## English

### [1.8.0] — 2026-11-13

Practice grid and interaction release.

#### Added

- Automatic bar lines.

### [1.9.0-beta.2] — 2026-11-15

Same refactor as \`1.9.0-beta.1\`, with the **macOS build** fixed: the installer
module was missing two imports.

Worth trying: everything from the beta.1 checklist.

## Русский

### [1.8.0] — 2026-11-13

Релиз сетки практики.
`;

describe("sectionsFor", () => {
  it("returns every section for the version, English first", () => {
    const sections = sectionsFor(CHANGELOG, "1.8.0");

    expect(sections).toHaveLength(2);
    expect(sections[0]).toContain("Practice grid and interaction release.");
    expect(sections[0]).toContain("Automatic bar lines.");
    expect(sections[1]).toContain("Релиз сетки практики.");
  });

  it("returns an empty list for an unknown version", () => {
    expect(sectionsFor(CHANGELOG, "2.0.0")).toEqual([]);
  });
});

describe("summaryFor", () => {
  it("takes the first paragraph of the English section", () => {
    expect(summaryFor(CHANGELOG, "1.8.0")).toBe("Practice grid and interaction release.");
  });

  it("strips inline markdown and joins wrapped lines", () => {
    expect(summaryFor(CHANGELOG, "1.9.0-beta.2")).toBe(
      "Same refactor as 1.9.0-beta.1, with the macOS build fixed: the installer module was missing two imports.",
    );
  });

  it("keeps the summary on one line and inside the banner limit", () => {
    const long = `# Changelog\n\n### [2.0.0] — 2026-01-01\n\n${"word ".repeat(200)}\n`;
    const summary = summaryFor(long, "2.0.0");

    expect(summary).not.toContain("\n");
    expect(summary.length).toBeLessThanOrEqual(300);
    expect(summary.endsWith("…")).toBe(true);
  });

  it("returns null when the section has no paragraph", () => {
    const onlyList = "# Changelog\n\n### [3.0.0] — 2026-01-01\n\n- only a bullet\n";
    expect(summaryFor(onlyList, "3.0.0")).toBeNull();
  });

  it("returns null for an unknown version", () => {
    expect(summaryFor(CHANGELOG, "9.9.9")).toBeNull();
  });
});
