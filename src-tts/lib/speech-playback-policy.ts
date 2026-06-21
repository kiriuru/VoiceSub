export const PLAYBACK_RATE_MIN = 0.5;

export const PLAYBACK_RATE_MAX = 2.0;

export const PLAYBACK_VOLUME_MAX = 1.5;

export function clampSpeechVolume(volume: number): number {
  if (!Number.isFinite(volume)) return 1;
  return Math.min(PLAYBACK_VOLUME_MAX, Math.max(0, volume));
}

export function clampPlaybackRate(rate: number): number {
  if (!Number.isFinite(rate) || rate <= 0) return 1;
  return Math.min(PLAYBACK_RATE_MAX, Math.max(PLAYBACK_RATE_MIN, rate));
}
