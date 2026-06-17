use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use voicesub_logging::{StructuredRuntimeLogger, browser_trace};

pub type StructuredLogFn = Arc<dyn Fn(&str, &str, Value) + Send + Sync>;

pub const BROWSER_LOG_CHANNEL: &str = "browser_recognition";
pub const BROWSER_LOG_SOURCE: &str = "browser_asr_gateway";

pub fn structured_log_from_runtime_logger(logger: Arc<StructuredRuntimeLogger>) -> StructuredLogFn {
    Arc::new(move |channel, event, fields| {
        let mut map = BTreeMap::new();
        if let Some(obj) = fields.as_object() {
            for (key, value) in obj {
                map.insert(key.clone(), value.clone());
            }
        }
        logger.log(channel, event, Some(BROWSER_LOG_SOURCE), Some(map));
    })
}

#[derive(Clone, Default)]
pub struct BrowserAsrLog {
    structured: Option<StructuredLogFn>,
}

impl BrowserAsrLog {
    pub fn new(structured: Option<StructuredLogFn>) -> Self {
        Self { structured }
    }

    pub(crate) fn emit(&self, event: &str, fields: Value) {
        if let Some(ref logger) = self.structured {
            logger(BROWSER_LOG_CHANNEL, event, fields.clone());
        }
        browser_trace(BROWSER_LOG_CHANNEL, BROWSER_LOG_SOURCE, event, fields);
    }
}
