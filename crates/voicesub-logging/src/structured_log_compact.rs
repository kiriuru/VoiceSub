use std::collections::BTreeMap;

use serde_json::Value;

use crate::compact_log_line::truncate_with_ellipsis;

const DEFAULT_MAX_STR: usize = 160;
const DEFAULT_MAX_LIST: usize = 10;
const DEFAULT_MAX_DEPTH: usize = 4;

pub fn compact_for_runtime_log(value: &Value, depth: usize) -> Value {
    match value {
        Value::Null => Value::Null,
        Value::Bool(_) | Value::Number(_) => value.clone(),
        Value::String(text) => {
            if text.len() <= DEFAULT_MAX_STR {
                value.clone()
            } else {
                Value::String(truncate_with_ellipsis(text, DEFAULT_MAX_STR))
            }
        }
        Value::Array(items) => {
            if depth >= DEFAULT_MAX_DEPTH {
                return Value::String(format!("<list len={}>", items.len()));
            }
            if items.len() <= DEFAULT_MAX_LIST {
                return Value::Array(
                    items
                        .iter()
                        .map(|item| compact_for_runtime_log(item, depth + 1))
                        .collect(),
                );
            }
            let preview: Vec<Value> = items
                .iter()
                .take(DEFAULT_MAX_LIST)
                .map(|item| compact_for_runtime_log(item, depth + 1))
                .collect();
            serde_json::json!({
                "_items_len": items.len(),
                "_items_preview": preview,
            })
        }
        Value::Object(map) => {
            if depth >= DEFAULT_MAX_DEPTH {
                return Value::String(format!("<dict keys={}>", map.len()));
            }
            let mut out = serde_json::Map::new();
            for (key, val) in map {
                let compacted = compact_for_runtime_log(val, depth + 1);
                if !compacted.is_null() {
                    out.insert(key.clone(), compacted);
                }
            }
            Value::Object(out)
        }
    }
}

pub fn compact_mapping_for_runtime_log(
    mapping: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let value = compact_for_runtime_log(&Value::Object(mapping.clone().into_iter().collect()), 0);
    match value {
        Value::Object(map) => map.into_iter().collect(),
        other => BTreeMap::from([("_value".into(), other)]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_long_strings() {
        let long = "e".repeat(400);
        let out = compact_for_runtime_log(&Value::String(long), 0);
        let text = out.as_str().unwrap_or("");
        assert!(text.len() < 200);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn truncates_cjk_without_panic() {
        let text = "言".repeat(120);
        let out = compact_for_runtime_log(&Value::String(text), 0);
        let truncated = out.as_str().expect("string");
        assert!(truncated.ends_with('…'));
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }
}
