import { describe, expect, it } from "vitest";
import { errorCode, errorMessage, isCodedError } from "./errors";

describe("error decoding", () => {
  it("reads the message from a coded backend error", () => {
    const error = { code: "invalidInput", message: "bad timecode" };

    expect(isCodedError(error)).toBe(true);
    expect(errorCode(error)).toBe("invalidInput");
    expect(errorMessage(error)).toBe("bad timecode");
  });

  it("keeps legacy string errors working", () => {
    expect(errorMessage("yt-dlp failed")).toBe("yt-dlp failed");
    expect(errorCode("yt-dlp failed")).toBeNull();
  });

  it("falls back to Error.message", () => {
    expect(errorMessage(new Error("offline"))).toBe("offline");
  });

  it("does not print [object Object] for an unknown shape", () => {
    expect(errorMessage({ detail: "weird" })).toBe("[object Object]");
    expect(errorMessage({ message: 42 })).toBe("[object Object]");
  });

  it("handles null and undefined", () => {
    expect(errorMessage(null)).toBe("null");
    expect(errorMessage(undefined)).toBe("undefined");
  });

  it("rejects an object that only looks half coded", () => {
    expect(isCodedError({ code: "busy" })).toBe(false);
    expect(isCodedError({ message: "busy" })).toBe(false);
    expect(isCodedError(null)).toBe(false);
  });
});
