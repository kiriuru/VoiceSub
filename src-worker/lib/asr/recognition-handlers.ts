import type { AsrManagerHost, BrowserAsrState } from "./types";
import type {
  WorkerSpeechRecognition,
  WorkerSpeechRecognitionErrorEvent,
  WorkerSpeechRecognitionEvent,
} from "./speech-types";
import { classifyRecognitionError, networkErrorHintMessages } from "./recognition-error-logic";
import { parseRecognitionResultEvent } from "./recognition-result-logic";
import {
  handleInactiveOverlapBuddyEnded,
  handleOverlapRecognitionEnded,
  markOverlapSlotActivity,
  onOverlapActiveSlotReady,
  overlapActiveSlotIndex,
  overlapResultAllowed,
  overlapSlotInactive,
  preStartNextOverlapInstance,
  recognitionOverlapActive,
  shouldIgnoreOverlapBuddyError,
} from "./overlap-logic";
import {
  maybeFlushAfterCommittedLongSegment,
  noteSegmentPartialPeak,
} from "./long-segment-flush-logic";
import { registerNetworkErrorForPreflight } from "./network-preflight-bridge";

function shouldIgnoreAbortedOverlapActiveGuard(
  state: BrowserAsrState,
  overlapSlotIndex: number | null
): boolean {
  if (!recognitionOverlapActive(state) || overlapSlotIndex == null) {
    return false;
  }
  const active = overlapActiveSlotIndex(state);
  const buddy = (active + 1) % 2;
  if (overlapSlotIndex !== active) {
    return false;
  }
  const buddyListening = Boolean(state.recognitionOverlapSlotListening?.[buddy]);
  const buddyPrestarted = Boolean(state.recognitionOverlapPrestarted);
  return buddyListening || buddyPrestarted;
}

function applyRecognitionError(
  manager: AsrManagerHost,
  _generationId: number,
  overlapSlotIndex: number | null,
  event: WorkerSpeechRecognitionErrorEvent
): void {
  const policy = manager.webSpeechPolicy();
  const classified = classifyRecognitionError(event, policy, manager.state);
  const { errorKind, errorMessage } = classified;

  if (classified.kind === "aborted" && shouldIgnoreAbortedOverlapActiveGuard(manager.state, overlapSlotIndex)) {
    return;
  }

  manager.setLastErrorInternal(errorKind, errorMessage);
  manager.markActivityInternal("error");

  switch (classified.kind) {
    case "phrases_retry":
      manager.state.webSpeechPhraseHintsSuppressed = true;
      manager.state.pendingRestartReason = "normal_onend";
      manager.setStatusInternal("restarting");
      manager.appendLogInternal(manager.translate("browser_asr.error.phrases_retry"));
      manager.emitWorkerStatus("recognition-error");
      return;
    case "language_retry": {
      manager.state.webSpeechLanguageSoftFallbackUsed = true;
      const stripTargets = recognitionOverlapActive(manager.state)
        ? manager.state.recognitionOverlapSlots || []
        : manager.state.recognition
          ? [manager.state.recognition]
          : [];
      stripTargets.forEach((rec) => {
        if (rec) {
          manager.stripChromeOnDeviceHints(rec);
        }
      });
      manager.state.pendingRestartReason = "normal_onend";
      manager.setStatusInternal("restarting");
      manager.appendLogInternal(manager.translate("browser_asr.error.language_retry"));
      manager.emitWorkerStatus("recognition-error");
      return;
    }
    case "no_speech":
      manager.state.noSpeechCount = Number(manager.state.noSpeechCount || 0) + 1;
      manager.state.pendingRestartReason = "no_speech";
      manager.setStatusInternal("restarting");
      manager.emitWorkerStatus("recognition-error");
      return;
    case "network":
      manager.state.networkErrorCount = Number(manager.state.networkErrorCount || 0) + 1;
      manager.state.pendingRestartReason = "network";
      manager.setSupervisorStateInternal("backoff");
      manager.setStatusInternal("socket-reconnecting");
      {
        const now = manager.now();
        const last = Number(manager._lastWebSpeechNetworkHintAtMs || 0);
        if (now - last > 15000) {
          manager._lastWebSpeechNetworkHintAtMs = now;
          manager.appendLogInternal(networkErrorHintMessages((key) => manager.translate(key)));
        }
      }
      registerNetworkErrorForPreflight(manager);
      manager.emitWorkerStatus("recognition-error");
      return;
    case "aborted":
      if (manager.state.desiredRunning) {
        manager.state.pendingRestartReason = "normal_onend";
      }
      manager.emitWorkerStatus("recognition-error");
      return;
    case "terminal_permission":
      manager.state.desiredRunning = false;
      manager.state.pendingStart = false;
      manager.clearAllTimersInternal();
      manager.setSupervisorStateInternal("fatal");
      manager.setStatusInternal(manager.translate("browser_asr.error.terminal_status", { errorKind }));
      manager.setTerminalDegradedReasonInternal(classified.terminalReason || "permission_denied");
      manager.emitWorkerStatus("terminal-error");
      return;
    case "terminal_language":
      manager.state.desiredRunning = false;
      manager.state.pendingStart = false;
      manager.clearAllTimersInternal();
      manager.setSupervisorStateInternal("fatal");
      manager.setStatusInternal(manager.translate("browser_asr.error.terminal_status", { errorKind }));
      manager.setTerminalDegradedReasonInternal(classified.terminalReason || "language_not_supported");
      manager.emitWorkerStatus("terminal-error");
      return;
    default:
      break;
  }
}

