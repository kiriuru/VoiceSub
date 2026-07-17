//! SST `local_asr_realtime_settings` parity — latency presets → decode/window/VAD timings.

use crate::config::{LocalAsrConfig, LocalAsrRealtimeConfig, LocalAsrVadConfig};
use voicesub_partial_emit::PartialEmitSettings;

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRealtimeSettings {
    pub latency_preset: String,
    pub streaming_decode: bool,
    pub decode_interval_ms: u32,
    /// Minimum buffered speech (ms) before the first partial decode attempt.
    pub window_ms: u32,
    pub segment_enqueue_delta_ms: u32,
    pub first_partial_min_speech_ms: u32,
    pub partial_emit_interval_ms: u32,
    /// Effective VAD silence hold (ms) — from config, aligned with latency preset on apply.
    pub silence_hold_ms: u32,
    /// Effective VAD finalization hold / min silence (ms).
    pub finalization_hold_ms: u32,
    pub min_speech_ms: u32,
    pub partial_emit: PartialEmitSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PresetDefaults {
    decode_interval_ms: u32,
    first_partial_min_speech_ms: u32,
    silence_hold_ms: u32,
    finalization_hold_ms: u32,
    min_speech_ms: u32,
    partial_min_delta_chars: u32,
    partial_coalescing_ms: u32,
}

impl ResolvedRealtimeSettings {
    pub fn from_config(config: &LocalAsrConfig) -> Self {
        let preset = normalize_latency_preset(&config.realtime.latency_preset);
        let base = preset_defaults(&preset);
        let decode_interval_ms = config
            .realtime
            .decode_interval_ms
            .unwrap_or(base.decode_interval_ms);
        let window_ms = config
            .realtime
            .window_ms
            .unwrap_or_else(|| resolved_chunk_window_ms(decode_interval_ms));
        let segment_enqueue_delta_ms = config
            .realtime
            .segment_enqueue_delta_ms
            .unwrap_or(decode_interval_ms);
        let first_partial_min_speech_ms = config
            .realtime
            .first_partial_min_speech_ms
            .unwrap_or(base.first_partial_min_speech_ms);
        let (partial_min_delta_chars, partial_coalescing_ms) =
            resolve_partial_emit_overrides(&config.realtime, &base);

        Self {
            latency_preset: preset,
            streaming_decode: config.realtime.streaming_decode,
            decode_interval_ms,
            window_ms,
            segment_enqueue_delta_ms,
            first_partial_min_speech_ms,
            partial_emit_interval_ms: config
                .vad
                .partial_emit_interval_ms
                .unwrap_or(decode_interval_ms),
            silence_hold_ms: config.vad.silence_hold_ms,
            finalization_hold_ms: config.vad.min_silence_ms,
            min_speech_ms: config.vad.min_speech_ms,
            partial_emit: PartialEmitSettings::from_fields(
                &config.realtime.partial_emit_mode,
                config.realtime.partial_min_new_words,
                partial_min_delta_chars,
                partial_coalescing_ms,
            ),
        }
    }
}

/// SST `parakeet_provider._resolved_streaming_window_ms` chunk size when unset.
pub fn resolved_chunk_window_ms(decode_interval_ms: u32) -> u32 {
    let scaled = (f64::from(decode_interval_ms) * 1.6).round() as u32;
    scaled.clamp(640, 1200)
}

fn preset_defaults(preset: &str) -> PresetDefaults {
    match preset {
        "quality" => PresetDefaults {
            decode_interval_ms: 650,
            first_partial_min_speech_ms: 260,
            silence_hold_ms: 260,
            finalization_hold_ms: 520,
            min_speech_ms: 260,
            partial_min_delta_chars: 1,
            partial_coalescing_ms: 80,
        },
        "balanced" => PresetDefaults {
            decode_interval_ms: 280,
            first_partial_min_speech_ms: 180,
            silence_hold_ms: 180,
            // Slightly longer finalize so Final matches the last typed partial (§4.8 C1).
            finalization_hold_ms: 400,
            min_speech_ms: 180,
            partial_min_delta_chars: 0,
            partial_coalescing_ms: 0,
        },
        _ => PresetDefaults {
            // Subtitle contract (§4.8): faster first word, slightly more tail rewrite.
            decode_interval_ms: 200,
            first_partial_min_speech_ms: 140,
            silence_hold_ms: 120,
            finalization_hold_ms: 220,
            min_speech_ms: 180,
            partial_min_delta_chars: 0,
            partial_coalescing_ms: 0,
        },
    }
}

fn resolve_partial_emit_overrides(
    realtime: &LocalAsrRealtimeConfig,
    base: &PresetDefaults,
) -> (u32, u32) {
    let delta_chars = if realtime.partial_min_delta_chars > 0 {
        realtime.partial_min_delta_chars
    } else {
        base.partial_min_delta_chars
    };
    let coalescing = if realtime.partial_coalescing_ms > 0 {
        realtime.partial_coalescing_ms
    } else {
        base.partial_coalescing_ms
    };
    (delta_chars, coalescing)
}

/// Preset timings for UI apply-on-select; runtime uses saved `config.vad` as source of truth.
pub fn vad_config_for_preset(preset: &str) -> LocalAsrVadConfig {
    let base = preset_defaults(preset);
    LocalAsrVadConfig {
        min_speech_ms: base.min_speech_ms,
        silence_hold_ms: base.silence_hold_ms,
        min_silence_ms: base.finalization_hold_ms,
        max_segment_ms: crate::decode_pacing::max_segment_ms_for_preset(preset),
        ..LocalAsrVadConfig::default()
    }
}

/// Apply latency preset → realtime partial defaults + VAD holds (TS `applyLatencyPreset` parity).
pub fn apply_latency_preset_to_config(config: &mut LocalAsrConfig) {
    let preset = normalize_latency_preset(&config.realtime.latency_preset);
    let overrides = realtime_overrides_for_preset(&preset);
    let vad = vad_config_for_preset(&preset);
    config.realtime = LocalAsrRealtimeConfig {
        decode_interval_ms: None,
        window_ms: None,
        segment_enqueue_delta_ms: None,
        first_partial_min_speech_ms: None,
        ..overrides
    };
    config.vad.min_speech_ms = vad.min_speech_ms;
    config.vad.silence_hold_ms = vad.silence_hold_ms;
    config.vad.min_silence_ms = vad.min_silence_ms;
    config.vad.max_segment_ms = vad.max_segment_ms;
}

pub fn realtime_overrides_for_preset(preset: &str) -> LocalAsrRealtimeConfig {
    let base = preset_defaults(preset);
    LocalAsrRealtimeConfig {
        latency_preset: preset.to_string(),
        streaming_decode: true,
        partial_emit_mode: "word_growth".into(),
        partial_min_new_words: 1,
        partial_min_delta_chars: base.partial_min_delta_chars,
        partial_coalescing_ms: base.partial_coalescing_ms,
        decode_interval_ms: None,
        window_ms: None,
        segment_enqueue_delta_ms: None,
        first_partial_min_speech_ms: Some(base.first_partial_min_speech_ms),
    }
}

pub fn normalize_latency_preset(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "quality" | "high" => "quality".into(),
        "balanced" | "medium" => "balanced".into(),
        "low" | "low_latency" | "fast" | "ultra_low_latency" | "ultra_low" => "low".into(),
        "" => "balanced".into(),
        _ => "balanced".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_preset_resolves_fast_decode() {
        let mut config = LocalAsrConfig::default();
        config.realtime.latency_preset = "low".into();
        let resolved = ResolvedRealtimeSettings::from_config(&config);
        assert_eq!(resolved.decode_interval_ms, 200);
        assert_eq!(resolved.first_partial_min_speech_ms, 140);
        assert!(resolved.streaming_decode);
        assert_eq!(resolved.partial_emit.partial_emit_mode, "word_growth");
        assert_eq!(resolved.partial_emit.partial_min_new_words, 1);
    }

    #[test]
    fn balanced_preset_matches_expected_defaults() {
        let mut config = LocalAsrConfig::default();
        config.realtime.latency_preset = "balanced".into();
        let resolved = ResolvedRealtimeSettings::from_config(&config);
        assert_eq!(resolved.decode_interval_ms, 280);
        assert_eq!(resolved.first_partial_min_speech_ms, 180);
        assert_eq!(resolved.window_ms, 640);
        assert_eq!(resolved.segment_enqueue_delta_ms, 280);
    }

    #[test]
    fn quality_preset_applies_coalescing_defaults() {
        let mut config = LocalAsrConfig::default();
        config.realtime.latency_preset = "quality".into();
        let resolved = ResolvedRealtimeSettings::from_config(&config);
        assert_eq!(resolved.partial_emit.partial_min_delta_chars, 1);
        assert_eq!(resolved.partial_emit.partial_coalescing_ms, 80);
    }

    #[test]
    fn advanced_override_wins_over_preset() {
        let mut config = LocalAsrConfig::default();
        config.realtime.latency_preset = "low".into();
        config.realtime.decode_interval_ms = Some(750);
        let resolved = ResolvedRealtimeSettings::from_config(&config);
        assert_eq!(resolved.decode_interval_ms, 750);
    }

    #[test]
    fn chunk_window_formula_matches_expected_defaults() {
        assert_eq!(resolved_chunk_window_ms(280), 640);
        assert_eq!(resolved_chunk_window_ms(200), 640);
        assert_eq!(resolved_chunk_window_ms(650), 1040);
    }

    #[test]
    fn preset_helpers_match_balanced_vad() {
        let vad = vad_config_for_preset("balanced");
        assert_eq!(vad.min_speech_ms, 180);
        assert_eq!(vad.min_silence_ms, 400);
        assert_eq!(vad.silence_hold_ms, 180);
        assert_eq!(vad.max_segment_ms, 120_000);
        let rt = realtime_overrides_for_preset("balanced");
        assert_eq!(rt.latency_preset, "balanced");
        assert_eq!(rt.first_partial_min_speech_ms, Some(180));
    }

    #[test]
    fn apply_latency_preset_writes_vad_holds() {
        let mut config = LocalAsrConfig::default();
        config.realtime.latency_preset = "quality".into();
        apply_latency_preset_to_config(&mut config);
        assert_eq!(config.vad.silence_hold_ms, 260);
        assert_eq!(config.vad.min_silence_ms, 520);
        assert_eq!(config.realtime.partial_coalescing_ms, 80);
        let resolved = ResolvedRealtimeSettings::from_config(&config);
        assert_eq!(resolved.silence_hold_ms, 260);
        assert_eq!(resolved.finalization_hold_ms, 520);
    }
}
