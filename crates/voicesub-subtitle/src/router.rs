use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use tokio::sync::{OnceCell, oneshot};

use crate::trace::{StructuredLogFn, SubtitleLog};
use crate::types::{SubtitlePayloadEvent, TranscriptEvent, TranscriptKind, TranslationEvent};

use crate::router_actor::{
    SourceCommand, SubtitleActorHandles, TranslationCommand, spawn_subtitle_actor,
};

pub type ConfigGetter = Arc<dyn Fn() -> Value + Send + Sync>;
pub type PublishCallback = Arc<dyn Fn(SubtitlePayloadEvent) + Send + Sync>;

struct RouterInit {
    config_getter: ConfigGetter,
    publish: PublishCallback,
    log: SubtitleLog,
    stale_count: Arc<AtomicU64>,
    mismatch_count: Arc<AtomicU64>,
}

struct ActorBundle {
    handles: SubtitleActorHandles,
    _task: tokio::task::JoinHandle<()>,
}

pub struct SubtitleRouter {
    init: Arc<RouterInit>,
    actor: OnceCell<ActorBundle>,
}

impl SubtitleRouter {
    pub fn new(
        config_getter: ConfigGetter,
        publish: PublishCallback,
        structured_log: Option<StructuredLogFn>,
    ) -> Arc<Self> {
        Arc::new(SubtitleRouter {
            init: Arc::new(RouterInit {
                config_getter,
                publish,
                log: SubtitleLog::new(structured_log),
                stale_count: Arc::new(AtomicU64::new(0)),
                mismatch_count: Arc::new(AtomicU64::new(0)),
            }),
            actor: OnceCell::new(),
        })
    }

    async fn actor(&self) -> &SubtitleActorHandles {
        &self
            .actor
            .get_or_init(|| async {
                let init = self.init.clone();
                let (handles, task) = spawn_subtitle_actor(
                    init.config_getter.clone(),
                    init.publish.clone(),
                    init.log.clone(),
                    init.stale_count.clone(),
                    init.mismatch_count.clone(),
                );
                ActorBundle {
                    handles,
                    _task: task,
                }
            })
            .await
            .handles
    }

    pub fn diagnostic_counters(&self) -> Value {
        json!({
            "overlay_stale_translation_suppressed": self
                .init
                .stale_count
                .load(Ordering::Relaxed),
            "overlay_payload_mismatch_count": self.init.mismatch_count.load(Ordering::Relaxed),
        })
    }

    pub async fn reset(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::Reset(reply_tx))
            .await;
        let _ = reply_rx.await;
    }

    pub async fn republish_latest(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::Republish(reply_tx))
            .await;
        let _ = reply_rx.await;
    }

    pub async fn clear_active_partial(&self) {
        let _ = self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::ClearPartial)
            .await;
        self.flush_overlay_publish().await;
    }

    /// Enqueue source/transcript work on the high-priority actor queue.
    /// Final transcripts are applied before this call returns so translation
    /// jobs see up-to-date lifecycle state.
    pub async fn handle_transcript(&self, event: TranscriptEvent) {
        let (processed, reply_rx) = if event.event == TranscriptKind::Final {
            let (tx, rx) = oneshot::channel();
            (Some(tx), Some(rx))
        } else {
            (None, None)
        };
        let _ = self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::Transcript { event, processed })
            .await;
        if let Some(reply_rx) = reply_rx {
            let _ = reply_rx.await;
        }
    }

    /// Enqueue translation work on the low-priority actor queue.
    /// Returns after the actor has applied the translation to lifecycle state.
    pub async fn handle_translation(&self, event: TranslationEvent) {
        let (processed, reply_rx) = oneshot::channel();
        let _ = self
            .actor()
            .await
            .translation_tx
            .send(TranslationCommand::Translation {
                event,
                processed: Some(processed),
            })
            .await;
        let _ = reply_rx.await;
    }

    /// Await any coalesced overlay publish (for deterministic tests).
    #[doc(hidden)]
    pub async fn flush_overlay_publish(&self) {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::FlushPublish(reply_tx))
            .await
            .is_err()
        {
            return;
        }
        let _ = reply_rx.await;
    }

    /// Back-compat alias for tests written before unified overlay coalescing.
    #[doc(hidden)]
    pub async fn flush_translation_publish(&self) {
        self.flush_overlay_publish().await;
    }

    pub async fn ingest_browser_text(
        &self,
        text: &str,
        is_final: bool,
        source_lang: Option<&str>,
        provider: Option<&str>,
    ) -> u64 {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::IngestBrowserText {
                text: text.to_string(),
                is_final,
                source_lang: source_lang.map(str::to_string),
                provider: provider.map(str::to_string),
                reply: reply_tx,
            })
            .await;
        reply_rx.await.unwrap_or(0)
    }

    pub async fn is_sequence_relevant_for_translation(&self, sequence: u64) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::QueryTranslationRelevance {
                sequence,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn is_sequence_relevant_for_presentation(&self, sequence: u64) -> bool {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::QueryPresentationRelevance {
                sequence,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return false;
        }
        reply_rx.await.unwrap_or(false)
    }

    pub async fn record_for_sequence(&self, sequence: u64) -> Option<Value> {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self
            .actor()
            .await
            .source_tx
            .send(SourceCommand::QueryRecord {
                sequence,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return None;
        }
        reply_rx.await.ok().flatten()
    }
}

pub(crate) fn boxed_config_getter(getter: &ConfigGetter) -> Box<dyn Fn() -> Value + Send + Sync> {
    let getter = getter.clone();
    Box::new(move || (getter)())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TranscriptKind;

    #[tokio::test]
    async fn republish_latest_emits_current_payload() {
        let config: ConfigGetter = Arc::new(|| json!({ "source_lang": "en" }));
        let count = Arc::new(AtomicU64::new(0));
        let count_cb = count.clone();
        let publish: PublishCallback = Arc::new(move |_| {
            count_cb.fetch_add(1, Ordering::Relaxed);
        });
        let router = SubtitleRouter::new(config, publish, None);

        router
            .handle_transcript(TranscriptEvent {
                event: TranscriptKind::Partial,
                text: "hello".into(),
                sequence: 1,
                segment: None,
            })
            .await;
        router.flush_overlay_publish().await;
        assert_eq!(count.load(Ordering::Relaxed), 1);

        router.republish_latest().await;
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn clear_active_partial_clears_partial_and_republishes() {
        let config: ConfigGetter = Arc::new(|| {
            json!({
                "source_lang": "en",
                "subtitle_output": { "show_source": true, "show_translations": false },
                "overlay": { "preset": "single", "compact": false },
            })
        });
        let messages = Arc::new(std::sync::Mutex::new(Vec::<SubtitlePayloadEvent>::new()));
        let messages_cb = messages.clone();
        let publish: PublishCallback = Arc::new(move |payload| {
            messages_cb.lock().unwrap().push(payload);
        });
        let router = SubtitleRouter::new(config, publish, None);

        router
            .handle_transcript(TranscriptEvent {
                event: TranscriptKind::Partial,
                text: "typing".into(),
                sequence: 1,
                segment: None,
            })
            .await;
        router.flush_overlay_publish().await;
        {
            let guard = messages.lock().unwrap();
            assert_eq!(
                guard.last().map(|p| p.lifecycle_state),
                Some(crate::types::LifecycleState::PartialOnly)
            );
        }

        router.clear_active_partial().await;
        {
            let guard = messages.lock().unwrap();
            assert_eq!(
                guard.last().map(|p| p.lifecycle_state),
                Some(crate::types::LifecycleState::Idle)
            );
        }
    }
}
