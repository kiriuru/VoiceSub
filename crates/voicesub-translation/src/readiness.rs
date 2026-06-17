use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::engine::NormalizedLine;
use crate::providers::{TranslationProvider, canonical_provider_name};

const CONNECT_PROBE_TIMEOUT: Duration = Duration::from_millis(400);

fn required_fields(provider_name: &str) -> &'static [&'static str] {
    match provider_name {
        "google_translate_v2" => &["api_key"],
        "google_cloud_translation_v3" => &["project_id", "access_token"],
        "google_gas_url" => &["gas_url"],
        "azure_translator" => &["api_key", "endpoint"],
        "deepl" => &["api_key"],
        "openai" | "openrouter" => &["api_key", "model"],
        "lm_studio" | "ollama" => &["base_url", "model"],
        _ => &[],
    }
}

fn provider_endpoint_summary(
    provider_name: &str,
    settings: &HashMap<String, String>,
) -> Option<String> {
    match provider_name {
        "google_translate_v2" => {
            Some("https://translation.googleapis.com/language/translate/v2".into())
        }
        "google_cloud_translation_v3" => {
            let project_id = settings.get("project_id").map(|s| s.trim()).unwrap_or("");
            let location = settings
                .get("location")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .unwrap_or("global");
            if project_id.is_empty() {
                None
            } else {
                Some(format!(
                    "https://translation.googleapis.com/v3/projects/{project_id}/locations/{location}:translateText"
                ))
            }
        }
        "google_gas_url" => settings
            .get("gas_url")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        "azure_translator" => Some(
            settings
                .get("endpoint")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "https://api.cognitive.microsofttranslator.com".into()),
        ),
        "deepl" | "libretranslate" | "public_libretranslate_mirror" => settings
            .get("api_url")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        "openai" | "openrouter" | "lm_studio" | "ollama" => settings
            .get("base_url")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()),
        _ => None,
    }
}

fn local_endpoint_reachable(endpoint: &str) -> (bool, Option<String>) {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return (false, Some("endpoint missing".into()));
    }
    let Ok(url) = reqwest::Url::parse(trimmed) else {
        return (false, Some("invalid endpoint URL".into()));
    };
    let host = match url.host_str() {
        Some(host) => host,
        None => return (false, Some("endpoint host missing".into())),
    };
    let port = url
        .port()
        .unwrap_or(if url.scheme() == "https" { 443 } else { 80 });
    let addr = format!("{host}:{port}");
    let Ok(socket_addr) = addr.parse::<SocketAddr>() else {
        return (false, Some("endpoint address invalid".into()));
    };
    match TcpStream::connect_timeout(&socket_addr, CONNECT_PROBE_TIMEOUT) {
        Ok(_) => (true, None),
        Err(err) => (false, Some(format!("unreachable: {err}"))),
    }
}

