//! SST `backend/config/normalizers/translation.py` parity.

use serde_json::{Map, Value, json};

use crate::secrets::{
    normalize_google_translate_api_key, normalize_provider_secret, normalize_provider_text_value,
};

pub const SUPPORTED_TRANSLATION_PROVIDERS: [&str; 13] = [
    "google_translate_v2",
    "google_cloud_translation_v3",
    "google_gas_url",
    "google_web",
    "azure_translator",
    "deepl",
    "libretranslate",
    "openai",
    "openrouter",
    "lm_studio",
    "ollama",
    "public_libretranslate_mirror",
    "free_web_translate",
];

const CANONICAL_SLOT_IDS: [&str; 5] = [
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
];

fn str_value(value: Option<&Value>) -> String {
    value
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn default_translation_provider_settings() -> Value {
    json!({
        "google_translate_v2": { "api_key": "" },
        "google_cloud_translation_v3": {
            "project_id": "",
            "access_token": "",
            "location": "global",
            "model": ""
        },
        "google_gas_url": { "gas_url": "" },
        "google_web": {},
        "azure_translator": {
            "api_key": "",
            "endpoint": "https://api.cognitive.microsofttranslator.com",
            "region": ""
        },
        "deepl": {
            "api_key": "",
            "api_url": "https://api-free.deepl.com/v2/translate"
        },
        "libretranslate": {
            "api_key": "",
            "api_url": "https://libretranslate.com/translate"
        },
        "openai": {
            "api_key": "",
            "base_url": "https://api.openai.com/v1",
            "model": "",
            "custom_prompt": ""
        },
        "openrouter": {
            "api_key": "",
            "base_url": "https://openrouter.ai/api/v1",
            "model": "",
            "custom_prompt": ""
        },
        "lm_studio": {
            "api_key": "",
            "base_url": "http://127.0.0.1:1234/v1",
            "model": "",
            "custom_prompt": ""
        },
        "ollama": {
            "api_key": "",
            "base_url": "http://127.0.0.1:11434/v1",
            "model": "",
            "custom_prompt": ""
        },
        "public_libretranslate_mirror": {
            "api_url": "https://translate.fedilab.app/translate"
        },
        "free_web_translate": {}
    })
}

fn normalize_google_v3(current: &Map<String, Value>, defaults: &Map<String, Value>) -> Value {
    let mut access_token = str_value(current.get("access_token"));
    if access_token.is_empty() {
        access_token = str_value(current.get("api_key"));
    }
    if access_token.is_empty() {
        access_token = str_value(defaults.get("access_token"));
    }
    let access_token = normalize_provider_secret(&access_token);

    let mut project_id = str_value(current.get("project_id"));
    if project_id.is_empty() {
        project_id = str_value(current.get("endpoint"));
    }
    if project_id.is_empty() {
        project_id = str_value(defaults.get("project_id"));
    }
    project_id = normalize_provider_text_value(&project_id);

    let mut location = str_value(current.get("location"));
    if location.is_empty() {
        location = str_value(current.get("region"));
    }
    if location.is_empty() {
        location = str_value(defaults.get("location"));
    }
    location = normalize_provider_text_value(&location);

    let model = {
        let model = str_value(current.get("model"));
        if model.is_empty() {
            str_value(defaults.get("model"))
        } else {
            model
        }
    };
    let model = normalize_provider_text_value(&model);

    json!({
        "project_id": project_id,
        "access_token": access_token,
        "location": if location.is_empty() { "global" } else { location.as_str() },
        "model": model
    })
}

fn normalize_provider_block(
    provider_name: &str,
    current: &Map<String, Value>,
    defaults: &Map<String, Value>,
) -> Value {
    let mut normalized = Map::new();
    for (key, default_value) in defaults {
        let value = current.get(key).unwrap_or(default_value);
        normalized.insert(key.clone(), json!(str_value(Some(value))));
    }

    match provider_name {
        "google_translate_v2" => json!({
            "api_key": normalize_google_translate_api_key(
                normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
            )
        }),
        "google_cloud_translation_v3" => normalize_google_v3(current, defaults),
        "google_web" | "free_web_translate" => json!({}),
        "azure_translator" => {
            let endpoint = str_value(normalized.get("endpoint"));
            let endpoint = if endpoint.is_empty() {
                "https://api.cognitive.microsofttranslator.com".to_string()
            } else {
                normalize_provider_text_value(&endpoint)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "endpoint": endpoint,
                "region": normalize_provider_text_value(
                    normalized.get("region").and_then(|v| v.as_str()).unwrap_or(""),
                ),
            })
        }
        "deepl" => {
            let api_url = str_value(normalized.get("api_url"));
            let api_url = if api_url.is_empty() {
                "https://api-free.deepl.com/v2/translate".to_string()
            } else {
                normalize_provider_text_value(&api_url)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "api_url": api_url,
            })
        }
        "libretranslate" => {
            let api_url = str_value(normalized.get("api_url"));
            let api_url = if api_url.is_empty() {
                "https://libretranslate.com/translate".to_string()
            } else {
                normalize_provider_text_value(&api_url)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "api_url": api_url,
            })
        }
        "openai" => {
            let base_url = str_value(normalized.get("base_url"));
            let base_url = if base_url.is_empty() {
                "https://api.openai.com/v1".to_string()
            } else {
                normalize_provider_text_value(&base_url)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "base_url": base_url,
                "model": normalize_provider_text_value(
                    normalized.get("model").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "custom_prompt": normalize_provider_text_value(
                    normalized.get("custom_prompt").and_then(|v| v.as_str()).unwrap_or(""),
                ),
            })
        }
        "openrouter" => {
            let base_url = str_value(normalized.get("base_url"));
            let base_url = if base_url.is_empty() {
                "https://openrouter.ai/api/v1".to_string()
            } else {
                normalize_provider_text_value(&base_url)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "base_url": base_url,
                "model": normalize_provider_text_value(
                    normalized.get("model").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "custom_prompt": normalize_provider_text_value(
                    normalized.get("custom_prompt").and_then(|v| v.as_str()).unwrap_or(""),
                ),
            })
        }
        "lm_studio" => {
            let base_url = str_value(normalized.get("base_url"));
            let base_url = if base_url.is_empty() {
                "http://127.0.0.1:1234/v1".to_string()
            } else {
                normalize_provider_text_value(&base_url)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "base_url": base_url,
                "model": normalize_provider_text_value(
                    normalized.get("model").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "custom_prompt": normalize_provider_text_value(
                    normalized.get("custom_prompt").and_then(|v| v.as_str()).unwrap_or(""),
                ),
            })
        }
        "ollama" => {
            let base_url = str_value(normalized.get("base_url"));
            let base_url = if base_url.is_empty() {
                "http://127.0.0.1:11434/v1".to_string()
            } else {
                normalize_provider_text_value(&base_url)
            };
            json!({
                "api_key": normalize_provider_secret(
                    normalized.get("api_key").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "base_url": base_url,
                "model": normalize_provider_text_value(
                    normalized.get("model").and_then(|v| v.as_str()).unwrap_or(""),
                ),
                "custom_prompt": normalize_provider_text_value(
                    normalized.get("custom_prompt").and_then(|v| v.as_str()).unwrap_or(""),
                ),
            })
        }
        "public_libretranslate_mirror" => {
            let api_url = str_value(normalized.get("api_url"));
            let api_url = if api_url.is_empty() {
                "https://translate.fedilab.app/translate".to_string()
            } else {
                normalize_provider_text_value(&api_url)
            };
            json!({
                "api_url": api_url,
            })
        }
        "google_gas_url" => json!({
            "gas_url": normalize_provider_text_value(
                normalized.get("gas_url").and_then(|v| v.as_str()).unwrap_or(""),
            )
        }),
        _ => {
            let mut out = Map::new();
            for (key, value) in normalized {
                let text = str_value(Some(&value));
                let text = if matches!(key.as_str(), "api_key" | "access_token") {
                    normalize_provider_secret(&text)
                } else {
                    normalize_provider_text_value(&text)
                };
                out.insert(key, json!(text));
            }
            Value::Object(out)
        }
    }
}

