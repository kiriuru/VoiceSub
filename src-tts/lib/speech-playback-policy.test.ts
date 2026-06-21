import { describe, expect, it } from "vitest";

import {
  clampPlaybackRate,
  clampSpeechVolume,
  PLAYBACK_VOLUME_MAX,
} from "./speech-playback-policy";

describe("clampSpeechVolume", () => {
  it("clamps to 0–150%", () => {
    expect(clampSpeechVolume(-1)).toBe(0);
    expect(clampSpeechVolume(1)).toBe(1);
    expect(clampSpeechVolume(PLAYBACK_VOLUME_MAX)).toBe(PLAYBACK_VOLUME_MAX);
    expect(clampSpeechVolume(2)).toBe(PLAYBACK_VOLUME_MAX);
  });
});

describe("clampPlaybackRate", () => {
  it("clamps to 0.5–2.0", () => {
    expect(clampPlaybackRate(0.1)).toBe(0.5);
    expect(clampPlaybackRate(1.25)).toBe(1.25);
    expect(clampPlaybackRate(3)).toBe(2);
  });
});
