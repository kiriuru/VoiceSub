import type { AsrManagerHost, BrowserAsrState } from "./types";
import { normalizeTranscriptText } from "./transcript-logic";
import { overlapActiveSlotIndex, recognitionOverlapActive } from "./overlap-logic";
import { requestRecognitionFlush } from "./recognition-lifecycle";

export const DEFAULT_LONG_SEGMENT_FLUSH_MIN_CHARS = 200;

export function shouldFlushAfterLongSegment(
  state: BrowserAsrState,
  finalText: string,
  minChars = DEFAULT_LONG_SEGMENT_FLUSH_MIN_CHARS,
): boolean {
  if (!state.desiredRunning) {
    return false;
  }
  if (state.browserSupervisorState === "stopping" || state.browserSupervisorState === "starting") {
    return false;
  }
  if (state.pendingRestartReason) {
    return false;
  }
  const normalizedFinal = normalizeTranscriptText(finalText);
  if (!normalizedFinal) {
    return false;
  }
  const peak = Math.max(
    Number(state.currentSegmentPeakPartialChars || 0),
    normalizedFinal.length,
  );
  const threshold = Math.max(50, Number(minChars || DEFAULT_LONG_SEGMENT_FLUSH_MIN_CHARS));
  return peak >= threshold;
}

export function noteSegmentPartialPeak(state: BrowserAsrState, partialText: string): void {
  const normalized = normalizeTranscriptText(partialText);
  if (!normalized) {
    return;
  }
  const nextPeak = normalized.length;
  if (nextPeak > Number(state.currentSegmentPeakPartialChars || 0)) {
    state.currentSegmentPeakPartialChars = nextPeak;
  }
}

export function resetSegmentPartialPeak(state: BrowserAsrState): void {
  state.currentSegmentPeakPartialChars = 0;
}

function flushOverlapActiveSlot(manager: AsrManagerHost, reason: string): void {
  if (!recognitionOverlapActive(manager.state)) {
    return;
  }
  const active = overlapActiveSlotIndex(manager.state);
  const activeRec = manager.state.recognitionOverlapSlots?.[active];
  if (!activeRec) {
    return;
  }
  try {
    activeRec.stop();
    manager.appendLogInternal(`overlap: active slot flush (${reason})`);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error || "stop failed");
    manager.appendLogInternal(`overlap: active slot flush failed (${reason}): ${message}`);
  }
}

/**
 * After a committed long segment, Web Speech can keep a bloated results buffer in
 * native_continuous mode and emit many short finals. Overlap mode may keep the active
 * slot alive too long without a clean buddy handoff. Flush only on segment boundary.
 */
export function maybeFlushAfterCommittedLongSegment(
  manager: AsrManagerHost,
  finalText: string,
  source: string,
): void {
  if (!shouldFlushAfterLongSegment(manager.state, finalText)) {
    return;
  }
  manager.state.longSegmentFlushCount = Number(manager.state.longSegmentFlushCount || 0) + 1;
  resetSegmentPartialPeak(manager.state);
  const reason = "long_segment_flush";
  if (recognitionOverlapActive(manager.state)) {
    flushOverlapActiveSlot(manager, source);
    return;
  }
  if (manager.state.actualContinuous !== false) {
    requestRecognitionFlush(manager, reason);
    manager.appendLogInternal(`recognition long-segment flush scheduled (${source})`);
  }
}
