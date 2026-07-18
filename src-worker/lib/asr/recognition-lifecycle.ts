import type { AsrManagerHost } from "./types";
import type { WorkerSpeechRecognition } from "./speech-types";
import { ensureMicrophonePermission } from "./mic-permission-bridge";
import {
  createOverlapRecognitionPair,
  recognitionOverlapActive,
  recognitionOverlapModeDesired,
  resetOverlapSlotTracking,
} from "./overlap-logic";
import { wireRecognitionHandlers } from "./recognition-handlers";

function collectRecognitionInstances(manager: AsrManagerHost): WorkerSpeechRecognition[] {
  const slots: WorkerSpeechRecognition[] = [];
  if (recognitionOverlapActive(manager.state)) {
    (manager.state.recognitionOverlapSlots || []).forEach((rec) => {
      if (rec) {
        slots.push(rec);
      }
    });
  } else if (manager.state.recognition) {
    slots.push(manager.state.recognition);
  }
  return slots;
}

function handleRecognitionStartFailure(manager: AsrManagerHost, message: string): boolean {
  if (String(message).toLowerCase().includes("already started")) {
    manager.setSupervisorStateInternal("running");
    manager.setRecognitionStateInternal("running");
    manager.setStatusInternal("listening");
    return true;
  }
  manager.setRecognitionStateInternal("idle");
  manager.setSupervisorStateInternal("restarting");
  return false;
}

function invokeRecognitionStart(
  manager: AsrManagerHost,
  recognition: WorkerSpeechRecognition,
  reason: string,
  startLogThrottle: { gapMs: number; key: string | null },
  logSuffix: string | null
): boolean {
  try {
    recognition.start();
    const line = logSuffix ? `recognition.start ${logSuffix} (${reason})` : `recognition.start (${reason})`;
    if (startLogThrottle.key) {
      manager.appendLogThrottledInternal(line, startLogThrottle.key, startLogThrottle.gapMs);
    } else {
      manager.appendLogInternal(line);
    }
    return true;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error || "start failed");
    if (handleRecognitionStartFailure(manager, message)) {
      return true;
    }
    manager.appendLogInternal(
      `${logSuffix ? `recognition.start ${logSuffix}` : "recognition.start"} failed: ${message}`
    );
    manager.scheduleRestartInternal("network");
    return false;
  }
}

export function cleanupRecognitionInstance(manager: AsrManagerHost, generationId: number): void {
  if (generationId !== manager.state.recognitionGenerationId) {
    return;
  }
  collectRecognitionInstances(manager).forEach((recognition) => {
    try {
      recognition.abort();
    } catch {
      // best effort
    }
    recognition.onstart = null;
    recognition.onend = null;
    recognition.onerror = null;
    recognition.onresult = null;
    recognition.onsoundstart = null;
    recognition.onsoundend = null;
    recognition.onspeechstart = null;
    recognition.onspeechend = null;
    recognition.onaudiostart = null;
    recognition.onaudioend = null;
  });
  manager.state.recognitionOverlapSlots = null;
  manager.state.recognitionOverlapActiveSlot = null;
  manager.state.recognitionOverlapPrestarted = false;
  manager.state.recognitionOverlapSlotListening = null;
  resetOverlapSlotTracking(manager.state);
  manager.state.recognition = null;
}

export function createRecognition(manager: AsrManagerHost, generationId: number): WorkerSpeechRecognition {
  const recognition = new manager.SpeechRecognitionCtor!();
  recognition.maxAlternatives = 1;
  manager.state.recognitionGenerationId = generationId;
  manager.state.recognitionOverlapSlots = null;
  manager.state.recognitionOverlapActiveSlot = null;
  manager.state.recognitionOverlapPrestarted = false;
  manager.state.recognitionOverlapSlotListening = null;
  resetOverlapSlotTracking(manager.state);
  manager.state.recognition = recognition;
  manager.applyRecognitionSettings();
  wireRecognitionHandlers(manager, recognition, generationId, null);
  return recognition;
}

export function performControlledStart(manager: AsrManagerHost, reason: string): void {
  if (!manager.state.desiredRunning) {
    return;
  }
  if (manager.state.browserSupervisorState === "starting" || manager.state.browserSupervisorState === "running") {
    return;
  }
  if (manager.state.browserSupervisorState === "stopping") {
    manager.state.pendingStart = true;
    manager.appendLogInternal("recognition.start deferred: recognition is stopping");
    return;
  }
  // Bump generation on every controlled start (including session_cycle) so late events
  // from the previous recognition are rejected server-side and by local handlers.
  manager.state.generationId = Number(manager.state.generationId || 0) + 1;
  const generationId = Number(manager.state.generationId || 0);
  cleanupRecognitionInstance(manager, manager.state.recognitionGenerationId);
  manager.setSupervisorStateInternal("starting");
  manager.setRecognitionStateInternal("starting");
  manager.state.stoppingSinceMs = null;
  manager.state.providerName = manager.state.browserMode || "browser_google";
  manager.state.pendingRestartReason = null;
  manager.ensureSocketConnectedInternal();
  const startLogThrottle = manager.recognitionStartBurstThrottleInternal(reason);
  const policy = manager.webSpeechPolicy();
  const settings = manager.getRecognitionSettings();
  if (recognitionOverlapModeDesired(settings, policy)) {
    createOverlapRecognitionPair(manager, generationId);
    invokeRecognitionStart(
      manager,
      manager.state.recognitionOverlapSlots![0]!,
      reason,
      startLogThrottle,
      "overlap slot0"
    );
    return;
  }
  const recognition = createRecognition(manager, generationId);
  invokeRecognitionStart(manager, recognition, reason, startLogThrottle, null);
}

