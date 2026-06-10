use serde::{Deserialize, Serialize};

/// Inbound `external_asr_update` from browser worker (SST `BrowserAsrService.handle_external_update`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalAsrUpdate {
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(default)]
    pub partial: String,
    #[serde(default, rename = "final")]
    pub final_text: String,
    #[serde(default)]
    pub is_final: bool,
    #[serde(default)]
    pub generation_id: u64,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub client_segment_id: Option<String>,
    #[serde(default)]
    pub forced_final: bool,
    #[serde(default)]
    pub source_lang: Option<String>,
}

impl ExternalAsrUpdate {
    pub fn transcript_text(&self) -> String {
        if self.is_final && !self.final_text.is_empty() {
            return self.final_text.trim().to_string();
        }
        self.partial.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_final_when_is_final() {
        let update = ExternalAsrUpdate {
            message_type: "external_asr_update".into(),
            partial: "hel".to_string(),
            final_text: "hello".into(),
            is_final: true,
            generation_id: 1,
            session_id: None,
            client_segment_id: None,
            forced_final: false,
            source_lang: None,
        };
        assert_eq!(update.transcript_text(), "hello");
    }

    #[test]
    fn uses_partial_when_not_final() {
        let update = ExternalAsrUpdate {
            message_type: "external_asr_update".into(),
            partial: "hel".into(),
            final_text: String::new(),
            is_final: false,
            generation_id: 1,
            session_id: None,
            client_segment_id: None,
            forced_final: false,
            source_lang: None,
        };
        assert_eq!(update.transcript_text(), "hel");
    }
}
