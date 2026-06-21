import { clampPlaybackRate, clampSpeechVolume } from "./speech-playback-policy";

/** Human-readable playback speed for TTS UI (matches engine clamp 0.5–2.0). */
export function formatPlaybackRate(rate: number | null | undefined): string {
  const numeric = typeof rate === "number" && Number.isFinite(rate) && rate > 0 ? rate : 1;
  return `${clampPlaybackRate(numeric).toFixed(2)}×`;
}

/** Human-readable volume for TTS UI (0–150%). */
export function formatSpeechVolume(volume: number | null | undefined): string {
  const clamped = clampSpeechVolume(
    typeof volume === "number" && Number.isFinite(volume) ? volume : 1,
  );
  return `${Math.round(clamped * 100)}%`;
}
