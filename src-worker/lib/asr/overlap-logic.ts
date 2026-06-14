import type { AsrManagerHost, BrowserAsrState, RecognitionSettings } from "./types";

import type { WorkerSpeechRecognition } from "./speech-types";

import { webSpeechRecognitionPolicy } from "./web-speech-policy";



export const DEFAULT_OVERLAP_TIME_PRESTART_MS = 8000;

export const DEFAULT_OVERLAP_BUDDY_GHOST_TIMEOUT_MS = 6000;

export const DEFAULT_OVERLAP_BUDDY_GHOST_ACTIVE_MIC_MS = 3000;



export function recognitionOverlapModeDesired(

  settings: RecognitionSettings | null | undefined,

  policy = webSpeechRecognitionPolicy

): boolean {

  if (policy && typeof policy.shouldEnableRecognitionOverlap === "function") {

    return Boolean(policy.shouldEnableRecognitionOverlap(settings));

  }

  return Boolean(settings && settings.continuous === false && settings.overlap_recognition_sessions !== false);

}



export function recognitionOverlapActive(state: BrowserAsrState): boolean {

  return Array.isArray(state.recognitionOverlapSlots) && state.recognitionOverlapSlots.length === 2;

}



export function overlapActiveSlotIndex(state: BrowserAsrState): number {

  return Number(state.recognitionOverlapActiveSlot || 0) % 2;

}



export function overlapSlotInactive(state: BrowserAsrState, overlapSlotIndex: number | null | undefined): boolean {

  return overlapSlotIndex != null && overlapSlotIndex !== overlapActiveSlotIndex(state);

}



export function overlapLifecycleLimits(state: BrowserAsrState): {

  prestartAfterMs: number;

  buddyGhostTimeoutMs: number;

  buddyGhostActiveMicMs: number;

} {

  const cfg = state.browserLifecycleConfig;

  return {

    prestartAfterMs: Math.max(

      1000,

      Number(cfg?.overlapPrestartAfterMs || DEFAULT_OVERLAP_TIME_PRESTART_MS)

    ),

    buddyGhostTimeoutMs: Math.max(

      2000,

      Number(cfg?.overlapBuddyGhostTimeoutMs || DEFAULT_OVERLAP_BUDDY_GHOST_TIMEOUT_MS)

    ),

    buddyGhostActiveMicMs: Math.max(

      500,

      Number(cfg?.overlapBuddyGhostActiveMicMs || DEFAULT_OVERLAP_BUDDY_GHOST_ACTIVE_MIC_MS)

    ),

  };

}



const OVERLAP_BUDDY_TERMINAL_ERRORS = new Set(["not-allowed", "service-not-allowed", "audio-capture"]);



function ensureOverlapSlotTrackingArrays(state: BrowserAsrState): void {

  if (!state.recognitionOverlapSlotListenSinceMs) {

    state.recognitionOverlapSlotListenSinceMs = [null, null];

  }

  if (!state.recognitionOverlapSlotActivityAtMs) {

    state.recognitionOverlapSlotActivityAtMs = [null, null];

  }

}



export function clearOverlapTimeBasedPrestart(state: BrowserAsrState): void {

  if (state.recognitionOverlapPrestartTimer) {

    clearTimeout(state.recognitionOverlapPrestartTimer);

    state.recognitionOverlapPrestartTimer = null;

  }

}



export function resetOverlapSlotTracking(state: BrowserAsrState): void {

  clearOverlapTimeBasedPrestart(state);

  state.recognitionOverlapActiveListenSinceMs = null;

  state.recognitionOverlapSlotListenSinceMs = null;

  state.recognitionOverlapSlotActivityAtMs = null;

}



export function markOverlapSlotListenStarted(state: BrowserAsrState, overlapSlotIndex: number, nowMs: number): void {

  ensureOverlapSlotTrackingArrays(state);

  state.recognitionOverlapSlotListenSinceMs![overlapSlotIndex] = nowMs;

  state.recognitionOverlapSlotActivityAtMs![overlapSlotIndex] = null;

  if (overlapSlotIndex === overlapActiveSlotIndex(state)) {

    state.recognitionOverlapActiveListenSinceMs = nowMs;

  }

}



