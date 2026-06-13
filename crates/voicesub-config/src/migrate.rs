use serde_json::{json, Map, Value};

use crate::defaults::CURRENT_CONFIG_VERSION;

const CANONICAL_SLOTS: [&str; 5] = [
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
];

fn as_object(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn parse_version(value: &Value) -> i64 {
    value
        .as_i64()
        .or_else(|| value.as_u64().map(|v| v as i64))
        .unwrap_or(1)
        .max(1)
}

fn normalize_provider(raw: &Value, fallback: &str) -> String {
    crate::normalize::canonical_translation_provider(
        raw.as_str().unwrap_or(fallback),
        fallback,
    )
}

fn build_translation_lines(translation: &Map<String, Value>) -> Vec<Value> {
    let fallback = normalize_provider(
        translation.get("provider").unwrap_or(&Value::Null),
        "google_translate_v2",
    );
    if let Some(lines) = translation.get("lines").and_then(|v| v.as_array()) {
        let mut out = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let Some(obj) = line.as_object() else {
                continue;
            };
            let slot_id = obj
                .get("slot_id")
                .and_then(|v| v.as_str())
                .unwrap_or(CANONICAL_SLOTS.get(index).copied().unwrap_or(""));
            let target_lang = obj
                .get("target_lang")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            if slot_id.is_empty() || target_lang.is_empty() {
                continue;
            }
            out.push(json!({
                "slot_id": slot_id,
                "enabled": obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                "target_lang": target_lang,
                "provider": normalize_provider(obj.get("provider").unwrap_or(&Value::Null), &fallback),
                "label": obj.get("label").and_then(|v| v.as_str()).unwrap_or(&target_lang.to_uppercase()),
            }));
        }
        if !out.is_empty() {
            return out.into_iter().take(CANONICAL_SLOTS.len()).collect();
        }
    }

    let legacy_targets: Vec<String> = match translation
        .get("target_languages")
        .and_then(|v| v.as_array())
    {
        Some(arr) => {
            let targets: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if targets.is_empty() {
                vec!["en".into()]
            } else {
                targets
            }
        }
        None => vec!["en".into()],
    };

    legacy_targets
        .iter()
        .take(CANONICAL_SLOTS.len())
        .enumerate()
        .map(|(index, target_lang)| {
            json!({
                "slot_id": CANONICAL_SLOTS[index],
                "enabled": true,
                "target_lang": target_lang,
                "provider": fallback,
                "label": target_lang.to_uppercase(),
            })
        })
        .collect()
}

fn compat_target_languages(lines: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let Some(obj) = line.as_object() else {
            continue;
        };
        if !obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            continue;
        }
        let lang = obj
            .get("target_lang")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !lang.is_empty() && !out.contains(&lang) {
            out.push(lang);
        }
    }
    out
}

