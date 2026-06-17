use serde_json::{Value, json};
use voicesub_logging::tts_trace;

pub fn trace(component: &str, event: &str, fields: Value) {
    tracing::debug!(
        target: "voicesub.tts",
        component = component,
        event = event,
        fields = %fields,
    );
    tts_trace(component, event, fields);
}

pub fn text_fields(text: &str) -> Value {
    let preview: String = text.chars().take(80).collect();
    let truncated = text.chars().count() > 80;
    json!({
        "text_len": text.chars().count(),
        "preview": if truncated { format!("{preview}…") } else { preview },
    })
}