export function requestRecognitionFlush(manager: AsrManagerHost, reason: string): void {
  if (!manager.state.desiredRunning) {
    return;
  }
  if (
    manager.state.browserSupervisorState === "stopping" ||
    manager.state.browserSupervisorState === "starting"
  ) {
    return;
  }
  const slots = collectRecognitionInstances(manager);
  if (!slots.length) {
    manager.scheduleRestartInternal(reason);
    return;
  }
  manager.state.pendingStart = true;
  manager.state.pendingRestartReason = reason;
  manager.clearForceFinalizeTimerInternal();
  if (manager.state.browserSupervisorState !== "stopping") {
    manager.setSupervisorStateInternal("stopping");
  }
  manager.setRecognitionStateInternal("stopping");
  manager.state.stoppingSinceMs = manager.now();
  manager.setStatusInternal("stopping");
  try {
    slots.forEach((rec) => {
      try {
        rec.stop();
      } catch {
        // best effort
      }
    });
    manager.appendLogInternal(`recognition.flush (${reason})`);
  } catch {
    cleanupRecognitionInstance(manager, manager.state.recognitionGenerationId);
    manager.setRecognitionStateInternal("idle");
    manager.setSupervisorStateInternal(manager.state.desiredRunning ? "restarting" : "idle");
    if (manager.state.desiredRunning) {
      manager.scheduleRestartInternal(manager.state.pendingRestartReason || reason);
    }
  }
}

export function transitionToStopping(manager: AsrManagerHost, reason: string): void {
  const slots = collectRecognitionInstances(manager);
  if (reason !== "user-stop" && reason !== "long_segment_flush") {
    manager.forceFinalizeOnInterruptionInternal("browser_recognition_interrupted");
  }
  if (!slots.length) {
    cleanupRecognitionInstance(manager, manager.state.recognitionGenerationId);
    manager.setRecognitionStateInternal("idle");
    manager.setSupervisorStateInternal(manager.state.desiredRunning ? "restarting" : "idle");
    manager.setStatusInternal(manager.state.desiredRunning ? "restarting" : "stopped");
    if (manager.state.desiredRunning) {
      manager.scheduleRestartInternal(manager.state.pendingRestartReason || "normal_onend");
    }
    return;
  }
  if (manager.state.browserSupervisorState !== "stopping") {
    manager.setSupervisorStateInternal("stopping");
  }
  manager.setRecognitionStateInternal("stopping");
  manager.state.stoppingSinceMs = manager.now();
  manager.setStatusInternal("stopping");
  try {
    slots.forEach((rec) => {
      try {
        rec.stop();
      } catch {
        // best effort
      }
    });
    manager.appendLogInternal(`recognition.stop (${reason})`);
  } catch {
    cleanupRecognitionInstance(manager, manager.state.recognitionGenerationId);
    manager.setRecognitionStateInternal("idle");
    manager.setSupervisorStateInternal(manager.state.desiredRunning ? "restarting" : "idle");
    if (manager.state.desiredRunning) {
      manager.scheduleRestartInternal(manager.state.pendingRestartReason || "normal_onend");
    }
  }
}

export function scheduleRestart(
  manager: AsrManagerHost,
  reason: string,
  options: { backoffMs?: number } = {}
): void {
  if (!manager.state.desiredRunning) {
    manager.setSupervisorStateInternal("idle");
    return;
  }
  const normalizedReason = String(reason || "normal_onend")
    .trim()
    .toLowerCase();
  const requestedDelayMs = Math.max(
    0,
    Number(options.backoffMs != null ? options.backoffMs : manager.restartDelayForReasonInternal(normalizedReason))
  );
  const delayMs = manager.minimumReconnectGuardDelayMsInternal(requestedDelayMs);
  manager.clearRestartTimerInternal();
  manager.state.restartCount = Number(manager.state.restartCount || 0) + 1;
  manager.state.lastRestartReason = normalizedReason;
  manager.setSupervisorStateInternal(
    delayMs > (manager.restartDelayByReasonMs.normal_onend ?? 0) ? "backoff" : "restarting"
  );
  manager.setStatusInternal("restarting");
  const capturedStopEpoch = Number(manager.state.stopEpoch || 0);
  manager.state.restartTimer = window.setTimeout(() => {
    if (!manager.state.desiredRunning) {
      return;
    }
    // Only user/control stop cancels a scheduled restart (not generation bumps on cycle).
    if (capturedStopEpoch !== Number(manager.state.stopEpoch || 0)) {
      return;
    }
    if (manager.state.browserSupervisorState === "stopping") {
      manager.state.pendingStart = true;
      return;
    }
    void resumeRecognitionAfterRestartDelay(manager, normalizedReason);
  }, delayMs);
  manager.emitWorkerStatus("restart-scheduled");
}

async function resumeRecognitionAfterRestartDelay(
  manager: AsrManagerHost,
  normalizedReason: string
): Promise<void> {
  if (!manager.state.desiredRunning) {
    return;
  }
  if (normalizedReason === "audio_capture") {
    try {
      await ensureMicrophonePermission(manager);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error || "microphone re-acquire failed");
      manager.setLastErrorInternal("audio-capture", message);
      manager.state.pendingRestartReason = "audio_capture";
      manager.appendLogInternal(`audio-capture retry: microphone re-acquire failed: ${message}`);
      manager.scheduleRestartInternal("audio_capture");
      return;
    }
  }
  performControlledStart(manager, normalizedReason);
}