export function markOverlapSlotActivity(state: BrowserAsrState, overlapSlotIndex: number, nowMs: number): void {

  ensureOverlapSlotTrackingArrays(state);

  state.recognitionOverlapSlotActivityAtMs![overlapSlotIndex] = nowMs;

}



export function scheduleOverlapTimeBasedPrestart(manager: AsrManagerHost): void {

  if (!recognitionOverlapActive(manager.state) || !manager.state.desiredRunning) {

    return;

  }

  clearOverlapTimeBasedPrestart(manager.state);

  const active = overlapActiveSlotIndex(manager.state);

  const limits = overlapLifecycleLimits(manager.state);

  const listenSince = manager.state.recognitionOverlapActiveListenSinceMs;

  if (listenSince == null) {

    return;

  }

  const elapsed = manager.now() - listenSince;

  const delayMs = Math.max(0, limits.prestartAfterMs - elapsed);

  const capturedGeneration = Number(manager.state.recognitionGenerationId || 0);



  manager.state.recognitionOverlapPrestartTimer = globalThis.setTimeout(() => {

    manager.state.recognitionOverlapPrestartTimer = null;

    if (!recognitionOverlapActive(manager.state) || !manager.state.desiredRunning) {

      return;

    }

    if (Number(manager.state.recognitionGenerationId || 0) !== capturedGeneration) {

      return;

    }

    if (overlapActiveSlotIndex(manager.state) !== active) {

      return;

    }

    prestartOverlapBuddyIfNeeded(manager, active);

  }, delayMs);

}



export function onOverlapActiveSlotReady(manager: AsrManagerHost, overlapSlotIndex: number | null): void {

  if (overlapSlotIndex == null || !recognitionOverlapActive(manager.state)) {

    return;

  }

  markOverlapSlotListenStarted(manager.state, overlapSlotIndex, manager.now());

  if (overlapSlotInactive(manager.state, overlapSlotIndex)) {

    return;

  }

  scheduleOverlapTimeBasedPrestart(manager);

}



/**

 * Pre-started buddy sessions often end with no-speech/aborted while the active slot

 * is still listening. Those events must not schedule a global restart.

 */

export function shouldIgnoreOverlapBuddyError(

  state: BrowserAsrState,

  overlapSlotIndex: number | null | undefined,

  errorKind: string

): boolean {

  if (!recognitionOverlapActive(state) || !overlapSlotInactive(state, overlapSlotIndex)) {

    return false;

  }

  const normalized = String(errorKind || "")

    .trim()

    .toLowerCase();

  if (OVERLAP_BUDDY_TERMINAL_ERRORS.has(normalized)) {

    return false;

  }

  if (normalized === "language-not-supported" || normalized === "phrases-not-supported") {

    return false;

  }

  return true;

}



/** @returns true when the inactive buddy end was consumed locally */

export function handleInactiveOverlapBuddyEnded(

  manager: AsrManagerHost,

  overlapSlotIndex: number

): boolean {

  if (!recognitionOverlapActive(manager.state) || !overlapSlotInactive(manager.state, overlapSlotIndex)) {

    return false;

  }

  // Buddy retry errors must fall through to global restart — do not swallow onend.

  if (manager.state.pendingRestartReason) {

    return false;

  }

  if (!manager.state.recognitionOverlapSlotListening) {

    manager.state.recognitionOverlapSlotListening = [false, false];

  }

  manager.state.recognitionOverlapSlotListening[overlapSlotIndex] = false;

  manager.state.recognitionOverlapPrestarted = false;

  ensureOverlapSlotTrackingArrays(manager.state);

  manager.state.recognitionOverlapSlotListenSinceMs![overlapSlotIndex] = null;

  manager.state.recognitionOverlapSlotActivityAtMs![overlapSlotIndex] = null;

  const active = overlapActiveSlotIndex(manager.state);

  if (manager.state.recognitionOverlapSlotListening[active]) {

    prestartOverlapBuddyIfNeeded(manager, active);

  }

  manager.emitWorkerStatus("overlap-buddy-ended");

  return true;

}