function handleRecognitionResult(
  manager: AsrManagerHost,
  generationId: number,
  overlapSlotIndex: number | null,
  event: WorkerSpeechRecognitionEvent
): void {
  if (!manager.isActiveGeneration(generationId)) {
    return;
  }
  if (!overlapResultAllowed(manager.state, overlapSlotIndex)) {
    return;
  }
  if (overlapSlotIndex != null) {
    markOverlapSlotActivity(manager.state, overlapSlotIndex, manager.now());
  }
  const { interimText, finalText, resultIndex } = parseRecognitionResultEvent(event);
  manager.state.lastResultIndex = resultIndex;
  manager.state.restartBackoffMs = 0;

  if (interimText) {
    manager.markActivityInternal("result");
    const clientSegmentId = manager.ensureClientSegmentIdInternal();
    const nowMs = manager.now();
    const normalizedInterimText = manager.normalizeTranscriptTextInternal(interimText);
    if (normalizedInterimText !== manager.state.currentSegmentLastPartialText) {
      manager.state.currentPartialStableSinceMs = nowMs;
    }
    manager.state.currentPartial = interimText;
    manager.options.setPartialText?.(interimText);
    if (!manager.shouldSuppressDuplicatePartialInternal(interimText)) {
      manager.state.currentSegmentLastPartialText = normalizedInterimText;
      manager.state.currentSegmentForcedFinalized = false;
      noteSegmentPartialPeak(manager.state, interimText);
      manager.sendUpdateInternal({
        partial: interimText,
        final: "",
        is_final: false,
        source_lang: manager.state.sourceLang,
        client_segment_id: clientSegmentId,
        forced_final: false,
      });
    }
    manager.scheduleForceFinalizeInternal();
    manager.setStatusInternal("interim");
  }

  if (finalText) {
    manager.markActivityInternal("result");
    const clientSegmentId = manager.state.currentClientSegmentId || manager.ensureClientSegmentIdInternal();
    if (manager.shouldSuppressFinalInternal(finalText)) {
      manager.clearForceFinalizeTimerInternal();
      manager.state.currentPartial = "";
      manager.options.setPartialText?.("");
      manager.emitWorkerStatus("result");
      manager.updateCountersInternal();
      return;
    }
    manager.clearForceFinalizeTimerInternal();
    manager.state.currentPartial = "";
    manager.state.currentPartialStableSinceMs = 0;
    manager.state.finalCount = Number(manager.state.finalCount || 0) + 1;
    manager.state.currentSegmentLastFinalText = manager.normalizeTranscriptTextInternal(finalText);
    manager.options.setFinalText?.(finalText);
    manager.options.setPartialText?.("");
    manager.sendUpdateInternal({
      partial: "",
      final: finalText,
      is_final: true,
      source_lang: manager.state.sourceLang,
      client_segment_id: clientSegmentId,
      forced_final: false,
    });
    manager.consumeCompletedSegmentInternal();
    // Pre-start buddy only on segment final (hasNewFinal).
    if (overlapSlotIndex === overlapActiveSlotIndex(manager.state)) {
      preStartNextOverlapInstance(manager, "natural-final");
    }
    maybeFlushAfterCommittedLongSegment(manager, finalText, "natural-final");
    manager.setStatusInternal("final");
  }

  manager.emitWorkerStatus("result");
  manager.updateCountersInternal();
}