/// SST `normalize_provider_settings`.
pub fn normalize_translation_provider_settings(payload: &Value) -> Value {
    let defaults = default_translation_provider_settings();
    let defaults_map = defaults.as_object().expect("defaults object");
    let current_map = payload.as_object().cloned().unwrap_or_default();
    let mut normalized = Map::new();

    for provider_name in SUPPORTED_TRANSLATION_PROVIDERS {
        let provider_defaults = defaults_map
            .get(provider_name)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        let provider_current = current_map
            .get(provider_name)
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        normalized.insert(
            provider_name.into(),
            normalize_provider_block(provider_name, &provider_current, &provider_defaults),
        );
    }

    Value::Object(normalized)
}

fn normalize_provider(raw_provider: Option<&Value>, fallback: &str) -> String {
    let provider = str_value(raw_provider);
    let provider = if provider.is_empty() {
        fallback.to_string()
    } else {
        provider
    };
    if SUPPORTED_TRANSLATION_PROVIDERS.contains(&provider.as_str()) {
        provider
    } else {
        fallback.to_string()
    }
}

fn normalize_target_languages(
    raw_target_languages: &Value,
    fallback_targets: &Value,
) -> Vec<String> {
    let target_languages = if raw_target_languages.is_array() {
        raw_target_languages
    } else {
        fallback_targets
    };
    let Some(items) = target_languages.as_array() else {
        return vec!["en".into()];
    };
    items
        .iter()
        .map(|item| str_value(Some(item)).to_ascii_lowercase())
        .filter(|item| !item.is_empty())
        .collect()
}

