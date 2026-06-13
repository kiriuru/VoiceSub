import { describe, expect, it } from "vitest";

import {
  effectivePlaybackRate,
  queueDepthForPlayback,
  SPEECH_QUEUE_BOOST_THRESHOLD,
} from "./speech-playback-policy";

describe("queueDepthForPlayback", () => {
  it("includes the active clip", () => {
    expect(queueDepthForPlayback(0)).toBe(1);
    expect(queueDepthForPlayback(2)).toBe(3);
  });
});

describe("effectivePlaybackRate", () => {
  it("keeps base rate when queue depth is at threshold", () => {
    expect(effectivePlaybackRate(1.3, SPEECH_QUEUE_BOOST_THRESHOLD)).toBe(1.3);
    expect(effectivePlaybackRate(1.25, 2)).toBe(1.25);
  });

  it("boosts when more than two messages are pending", () => {
    expect(effectivePlaybackRate(1.3, 3)).toBeCloseTo(1.42, 5);
    expect(effectivePlaybackRate(1.3, 8)).toBe(2);
  });

  it("boosts twitch channel independently", () => {
    expect(effectivePlaybackRate(1.25, 5)).toBeCloseTo(1.61, 5);
  });

  it("never exceeds max playback rate", () => {
    expect(effectivePlaybackRate(1.9, 20)).toBe(2);
  });

  it("defers boost for the next audible clip when backlog is shallow", () => {
    expect(effectivePlaybackRate(1.3, 3, { deferBoost: true })).toBe(1.3);
    expect(effectivePlaybackRate(1.3, 4, { deferBoost: true })).toBeCloseTo(1.42, 5);
  });
});
