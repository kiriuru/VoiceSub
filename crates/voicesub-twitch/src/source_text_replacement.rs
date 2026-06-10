use std::sync::OnceLock;

use regex::RegexBuilder;
use serde::Deserialize;
use serde_json::Value;

use crate::settings::TwitchTtsSettings;

const BUILTIN_PAIRS_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/source_text_builtin_pairs.json"));

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceTextReplacementPair {
    pub source: String,
    #[serde(default)]
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceTextReplacementSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true", alias = "include_builtin_profanity")]
    pub include_builtin: bool,
    #[serde(default = "default_true")]
    pub case_insensitive: bool,
    #[serde(default = "default_true", alias = "whole_word_only")]
    pub whole_words: bool,
    #[serde(default)]
    pub pairs: Vec<SourceTextReplacementPair>,
}

impl Default for SourceTextReplacementSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: Vec::new(),
        }
    }
}

fn bool_field(obj: &serde_json::Map<String, Value>, keys: &[&str], default: bool) -> bool {
    for key in keys {
        if let Some(value) = obj.get(*key).and_then(|v| v.as_bool()) {
            return value;
        }
    }
    default
}

fn parse_pairs(value: Option<&Value>) -> Vec<SourceTextReplacementPair> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items.iter().take(100) {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let source = obj
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if source.is_empty() {
            continue;
        }
        let target = obj
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        out.push(SourceTextReplacementPair {
            source: source.to_string(),
            target,
        });
    }
    out
}

/// Parse replacement settings from a config root (`source_text_replacement` section).
pub fn settings_from_config_value(config: &Value) -> SourceTextReplacementSettings {
    config
        .get("source_text_replacement")
        .map(settings_from_section_value)
        .unwrap_or_default()
}

/// Parse replacement settings from the `source_text_replacement` object.
pub fn settings_from_section_value(section: &Value) -> SourceTextReplacementSettings {
    let Some(obj) = section.as_object() else {
        return SourceTextReplacementSettings::default();
    };
    let whole_words = match (
        obj.get("whole_words").and_then(|v| v.as_bool()),
        obj.get("whole_word_only").and_then(|v| v.as_bool()),
    ) {
        (Some(false), _) | (_, Some(false)) => false,
        (Some(true), _) | (_, Some(true)) => true,
        _ => true,
    };
    SourceTextReplacementSettings {
        enabled: obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
        include_builtin: bool_field(
            obj,
            &["include_builtin", "include_builtin_profanity"],
            true,
        ),
        case_insensitive: bool_field(obj, &["case_insensitive"], true),
        whole_words,
        pairs: parse_pairs(obj.get("pairs")),
    }
}

fn default_true() -> bool {
    true
}

fn builtin_pairs_raw() -> &'static [SourceTextReplacementPair] {
    static CACHE: OnceLock<Vec<SourceTextReplacementPair>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<Vec<SourceTextReplacementPair>>(BUILTIN_PAIRS_JSON)
            .unwrap_or_default()
            .into_iter()
            .filter(|pair| !pair.source.trim().is_empty())
            .collect()
    })
}

fn normalize_custom_pairs(pairs: &[SourceTextReplacementPair]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for pair in pairs.iter().take(100) {
        let source = pair.source.trim();
        if source.is_empty() {
            continue;
        }
        out.push((source.to_string(), pair.target.clone()));
    }
    out
}

pub fn effective_replacement_pairs(settings: &SourceTextReplacementSettings) -> Vec<(String, String)> {
    if !settings.enabled {
        return Vec::new();
    }

    let case_insensitive = settings.case_insensitive;
    let key = |source: &str| {
        if case_insensitive {
            source.to_ascii_lowercase()
        } else {
            source.to_string()
        }
    };

    let mut by_key: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for (source, target) in normalize_custom_pairs(&settings.pairs) {
        by_key.insert(key(&source), (source, target));
    }
    if settings.include_builtin {
        for pair in builtin_pairs_raw() {
            let source = pair.source.trim();
            if source.is_empty() {
                continue;
            }
            let k = key(source);
            by_key
                .entry(k)
                .or_insert_with(|| (source.to_string(), pair.target.clone()));
        }
    }

    let mut merged: Vec<(String, String)> = by_key.into_values().collect();
    merged.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
    merged
}

pub fn apply_replacement_rules(
    text: &str,
    pairs: &[(String, String)],
    case_insensitive: bool,
    whole_words: bool,
) -> String {
    if text.is_empty() || pairs.is_empty() {
        return text.to_string();
    }

    let mut result = text.to_string();

    for (source, target) in pairs {
        if source.is_empty() {
            continue;
        }
        let escaped = regex::escape(source);
        let pattern = if whole_words {
            format!(r"\b{escaped}\b")
        } else {
            escaped
        };
        let Ok(rx) = RegexBuilder::new(&pattern)
            .case_insensitive(case_insensitive)
            .build()
        else {
            continue;
        };
        result = rx.replace_all(&result, target.as_str()).to_string();
    }
    result
}

pub fn apply_source_text_replacement(
    text: &str,
    settings: &SourceTextReplacementSettings,
) -> String {
    if !settings.enabled {
        return text.to_string();
    }
    let pairs = effective_replacement_pairs(settings);
    if pairs.is_empty() {
        return text.to_string();
    }
    apply_replacement_rules(
        text,
        &pairs,
        settings.case_insensitive,
        settings.whole_words,
    )
}