fn normalize_translation_lines(
    translation: &Map<String, Value>,
    fallback_provider: &str,
    target_languages: &[String],
) -> Vec<Value> {
    let mut normalized_lines = Vec::new();
    if let Some(raw_lines) = translation.get("lines").and_then(|v| v.as_array()) {
        for (index, raw_line) in raw_lines.iter().enumerate() {
            let Some(obj) = raw_line.as_object() else {
                continue;
            };
            let mut slot_id = str_value(obj.get("slot_id")).to_ascii_lowercase();
            if !CANONICAL_SLOT_IDS.contains(&slot_id.as_str()) {
                slot_id = CANONICAL_SLOT_IDS
                    .get(index)
                    .map(|slot| (*slot).to_string())
                    .unwrap_or_default();
            }
            let target_lang = str_value(obj.get("target_lang")).to_ascii_lowercase();
            if slot_id.is_empty() || target_lang.is_empty() {
                continue;
            }
            let provider = normalize_provider(obj.get("provider"), fallback_provider);
            let label = str_value(obj.get("label"));
            normalized_lines.push(json!({
                "slot_id": slot_id,
                "enabled": obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                "target_lang": target_lang,
                "provider": provider,
                "label": if label.is_empty() {
                    Value::String(target_lang.to_ascii_uppercase())
                } else {
                    Value::String(label)
                },
            }));
        }
    }

    if normalized_lines.is_empty() {
        let fallback_targets = if target_languages.is_empty() {
            vec!["en".to_string()]
        } else {
            target_languages.to_vec()
        };
        normalized_lines = fallback_targets
            .iter()
            .take(CANONICAL_SLOT_IDS.len())
            .enumerate()
            .map(|(index, target_lang)| {
                json!({
                    "slot_id": CANONICAL_SLOT_IDS[index],
                    "enabled": true,
                    "target_lang": target_lang,
                    "provider": fallback_provider,
                    "label": target_lang.to_ascii_uppercase(),
                })
            })
            .collect();
    }

    normalized_lines.truncate(CANONICAL_SLOT_IDS.len());
    normalized_lines
}

fn build_compat_target_languages(lines: &[Value]) -> Vec<String> {
    let mut compat = Vec::new();
    for line in lines {
        let Some(obj) = line.as_object() else {
            continue;
        };
        if !obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            continue;
        }
        let target_lang = str_value(obj.get("target_lang")).to_ascii_lowercase();
        if !target_lang.is_empty() && !compat.contains(&target_lang) {
            compat.push(target_lang);
        }
    }
    compat
}

fn normalize_translation_cache(payload: &Value, defaults: &Value) -> Value {
    let cache_defaults = defaults
        .get("cache")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let current = payload.as_object().cloned().unwrap_or_default();
    let enabled_default = cache_defaults
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let persist_default = cache_defaults
        .get("persist")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let enabled = current
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(enabled_default);
    let persist = current
        .get("persist")
        .and_then(|v| v.as_bool())
        .unwrap_or(persist_default);
    let max_entries = current
        .get("max_entries")
        .or_else(|| cache_defaults.get("max_entries"))
        .and_then(|v| v.as_u64())
        .unwrap_or(5000) as i64;
    let max_entries = max_entries.clamp(0, 50_000);
    json!({
        "enabled": enabled,
        "persist": persist,
        "max_entries": max_entries,
    })
}

fn normalize_provider_limits(payload: &Value) -> Value {
    let Some(current) = payload.as_object() else {
        return json!({});
    };
    let mut normalized = Map::new();
    for (provider_name, cfg) in current {
        let name = provider_name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(cfg_obj) = cfg.as_object() else {
            continue;
        };
        let mut inner = Map::new();
        for (key, value) in cfg_obj {
            let key_str = key.trim();
            if key_str.is_empty() {
                continue;
            }
            inner.insert(key_str.to_string(), value.clone());
        }
        if !inner.is_empty() {
            normalized.insert(name.to_string(), Value::Object(inner));
        }
    }
    Value::Object(normalized)
}

fn int_or(value: Option<&Value>, fallback: i64) -> i64 {
    value
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
        .unwrap_or(fallback)
}

