import type { BrowserAsrState } from "./types";

export function normalizeTranscriptText(value: string): string {
  return String(value || "")
    .trim()
    .replace(/\s+/g, " ");
}

export function currentGenerationId(state: BrowserAsrState): number {
  return Number(state.generationId || 0);
}

export function ensureClientSegmentId(state: BrowserAsrState): string {
  if (state.currentClientSegmentId && !state.currentSegmentForcedFinalized) {
    return state.currentClientSegmentId;
  }
  state.nextClientSegmentOrdinal = Number(state.nextClientSegmentOrdinal || 0) + 1;
  const ordinal = state.nextClientSegmentOrdinal;
  const sessionId = String(state.sessionId || "browser-worker").replace(/[^a-z0-9_-]+/gi, "-");
  const generationId = currentGenerationId(state);
  state.currentClientSegmentId = `${sessionId}-g${generationId}-s${ordinal}`;
  state.currentSegmentLastPartialText = "";
  state.currentSegmentLastFinalText = "";
  state.currentSegmentForcedFinalized = false;
  return state.currentClientSegmentId;
}

export function consumeCompletedSegment(state: BrowserAsrState): void {
  state.currentClientSegmentId = null;
  state.currentSegmentLastPartialText = "";
  state.currentSegmentLastFinalText = "";
  state.currentSegmentForcedFinalized = false;
}

export function resetSegmentTrackingFields(state: BrowserAsrState): void {
  state.currentClientSegmentId = null;
  state.currentSegmentLastPartialText = "";
  state.currentSegmentLastFinalText = "";
  state.currentPartialStableSinceMs = 0;
  state.currentSegmentForcedFinalized = false;
  state.lastForcedFinal = null;
}

export function shouldSuppressDuplicatePartial(state: BrowserAsrState, text: string): boolean {
  const normalizedText = normalizeTranscriptText(text);
  if (!normalizedText) {
    return true;
  }
  if (normalizedText === state.currentSegmentLastPartialText) {
    state.duplicatePartialSuppressed = Number(state.duplicatePartialSuppressed || 0) + 1;
    return true;
  }
  return false;
}

export function shouldSuppressFinal(
  state: BrowserAsrState,
  text: string,
  { forcedFinal = false }: { forcedFinal?: boolean } = {}
): boolean {
  const normalizedText = normalizeTranscriptText(text);
  if (!normalizedText) {
    return true;
  }
  const lateForcedFinal = state.lastForcedFinal;
  if (
    !forcedFinal &&
    state.currentSegmentForcedFinalized &&
    lateForcedFinal &&
    Number(lateForcedFinal.generation_id || 0) === currentGenerationId(state) &&
    normalizeTranscriptText(lateForcedFinal.text) === normalizedText
  ) {
    state.lateForcedFinalSuppressed = Number(state.lateForcedFinalSuppressed || 0) + 1;
    consumeCompletedSegment(state);
    return true;
  }
  if (normalizedText === state.currentSegmentLastFinalText) {
    state.duplicateFinalSuppressed = Number(state.duplicateFinalSuppressed || 0) + 1;
    return true;
  }
  return false;
}

export function canForceFinalizeOnInterruption(
  state: BrowserAsrState,
  isForceFinalizationEnabled: boolean
): boolean {
  if (!state.forceFinalOnInterruption || isForceFinalizationEnabled === false) {
    return false;
  }
  const normalizedText = normalizeTranscriptText(state.currentPartial);
  if (!normalizedText || normalizedText.length < Math.max(1, Number(state.forceFinalMinChars || 0))) {
    return false;
  }
  if (normalizedText === state.currentSegmentLastFinalText) {
    return false;
  }
  const stableSinceMs = Number(state.currentPartialStableSinceMs || 0);
  if (!stableSinceMs) {
    return false;
  }
  return Date.now() - stableSinceMs >= Math.max(0, Number(state.forceFinalMinStableMs || 0));
}

export function buildTranscriptUpdatePayload(
  state: BrowserAsrState,
  payload: Record<string, unknown>,
  nowMs: number
): Record<string, unknown> {
  state.workerTranscriptMessageSequence = Number(state.workerTranscriptMessageSequence || 0) + 1;
  return {
    partial: payload.partial || "",
    final: payload.final || "",
    is_final: Boolean(payload.is_final),
    source_lang: payload.source_lang || state.sourceLang || "auto",
    client_segment_id: payload.client_segment_id || state.currentClientSegmentId || null,
    forced_final: Boolean(payload.forced_final),
    forced_final_reason: payload.forced_final_reason || null,
    asr_result_created_at_ms: payload.asr_result_created_at_ms || nowMs,
    worker_send_started_at_ms: nowMs,
    worker_message_sequence: state.workerTranscriptMessageSequence,
  };
}

export function markResultActivity(state: BrowserAsrState, nowMs: number): void {
  state.lastEventAtMs = nowMs;
  state.lastResultAtMs = state.lastEventAtMs;
  state.noSpeechBackoffMs = 0;
  state.restartBackoffMs = 0;
}
