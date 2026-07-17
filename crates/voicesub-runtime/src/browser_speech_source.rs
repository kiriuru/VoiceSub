//! Port of SST `BrowserSpeechSource` — stale/overlap filtering before transcript pipeline.

use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use tokio::sync::Mutex;
use voicesub_browser::{BrowserAsrGateway, IngestedAsrUpdate};
use voicesub_logging::is_config_full_logging_enabled;

use crate::browser_event_builder::BrowserTranscriptEventBuilder;
use crate::http::RuntimeMetricsCollector;
use crate::transcript_controller::TranscriptController;

#[derive(Debug, Clone, Default)]
struct SessionGenerationState {
    active_session_id: Option<String>,
    active_generation_id: u64,
}

/// Mirrors `BrowserAsrService::accept_payload` session/generation rules.
fn accept_browser_session_generation(
    state: &mut SessionGenerationState,
    session_id: Option<&str>,
    generation_id: u64,
) -> bool {
    if let Some(session_id) = session_id {
        if let Some(active) = state.active_session_id.as_deref() {
            if active != session_id {
                if generation_id > 0 && generation_id <= state.active_generation_id {
                    return false;
                }
                state.active_session_id = Some(session_id.to_string());
                if generation_id > 0 {
                    state.active_generation_id = generation_id;
                }
                return true;
            }
        } else {
            state.active_session_id = Some(session_id.to_string());
        }
    }
    if generation_id > 0 && generation_id < state.active_generation_id {
        return false;
    }
    if generation_id > 0 {
        state.active_generation_id = generation_id;
    }
    true
}

enum IngestWorkKind {
    Partial {
        text: String,
        source_lang: String,
        client_segment_id: Option<String>,
    },
    Final {
        text: String,
        source_lang: String,
        client_segment_id: Option<String>,
        forced_final: bool,
    },
}

struct IngestWork {
    event_builder: Arc<BrowserTranscriptEventBuilder>,
    transcript: Arc<TranscriptController>,
    gateway: Arc<std::sync::Mutex<BrowserAsrGateway>>,
    kind: IngestWorkKind,
}

pub struct BrowserSpeechSource {
    session: SessionGenerationState,
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
            session: SessionGenerationState::default(),
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
        self.session = SessionGenerationState::default();
        self.sequence_watermark.clear();
        if let Ok(mut gateway) = self.gateway.lock() {
            gateway.reset_ingest_session();
        }
    }

    pub async fn stop(&mut self) {
        self.start().await;
    }

    fn seq_key(session_id: Option<&str>, generation_id: u64) -> String {
        format!("{}:{generation_id}", session_id.unwrap_or(""))
    }

    fn browser_source_lang(snapshot: &std::sync::RwLock<serde_json::Value>) -> String {
        snapshot
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get("source_lang")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .trim()
            .to_ascii_lowercase()
    }

    fn note_stale_ignored(gateway: &Arc<std::sync::Mutex<BrowserAsrGateway>>) {
        if let Ok(mut gateway) = gateway.lock() {
            gateway.note_stale_worker_event_ignored();
        }
    }

    /// Sync validation and session bookkeeping only — no await while session lock is held upstream.
    fn accept_update(&mut self, update: IngestedAsrUpdate) -> Option<IngestWork> {
        if !self.runtime_running.load(Ordering::Relaxed) {
            return None;
        }
        self.metrics.record_browser_transcript_received();

        let normalized_session_id = update
            .session_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let normalized_generation_id = update.generation_id;
        let previous_session_id = self.session.active_session_id.clone();

        if !accept_browser_session_generation(
            &mut self.session,
            normalized_session_id.as_deref(),
            normalized_generation_id,
        ) {
            Self::note_stale_ignored(&self.gateway);
            return None;
        }
        if previous_session_id.as_deref() != self.session.active_session_id.as_deref() {
            self.sequence_watermark.clear();
        }

        if let Some(worker_message_sequence) = update.worker_message_sequence {
            let key = Self::seq_key(normalized_session_id.as_deref(), normalized_generation_id);
            let seq = worker_message_sequence as i64;
            let prev = self.sequence_watermark.get(&key).copied().unwrap_or(-1);
            if seq <= prev {
                Self::note_stale_ignored(&self.gateway);
                return None;
            }
            self.sequence_watermark.insert(key, seq);
        }

        let normalized_source_lang = update
            .source_lang
            .as_deref()
            .map(str::trim)
            .filter(|lang| !lang.is_empty())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| Self::browser_source_lang(&self.config_snapshot));

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
            .filter(|id| !id.is_empty())
            .map(str::to_string);

        if !partial_text.is_empty() && !update.is_final {
            return Some(IngestWork {
                event_builder: self.event_builder.clone(),
                transcript: self.transcript.clone(),
                gateway: self.gateway.clone(),
                kind: IngestWorkKind::Partial {
                    text: partial_text,
                    source_lang: normalized_source_lang,
                    client_segment_id,
                },
            });
        }

        if update.is_final && !final_text.is_empty() {
            return Some(IngestWork {
                event_builder: self.event_builder.clone(),
                transcript: self.transcript.clone(),
                gateway: self.gateway.clone(),
                kind: IngestWorkKind::Final {
                    text: final_text,
                    source_lang: normalized_source_lang,
                    client_segment_id,
                    forced_final: update.forced_final,
                },
            });
        }

        None
    }

    pub async fn ingest_external_asr_update(&mut self, update: IngestedAsrUpdate) {
        if let Some(work) = self.accept_update(update) {
            process_ingest_work(work).await;
        }
    }
}