pub fn profanity_settings_for_twitch(chat: &TwitchTtsSettings) -> SourceTextReplacementSettings {
    SourceTextReplacementSettings {
        enabled: true,
        include_builtin: chat.include_builtin_profanity,
        case_insensitive: true,
        whole_words: true,
        pairs: Vec::new(),
    }
}

/// Builtin profanity list only (`include_builtin_profanity` on Twitch settings).
pub fn apply_builtin_profanity(
    text: &str,
    settings: &SourceTextReplacementSettings,
) -> String {
    if !settings.include_builtin {
        return text.to_string();
    }
    let mut pairs: Vec<(String, String)> = builtin_pairs_raw()
        .iter()
        .map(|pair| (pair.source.clone(), pair.target.clone()))
        .collect();
    if pairs.is_empty() {
        return text.to_string();
    }
    pairs.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
    apply_replacement_rules(
        text,
        &pairs,
        settings.case_insensitive,
        settings.whole_words,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_pairs(pairs: Vec<(&str, &str)>) -> SourceTextReplacementSettings {
        SourceTextReplacementSettings {
            enabled: true,
            include_builtin: false,
            case_insensitive: true,
            whole_words: true,
            pairs: pairs
                .into_iter()
                .map(|(source, target)| SourceTextReplacementPair {
                    source: source.into(),
                    target: target.into(),
                })
                .collect(),
        }
    }

    #[test]
    fn whole_word_pattern_compiles_and_matches() {
        let rx = RegexBuilder::new(r"\bbad\b")
            .case_insensitive(true)
            .build()
            .expect("whole-word pattern should compile");
        assert_eq!(rx.replace("BAD word", "X"), "X word");
    }

    #[test]
    fn disabled_returns_original() {
        let settings = SourceTextReplacementSettings {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(
            apply_source_text_replacement("bad word", &settings),
            "bad word"
        );
    }

    #[test]
    fn replaces_whole_word_case_insensitive() {
        let settings = settings_with_pairs(vec![("bad", "X")]);
        assert_eq!(
            apply_source_text_replacement("BAD word", &settings),
            "X word"
        );
    }

    #[test]
    fn builtin_profanity_replaces_known_term() {
        let settings = SourceTextReplacementSettings {
            enabled: true,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: Vec::new(),
        };
        let out = apply_source_text_replacement("what the fuck", &settings);
        assert_eq!(out, "what the ***");
    }

    #[test]
    fn settings_from_ui_aliases_enable_builtin_and_custom_pairs() {
        let settings = settings_from_config_value(&serde_json::json!({
            "source_text_replacement": {
                "enabled": true,
                "include_builtin_profanity": true,
                "whole_word_only": true,
                "pairs": [{"source": "bad", "target": "X"}]
            }
        }));
        assert!(settings.enabled);
        assert!(settings.include_builtin);
        assert!(settings.whole_words);
        assert_eq!(
            apply_source_text_replacement("BAD word", &settings),
            "X word"
        );
    }

    #[test]
    fn settings_from_section_respects_disabled_builtin_flag() {
        let settings = settings_from_config_value(&serde_json::json!({
            "source_text_replacement": {
                "enabled": true,
                "include_builtin_profanity": false,
                "pairs": [{"source": "bad", "target": "X"}]
            }
        }));
        assert!(!settings.include_builtin);
        assert_eq!(
            apply_source_text_replacement("what the fuck", &settings),
            "what the fuck"
        );
        assert_eq!(apply_source_text_replacement("bad", &settings), "X");
    }

    #[test]
    fn builtin_cjk_profanity_replaced_without_word_boundaries() {
        let settings = SourceTextReplacementSettings {
            enabled: true,
            include_builtin: true,
            case_insensitive: true,
            whole_words: false,
            pairs: Vec::new(),
        };
        assert_eq!(
            apply_source_text_replacement("これは くそ だ", &settings),
            "これは *** だ"
        );
        assert_eq!(
            apply_source_text_replacement("아 씨발 진짜", &settings),
            "아 *** 진짜"
        );
        assert_eq!(
            apply_source_text_replacement("你他妈的", &settings),
            "你***"
        );
    }

    #[test]
    fn deserializes_ui_field_aliases() {
        let settings: SourceTextReplacementSettings = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "include_builtin_profanity": true,
            "case_insensitive": true,
            "whole_word_only": true,
            "pairs": [{"source": "bad", "target": "X"}]
        }))
        .expect("ui-shaped config should deserialize");
        assert!(settings.enabled);
        assert!(settings.include_builtin);
        assert!(settings.whole_words);
        assert_eq!(
            apply_source_text_replacement("BAD word", &settings),
            "X word"
        );
    }

    #[test]
    fn custom_overrides_builtin() {
        let settings = SourceTextReplacementSettings {
            enabled: true,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: vec![SourceTextReplacementPair {
                source: "fuck".into(),
                target: "duck".into(),
            }],
        };
        assert_eq!(
            apply_source_text_replacement("fuck", &settings),
            "duck"
        );
    }
}