pub fn summarize_readiness(
    providers: &HashMap<String, Arc<dyn TranslationProvider>>,
    translation_config: &Value,
    configured_lines: &[NormalizedLine],
    provider_settings: &HashMap<String, HashMap<String, String>>,
) -> Value {
    let enabled = translation_config
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !enabled {
        return json!({
            "enabled": false,
            "status": "disabled",
            "summary": "Translation disabled.",
        });
    }

    let enabled_lines: Vec<_> = configured_lines
        .iter()
        .filter(|line| line.enabled)
        .collect();

    if enabled_lines.is_empty() {
        return json!({
            "enabled": true,
            "provider": canonical_provider_name(
                translation_config
                    .get("provider")
                    .and_then(|v| v.as_str())
                    .unwrap_or("google_translate_v2"),
            ),
            "configured": false,
            "ready": false,
            "degraded": true,
            "status": "partial",
            "summary": "Translation enabled, but no translation lines are configured.",
            "reason": "Enable at least one translation line.",
            "target_languages": [],
            "line_count": configured_lines.len(),
            "enabled_line_count": 0,
            "line_providers": [],
            "line_target_languages": [],
            "line_missing_fields": {},
        });
    }

    let mut line_providers = Vec::new();
    let mut line_target_languages = Vec::new();
    let mut line_missing_fields = BTreeMap::new();
    let mut missing_fields = Vec::new();
    let mut diagnostics_by_provider = Vec::new();
    let mut used_default_prompt = false;
    let mut any_experimental = false;
    let mut any_local = false;
    let mut all_ready = true;
    let mut unreachable_local = BTreeMap::new();

    for line in &enabled_lines {
        let provider_name = canonical_provider_name(&line.provider);
        line_providers.push(provider_name.clone());
        line_target_languages.push(line.target_lang.clone());
        let settings = provider_settings
            .get(&provider_name)
            .cloned()
            .unwrap_or_default();
        let normalized: HashMap<String, String> = settings
            .into_iter()
            .map(|(k, v)| (k, v.trim().to_string()))
            .collect();

        let Some(provider) = providers.get(&provider_name) else {
            let missing = vec!["provider".to_string()];
            line_missing_fields.insert(line.slot_id.clone(), missing.clone());
            missing_fields.extend(missing);
            all_ready = false;
            continue;
        };

        let diag = provider.diagnostics(&normalized);
        diagnostics_by_provider.push(diag.clone());
        used_default_prompt |= diag
            .get("used_default_prompt")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        any_experimental |= provider.info().experimental;
        any_local |= provider.info().local_provider;

        let missing: Vec<String> = required_fields(&provider_name)
            .iter()
            .filter(|field| {
                normalized
                    .get(**field)
                    .map(|s| s.as_str())
                    .unwrap_or("")
                    .is_empty()
            })
            .map(|field| (*field).to_string())
            .collect();
        if !missing.is_empty() {
            line_missing_fields.insert(line.slot_id.clone(), missing.clone());
            missing_fields.extend(missing);
            all_ready = false;
            continue;
        }

        if provider.info().local_provider
            && let Some(endpoint) = provider_endpoint_summary(&provider_name, &normalized)
        {
            let (reachable, reason) = local_endpoint_reachable(&endpoint);
            if !reachable {
                if let Some(reason) = reason {
                    unreachable_local.insert(line.slot_id.clone(), reason);
                }
                all_ready = false;
            }
        }
    }

    let unique_providers: HashSet<_> = line_providers.iter().cloned().collect();
    let primary_provider = if unique_providers.len() == 1 {
        line_providers
            .first()
            .cloned()
            .unwrap_or_else(|| "mixed".into())
    } else {
        "mixed".into()
    };
    let groups: HashSet<String> = diagnostics_by_provider
        .iter()
        .filter_map(|d| {
            d.get("provider_group")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    let primary_group = if groups.len() == 1 {
        groups.into_iter().next().unwrap_or_else(|| "mixed".into())
    } else {
        "mixed".into()
    };
    let endpoint = line_providers
        .first()
        .and_then(|name| provider_settings.get(name))
        .and_then(|settings| provider_endpoint_summary(line_providers.first()?, settings));

    let missing_unique: Vec<String> = missing_fields
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if !line_missing_fields.is_empty() {
        let summary = if unique_providers.len() > 1 {
            format!(
                "Translation providers are partially configured across {} enabled line(s).",
                enabled_lines.len()
            )
        } else {
            format!("Translation provider '{primary_provider}' is partially configured.")
        };
        return json!({
            "enabled": true,
            "provider": primary_provider,
            "provider_group": primary_group,
            "experimental": any_experimental,
            "local_provider": any_local,
            "configured": true,
            "ready": false,
            "degraded": true,
            "status": "partial",
            "summary": summary,
            "reason": "Missing required settings on one or more translation lines.",
            "missing_fields": missing_unique,
            "target_languages": line_target_languages,
            "provider_endpoint": endpoint,
            "uses_default_prompt": used_default_prompt,
            "line_count": configured_lines.len(),
            "enabled_line_count": enabled_lines.len(),
            "line_providers": line_providers,
            "line_target_languages": line_target_languages,
            "line_missing_fields": line_missing_fields,
        });
    }

    if !unreachable_local.is_empty() {
        let reason = unreachable_local
            .iter()
            .map(|(slot, msg)| format!("{slot}: {msg}"))
            .collect::<Vec<_>>()
            .join("; ");
        return json!({
            "enabled": true,
            "provider": primary_provider,
            "provider_group": primary_group,
            "experimental": any_experimental,
            "local_provider": any_local,
            "configured": true,
            "ready": false,
            "degraded": true,
            "status": "degraded",
            "summary": "One or more local translation providers are configured but unreachable.",
            "reason": reason,
            "missing_fields": [],
            "target_languages": line_target_languages,
            "provider_endpoint": endpoint,
            "uses_default_prompt": used_default_prompt,
            "line_count": configured_lines.len(),
            "enabled_line_count": enabled_lines.len(),
            "line_providers": line_providers,
            "line_target_languages": line_target_languages,
            "line_missing_fields": {},
        });
    }

    if any_experimental && !any_local {
        return json!({
            "enabled": true,
            "provider": primary_provider,
            "provider_group": primary_group,
            "experimental": true,
            "local_provider": any_local,
            "configured": true,
            "ready": true,
            "degraded": true,
            "status": "experimental",
            "summary": "Experimental translation provider configuration is active on one or more lines.",
            "reason": "Experimental providers may fail or change behavior without notice.",
            "missing_fields": [],
            "target_languages": line_target_languages,
            "provider_endpoint": endpoint,
            "uses_default_prompt": used_default_prompt,
            "line_count": configured_lines.len(),
            "enabled_line_count": enabled_lines.len(),
            "line_providers": line_providers,
            "line_target_languages": line_target_languages,
            "line_missing_fields": {},
        });
    }

    let summary = if primary_provider == "mixed" {
        format!(
            "Mixed-provider translation is configured across {} enabled line(s).",
            enabled_lines.len()
        )
    } else {
        format!("Translation provider '{primary_provider}' is configured.")
    };

    json!({
        "enabled": true,
        "provider": primary_provider,
        "provider_group": primary_group,
        "experimental": any_experimental,
        "local_provider": any_local,
        "configured": true,
        "ready": all_ready,
        "degraded": false,
        "status": "ready",
        "summary": summary,
        "reason": null,
        "missing_fields": [],
        "target_languages": line_target_languages,
        "provider_endpoint": endpoint,
        "uses_default_prompt": used_default_prompt,
        "line_count": configured_lines.len(),
        "enabled_line_count": enabled_lines.len(),
        "line_providers": line_providers,
        "line_target_languages": line_target_languages,
        "line_missing_fields": {},
    })
}
