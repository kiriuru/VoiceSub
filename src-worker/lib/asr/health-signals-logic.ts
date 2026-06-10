import type { BrowserAsrState, TimingLimits } from "./types";

export function computeHealthDegradedReason(ctx: {
  state: BrowserAsrState;
  nowMs: number;
  documentHidden: boolean;
  limits: TimingLimits;
}): string | null {
  const { state, nowMs, limits } = ctx;
  const trackReadyState = String(state.micTrackReadyState || "")
    .trim()
    .toLowerCase();
  const micActivityAgeMs = state.lastMicActivityAt > 0 ? Math.max(0, nowMs - Number(state.lastMicActivityAt)) : null;
  const recognitionQuietMs = Math.max(
    0,
    nowMs - Math.max(Number(state.lastEventAtMs || 0), Number(state.lastResultAtMs || 0), Number(state.lastStartAtMs || 0))
  );
  state.micActiveRecentMs = micActivityAgeMs;

  if (!state.desiredRunning) {
    return null;
  }
  if (trackReadyState && trackReadyState !== "live") {
    return "mic_track_unavailable";
  }
  if (
    !ctx.documentHidden &&
    state.browserSupervisorState === "running" &&
    micActivityAgeMs != null &&
    micActivityAgeMs >= limits.micSilentDegradedAfterMs
  ) {
    return "mic_silent";
  }
  const micRms = Number(state.micRms || 0);
  const voiceLevelGoodRecently =
    micRms >= limits.voiceBelowRecognitionRmsThreshold ||
    (micActivityAgeMs != null &&
      micActivityAgeMs <= limits.voiceBelowRecognitionMicWindowMs &&
      Number(state.noSpeechCount || 0) >= limits.voiceBelowRecognitionMinNoSpeech);
  if (
    !ctx.documentHidden &&
    state.browserSupervisorState === "running" &&
    recognitionQuietMs >= limits.voiceBelowRecognitionGraceMs &&
    voiceLevelGoodRecently &&
    Number(state.noSpeechCount || 0) >= limits.voiceBelowRecognitionMinNoSpeech
  ) {
    return "voice_below_recognition_threshold";
  }
  if (
    !ctx.documentHidden &&
    state.browserSupervisorState === "running" &&
    recognitionQuietMs >= limits.stallDegradedAfterMs &&
    micActivityAgeMs != null &&
    micActivityAgeMs <= limits.recentMicActivityWindowMs
  ) {
    return "web_speech_stalled";
  }
  return null;
}
