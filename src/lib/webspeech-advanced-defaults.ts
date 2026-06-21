/** Default values for dashboard “Advanced Web Speech settings” (asr.browser + asr.realtime). */
export const WEBSPEECH_BROWSER_ADVANCED_DEFAULTS = {
  force_final_min_chars: 8,
  force_final_min_stable_ms: 750,
  minimum_reconnect_interval_ms: 500,
  normal_restart_delay_ms: 150,
  no_speech_restart_delay_ms: 150,
  stuck_stopping_timeout_ms: 2000,
  network_reconnect_initial_ms: 500,
  network_reconnect_max_ms: 30_000,
  max_browser_session_age_ms: 180_000,
  prepare_cycle_before_ms: 30_000,
} as const;

export const WEBSPEECH_REALTIME_ADVANCED_DEFAULTS = {
  partial_min_delta_chars: 0,
  partial_coalescing_ms: 0,
} as const;
