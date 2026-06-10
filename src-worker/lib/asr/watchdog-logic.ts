import type { BrowserAsrState } from "./types";
import { currentSessionAgeMs } from "./restart-timing-logic";

export function evaluateWatchdogTick(ctx: {
  state: BrowserAsrState;
  nowMs: number;
  limits: {
    maxBrowserSessionAgeMs: number;
    prepareCycleBeforeMs: number;
    maxStoppingMs: number;
    hiddenIdleRestartMs: number;
    visibleIdleRestartMs: number;
  };
  documentHidden: boolean;
}): { type: string } {
  const { state, limits } = ctx;
  const now = Number(ctx.nowMs || 0);
  const documentHidden = Boolean(ctx.documentHidden);

  if (!state?.desiredRunning) {
    return { type: "noop" };
  }

  const sessionAgeMs = currentSessionAgeMs(state, now);
  const maxSessionAgeMs = Number(state.maxBrowserSessionAgeMs || limits.maxBrowserSessionAgeMs || 0);
  const prepareCycleBeforeMs = Number(state.prepareCycleBeforeMs || limits.prepareCycleBeforeMs || 0);
  const prepareAtMs = Math.max(0, maxSessionAgeMs - prepareCycleBeforeMs);

  if (
    state.browserSupervisorState === "running" &&
    sessionAgeMs != null &&
    maxSessionAgeMs > 0 &&
    sessionAgeMs >= maxSessionAgeMs
  ) {
    return { type: "session_cycle" };
  }

  if (
    state.browserSupervisorState === "running" &&
    sessionAgeMs != null &&
    sessionAgeMs >= prepareAtMs &&
    !state.browserCyclePending
  ) {
    return { type: "cycle_pending" };
  }

  if (
    state.browserSupervisorState === "stopping" &&
    state.stoppingSinceMs &&
    now - Number(state.stoppingSinceMs) >= Number(limits.maxStoppingMs || 2500)
  ) {
    return { type: "stopping_timeout" };
  }

  // Use transcript/session markers only — onsoundstart updates lastEventAtMs and would
  // mask a stalled native-continuous session while the mic still hears audio.
  const lastActivityAt = Math.max(Number(state.lastStartAtMs || 0), Number(state.lastResultAtMs || 0));
  const idleThresholdMs = documentHidden
    ? Number(limits.hiddenIdleRestartMs || 60000)
    : Number(limits.visibleIdleRestartMs || 30000);
  if (lastActivityAt > 0 && now - lastActivityAt >= idleThresholdMs && state.browserSupervisorState === "running") {
    return { type: "idle_rearm" };
  }

  return { type: "heartbeat" };
}
