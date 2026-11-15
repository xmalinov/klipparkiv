/**
 * One implementation of every user-visible number.
 *
 * These used to live in `api/library.ts`, `api/tasks.ts` and `api/updates.ts`
 * with slightly different rules (the update banner had no GB branch), so the
 * same size could render differently on two screens.
 */

export function formatBytes(
  bytes: number | null,
  language: "ru" | "en" = "ru",
): string {
  if (bytes === null || !Number.isFinite(bytes) || bytes < 0) return "";
  const separator = language === "ru" ? "," : ".";
  const units =
    language === "ru"
      ? { gb: "ГБ", mb: "МБ", kb: "КБ", b: "Б" }
      : { gb: "GB", mb: "MB", kb: "KB", b: "B" };
  if (bytes < 1024) {
    return `${Math.round(bytes)} ${units.b}`;
  }
  const kb = bytes / 1024;
  if (kb < 1024) {
    return `${Math.round(kb)} ${units.kb}`;
  }
  const mb = kb / 1024;
  if (mb < 1024) {
    return `${mb.toFixed(1).replace(".", separator)} ${units.mb}`;
  }
  return `${(mb / 1024).toFixed(1).replace(".", separator)} ${units.gb}`;
}

export function formatFileCount(count: number, language: "ru" | "en" = "ru"): string {
  if (language === "en") {
    return `${count} ${count === 1 ? "file" : "files"}`;
  }
  const mod10 = count % 10;
  const mod100 = count % 100;
  if (mod10 === 1 && mod100 !== 11) return `${count} файл`;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) {
    return `${count} файла`;
  }
  return `${count} файлов`;
}

export function formatItemDate(mtime: number, language: "ru" | "en" = "ru"): string {
  if (!mtime) return "";
  const date = new Date(mtime * 1000);
  const day = date.getDate().toString().padStart(2, "0");
  const month = (date.getMonth() + 1).toString().padStart(2, "0");
  return language === "ru" ? `${day}.${month}` : `${month}/${day}`;
}

export function formatResolution(width: number | null, height: number | null): string {
  if (!width || !height) return "";
  return height >= 2160 ? "4K" : `${height}p`;
}

export function formatPercent(percent: number): string {
  return `${Math.round(percent)}%`;
}

export function formatSpeed(
  bytesPerSecond: number | null,
  language: "ru" | "en" = "ru",
): string {
  if (bytesPerSecond === null || !Number.isFinite(bytesPerSecond)) return "—";
  const separator = language === "ru" ? "," : ".";
  if (bytesPerSecond >= 1024 * 1024) {
    return `${(bytesPerSecond / (1024 * 1024)).toFixed(1).replace(".", separator)} ${
      language === "ru" ? "МБ/с" : "MB/s"
    }`;
  }
  if (bytesPerSecond >= 1024) {
    return `${Math.round(bytesPerSecond / 1024)} ${
      language === "ru" ? "КБ/с" : "KB/s"
    }`;
  }
  return `${Math.round(bytesPerSecond)} ${language === "ru" ? "Б/с" : "B/s"}`;
}

export function formatEta(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return "";
  const total = Math.round(seconds);
  const minutes = Math.floor(total / 60);
  const rest = total % 60;
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return "";
  const total = Math.round(seconds);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const rest = total % 60;
  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${rest
      .toString()
      .padStart(2, "0")}`;
  }
  return `${minutes}:${rest.toString().padStart(2, "0")}`;
}

export function formatViews(count: number | null, language: "ru" | "en" = "ru"): string {
  if (count === null || !Number.isFinite(count)) return "";
  const separator = language === "ru" ? "," : ".";
  const decimal = (value: number) => value.toFixed(1).replace(".", separator);
  if (language === "en") {
    if (count >= 1_000_000_000) return `${decimal(count / 1_000_000_000)}B views`;
    if (count >= 1_000_000) return `${decimal(count / 1_000_000)}M views`;
    if (count >= 1_000) return `${Math.round(count / 1_000)}K views`;
    return `${count} ${count === 1 ? "view" : "views"}`;
  }
  if (count >= 1_000_000_000) {
    return `${decimal(count / 1_000_000_000)} млрд просмотров`;
  }
  if (count >= 1_000_000) {
    return `${decimal(count / 1_000_000)} млн просмотров`;
  }
  if (count >= 1_000) {
    return `${Math.round(count / 1_000)} тыс. просмотров`;
  }
  return `${count} просмотров`;
}

export function formatUploadDate(
  raw: string | null,
  language: "ru" | "en" = "ru",
): string {
  if (!raw || !/^\d{8}$/.test(raw)) return raw ?? "";
  const day = raw.slice(6, 8);
  const month = raw.slice(4, 6);
  const year = raw.slice(0, 4);
  return language === "ru" ? `${day}.${month}.${year}` : `${month}/${day}/${year}`;
}