async fn process_ingest_work(work: IngestWork) {
    let empty_config = serde_json::Value::Null;
    let ingest_started = is_config_full_logging_enabled().then(std::time::Instant::now);

    match work.kind {
        IngestWorkKind::Partial {
            text,
            source_lang,
            client_segment_id,
        } => {
            let Some(event) = work
                .event_builder
                .build_partial_event(
                    &text,
                    &source_lang,
                    client_segment_id.as_deref(),
                    &empty_config,
                )
                .await
            else {
                return;
            };
            if let Ok(mut gateway) = work.gateway.lock() {
                gateway.note_partial(
                    text.chars().count(),
                    Some(&source_lang),
                    Some(event.sequence),
                );
            }
            work.transcript.handle_event(event, ingest_started).await;
        }
        IngestWorkKind::Final {
            text,
            source_lang,
            client_segment_id,
            forced_final,
        } => {
            let Some(event) = work
                .event_builder
                .build_final_event(&text, &source_lang, client_segment_id.as_deref())
                .await
            else {
                return;
            };
            if let Ok(mut gateway) = work.gateway.lock() {
                gateway.note_final(
                    text.chars().count(),
                    Some(&source_lang),
                    Some(event.sequence),
                    forced_final,
                );
            }
            work.transcript.handle_event(event, ingest_started).await;
        }
    }
}

pub struct SharedBrowserSpeechSource {
    inner: Mutex<BrowserSpeechSource>,
}

/// Serializes browser transcript ingest on a single async task (preserves WS order).
pub struct OrderedBrowserSpeechIngest {
    speech: Arc<SharedBrowserSpeechSource>,
    /// Local ASR capture runs on std threads without a tokio handle — use a std channel bridge.
    cross_thread_tx: std::sync::Mutex<std::sync::mpsc::Sender<IngestedAsrUpdate>>,
    bridge_rx: std::sync::Mutex<Option<std::sync::mpsc::Receiver<IngestedAsrUpdate>>>,
    pump_started: AtomicBool,
}

impl OrderedBrowserSpeechIngest {
    pub fn new(speech: Arc<SharedBrowserSpeechSource>) -> Self {
        let (cross_thread_tx, cross_thread_rx) = std::sync::mpsc::channel();
        Self {
            speech,
            cross_thread_tx: std::sync::Mutex::new(cross_thread_tx),
            bridge_rx: std::sync::Mutex::new(Some(cross_thread_rx)),
            pump_started: AtomicBool::new(false),
        }
    }

    /// Starts the cross-thread → tokio ingest pump. Call once from a tokio runtime (HTTP start).
    pub fn spawn_pump(&self, handle: tokio::runtime::Handle) {
        if self.pump_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(std_rx) = self
            .bridge_rx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        else {
            tracing::warn!("ordered browser speech ingest pump already consumed receiver");
            return;
        };

        let (tokio_tx, mut tokio_rx) = tokio::sync::mpsc::unbounded_channel();
        if let Err(err) = std::thread::Builder::new()
            .name("browser-speech-ingest-bridge".into())
            .spawn(move || {
                while let Ok(update) = std_rx.recv() {
                    if tokio_tx.send(update).is_err() {
                        break;
                    }
                }
            })
        {
            tracing::error!(error = %err, "failed to spawn browser speech ingest bridge thread");
        }

        let speech = self.speech.clone();
        handle.spawn(async move {
            while let Some(update) = tokio_rx.recv().await {
                let speech = speech.clone();
                if let Err(err) =
                    tokio::spawn(async move { speech.ingest(update).await }).await
                {
                    tracing::error!(error = %err, "browser speech ingest task failed; continuing");
                }
            }
            tracing::warn!("ordered browser speech ingest pump exited");
        });
    }

    pub fn enqueue(&self, update: IngestedAsrUpdate) {
        let tx = self
            .cross_thread_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if tx.send(update).is_err() {
            tracing::warn!("ordered browser speech ingest channel closed");
        }
    }
}

impl SharedBrowserSpeechSource {
    pub fn new(source: BrowserSpeechSource) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(source),
        })
    }

    pub async fn ingest(&self, update: IngestedAsrUpdate) {
        let work = {
            let mut inner = self.inner.lock().await;
            inner.accept_update(update)
        };
        if let Some(work) = work {
            process_ingest_work(work).await;
        }
    }

    pub async fn start(&self) {
        self.inner.lock().await.start().await;
    }

    pub async fn stop(&self) {
        self.inner.lock().await.stop().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_rollover_accepts_new_session_when_generation_advances() {
        let mut session = SessionGenerationState::default();
        assert!(accept_browser_session_generation(
            &mut session,
            Some("session-a"),
            1
        ));
        assert!(accept_browser_session_generation(
            &mut session,
            Some("session-b"),
            2
        ));
        assert_eq!(session.active_session_id.as_deref(), Some("session-b"));
        assert_eq!(session.active_generation_id, 2);
    }

    #[test]
    fn stale_generation_with_different_session_is_rejected() {
        let mut session = SessionGenerationState::default();
        assert!(accept_browser_session_generation(
            &mut session,
            Some("session-a"),
            3
        ));
        assert!(!accept_browser_session_generation(
            &mut session,
            Some("session-b"),
            2
        ));
        assert_eq!(session.active_session_id.as_deref(), Some("session-a"));
        assert_eq!(session.active_generation_id, 3);
    }

    #[test]
    fn generation_monotonic_within_same_session() {
        let mut session = SessionGenerationState::default();
        assert!(accept_browser_session_generation(
            &mut session,
            Some("session-a"),
            2
        ));
        assert!(!accept_browser_session_generation(
            &mut session,
            Some("session-a"),
            1
        ));
        assert!(accept_browser_session_generation(
            &mut session,
            Some("session-a"),
            4
        ));
        assert_eq!(session.active_generation_id, 4);
    }
}