/// SST `normalize_translation_config`.
pub fn normalize_translation_config(
    payload: &Value,
    defaults: &Value,
    fallback_targets: &Value,
) -> Value {
    let translation = payload.as_object().cloned().unwrap_or_default();
    let default_provider = str_value(defaults.get("provider"));
    let fallback_provider = if default_provider.is_empty() {
        "google_translate_v2"
    } else {
        default_provider.as_str()
    };
    let provider = normalize_provider(translation.get("provider"), fallback_provider);
    let target_languages = normalize_target_languages(
        translation
            .get("target_languages")
            .unwrap_or(fallback_targets),
        fallback_targets,
    );
    let timeout_ms = int_or(
        translation.get("timeout_ms").or(defaults.get("timeout_ms")),
        10_000,
    )
    .clamp(1_000, 60_000);
    let queue_max_size = int_or(
        translation
            .get("queue_max_size")
            .or(defaults.get("queue_max_size")),
        8,
    )
    .clamp(1, 64);
    let max_concurrent_jobs = int_or(
        translation
            .get("max_concurrent_jobs")
            .or(defaults.get("max_concurrent_jobs")),
        2,
    )
    .clamp(1, 8);

    let lines = normalize_translation_lines(&translation, &provider, &target_languages);
    let compat_target_languages = build_compat_target_languages(&lines);
    let provider_settings_defaults = defaults
        .get("provider_settings")
        .cloned()
        .unwrap_or_else(default_translation_provider_settings);

    json!({
        "enabled": translation.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
        "provider": provider,
        "target_languages": if compat_target_languages.is_empty() {
            Value::Array(vec![json!("en")])
        } else {
            Value::Array(compat_target_languages.into_iter().map(Value::String).collect())
        },
        "lines": lines,
        "timeout_ms": timeout_ms,
        "queue_max_size": queue_max_size,
        "max_concurrent_jobs": max_concurrent_jobs,
        "provider_settings": normalize_translation_provider_settings(
            translation.get("provider_settings").unwrap_or(&provider_settings_defaults),
        ),
        "cache": normalize_translation_cache(
            translation.get("cache").unwrap_or(&Value::Null),
            defaults,
        ),
        "provider_limits": normalize_provider_limits(
            translation.get("provider_limits").unwrap_or(&json!({})),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_empty_provider_settings_with_sst_defaults() {
        let out = normalize_translation_provider_settings(&json!({}));
        assert_eq!(
            out["deepl"]["api_url"],
            "https://api-free.deepl.com/v2/translate"
        );
        assert_eq!(
            out["azure_translator"]["endpoint"],
            "https://api.cognitive.microsofttranslator.com"
        );
        assert_eq!(out["google_cloud_translation_v3"]["location"], "global");
    }

    #[test]
    fn strips_bearer_prefix_from_access_token() {
        let out = normalize_translation_provider_settings(&json!({
            "google_cloud_translation_v3": {
                "access_token": "Bearer ya29.test-token"
            }
        }));
        assert_eq!(
            out["google_cloud_translation_v3"]["access_token"],
            "ya29.test-token"
        );
    }

    #[test]
    fn migrates_google_v3_ui_aliases_to_canonical_keys() {
        let out = normalize_translation_provider_settings(&json!({
            "google_cloud_translation_v3": {
                "api_key": "token-1",
                "endpoint": "proj-1",
                "region": "eu"
            }
        }));
        assert_eq!(
            out["google_cloud_translation_v3"]["access_token"],
            "token-1"
        );
        assert_eq!(out["google_cloud_translation_v3"]["project_id"], "proj-1");
        assert_eq!(out["google_cloud_translation_v3"]["location"], "eu");
        assert!(out["google_cloud_translation_v3"].get("api_key").is_none());
    }

    #[test]
    fn normalize_translation_config_clamps_timeout_and_builds_lines() {
        let defaults = json!({
            "provider": "google_translate_v2",
            "timeout_ms": 10000,
            "queue_max_size": 8,
            "max_concurrent_jobs": 2,
            "provider_settings": default_translation_provider_settings(),
            "cache": { "enabled": true, "persist": true, "max_entries": 5000 }
        });
        let out = normalize_translation_config(
            &json!({
                "enabled": true,
                "timeout_ms": 999999,
                "target_languages": ["de", "fr"]
            }),
            &defaults,
            &json!(["en"]),
        );
        assert_eq!(out["timeout_ms"], 60_000);
        assert_eq!(out["lines"].as_array().unwrap().len(), 2);
        assert_eq!(out["target_languages"], json!(["de", "fr"]));
    }
}
