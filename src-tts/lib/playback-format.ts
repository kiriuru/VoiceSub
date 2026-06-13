import { clampPlaybackRate } from "./speech-playback-policy";

/** Human-readable playback speed for TTS UI (matches engine clamp 0.5–2.0). */
export function formatPlaybackRate(rate: number | null | undefined): string {
  const numeric = typeof rate === "number" && Number.isFinite(rate) && rate > 0 ? rate : 1;
  return `${clampPlaybackRate(numeric).toFixed(2)}×`;
}

/** Human-readable volume for TTS UI (0–100%). */
export function formatSpeechVolume(volume: number | null | undefined): string {
  const numeric = typeof volume === "number" && Number.isFinite(volume) ? volume : 1;
  const clamped = Math.min(1, Math.max(0, numeric));
  return `${Math.round(clamped * 100)}%`;
}
