// Reads the bilingual CHANGELOG.md sections used by the release scripts.

export const SUMMARY_LIMIT = 300;

/**
 * Returns every `### [<version>]` section body, in file order. The English
 * block comes first and the Russian block last, so callers can pick
 * `sections[0]` / `sections.at(-1)`.
 */
export function sectionsFor(text, version) {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
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

  return sections;
}

/** Markdown decoration that must not leak into the one-line update banner. */
function plainLine(line) {
  return line
    .replace(/^[\s>]+/, "")
    .replace(/^#{1,6}\s*/, "")
    .replace(/^[-*+]\s+/, "")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

function truncate(value, limit) {
  if (value.length <= limit) return value;
  const cut = value.slice(0, limit - 1);
  const boundary = cut.lastIndexOf(" ");
  return `${(boundary > limit * 0.6 ? cut.slice(0, boundary) : cut).trimEnd()}…`;
}

/**
 * One plain-text line describing the release, for the update banner. It is the
 * first paragraph of the English section; the banner is a single line with an
 * ellipsis, so the value is capped at SUMMARY_LIMIT characters.
 */
export function summaryFor(text, version) {
  const [english] = sectionsFor(text, version);
  if (!english) return null;

  const paragraph = [];
  for (const raw of english.split("\n")) {
    const line = plainLine(raw);
    if (line === "") {
      if (paragraph.length > 0) break;
      continue;
    }
    // A heading or list item is not a summary line.
    if (/^#{1,6}\s/.test(raw.trim()) || /^[-*+]\s/.test(raw.trim())) break;
    paragraph.push(line);
  }

  if (paragraph.length === 0) return null;
  return truncate(paragraph.join(" "), SUMMARY_LIMIT);
}
