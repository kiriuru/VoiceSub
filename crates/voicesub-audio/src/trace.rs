use serde_json::{Value, json};
use voicesub_logging::tts_trace;

pub fn trace(component: &str, event: &str, fields: Value) {
    tracing::debug!(
        target: "voicesub.tts.audio",
        component = component,
        event = event,
        fields = %fields,
    );
    tts_trace(component, event, fields);
}

pub fn device_id_field(device_id: &str) -> Value {
    if device_id.is_empty() {
        json!("default")
    } else {
        json!(device_id)
    }
}
