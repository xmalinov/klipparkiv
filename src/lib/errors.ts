/**
 * Error decoding for the IPC boundary.
 *
 * Queue commands return `{ code, message }`; older commands still return a
 * plain string. Both must render the same, and the code must stay available for
 * branching once more layers migrate.
 */

export interface CodedError {
  code: string;
  message: string;
}

export function isCodedError(value: unknown): value is CodedError {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as { code?: unknown; message?: unknown };
  return typeof candidate.code === "string" && typeof candidate.message === "string";
}

/** Stable error code, or `null` for a legacy string/Error. */
export function errorCode(cause: unknown): string | null {
  return isCodedError(cause) ? cause.code : null;
}

/** Human-readable message for anything a command may throw. */
export function errorMessage(cause: unknown): string {
  if (typeof cause === "string") return cause;
  if (isCodedError(cause)) return cause.message;
  if (cause instanceof Error) return cause.message;
  if (typeof cause === "object" && cause !== null) {
    const candidate = (cause as { message?: unknown }).message;
    if (typeof candidate === "string") return candidate;
  }
  return String(cause);
}
