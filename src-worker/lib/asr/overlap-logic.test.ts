import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createBrowserAsrStateSeed } from "./session-state";
import {
  evaluateOverlapBuddyGhost,
  handleInactiveOverlapBuddyEnded,
  handleOverlapRecognitionEnded,
  overlapSlotInactive,
  recognitionOverlapActive,
  recoverGhostOverlapBuddy,
  scheduleOverlapTimeBasedPrestart,
  shouldIgnoreOverlapBuddyError,
} from "./overlap-logic";
import type { AsrManagerHost, BrowserAsrState } from "./types";

function overlapState(activeSlot = 0, nowMs = 10_000): BrowserAsrState {
  const state = createBrowserAsrStateSeed({
    desiredRunning: true,
    browserSupervisorState: "running",
    lastResultAtMs: nowMs - 500,
    lastMicActivityAt: nowMs - 500,
  });
  state.recognitionOverlapSlots = [{ start: vi.fn(), abort: vi.fn() }, { start: vi.fn(), abort: vi.fn() }] as BrowserAsrState["recognitionOverlapSlots"];
  state.recognitionOverlapActiveSlot = activeSlot;
  state.recognitionOverlapPrestarted = true;
  state.recognitionOverlapSlotListening = [true, true];
  state.recognition = state.recognitionOverlapSlots![activeSlot];
  state.recognitionOverlapSlotListenSinceMs = [nowMs - 9000, nowMs - 7000];
  state.recognitionOverlapSlotActivityAtMs = [nowMs - 500, null];
  state.recognitionOverlapActiveListenSinceMs = nowMs - 9000;
  return state;
}

function mockManager(state: BrowserAsrState, nowMs = 10_000): AsrManagerHost {
  return {
    state,
    now: () => nowMs,
    appendLogInternal: vi.fn(),
    emitWorkerStatus: vi.fn(() => true),
    applyRecognitionSettings: vi.fn(),
    wireRecognitionHandlers: vi.fn(),
    setSupervisorStateInternal: vi.fn(),
    setRecognitionStateInternal: vi.fn(),
  } as unknown as AsrManagerHost;
}

describe("overlap-logic", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("detects inactive overlap slot", () => {
    const state = overlapState(0);
    expect(recognitionOverlapActive(state)).toBe(true);
    expect(overlapSlotInactive(state, 0)).toBe(false);
    expect(overlapSlotInactive(state, 1)).toBe(true);
  });

  it("ignores expected buddy errors while active slot listens", () => {
    const state = overlapState(0);
    expect(shouldIgnoreOverlapBuddyError(state, 1, "no-speech")).toBe(true);
    expect(shouldIgnoreOverlapBuddyError(state, 1, "aborted")).toBe(true);
    expect(shouldIgnoreOverlapBuddyError(state, 1, "network")).toBe(true);
    expect(shouldIgnoreOverlapBuddyError(state, 0, "no-speech")).toBe(false);
    expect(shouldIgnoreOverlapBuddyError(state, 1, "not-allowed")).toBe(false);
    expect(shouldIgnoreOverlapBuddyError(state, 1, "language-not-supported")).toBe(false);
    expect(shouldIgnoreOverlapBuddyError(state, 1, "phrases-not-supported")).toBe(false);
  });

  it("does not consume buddy onend when a global restart is pending", () => {
    const state = overlapState(0);
    state.pendingRestartReason = "normal_onend";
    const manager = mockManager(state);
    expect(handleInactiveOverlapBuddyEnded(manager, 1)).toBe(false);
    expect(state.recognitionOverlapSlotListening).toEqual([true, true]);
    expect(state.recognitionOverlapSlots![1].start).not.toHaveBeenCalled();
  });

  it("consumes inactive buddy onend without touching active slot", () => {
    const state = overlapState(0);
    const manager = mockManager(state);
    expect(handleInactiveOverlapBuddyEnded(manager, 1)).toBe(true);
    expect(state.recognitionOverlapSlotListening).toEqual([true, false]);
    expect(state.recognitionOverlapPrestarted).toBe(true);
    expect(state.recognitionOverlapSlots![1].start).toHaveBeenCalled();
    expect(manager.emitWorkerStatus).toHaveBeenCalledWith("overlap-buddy-ended");
    expect(handleInactiveOverlapBuddyEnded(manager, 0)).toBe(false);
  });

  it("schedules time-based buddy prestart after active slot listen threshold", () => {
    const state = overlapState(0, 0);
    state.recognitionOverlapPrestarted = false;
    state.recognitionOverlapSlotListening = [true, false];
    state.recognitionOverlapActiveListenSinceMs = 0;
    state.recognitionGenerationId = 1;
    const manager = mockManager(state, 0);
    scheduleOverlapTimeBasedPrestart(manager);
    expect(state.recognitionOverlapPrestartTimer).not.toBeNull();
    vi.advanceTimersByTime(7999);
    expect(state.recognitionOverlapSlots![1].start).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1);
    expect(state.recognitionOverlapSlots![1].start).toHaveBeenCalled();
  });

  it("does not treat silent buddy as ghost while active slot is transcribing", () => {
    const state = overlapState(0, 20_000);
    expect(evaluateOverlapBuddyGhost(state, 20_000)).toBe(false);
  });

  it("does not treat buddy as ghost during short inter-phrase pause", () => {
    const state = overlapState(0, 20_000);
    state.lastResultAtMs = 16_000;
    state.lastMicActivityAt = 16_000;
    state.recognitionOverlapSlotActivityAtMs = [16_000, null];
    expect(evaluateOverlapBuddyGhost(state, 20_000)).toBe(false);
  });

  it("detects ghost buddy only after sustained idle on both slots", () => {
    const state = overlapState(0, 20_000);
    state.lastResultAtMs = 0;
    state.lastMicActivityAt = 0;
    state.recognitionOverlapSlotActivityAtMs = [null, null];
    expect(evaluateOverlapBuddyGhost(state, 20_000)).toBe(true);
  });

  it("does not treat buddy as ghost when it has slot activity", () => {
    const state = overlapState(0, 20_000);
    state.lastResultAtMs = 0;
    state.lastMicActivityAt = 0;
    state.recognitionOverlapSlotActivityAtMs = [null, 19_800];
    expect(evaluateOverlapBuddyGhost(state, 20_000)).toBe(false);
  });

  it("handoff clears stale pending restart reason from active slot", () => {
    const state = overlapState(0);
    state.pendingRestartReason = "no_speech";
    state.recognitionOverlapSlotListening = [true, true];
    const manager = mockManager(state);
    expect(handleOverlapRecognitionEnded(manager, 0)).toBe(true);
    expect(state.recognitionOverlapActiveSlot).toBe(1);
    expect(state.pendingRestartReason).toBeNull();
    expect(state.recognitionOverlapSlotListening).toEqual([false, true]);
  });

  it("recovers ghost buddy when both slots appear idle", () => {
    const state = overlapState(0, 20_000);
    state.lastResultAtMs = 0;
    state.lastMicActivityAt = 0;
    state.recognitionOverlapSlotActivityAtMs = [null, null];
    const manager = mockManager(state, 20_000);
    expect(recoverGhostOverlapBuddy(manager, 20_000)).toBe(true);
    expect(state.recognitionOverlapSlots![1].abort).toHaveBeenCalled();
    expect(state.recognitionOverlapPrestarted).toBe(true);
    expect(manager.emitWorkerStatus).toHaveBeenCalledWith("overlap-buddy-ghost-recovered");
  });
});
