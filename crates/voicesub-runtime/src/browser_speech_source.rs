//! Port of SST `BrowserSpeechSource` — stale/overlap filtering before transcript pipeline.

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::sync::Mutex;
use voicesub_browser::{BrowserAsrGateway, IngestedAsrUpdate};

use crate::browser_event_builder::BrowserTranscriptEventBuilder;
use crate::http::RuntimeMetricsCollector;
use crate::transcript_controller::TranscriptController;

pub struct BrowserSpeechSource {
    active_session_id: Option<String>,
    active_generation_id: u64,
    sequence_watermark: HashMap<String, i64>,
    runtime_running: Arc<AtomicBool>,
    event_builder: Arc<BrowserTranscriptEventBuilder>,
    transcript: Arc<TranscriptController>,
    config_snapshot: Arc<std::sync::RwLock<serde_json::Value>>,
    gateway: Arc<std::sync::Mutex<BrowserAsrGateway>>,
    metrics: Arc<RuntimeMetricsCollector>,
}

impl BrowserSpeechSource {
    pub fn new(
        runtime_running: Arc<AtomicBool>,
        event_builder: Arc<BrowserTranscriptEventBuilder>,
        transcript: Arc<TranscriptController>,
        config_snapshot: Arc<std::sync::RwLock<serde_json::Value>>,
        gateway: Arc<std::sync::Mutex<BrowserAsrGateway>>,
        metrics: Arc<RuntimeMetricsCollector>,
    ) -> Self {
        Self {
            active_session_id: None,
            active_generation_id: 0,
            sequence_watermark: HashMap::new(),
            runtime_running,
            event_builder,
            transcript,
            config_snapshot,
            gateway,
            metrics,
        }
    }

    pub async fn start(&mut self) {
        self.active_session_id = None;
        self.active_generation_id = 0;
        self.sequence_watermark.clear();
    }

    pub async fn stop(&mut self) {
        self.start().await;
    }

    fn seq_key(session_id: Option<&str>, generation_id: u64) -> String {
        format!("{}:{generation_id}", session_id.unwrap_or(""))
    }

    fn browser_source_lang(config: &serde_json::Value) -> String {
        config
            .get("source_lang")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .trim()
            .to_ascii_lowercase()
    }

    pub async fn ingest_external_asr_update(&mut self, update: IngestedAsrUpdate) {
        if !self.runtime_running.load(Ordering::Relaxed) {
            return;
        }
        self.metrics.record_browser_transcript_received();

        let normalized_session_id = update
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let normalized_generation_id = update.generation_id;

        if let Some(session_id) = normalized_session_id.as_deref() {
            if self
                .active_session_id
                .as_deref()
                .is_some_and(|active| active != session_id)
            {
                if let Ok(mut gateway) = self.gateway.lock() {
                    gateway.note_stale_worker_event_ignored();
                }
                return;
            }
            self.active_session_id = Some(session_id.to_string());
        }
        if normalized_generation_id > 0 {
            if normalized_generation_id < self.active_generation_id {
                if let Ok(mut gateway) = self.gateway.lock() {
                    gateway.note_stale_worker_event_ignored();
                }
                return;
            }
            self.active_generation_id = normalized_generation_id;
        }

        if let Some(worker_message_sequence) = update.worker_message_sequence {
            let key = Self::seq_key(normalized_session_id.as_deref(), normalized_generation_id);
            let seq = worker_message_sequence as i64;
            let prev = self.sequence_watermark.get(&key).copied().unwrap_or(-1);
            if seq <= prev {
                return;
            }
            self.sequence_watermark.insert(key, seq);
        }

        let config = self
            .config_snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let normalized_source_lang = update
            .source_lang
            .as_deref()
            .map(str::trim)
            .filter(|lang| !lang.is_empty())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| Self::browser_source_lang(&config));

        let mut partial_text = update.partial.trim().to_string();
        let mut final_text = update.final_text.trim().to_string();
        if update.is_final && final_text.is_empty() && !partial_text.is_empty() {
            final_text = partial_text.clone();
            partial_text.clear();
        }

        let client_segment_id = update
            .client_segment_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());

        if !partial_text.is_empty() && !update.is_final {
            let Some(event) = self
                .event_builder
                .build_partial_event(
                    &partial_text,
                    &normalized_source_lang,
                    client_segment_id,
                    &config,
                )
                .await
            else {
                return;
            };
            if let Ok(mut gateway) = self.gateway.lock() {
                gateway.note_partial(
                    partial_text.chars().count(),
                    Some(&normalized_source_lang),
                    Some(event.sequence),
                );
            }
            self.transcript.handle_event(event).await;
            return;
        }

        if update.is_final && !final_text.is_empty() {
            let Some(event) = self
                .event_builder
                .build_final_event(&final_text, &normalized_source_lang, client_segment_id)
                .await
            else {
                return;
            };
            if let Ok(mut gateway) = self.gateway.lock() {
                gateway.note_final(
                    final_text.chars().count(),
                    Some(&normalized_source_lang),
                    Some(event.sequence),
                );
            }
            self.transcript.handle_event(event).await;
        }
    }
}

pub struct SharedBrowserSpeechSource {
    inner: Mutex<BrowserSpeechSource>,
}

impl SharedBrowserSpeechSource {
    pub fn new(source: BrowserSpeechSource) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(source),
        })
    }

    pub async fn ingest(&self, update: IngestedAsrUpdate) {
        self.inner.lock().await.ingest_external_asr_update(update).await;
    }

    pub async fn start(&self) {
        self.inner.lock().await.start().await;
    }

    pub async fn stop(&self) {
        self.inner.lock().await.stop().await;
    }
}
