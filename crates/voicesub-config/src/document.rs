use serde_json::Value;

use crate::defaults::default_config_payload;

#[derive(Debug, Clone)]
pub struct ConfigDocument {
    payload: Value,
    loaded_from: String,
}

impl ConfigDocument {
    pub fn with_defaults() -> Self {
        Self {
            payload: default_config_payload(),
            loaded_from: "defaults".into(),
        }
    }

    pub fn from_payload(payload: Value, loaded_from: impl Into<String>) -> Self {
        Self {
            payload,
            loaded_from: loaded_from.into(),
        }
    }

    pub fn payload(&self) -> &Value {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut Value {
        &mut self.payload
    }

    pub fn loaded_from(&self) -> &str {
        &self.loaded_from
    }

    pub fn set_loaded_from(&mut self, loaded_from: impl Into<String>) {
        self.loaded_from = loaded_from.into();
    }

    pub fn merge_save_request(&mut self, incoming: &Value) {
        if let Some(obj) = incoming.as_object() {
            if let Some(root) = self.payload.as_object_mut() {
                for (key, value) in obj {
                    root.insert(key.clone(), value.clone());
                }
            }
        }
    }
}
