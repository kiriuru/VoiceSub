import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { isTtsFullLoggingEnabled, setTtsFullLoggingEnabled, ttsTrace } from "./tts-trace";

describe("ttsTrace", () => {
  beforeEach(() => {
    setTtsFullLoggingEnabled(false);
    vi.stubGlobal(
      "fetch",
      vi.fn(() => Promise.resolve({ ok: true } as Response)),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("skips network calls in compact mode", () => {
    ttsTrace("engine", "speak_end", { channel: "speech" });
    expect(fetch).not.toHaveBeenCalled();
    expect(isTtsFullLoggingEnabled()).toBe(false);
  });

  it("posts ui and client logs when full logging is enabled", () => {
    vi.stubGlobal("window", {
      __VOICESUB_API_TOKEN__: "test-loopback-token",
    });
    setTtsFullLoggingEnabled(true);
    ttsTrace("engine", "speak_end", { channel: "speech" });
    expect(fetch).toHaveBeenCalledTimes(2);
    expect(isTtsFullLoggingEnabled()).toBe(true);
  });
});
