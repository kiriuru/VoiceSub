import { describe, expect, it } from "vitest";
import { createBrowserAsrStateSeed } from "./session-state";
import {
  minimumReconnectGuardDelayMs,
  registerNetworkErrorBurst,
  restartDelayForReason,
  shouldRunNetworkPreflight,
} from "./restart-timing-logic";

const limits = {
  restartDelayByReasonMs: { normal_onend: 350 },
  initialNoSpeechDelayMs: 350,
  maxNoSpeechDelayMs: 5000,
  initialNetworkBackoffMs: 1000,
  maxNetworkBackoffMs: 30000,
  networkPreflightBurstThreshold: 3,
  networkPreflightBurstWindowMs: 12000,
  networkPreflightCooldownMs: 30000,
  micSilentDegradedAfterMs: 5000,
  voiceBelowRecognitionRmsThreshold: 0.025,
  voiceBelowRecognitionGraceMs: 8000,
  voiceBelowRecognitionMicWindowMs: 2000,
  voiceBelowRecognitionMinNoSpeech: 1,
  stallDegradedAfterMs: 6000,
  recentMicActivityWindowMs: 2000,
};

describe("restart-timing-logic", () => {
  it("backs off no_speech delays", () => {
    const state = createBrowserAsrStateSeed();
    const first = restartDelayForReason(state, "no_speech", limits);
    const second = restartDelayForReason(state, "no_speech", limits);
    expect(second).toBeGreaterThanOrEqual(first);
  });

  it("extends reconnect delay when minimum interval not met", () => {
    const state = createBrowserAsrStateSeed({ minimumReconnectIntervalMs: 500, lastStartAtMs: 1000 });
    const now = 1100;
    const delay = minimumReconnectGuardDelayMs(state, 100, now, 500);
    expect(delay).toBeGreaterThan(100);
    expect(state.browserMinimumReconnectSuppressedCount).toBe(1);
  });

  it("backs off audio_capture like network", () => {
    const state = createBrowserAsrStateSeed();
    const first = restartDelayForReason(state, "audio_capture", limits);
    const second = restartDelayForReason(state, "audio_capture", limits);
    expect(first).toBeGreaterThan(0);
    expect(second).toBeGreaterThanOrEqual(first);
  });

  it("triggers network preflight after burst threshold", () => {
    const state = createBrowserAsrStateSeed();
    const now = 10_000;
    registerNetworkErrorBurst(state, now, limits);
    registerNetworkErrorBurst(state, now + 100, limits);
    expect(shouldRunNetworkPreflight(state, now + 200, limits)).toBe(false);
    registerNetworkErrorBurst(state, now + 200, limits);
    expect(shouldRunNetworkPreflight(state, now + 300, limits)).toBe(true);
  });
});