/// Port of SST `migrate_config` (subset critical for import).
pub fn migrate_sst_payload(payload: Value) -> Value {
    let mut root = as_object(&payload);
    let version = parse_version(root.get("config_version").unwrap_or(&Value::Null));

    if version < 2 {
        let mut ui = as_object(root.get("ui").unwrap_or(&Value::Null));
        if ui.get("theme").is_none() {
            ui.insert("theme".into(), json!("dark"));
        }
        root.insert("ui".into(), Value::Object(ui));

        let mut translation = as_object(root.get("translation").unwrap_or(&Value::Null));
        if translation.get("target_languages").is_none() {
            if let Some(targets) = root.get("targets").and_then(|v| v.as_array()) {
                translation.insert("target_languages".into(), Value::Array(targets.clone()));
            }
        }
        root.insert("translation".into(), Value::Object(translation));
    }

    let mut translation = as_object(root.get("translation").unwrap_or(&Value::Null));
    let lines = build_translation_lines(&translation);
    translation.insert(
        "provider".into(),
        Value::String(normalize_provider(
            translation.get("provider").unwrap_or(&Value::Null),
            "google_translate_v2",
        )),
    );
    translation.insert("lines".into(), Value::Array(lines.clone()));
    translation.insert(
        "target_languages".into(),
        Value::Array(
            compat_target_languages(&lines)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    root.insert("translation".into(), Value::Object(translation));

    root.insert("config_version".into(), json!(CURRENT_CONFIG_VERSION));
    Value::Object(root)
}

/// VoiceSub roadmap §9 — SST v7 JSON → active VoiceSub semantics.
pub fn apply_voicesub_import_rules(payload: Value) -> Value {
    let mut root = as_object(&payload);
    let mut asr = as_object(root.get("asr").unwrap_or(&Value::Null));
    let mode = asr
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();

    match mode.as_str() {
        "local" | "browser_google_experimental" | "browser_google_experimental_edge" => {
            asr.insert("mode".into(), json!("browser_google"));
            asr.insert(
                "import_hint".into(),
                json!(format!(
                    "SST mode '{mode}' mapped to browser_google; Parakeet is a future module"
                )),
            );
        }
        "" => {
            asr.insert("mode".into(), json!("browser_google"));
        }
        _ if mode != "browser_google" && mode != "browser_google_edge" => {
            asr.insert("mode".into(), json!("browser_google"));
        }
        _ => {}
    }

    asr.remove("provider_preference");
    asr.remove("realtime");
    asr.remove("prefer_gpu");
    asr.remove("model_load_mode");
    asr.remove("model_revision");
    asr.remove("rnnoise_enabled");
    asr.remove("rnnoise_strength");

    let mut browser = as_object(asr.get("browser").unwrap_or(&Value::Null));
    browser.remove("experimental");
    browser.remove("worker_ui");
    asr.insert("browser".into(), Value::Object(browser));
    root.insert("asr".into(), Value::Object(asr));

    root.remove("remote");

    let mut ui = as_object(root.get("ui").unwrap_or(&Value::Null));
    ui.remove("show_remote_tools");
    root.insert("ui".into(), Value::Object(ui));

    root.insert("config_version".into(), json!(CURRENT_CONFIG_VERSION));
    Value::Object(root)
}

pub fn import_sst_json_value(payload: Value) -> Value {
    let source_version = payload
        .get("config_version")
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
        .unwrap_or(0);
    let mut imported = apply_voicesub_import_rules(migrate_sst_payload(payload));
    crate::normalize::repair_legacy_keep_completed_false(&mut imported, source_version);
    crate::normalize::normalize_config_payload(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_local_asr_to_browser_google() {
        let imported = import_sst_json_value(json!({
            "config_version": 1,
            "asr": { "mode": "local" }
        }));
        assert_eq!(imported["asr"]["mode"], "browser_google");
        assert!(imported["asr"]["import_hint"]
            .as_str()
            .unwrap()
            .contains("Parakeet"));
    }

    #[test]
    fn maps_experimental_browser_to_classic() {
        let imported = import_sst_json_value(json!({
            "config_version": 7,
            "asr": { "mode": "browser_google_experimental" }
        }));
        assert_eq!(imported["asr"]["mode"], "browser_google");
    }

    #[test]
    fn strips_remote_section() {
        let imported = import_sst_json_value(json!({
            "config_version": 7,
            "remote": { "enabled": true },
            "asr": { "mode": "browser_google" }
        }));
        assert!(imported.get("remote").is_none());
    }

    #[test]
    fn removed_mymemory_provider_falls_back_on_import() {
        let imported = import_sst_json_value(json!({
            "config_version": 7,
            "translation": {
                "enabled": true,
                "provider": "mymemory",
                "target_languages": ["en"]
            },
            "asr": { "mode": "browser_google" }
        }));
        assert_eq!(imported["translation"]["provider"], "google_translate_v2");
    }

    #[test]
    fn builds_translation_lines_from_legacy_targets() {
        let imported = import_sst_json_value(json!({
            "config_version": 1,
            "targets": ["ru", "ja"],
            "translation": { "provider": "google_translate_v2" }
        }));
        let lines = imported["translation"]["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["target_lang"], "ru");
    }
}