export function wireRecognitionHandlers(
  manager: AsrManagerHost,
  recognition: WorkerSpeechRecognition,
  generationId: number,
  overlapSlotIndex: number | null
): void {
  recognition.onstart = () => {
    if (!manager.isActiveGeneration(generationId)) {
      return;
    }
    if (overlapSlotIndex != null) {
      if (!manager.state.recognitionOverlapSlotListening) {
        manager.state.recognitionOverlapSlotListening = [false, false];
      }
      manager.state.recognitionOverlapSlotListening[overlapSlotIndex] = true;
      onOverlapActiveSlotReady(manager, overlapSlotIndex);
      if (overlapSlotInactive(manager.state, overlapSlotIndex)) {
        manager.markActivityInternal("start");
        return;
      }
    }
    manager.state.lastStartAtMs = manager.now();
    manager.state.lastSessionStartedAtMs = manager.state.lastStartAtMs;
    manager.state.stoppingSinceMs = null;
    manager.setLastErrorInternal(null, null);
    manager.state.noSpeechBackoffMs = 0;
    manager.state.restartBackoffMs = 0;
    manager.setTerminalDegradedReasonInternal(null);
    manager.state.pendingRestartReason = null;
    manager.state.browserCyclePending = false;
    manager.setRecognitionStateInternal("running");
    manager.setSupervisorStateInternal("running");
    manager.setStatusInternal("listening");
    manager.state.visibilityDegraded = Boolean(document.hidden && manager.state.desiredRunning);
    manager.refreshDegradedReasonInternal();
    manager.markActivityInternal("start");
    manager.emitWorkerStatus("recognition-started");
  };

  recognition.onsoundstart = () => {
    if (!manager.isActiveGeneration(generationId)) {
      return;
    }
    if (overlapSlotInactive(manager.state, overlapSlotIndex)) {
      return;
    }
    if (overlapSlotIndex != null) {
      markOverlapSlotActivity(manager.state, overlapSlotIndex, manager.now());
    }
    manager.state.onSound = true;
    manager.markActivityInternal("sound");
    manager.updateCountersInternal();
  };

  recognition.onsoundend = () => {
    if (!manager.isActiveGeneration(generationId)) {
      return;
    }
    if (overlapSlotInactive(manager.state, overlapSlotIndex)) {
      return;
    }
    manager.state.onSound = false;
    manager.updateCountersInternal();
  };

  recognition.onspeechstart = () => {
    if (!manager.isActiveGeneration(generationId)) {
      return;
    }
    if (overlapSlotInactive(manager.state, overlapSlotIndex)) {
      return;
    }
    if (overlapSlotIndex != null) {
      markOverlapSlotActivity(manager.state, overlapSlotIndex, manager.now());
    }
    manager.markActivityInternal("speech");
  };

  recognition.onerror = (event: WorkerSpeechRecognitionErrorEvent) => {
    if (!manager.isActiveGeneration(generationId)) {
      return;
    }
    const errorKind = String(event?.error || "")
      .trim()
      .toLowerCase();
    if (shouldIgnoreOverlapBuddyError(manager.state, overlapSlotIndex, errorKind)) {
      manager.emitWorkerStatus("overlap-buddy-error");
      return;
    }
    applyRecognitionError(manager, generationId, overlapSlotIndex, event);
  };

  recognition.onend = () => {
    if (!manager.isActiveGeneration(generationId)) {
      return;
    }
    if (!manager.state.desiredRunning) {
      manager.state.lastEndAtMs = manager.now();
      manager.state.lastSessionEndedAtMs = manager.state.lastEndAtMs;
      manager.state.onSound = false;
      manager.setRecognitionStateInternal("idle");
      manager.cleanupRecognitionInstance(generationId);
      manager.resetSegmentTrackingInternal();
      manager.setSupervisorStateInternal("idle");
      manager.setStatusInternal("stopped");
      manager.emitWorkerStatus("recognition-ended");
      return;
    }
    if (overlapSlotIndex != null && handleInactiveOverlapBuddyEnded(manager, overlapSlotIndex)) {
      return;
    }
    manager.state.lastEndAtMs = manager.now();
    manager.state.lastSessionEndedAtMs = manager.state.lastEndAtMs;
    manager.state.onSound = false;
    manager.setRecognitionStateInternal("idle");
    if (overlapSlotIndex != null && handleOverlapRecognitionEnded(manager, overlapSlotIndex)) {
      return;
    }
    manager.cleanupRecognitionInstance(generationId);
    manager.emitWorkerStatus("recognition-ended");
    if (manager.state.pendingStart) {
      manager.state.pendingStart = false;
      const pendingReason = manager.state.pendingRestartReason || "normal_onend";
      manager.state.pendingRestartReason = null;
      manager.scheduleRestartInternal(pendingReason);
      return;
    }
    const restartReason =
      manager.state.pendingRestartReason ||
      (manager.state.lastErrorKind === "network" ? "network" : null) ||
      (manager.state.lastErrorKind === "no-speech" ? "no_speech" : null) ||
      "normal_onend";
    manager.state.pendingRestartReason = null;
    manager.scheduleRestartInternal(restartReason);
  };

  recognition.onresult = (event: WorkerSpeechRecognitionEvent) => {
    handleRecognitionResult(manager, generationId, overlapSlotIndex, event);
  };
}
