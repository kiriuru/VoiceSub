//! SST `backend/core/runtime/translation_runtime_coordinator.py` parity.

use serde_json::{Value, json};

fn int_field(value: Option<&Value>) -> i64 {
    value
        .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
        .unwrap_or(0)
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

/// Merge engine readiness with dispatcher runtime metrics (SST `summarize_translation_diagnostics`).
pub fn summarize_translation_diagnostics(
    _translation_config: &Value,
    readiness: Value,
    dispatcher_snapshot: &Value,
) -> Value {
    let snapshot = dispatcher_snapshot.as_object().cloned().unwrap_or_default();
    let mut merged = readiness.as_object().cloned().unwrap_or_default();

    merged.insert(
        "queue_depth".into(),
        json!(int_field(snapshot.get("translation_queue_depth"))),
    );
    merged.insert(
        "jobs_started".into(),
        json!(int_field(snapshot.get("translation_jobs_started"))),
    );
    merged.insert(
        "jobs_cancelled".into(),
        json!(int_field(snapshot.get("translation_jobs_cancelled"))),
    );
    merged.insert(
        "stale_results_dropped".into(),
        json!(int_field(snapshot.get("translation_stale_results_dropped"))),
    );
    merged.insert(
        "last_queue_latency_ms".into(),
        snapshot
            .get("translation_queue_latency_ms")
            .cloned()
            .unwrap_or(Value::Null),
    );
    merged.insert(
        "last_provider_latency_ms".into(),
        snapshot
            .get("translation_provider_latency_ms")
            .cloned()
            .unwrap_or(Value::Null),
    );
    merged.insert(
        "last_runtime_reason".into(),
        optional_string(snapshot.get("translation_last_runtime_reason"))
            .map(Value::String)
            .unwrap_or(Value::Null),
    );

    Value::Object(merged)
}

/// SST error fallback when readiness cannot be produced.
pub fn translation_diagnostics_error(reason: impl Into<String>) -> Value {
    json!({
        "enabled": false,
        "status": "error",
        "summary": "Translation diagnostics unavailable.",
        "reason": reason.into(),
        "degraded": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_dispatcher_metrics_into_readiness() {
        let readiness = json!({
            "enabled": true,
            "ready": true,
            "status": "ready"
        });
        let snapshot = json!({
            "translation_queue_depth": 3,
            "translation_jobs_started": 9,
            "translation_jobs_cancelled": 1,
            "translation_stale_results_dropped": 2,
            "translation_queue_latency_ms": 12.5,
            "translation_provider_latency_ms": 88.0,
            "translation_last_runtime_reason": "stale"
        });
        let merged = summarize_translation_diagnostics(&json!({}), readiness, &snapshot);
        assert_eq!(merged["queue_depth"], 3);
        assert_eq!(merged["jobs_started"], 9);
        assert_eq!(merged["jobs_cancelled"], 1);
        assert_eq!(merged["stale_results_dropped"], 2);
        assert_eq!(merged["last_queue_latency_ms"], 12.5);
        assert_eq!(merged["last_provider_latency_ms"], 88.0);
        assert_eq!(merged["last_runtime_reason"], "stale");
        assert_eq!(merged["status"], "ready");
    }

    #[test]
    fn error_fallback_shape() {
        let out = translation_diagnostics_error("boom");
        assert_eq!(out["status"], "error");
        assert_eq!(out["reason"], "boom");
    }
}
