/**
 * Parakeet low-latency realtime presets — must stay aligned with
 * backend/core/runtime/local_asr_realtime_settings.py preset tables.
 */

export const PARAKEET_LATENCY_PRESET_IDS = ["ultra_low_latency", "balanced", "quality", "custom"];

/** @type {Record<string, { labelKey: string, realtime: object, lifecycle: object }>} */
export const PARAKEET_LATENCY_PRESET_BUNDLES = {
  ultra_low_latency: {
    labelKey: "tuning.preset.ultra_low_latency",
    realtime: {
      first_partial_min_speech_ms: 120,
      partial_emit_interval_ms: 240,
      silence_hold_ms: 120,
      finalization_hold_ms: 220,
      partial_emit_mode: "word_growth",
      partial_min_new_words: 1,
      partial_min_delta_chars: 0,
      partial_coalescing_ms: 0,
      streaming_decode: true,
    },
    lifecycle: {
      pause_to_finalize_ms: 220,
    },
  },
  balanced: {
    labelKey: "tuning.preset.balanced",
    realtime: {
      first_partial_min_speech_ms: 180,
      partial_emit_interval_ms: 280,
      silence_hold_ms: 180,
      finalization_hold_ms: 350,
      partial_emit_mode: "word_growth",
      partial_min_new_words: 1,
      partial_min_delta_chars: 0,
      partial_coalescing_ms: 0,
      streaming_decode: true,
    },
    lifecycle: {
      pause_to_finalize_ms: 350,
    },
  },
  quality: {
    labelKey: "tuning.preset.quality",
    realtime: {
      first_partial_min_speech_ms: 260,
      partial_emit_interval_ms: 650,
      silence_hold_ms: 260,
      finalization_hold_ms: 520,
      partial_emit_mode: "word_growth",
      partial_min_new_words: 1,
      partial_min_delta_chars: 1,
      partial_coalescing_ms: 80,
      streaming_decode: true,
    },
    lifecycle: {
      pause_to_finalize_ms: 520,
    },
  },
};

/**
 * Quick Tuning slider positions (1..5) matching SIMPLE_TUNING_OPTIONS indices
 * for each named latency preset — keeps preset row and sliders visually aligned.
 */
export const PARAKEET_LATENCY_PRESET_SIMPLE_LEVELS = {
  ultra_low_latency: { appearance: 5, finish: 5, stability: 3 },
  balanced: { appearance: 3, finish: 3, stability: 3 },
  quality: { appearance: 1, finish: 1, stability: 1 },
};

/**
 * @param {"ultra_low_latency"|"balanced"|"quality"|"custom"} presetResolved
 * @param {number} appearanceClosest
 * @param {number} finishClosest
 * @param {number} stabilityClosest
 */
export function getParakeetSimpleTuningLevelsForRender(
  presetResolved,
  appearanceClosest,
  finishClosest,
  stabilityClosest
) {
  const fixed = PARAKEET_LATENCY_PRESET_SIMPLE_LEVELS[presetResolved];
  if (fixed) {
    return { appearance: fixed.appearance, finish: fixed.finish, stability: fixed.stability };
  }
  return {
    appearance: appearanceClosest,
    finish: finishClosest,
    stability: stabilityClosest,
  };
}

const PRESET_MATCH_TOLERANCE_MS = 12;
const PRESET_MATCH_TOLERANCE_CHARS = 1;

function withinTolerance(actual, expected, tolerance) {
  return Math.abs(Number(actual ?? 0) - Number(expected ?? 0)) <= tolerance;
}

/**
 * @param {object} realtime
 * @param {object} lifecycle
 * @returns {"ultra_low_latency"|"balanced"|"quality"|"custom"}
 */
export function resolveParakeetLatencyPresetFromConfig(realtime = {}, lifecycle = {}) {
  const explicit = String(realtime.latency_preset || "").trim().toLowerCase();
  if (explicit === "custom") {
    return "custom";
  }
  for (const presetId of ["ultra_low_latency", "balanced", "quality"]) {
    if (configMatchesParakeetLatencyPreset(realtime, lifecycle, presetId)) {
      return presetId;
    }
  }
  if (explicit && PARAKEET_LATENCY_PRESET_BUNDLES[explicit]) {
    return explicit;
  }
  return "custom";
}

export function configMatchesParakeetLatencyPreset(realtime = {}, lifecycle = {}, presetId) {
  const bundle = PARAKEET_LATENCY_PRESET_BUNDLES[presetId];
  if (!bundle) {
    return false;
  }
  const rt = bundle.realtime;
  const lc = bundle.lifecycle;
  const keysMs = [
    "first_partial_min_speech_ms",
    "partial_emit_interval_ms",
    "silence_hold_ms",
    "finalization_hold_ms",
  ];
  for (const key of keysMs) {
    if (!withinTolerance(realtime[key], rt[key], PRESET_MATCH_TOLERANCE_MS)) {
      return false;
    }
  }
  if (String(realtime.partial_emit_mode || "") !== String(rt.partial_emit_mode)) {
    return false;
  }
  if (Number(realtime.partial_min_new_words ?? 1) !== Number(rt.partial_min_new_words ?? 1)) {
    return false;
  }
  if (!withinTolerance(realtime.partial_min_delta_chars, rt.partial_min_delta_chars, PRESET_MATCH_TOLERANCE_CHARS)) {
    return false;
  }
  if (!withinTolerance(realtime.partial_coalescing_ms, rt.partial_coalescing_ms, PRESET_MATCH_TOLERANCE_MS)) {
    return false;
  }
  if (Boolean(realtime.streaming_decode) !== Boolean(rt.streaming_decode)) {
    return false;
  }
  if (!withinTolerance(lifecycle.pause_to_finalize_ms, lc.pause_to_finalize_ms, PRESET_MATCH_TOLERANCE_MS)) {
    return false;
  }
  return true;
}

/**
 * Apply a bundled preset onto config draft (mutates draft).
 * @param {object} draft full config draft
 * @param {string} presetId
 */
export function applyParakeetLatencyPresetToDraft(draft, presetId) {
  if (!draft.asr) {
    draft.asr = {};
  }
  if (!draft.asr.realtime) {
    draft.asr.realtime = {};
  }
  if (!draft.subtitle_lifecycle) {
    draft.subtitle_lifecycle = {};
  }
  const bundle = PARAKEET_LATENCY_PRESET_BUNDLES[presetId];
  if (!bundle) {
    draft.asr.realtime.latency_preset = "custom";
    return;
  }
  Object.assign(draft.asr.realtime, bundle.realtime, { latency_preset: presetId });
  Object.assign(draft.subtitle_lifecycle, bundle.lifecycle);
  draft.asr.realtime.finalization_hold_ms = bundle.lifecycle.pause_to_finalize_ms;
}

export function markParakeetLatencyPresetCustom(draft) {
  if (!draft.asr) {
    draft.asr = {};
  }
  if (!draft.asr.realtime) {
    draft.asr.realtime = {};
  }
  draft.asr.realtime.latency_preset = "custom";
}
