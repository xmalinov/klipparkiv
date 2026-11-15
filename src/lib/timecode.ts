/**
 * Clip timecodes. The parser mirrors the Rust one: `SS`, `MM:SS`, `HH:MM:SS`.
 */

export function parseTimecode(raw: string): number | null {
  const text = raw.trim();
  if (!text) return null;
  const parts = text.split(":");
  if (parts.length > 3 || parts.some((part) => part === "")) return null;
  let seconds = 0;
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    if (!/^\d+$/.test(part)) return null;
    const value = Number(part);
    const isHours = parts.length === 3 && index === 0;
    if (parts.length > 1 && !isHours && value >= 60) return null;
    seconds = seconds * 60 + value;
  }
  return seconds;
}

/** `90` → `00:01:30`, matching the strings sent to the backend. */
export function formatTimecode(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const rest = total % 60;
  return [hours, minutes, rest]
    .map((value) => value.toString().padStart(2, "0"))
    .join(":");
}
