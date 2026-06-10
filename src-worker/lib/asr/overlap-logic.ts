import type { AsrManagerHost, BrowserAsrState, RecognitionSettings } from "./types";
import type { WorkerSpeechRecognition } from "./speech-types";
import { webSpeechRecognitionPolicy } from "./web-speech-policy";

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
      manager.setSupervisorStateInternal("running");
      manager.setRecognitionStateInternal("running");
      manager.emitWorkerStatus("recognition-ended");
      return true;
    }
  }
  return false;
}
