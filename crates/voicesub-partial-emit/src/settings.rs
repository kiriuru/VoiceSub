use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialEmitSettings {
    pub partial_emit_mode: String,
    pub partial_min_new_words: u32,
    pub partial_min_delta_chars: u32,
    pub partial_coalescing_ms: u32,
}

impl Default for PartialEmitSettings {
    fn default() -> Self {
        Self {
            partial_emit_mode: "word_growth".into(),
            partial_min_new_words: 1,
            partial_min_delta_chars: 0,
            partial_coalescing_ms: 0,
        }
    }
}

impl PartialEmitSettings {
    pub fn from_fields(
        mode: &str,
        min_new_words: u32,
        min_delta_chars: u32,
        coalescing_ms: u32,
    ) -> Self {
        let mut settings = Self::default();
        let normalized = mode.trim().to_ascii_lowercase();
        if normalized == "char_delta" || normalized == "word_growth" {
            settings.partial_emit_mode = normalized;
        }
        settings.partial_min_new_words = min_new_words.clamp(1, 32);
        settings.partial_min_delta_chars = min_delta_chars.clamp(0, 256);
        settings.partial_coalescing_ms = coalescing_ms.clamp(0, 10_000);
        settings
    }
}

pub fn partial_emit_settings_from_config(config: &Value) -> PartialEmitSettings {
    let realtime = config
        .get("asr")
        .and_then(|v| v.get("realtime"))
        .cloned()
        .unwrap_or(Value::Null);
    PartialEmitSettings::from_fields(
        realtime
            .get("partial_emit_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("word_growth"),
        realtime
            .get("partial_min_new_words")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32,
        realtime
            .get("partial_min_delta_chars")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        realtime
            .get("partial_coalescing_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    )
}
