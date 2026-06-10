use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptKind {
    Partial,
    Final,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TranscriptSegment {
    #[serde(default)]
    pub segment_id: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub is_final: bool,
    #[serde(default = "default_source_lang")]
    pub source_lang: String,
    pub provider: Option<String>,
    #[serde(default)]
    pub sequence: u64,
    #[serde(default)]
    pub revision: u64,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
}

fn default_source_lang() -> String {
    "auto".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptEvent {
    pub event: TranscriptKind,
    pub text: String,
    pub sequence: u64,
    pub segment: Option<TranscriptSegment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationItem {
    pub target_lang: String,
    pub text: String,
    pub provider: String,
    #[serde(default)]
    pub slot_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_group: Option<String>,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub local_provider: bool,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub cached: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TranslationItem {
    fn default() -> Self {
        Self {
            target_lang: String::new(),
            text: String::new(),
            provider: String::new(),
            slot_id: None,
            label: None,
            provider_group: None,
            experimental: false,
            local_provider: false,
            success: true,
            error: None,
            cached: false,
        }
    }
}

impl Default for TranslationEvent {
    fn default() -> Self {
        Self {
            sequence: 0,
            source_text: String::new(),
            source_lang: String::new(),
            translations: Vec::new(),
            provider: String::new(),
            provider_group: None,
            experimental: false,
            local_provider: false,
            used_default_prompt: false,
            status_message: None,
            is_complete: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TranslationEvent {
    pub sequence: u64,
    pub source_text: String,
    pub source_lang: String,
    pub translations: Vec<TranslationItem>,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_group: Option<String>,
    #[serde(default)]
    pub experimental: bool,
    #[serde(default)]
    pub local_provider: bool,
    #[serde(default)]
    pub used_default_prompt: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(default = "default_true")]
    pub is_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    Idle,
    PartialOnly,
    CompletedOnly,
    CompletedWithPartial,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleLineItem {
    pub kind: String,
    pub lang: String,
    pub label: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_slot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub visible: bool,
    #[serde(default = "default_true")]
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitlePayloadEvent {
    pub sequence: u64,
    pub source_lang: String,
    pub source_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    pub preset: String,
    pub compact: bool,
    pub display_order: Vec<String>,
    pub show_source: bool,
    pub show_translations: bool,
    pub max_translation_languages: u32,
    pub items: Vec<SubtitleLineItem>,
    pub visible_items: Vec<SubtitleLineItem>,
    #[serde(default)]
    pub style: Value,
    pub lifecycle_state: LifecycleState,
    pub completed_block_visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_expires_at_utc: Option<String>,
    pub active_partial_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_partial_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_partial_source_lang: Option<String>,
    pub line1: String,
    pub line2: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at_ms: Option<u64>,
}

impl Default for SubtitlePayloadEvent {
    fn default() -> Self {
        Self {
            sequence: 0,
            source_lang: "auto".into(),
            source_text: String::new(),
            provider: None,
            preset: "single".into(),
            compact: false,
            display_order: vec!["source".into()],
            show_source: true,
            show_translations: true,
            max_translation_languages: 0,
            items: Vec::new(),
            visible_items: Vec::new(),
            style: Value::Object(Default::default()),
            lifecycle_state: LifecycleState::Idle,
            completed_block_visible: false,
            completed_expires_at_utc: None,
            active_partial_text: String::new(),
            active_partial_sequence: None,
            active_partial_source_lang: None,
            line1: String::new(),
            line2: String::new(),
            created_at_ms: None,
        }
    }
}
