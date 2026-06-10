use std::collections::BTreeMap;

use serde_json::Value;

pub const REDACTED_VALUE: &str = "[redacted]";

const SENSITIVE_KEYS: &[&str] = &[
    "api_key",
    "key",
    "q",
    "text",
    "token",
    "secret",
    "password",
    "authorization",
    "credential",
    "credentials",
    "pair_code",
    "local_admin_token",
    "bearer",
];

const SENSITIVE_FRAGMENTS: &[&str] = &[
    "api_key",
    "token",
    "secret",
    "password",
    "authorization",
    "credential",
    "pair_code",
    "local_admin_token",
    "bearer",
];

pub fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if SENSITIVE_KEYS.contains(&normalized.as_str()) {
        return true;
    }
    SENSITIVE_FRAGMENTS
        .iter()
        .any(|fragment| normalized.contains(fragment))
}

pub fn redact_text(value: &str) -> String {
    let mut text = value.to_string();
    if text.is_empty() {
        return text;
    }
    let lower = text.to_ascii_lowercase();
    if let Some(start) = lower.find("bearer ") {
        let rest = &text[start + 7..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .unwrap_or(rest.len());
        let old = &text[start..start + 7 + end];
        text = text.replacen(old, "Bearer [redacted]", 1);
    }
    for key in SENSITIVE_KEYS {
        for pattern in [format!("{key}="), format!("&{key}="), format!("?{key}=")] {
            if let Some(start) = text
                .to_ascii_lowercase()
                .find(&pattern.to_ascii_lowercase())
            {
                let rest = &text[start + pattern.len()..];
                let end = rest
                    .find(|c: char| c == '&' || c.is_whitespace())
                    .unwrap_or(rest.len());
                let old = &text[start..start + pattern.len() + end];
                text = text.replacen(old, &format!("{key}={REDACTED_VALUE}"), 1);
            }
        }
    }
    text
}

pub fn redact_value(value: &Value, key: Option<&str>) -> Value {
    if key.is_some_and(is_sensitive_key) {
        return Value::String(REDACTED_VALUE.into());
    }
    match value {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), redact_value(v, Some(k))))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(|v| redact_value(v, None)).collect()),
        Value::String(s) => {
            if key.is_some_and(|k| k.eq_ignore_ascii_case("endpoint")) && s.contains("secret") {
                Value::String(REDACTED_VALUE.into())
            } else {
                Value::String(redact_text(s))
            }
        }
        other => other.clone(),
    }
}

pub fn redact_mapping(details: &BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    details
        .iter()
        .map(|(k, v)| (k.clone(), redact_value(v, Some(k))))
        .collect()
}

pub fn redact_data(value: &Value) -> Value {
    redact_value(value, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_nested_sensitive_keys_and_endpoint_query_values() {
        let payload = json!({
            "translation": {
                "api_key": "secret-value",
                "endpoint": "https://example.test/translate?token=abc123&mode=fast"
            },
            "providers": { "backup": { "key": "legacy-secret" } },
            "remote": {
                "pair_code": "123456",
                "controller": { "worker_url": "http://192.168.1.10:8765" }
            }
        });
        let redacted = redact_data(&payload);
        assert_eq!(redacted["translation"]["api_key"], REDACTED_VALUE);
        assert_eq!(redacted["providers"]["backup"]["key"], REDACTED_VALUE);
        let endpoint = redacted["translation"]["endpoint"].as_str().unwrap();
        assert!(endpoint.contains(&format!("token={REDACTED_VALUE}")));
        assert!(endpoint.contains("mode=fast"));
        assert_eq!(redacted["remote"]["pair_code"], REDACTED_VALUE);
        assert_eq!(
            redacted["remote"]["controller"]["worker_url"],
            "http://192.168.1.10:8765"
        );
    }

    #[test]
    fn redacts_bearer_tokens_in_text() {
        let text = "Authorization failed for Bearer super-secret-token";
        assert_eq!(
            redact_text(text),
            "Authorization failed for Bearer [redacted]"
        );
    }

    #[test]
    fn redacts_key_query_parameter_in_text() {
        let text = "key=legacy-secret&pair=sst-123";
        assert!(redact_text(text).contains(&format!("key={REDACTED_VALUE}")));
    }
}
