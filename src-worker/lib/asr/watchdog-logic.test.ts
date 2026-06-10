import { describe, expect, it } from "vitest";
import { createBrowserAsrStateSeed } from "./session-state";
import { evaluateWatchdogTick } from "./watchdog-logic";

const limits = {
  maxBrowserSessionAgeMs: 180_000,
  prepareCycleBeforeMs: 15_000,
  maxStoppingMs: 2500,
  hiddenIdleRestartMs: 60_000,
  visibleIdleRestartMs: 30_000,
};

describe("watchdog-logic", () => {
  it("idle_rearm ignores recent sound events when no transcript activity", () => {
    const state = createBrowserAsrStateSeed({
      desiredRunning: true,
      browserSupervisorState: "running",
      lastSessionStartedAtMs: 0,
      lastStartAtMs: 1000,
      lastResultAtMs: 0,
      lastEventAtMs: 40_000,
    });
    const tick = evaluateWatchdogTick({
      state,
      nowMs: 35_000,
      limits,
      documentHidden: false,
    });
    expect(tick.type).toBe("idle_rearm");
  });

  it("heartbeat while interim/final results keep arriving", () => {
    const state = createBrowserAsrStateSeed({
      desiredRunning: true,
      browserSupervisorState: "running",
      lastSessionStartedAtMs: 0,
      lastStartAtMs: 1000,
      lastResultAtMs: 34_000,
      lastEventAtMs: 34_500,
    });
    const tick = evaluateWatchdogTick({
      state,
      nowMs: 35_000,
      limits,
      documentHidden: false,
    });
    expect(tick.type).toBe("heartbeat");
  });
});
