/** Reserved for optional backlog catch-up (disabled in speech-engine for audio quality). */
export const SPEECH_QUEUE_BOOST_THRESHOLD = 2;

export const SPEECH_STUCK_TIMEOUT_MS = 60_000;

export const QUEUE_BOOST_STEP = 0.12;

export const QUEUE_BOOST_MAX = 0.7;

export const PLAYBACK_RATE_MIN = 0.5;

export const PLAYBACK_RATE_MAX = 2.0;

/** Waiting items in Rust snapshot + the clip currently being played. */
export function queueDepthForPlayback(waitingCount: number): number {
  return Math.max(0, waitingCount) + 1;
}

export function clampPlaybackRate(rate: number): number {
  if (!Number.isFinite(rate) || rate <= 0) return 1;
  return Math.min(PLAYBACK_RATE_MAX, Math.max(PLAYBACK_RATE_MIN, rate));
}

export type PlaybackRateOptions = {
  /**
   * Keep the next audible clip at the configured base rate so sonic time-stretch
   * does not add decode latency before the first sample.
   */
  deferBoost?: boolean;
};

/**
 * Raise speech rate when the channel backlog exceeds {@link SPEECH_QUEUE_BOOST_THRESHOLD}.
 * Speech and Twitch each pass their own waiting count.
 */
export function effectivePlaybackRate(
  baseRate: number,
  queueDepth: number,
  options?: PlaybackRateOptions,
): number {
  const base = clampPlaybackRate(baseRate);
  const depth = options?.deferBoost
    ? Math.max(SPEECH_QUEUE_BOOST_THRESHOLD, queueDepth - 1)
    : queueDepth;
  if (depth <= SPEECH_QUEUE_BOOST_THRESHOLD) {
    return base;
  }
  const excess = depth - SPEECH_QUEUE_BOOST_THRESHOLD;
  const boost = Math.min(QUEUE_BOOST_MAX, excess * QUEUE_BOOST_STEP);
  return clampPlaybackRate(base + boost);
}