export function overlapResultAllowed(state: BrowserAsrState, overlapSlotIndex: number | null | undefined): boolean {

  if (overlapSlotIndex == null) {

    return true;

  }

  if (!recognitionOverlapActive(state)) {

    return true;

  }

  const active = Number(state.recognitionOverlapActiveSlot || 0) % 2;

  if (overlapSlotIndex === active) {

    return true;

  }

  const buddy = (active + 1) % 2;

  return overlapSlotIndex === buddy && Boolean(state.recognitionOverlapPrestarted);

}



export function createOverlapRecognitionPair(manager: AsrManagerHost, generationId: number): WorkerSpeechRecognition[] {

  resetOverlapSlotTracking(manager.state);

  const slots = [new manager.SpeechRecognitionCtor!(), new manager.SpeechRecognitionCtor!()];

  slots[0].maxAlternatives = 1;

  slots[1].maxAlternatives = 1;

  manager.state.recognitionOverlapSlots = slots;

  manager.state.recognitionOverlapActiveSlot = 0;

  manager.state.recognitionOverlapPrestarted = false;

  manager.state.recognitionOverlapSlotListening = [false, false];

  manager.state.recognitionGenerationId = generationId;

  manager.state.recognition = slots[0];

  manager.applyRecognitionSettings();

  manager.wireRecognitionHandlers(slots[0], generationId, 0);

  manager.wireRecognitionHandlers(slots[1], generationId, 1);

  return slots;

}



export function prestartOverlapBuddyIfNeeded(manager: AsrManagerHost, overlapSlotIndex: number | null): void {

  if (overlapSlotIndex == null || !recognitionOverlapActive(manager.state)) {

    return;

  }

  if (Number(manager.state.recognitionOverlapActiveSlot || 0) !== overlapSlotIndex) {

    return;

  }

  if (manager.state.recognitionOverlapPrestarted) {

    return;

  }

  const slots = manager.state.recognitionOverlapSlots;

  if (!slots) {

    return;

  }

  const buddy = (overlapSlotIndex + 1) % 2;

  const buddyRec = slots[buddy];

  if (!buddyRec) {

    return;

  }

  if (manager.state.recognitionOverlapSlotListening && manager.state.recognitionOverlapSlotListening[buddy]) {

    manager.state.recognitionOverlapPrestarted = true;

    return;

  }

  try {

    buddyRec.start();

    manager.state.recognitionOverlapPrestarted = true;

    manager.appendLogInternal("overlap: pre-started buddy recognition slot");

  } catch (error) {

    const message = error instanceof Error ? error.message : String(error || "buddy start failed");

    manager.appendLogInternal(`overlap: buddy pre-start failed: ${message}`);

    scheduleOverlapTimeBasedPrestart(manager);

  }

}



export function handleOverlapRecognitionEnded(manager: AsrManagerHost, overlapSlotIndex: number): boolean {

  if (!recognitionOverlapActive(manager.state)) {

    return false;

  }

  if (!manager.state.recognitionOverlapSlotListening) {

    manager.state.recognitionOverlapSlotListening = [false, false];

  }

  manager.state.recognitionOverlapSlotListening[overlapSlotIndex] = false;

  ensureOverlapSlotTrackingArrays(manager.state);

  manager.state.recognitionOverlapSlotListenSinceMs![overlapSlotIndex] = null;

  manager.state.recognitionOverlapSlotActivityAtMs![overlapSlotIndex] = null;

  if (!manager.state.desiredRunning) {

    return false;

  }

  const active = Number(manager.state.recognitionOverlapActiveSlot || 0) % 2;

  const buddy = (active + 1) % 2;

  if (overlapSlotIndex === active) {

    if (manager.state.recognitionOverlapSlotListening[buddy]) {

      manager.state.recognitionOverlapActiveSlot = buddy;

      manager.state.recognition = manager.state.recognitionOverlapSlots![buddy];

      manager.state.recognitionOverlapPrestarted = false;

      manager.state.pendingRestartReason = null;

      onOverlapActiveSlotReady(manager, buddy);

      manager.setSupervisorStateInternal("running");

      manager.setRecognitionStateInternal("running");

      manager.emitWorkerStatus("recognition-ended");

      return true;

    }

  }

  return false;

}



