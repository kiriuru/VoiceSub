import { describe, expect, it } from "vitest";

import { formatPlaybackRate, formatSpeechVolume } from "./playback-format";

describe("formatPlaybackRate", () => {
  it("shows clamped rate with two decimals and times sign", () => {
    expect(formatPlaybackRate(1)).toBe("1.00×");
    expect(formatPlaybackRate(1.25)).toBe("1.25×");
    expect(formatPlaybackRate(3)).toBe("2.00×");
    expect(formatPlaybackRate(0.1)).toBe("0.50×");
  });
});

describe("formatSpeechVolume", () => {
  it("shows percent from zero to one", () => {
    expect(formatSpeechVolume(1)).toBe("100%");
    expect(formatSpeechVolume(0.85)).toBe("85%");
    expect(formatSpeechVolume(0)).toBe("0%");
  });
});
