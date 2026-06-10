use serde_json::Value;
use voicesub_logging::tts_trace;

pub fn trace(component: &str, event: &str, fields: Value) {
    tracing::debug!(
        target: "voicesub.twitch",
        component = component,
        event = event,
        fields = %fields,
    );
    tts_trace(component, event, fields);
}

pub fn text_fields(text: &str) -> Value {
    let preview: String = text.chars().take(80).collect();
    let truncated = text.chars().count() > 80;
    serde_json::json!({
        "text_len": text.chars().count(),
        "preview": if truncated { format!("{preview}…") } else { preview },
    })
}

pub fn with_text(base: Value, text: &str) -> Value {
    let mut obj = base.as_object().cloned().unwrap_or_default();
    if let Some(text_obj) = text_fields(text).as_object() {
        obj.extend(text_obj.clone());
    }
    Value::Object(obj)
}
