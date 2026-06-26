import type { BrowserAsrState, TimingLimits } from "./types";

export function currentSessionAgeMs(state: BrowserAsrState, nowMs: number): number | null {
  if (!state.lastSessionStartedAtMs) {
    return null;
  }
  return Math.max(0, nowMs - Number(state.lastSessionStartedAtMs || 0));
}

export function minimumReconnectGuardDelayMs(
  state: BrowserAsrState,
  delayMs: number,
  nowMs: number,
  instanceMinimumReconnectIntervalMs: number
): number {
  const minimumIntervalMs = Math.max(
    0,
    Number(state.minimumReconnectIntervalMs || instanceMinimumReconnectIntervalMs || 0)
  );
  if (!minimumIntervalMs) {
    return delayMs;
  }
  const anchorMs = Math.max(
    Number(state.lastSessionEndedAtMs || 0),
    Number(state.lastEndAtMs || 0),
    Number(state.lastStartAtMs || 0)
  );
  if (!anchorMs) {
    return delayMs;
  }
  const remainingMs = minimumIntervalMs - Math.max(0, nowMs - anchorMs);
  if (remainingMs <= 0 || remainingMs <= delayMs) {
    return delayMs;
  }
  state.browserMinimumReconnectSuppressedCount = Number(state.browserMinimumReconnectSuppressedCount || 0) + 1;
  return remainingMs;
}

export function nextNetworkBackoffMs(
  state: BrowserAsrState,
  initialNetworkBackoffMs: number,
  maxNetworkBackoffMs: number
): number {
  const current = Number(state.restartBackoffMs || 0);
  if (!current) {
    state.restartBackoffMs = Math.max(0, Number(state.networkReconnectInitialMs || initialNetworkBackoffMs || 1000));
  } else {
    state.restartBackoffMs = Math.min(
      maxNetworkBackoffMs,
      Math.max(Number(state.networkReconnectInitialMs || initialNetworkBackoffMs || 1000), current * 2)
    );
  }
  return state.restartBackoffMs;
}

export function restartDelayForReason(state: BrowserAsrState, reason: string, limits: TimingLimits): number {
  const normalized = String(reason || "")
    .trim()
    .toLowerCase();
  if (normalized === "no_speech") {
    if (!state.noSpeechBackoffMs) {
      state.noSpeechBackoffMs = Math.max(0, Number(state.noSpeechRestartDelayMs || limits.initialNoSpeechDelayMs || 350));
    } else {
      state.noSpeechBackoffMs = Math.min(
        limits.maxNoSpeechDelayMs,
        Math.max(
          Math.max(0, Number(state.noSpeechRestartDelayMs || limits.initialNoSpeechDelayMs || 350)),
          state.noSpeechBackoffMs + 800
        )
      );
    }
    return state.noSpeechBackoffMs;
  }
  if (normalized === "network" || normalized === "audio_capture") {
    return nextNetworkBackoffMs(state, limits.initialNetworkBackoffMs, limits.maxNetworkBackoffMs);
  }
  return limits.restartDelayByReasonMs[normalized] ?? limits.restartDelayByReasonMs.normal_onend ?? 0;
}

export function resetNetworkErrorBurst(state: BrowserAsrState): void {
  state.networkErrorBurstCount = 0;
  state.networkErrorBurstStartedAtMs = 0;
}

export function registerNetworkErrorBurst(state: BrowserAsrState, nowMs: number, limits: TimingLimits): boolean {
  const startedAt = Number(state.networkErrorBurstStartedAtMs || 0);
  if (!startedAt || nowMs - startedAt > limits.networkPreflightBurstWindowMs) {
    state.networkErrorBurstStartedAtMs = nowMs;
    state.networkErrorBurstCount = 1;
  } else {
    state.networkErrorBurstCount = Number(state.networkErrorBurstCount || 0) + 1;
  }
  return shouldRunNetworkPreflight(state, nowMs, limits);
}

export function shouldRunNetworkPreflight(state: BrowserAsrState, nowMs: number, limits: TimingLimits): boolean {
  if (state.networkPreflightInFlight) {
    return false;
  }
  if (Number(state.networkErrorBurstCount || 0) < limits.networkPreflightBurstThreshold) {
    return false;
  }
  const burstStartedAt = Number(state.networkErrorBurstStartedAtMs || 0);
  if (!burstStartedAt || nowMs - burstStartedAt > limits.networkPreflightBurstWindowMs) {
    return false;
  }
  const lastPreflightAt = Number(state.lastNetworkPreflightAtMs || 0);
  if (lastPreflightAt && nowMs - lastPreflightAt < limits.networkPreflightCooldownMs) {
    return false;
  }
  return true;
}