export function evaluateOverlapBuddyGhost(state: BrowserAsrState, nowMs: number): boolean {

  if (!recognitionOverlapActive(state) || state.browserSupervisorState !== "running") {

    return false;

  }

  if (!state.recognitionOverlapPrestarted || !state.recognitionOverlapSlotListening) {

    return false;

  }

  const active = overlapActiveSlotIndex(state);

  const buddy = (active + 1) % 2;

  if (!state.recognitionOverlapSlotListening[active] || !state.recognitionOverlapSlotListening[buddy]) {

    return false;

  }

  ensureOverlapSlotTrackingArrays(state);

  const buddyListenSince = state.recognitionOverlapSlotListenSinceMs![buddy];

  if (buddyListenSince == null) {

    return false;

  }

  const limits = overlapLifecycleLimits(state);

  if (nowMs - buddyListenSince < limits.buddyGhostTimeoutMs) {

    return false;

  }

  const buddyActivity = state.recognitionOverlapSlotActivityAtMs![buddy];

  if (buddyActivity != null && buddyActivity >= buddyListenSince) {

    return false;

  }

  const lastResultAt = Number(state.lastResultAtMs || 0);

  const lastMicAt = Number(state.lastMicActivityAt || 0);

  const activeSlotActivity = state.recognitionOverlapSlotActivityAtMs![active];

  const activeMicRecent = lastMicAt > 0 && nowMs - lastMicAt <= limits.buddyGhostActiveMicMs;

  const activeResultsRecent = lastResultAt > 0 && nowMs - lastResultAt <= limits.buddyGhostActiveMicMs;

  const activeSlotRecentlyActive =

    activeSlotActivity != null && nowMs - activeSlotActivity <= limits.buddyGhostActiveMicMs;

  // Buddy silence while active transcribes is normal overlap handoff prep — never abort then.

  if (activeMicRecent || activeResultsRecent || activeSlotRecentlyActive) {

    return false;

  }

  const micQuietFor = lastMicAt > 0 ? nowMs - lastMicAt : limits.buddyGhostTimeoutMs;

  const resultsQuietFor = lastResultAt > 0 ? nowMs - lastResultAt : limits.buddyGhostTimeoutMs;

  const slotQuietFor =

    activeSlotActivity != null ? nowMs - activeSlotActivity : limits.buddyGhostTimeoutMs;

  const activeQuietForMs = Math.min(micQuietFor, resultsQuietFor, slotQuietFor);

  // Require sustained idle on active before treating buddy as a zombie (avoids inter-phrase false positives).

  if (activeQuietForMs < limits.buddyGhostTimeoutMs) {

    return false;

  }

  return true;

}



/** @returns true when a ghost buddy slot was aborted and prestart was retried */

export function recoverGhostOverlapBuddy(manager: AsrManagerHost, nowMs: number): boolean {

  if (!evaluateOverlapBuddyGhost(manager.state, nowMs)) {

    return false;

  }

  const active = overlapActiveSlotIndex(manager.state);

  const buddy = (active + 1) % 2;

  const slots = manager.state.recognitionOverlapSlots;

  const buddyRec = slots?.[buddy];

  if (!buddyRec) {

    return false;

  }

  manager.appendLogInternal(

    "overlap: aborting ghost buddy slot (silent buddy while both slots appear idle; retrying prestart)"

  );

  try {

    buddyRec.abort();

  } catch {

    // best effort

  }

  if (!manager.state.recognitionOverlapSlotListening) {

    manager.state.recognitionOverlapSlotListening = [false, false];

  }

  manager.state.recognitionOverlapSlotListening[buddy] = false;

  manager.state.recognitionOverlapPrestarted = false;

  ensureOverlapSlotTrackingArrays(manager.state);

  manager.state.recognitionOverlapSlotListenSinceMs![buddy] = null;

  manager.state.recognitionOverlapSlotActivityAtMs![buddy] = null;

  prestartOverlapBuddyIfNeeded(manager, active);

  manager.emitWorkerStatus("overlap-buddy-ghost-recovered");

  return true;

}


