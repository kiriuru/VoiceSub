//! SST `backend/config/secrets.py` parity.

use std::collections::HashMap;

fn trim_str(raw: &str) -> String {
    raw.trim().to_string()
}

fn parse_query_values(query: &str) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    let query = query.trim_start_matches('?');
    if query.is_empty() {
        return out;
    }
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair
            .split_once('=')
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .unwrap_or_else(|| (pair.to_string(), String::new()));
        out.entry(key).or_default().push(value);
    }
    out
}

fn first_query_value(query_values: &HashMap<String, Vec<String>>, key: &str) -> Option<String> {
    query_values
        .get(key)
        .and_then(|values| values.first())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// SST `normalize_google_translate_api_key`.
pub fn normalize_google_translate_api_key(raw_value: &str) -> String {
    let trimmed = trim_str(raw_value);
    let mut normalized = trimmed.clone();

    if trimmed.contains("key=") {
        let query = if let Some((_, query)) = trimmed.split_once('?') {
            query
        } else {
            trimmed.as_str()
        };
        let query_values = parse_query_values(query);
        if let Some(candidate) = first_query_value(&query_values, "key") {
            normalized = candidate;
        }
    }

    if normalized.starts_with("AIza")
        && normalized.contains('&')
        && let Some(candidate) = normalized.split('&').next()
    {
        let candidate = candidate.trim();
        if !candidate.is_empty() {
            normalized = candidate.to_string();
        }
    }

    normalized
}

/// SST `normalize_provider_secret`.
pub fn normalize_provider_secret(raw_value: &str) -> String {
    normalize_provider_secret_with_keys(raw_value, &["key", "api_key"])
}

pub fn normalize_provider_secret_with_keys(raw_value: &str, query_keys: &[&str]) -> String {
    let mut normalized = trim_str(raw_value);

    if normalized.to_ascii_lowercase().starts_with("bearer ") {
        normalized = normalized[7..].trim().to_string();
    }

    if query_keys
        .iter()
        .any(|key| normalized.contains(&format!("{key}=")))
    {
        let query = if let Some((_, query)) = normalized.split_once('?') {
            query
        } else {
            normalized.as_str()
        };
        let query_values = parse_query_values(query);
        for key in query_keys {
            if let Some(candidate) = first_query_value(&query_values, key) {
                normalized = candidate;
                break;
            }
        }
    }

    if let Some((left, _)) = normalized.split_once('#') {
        normalized = left.trim().to_string();
    }

    if let Some((left, _)) = normalized.split_once('&') {
        let candidate = left.trim();
        if !candidate.is_empty() {
            normalized = candidate.to_string();
        }
    }

    normalized
}

/// SST `normalize_provider_text_value`.
pub fn normalize_provider_text_value(raw_value: &str) -> String {
    trim_str(raw_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_bearer_prefix() {
        assert_eq!(
            normalize_provider_secret("Bearer ya29.test-token"),
            "ya29.test-token"
        );
    }

    #[test]
    fn extracts_google_api_key_from_query_url() {
        assert_eq!(
            normalize_google_translate_api_key("https://example.com?key=AIza-demo&other=1"),
            "AIza-demo"
        );
    }
}
