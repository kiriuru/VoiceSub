use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tokio::time::sleep;

use crate::lifecycle::SubtitleLifecycleCore;
use crate::presentation::SubtitlePresentation;
use crate::trace::SubtitleLog;
use crate::types::{TranscriptEvent, TranscriptKind, TranslationEvent};

use crate::router::{ConfigGetter, PublishCallback, boxed_config_getter};

pub(crate) struct RouterInner {
    pub lifecycle: SubtitleLifecycleCore,
    pub presentation: SubtitlePresentation,
    pub next_sequence: u64,
    pub log: SubtitleLog,
    overlay_publish_pending: bool,
    overlay_publish_in_flight: bool,
}

pub(crate) enum SourceCommand {
    Transcript {
        event: TranscriptEvent,
        /// Final transcripts must be applied before translation jobs start.
        processed: Option<oneshot::Sender<()>>,
    },
    ExpiryTick(u64),
    ClearPartial,
    Reset(oneshot::Sender<()>),
    Republish(oneshot::Sender<()>),
    FlushPublish(oneshot::Sender<()>),
    IngestBrowserText {
        text: String,
        is_final: bool,
        source_lang: Option<String>,
        provider: Option<String>,
        reply: oneshot::Sender<u64>,
    },
    QueryTranslationRelevance {
        sequence: u64,
        reply: oneshot::Sender<bool>,
    },
    QueryPresentationRelevance {
        sequence: u64,
        reply: oneshot::Sender<bool>,
    },
    QueryRecord {
        sequence: u64,
        reply: oneshot::Sender<Option<Value>>,
    },
}

pub(crate) enum TranslationCommand {
    Translation {
        event: TranslationEvent,
        processed: Option<oneshot::Sender<()>>,
    },
}

pub(crate) struct SubtitleActorHandles {
    pub source_tx: mpsc::Sender<SourceCommand>,
    pub translation_tx: mpsc::Sender<TranslationCommand>,
}

pub(crate) fn spawn_subtitle_actor(
    config_getter: ConfigGetter,
    publish: PublishCallback,
    log: SubtitleLog,
    stale_count: Arc<AtomicU64>,
    mismatch_count: Arc<AtomicU64>,
) -> (SubtitleActorHandles, tokio::task::JoinHandle<()>) {
    let (source_tx, source_rx) = mpsc::channel(256);
    let (translation_tx, translation_rx) = mpsc::channel(512);

    let stale_cb = stale_count.clone();
    let mismatch_cb = mismatch_count.clone();
    let log_for_stale = log.clone();
    let log_for_mismatch = log.clone();

    let schedule_expiry = {
        let source_tx = source_tx.clone();
        Arc::new(move |sequence: u64, delay_secs: f64| {
            let source_tx = source_tx.clone();
            tokio::spawn(async move {
                sleep(Duration::from_secs_f64(delay_secs)).await;
                let _ = source_tx.send(SourceCommand::ExpiryTick(sequence)).await;
            });
        })
    };

    let presentation = SubtitlePresentation::new(
        boxed_config_getter(&config_getter),
        Box::new(move |amount| {
            stale_cb.fetch_add(amount, Ordering::Relaxed);
            if amount > 0 {
                log_for_stale.stale_translation_suppressed(0);
            }
        }),
        Box::new(move |amount| {
            mismatch_cb.fetch_add(amount, Ordering::Relaxed);
            if amount > 0 {
                log_for_mismatch.payload_mismatch(0);
            }
        }),
        log.clone(),
    );

    let lifecycle = SubtitleLifecycleCore::new(
        boxed_config_getter(&config_getter),
        schedule_expiry,
        log.clone(),
    );

    let inner = RouterInner {
        lifecycle,
        presentation,
        next_sequence: 0,
        log,
        overlay_publish_pending: false,
        overlay_publish_in_flight: false,
    };

    let handles = SubtitleActorHandles {
        source_tx: source_tx.clone(),
        translation_tx,
    };

    let task = tokio::spawn(run_actor(source_rx, translation_rx, inner, publish));

    (handles, task)
}

async fn run_actor(
    mut source_rx: mpsc::Receiver<SourceCommand>,
    translation_rx: mpsc::Receiver<TranslationCommand>,
    mut inner: RouterInner,
    publish: PublishCallback,
) {
    let mut translation_rx = Some(translation_rx);

    loop {
        while let Ok(command) = source_rx.try_recv() {
            process_source(&mut translation_rx, &mut inner, &publish, command).await;
        }

        let translation_recv = async {
            match &mut translation_rx {
                Some(rx) => rx.recv().await,
                None => std::future::pending().await,
            }
        };

        tokio::select! {
            biased;

            command = source_rx.recv() => {
                match command {
                    Some(command) => {
                        process_source(&mut translation_rx, &mut inner, &publish, command).await
                    }
                    None => break,
                }
            }

            command = translation_recv => {
                match command {
                    Some(TranslationCommand::Translation { event, processed }) => {
                        process_translation_command(&mut inner, &publish, event, processed).await;
                    }
                    None => {
                        translation_rx = None;
                    }
                }
            }
        }
    }

    if let Some(mut rx) = translation_rx {
        while let Some(TranslationCommand::Translation { event, processed }) = rx.recv().await {
            process_translation_command(&mut inner, &publish, event, processed).await;
        }
    }
}

async fn process_translation_command(
    inner: &mut RouterInner,
    publish: &PublishCallback,
    event: TranslationEvent,
    processed: Option<oneshot::Sender<()>>,
) {
    apply_translation(inner, event).await;
    schedule_overlay_publish(inner, publish).await;
    if let Some(processed) = processed {
        let _ = processed.send(());
    }
}

async fn drain_translation_queue(
    translation_rx: &mut Option<mpsc::Receiver<TranslationCommand>>,
    inner: &mut RouterInner,
    publish: &PublishCallback,
) -> bool {
    let Some(rx) = translation_rx.as_mut() else {
        return false;
    };
    let mut drained = false;
    while let Ok(TranslationCommand::Translation { event, processed }) = rx.try_recv() {
        drained = true;
        process_translation_command(inner, publish, event, processed).await;
    }
    drained
}

async fn process_source(
    translation_rx: &mut Option<mpsc::Receiver<TranslationCommand>>,
    inner: &mut RouterInner,
    publish: &PublishCallback,
    command: SourceCommand,
) {
    match command {
        SourceCommand::Transcript { event, processed } => {
            apply_transcript(inner, event).await;
            schedule_overlay_publish(inner, publish).await;
            if let Some(processed) = processed {
                let _ = processed.send(());
            }
        }
        SourceCommand::ExpiryTick(sequence) => {
            inner
                .lifecycle
                .on_expiry_tick(sequence, &inner.presentation)
                .await;
            schedule_overlay_publish(inner, publish).await;
        }
        SourceCommand::ClearPartial => {
            inner.lifecycle.clear_active_partial();
            schedule_overlay_publish(inner, publish).await;
        }
        SourceCommand::Reset(reply) => {
            inner.lifecycle.reset().await;
            inner.next_sequence = 0;
            schedule_overlay_publish(inner, publish).await;
            flush_overlay_publish(inner, publish).await;
            let _ = reply.send(());
        }
        SourceCommand::Republish(reply) => {
            schedule_overlay_publish(inner, publish).await;
            let _ = reply.send(());
        }
        SourceCommand::FlushPublish(reply) => {
            let drained_translations =
                drain_translation_queue(translation_rx, inner, publish).await;
            if drained_translations
                || inner.overlay_publish_pending
                || inner.overlay_publish_in_flight
            {
                schedule_overlay_publish(inner, publish).await;
            }
            let _ = reply.send(());
        }
        SourceCommand::IngestBrowserText {
            text,
            is_final,
            source_lang,
            provider,
            reply,
        } => {
            let sequence = ingest_browser_text(inner, &text, is_final, source_lang, provider).await;
            schedule_overlay_publish(inner, publish).await;
            flush_overlay_publish(inner, publish).await;
            let _ = reply.send(sequence);
        }
        SourceCommand::QueryTranslationRelevance { sequence, reply } => {
            let relevant = inner
                .lifecycle
                .is_sequence_relevant_for_translation(sequence, &inner.presentation);
            let _ = reply.send(relevant);
        }
        SourceCommand::QueryPresentationRelevance { sequence, reply } => {
            let relevant = inner
                .lifecycle
                .is_sequence_relevant_for_presentation(sequence, &inner.presentation);
            let _ = reply.send(relevant);
        }
        SourceCommand::QueryRecord { sequence, reply } => {
            let _ = reply.send(inner.lifecycle.record_for_sequence(sequence));
        }
    }
}

async fn apply_transcript(inner: &mut RouterInner, event: TranscriptEvent) {
    inner
        .lifecycle
        .handle_transcript(event, &inner.presentation)
        .await;
}

async fn apply_translation(inner: &mut RouterInner, event: TranslationEvent) {
    inner
        .lifecycle
        .handle_translation(event, &inner.presentation)
        .await;
}

async fn ingest_browser_text(
    inner: &mut RouterInner,
    text: &str,
    is_final: bool,
    source_lang: Option<String>,
    provider: Option<String>,
) -> u64 {
    inner.next_sequence += 1;
    let sequence = inner.next_sequence;
    let lang = source_lang.unwrap_or_else(|| {
        inner
            .presentation
            .config()
            .get("source_lang")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_string()
    });
    let event = TranscriptEvent {
        event: if is_final {
            TranscriptKind::Final
        } else {
            TranscriptKind::Partial
        },
        text: text.to_string(),
        sequence,
        segment: Some(crate::types::TranscriptSegment {
            segment_id: format!("browser-{sequence}"),
            text: text.to_string(),
            is_final,
            source_lang: lang,
            provider,
            sequence,
            revision: 0,
            start_ms: None,
            end_ms: None,
        }),
    };
    apply_transcript(inner, event).await;
    sequence
}

async fn schedule_overlay_publish(inner: &mut RouterInner, publish: &PublishCallback) {
    if inner.overlay_publish_in_flight {
        inner.overlay_publish_pending = true;
        return;
    }
    inner.overlay_publish_in_flight = true;
    inner.overlay_publish_pending = false;
    loop {
        publish_current(inner, publish);
        if inner.overlay_publish_pending {
            inner.overlay_publish_pending = false;
        } else {
            inner.overlay_publish_in_flight = false;
            break;
        }
    }
}

async fn flush_overlay_publish(inner: &mut RouterInner, publish: &PublishCallback) {
    inner.overlay_publish_pending = false;
    schedule_overlay_publish(inner, publish).await;
}

fn publish_current(inner: &mut RouterInner, publish: &PublishCallback) {
    let payload = inner
        .presentation
        .build_presentation_payload(&inner.lifecycle);
    inner.log.payload_published(&payload);
    (publish)(payload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::boxed_config_getter;
    use crate::types::TranscriptKind;
    use serde_json::json;
    use std::sync::Mutex;

    #[tokio::test]
    async fn source_queue_is_processed_before_translation_queue() {
        let (source_tx, source_rx) = mpsc::channel(16);
        let (translation_tx, translation_rx) = mpsc::channel(16);
        let payloads = Arc::new(Mutex::new(Vec::<String>::new()));
        let payloads_for_publish = payloads.clone();

        let publish: PublishCallback = Arc::new(move |payload| {
            payloads_for_publish
                .lock()
                .unwrap()
                .push(payload.active_partial_text.clone());
        });

        let log = SubtitleLog::new(None);
        let config_getter: ConfigGetter = Arc::new(|| {
            json!({
                "source_lang": "en",
                "translation": { "enabled": false },
                "subtitle_output": { "show_source": true, "show_translations": false },
                "overlay": { "preset": "single", "compact": false },
            })
        });

        let schedule_expiry: Arc<dyn Fn(u64, f64) + Send + Sync> = Arc::new(|_, _| {});
        let presentation = SubtitlePresentation::new(
            boxed_config_getter(&config_getter),
            Box::new(|_| {}),
            Box::new(|_| {}),
            log.clone(),
        );
        let lifecycle =
            SubtitleLifecycleCore::new(boxed_config_getter(&config_getter), schedule_expiry, log);
        let inner = RouterInner {
            lifecycle,
            presentation,
            next_sequence: 0,
            log: SubtitleLog::new(None),
            overlay_publish_pending: false,
            overlay_publish_in_flight: false,
        };

        let actor = tokio::spawn(run_actor(source_rx, translation_rx, inner, publish));

        translation_tx
            .send(TranslationCommand::Translation {
                event: TranslationEvent {
                    sequence: 1,
                    source_text: "hello".into(),
                    source_lang: "en".into(),
                    provider: "test".into(),
                    is_complete: true,
                    translations: vec![],
                    ..Default::default()
                },
                processed: None,
            })
            .await
            .expect("translation send");

        source_tx
            .send(SourceCommand::Transcript {
                event: TranscriptEvent {
                    event: TranscriptKind::Partial,
                    text: "live".into(),
                    sequence: 1,
                    segment: None,
                },
                processed: None,
            })
            .await
            .expect("source send");

        let (flush_tx, flush_rx) = oneshot::channel();
        source_tx
            .send(SourceCommand::FlushPublish(flush_tx))
            .await
            .expect("flush send");
        flush_rx.await.expect("flush ack");

        drop(source_tx);
        drop(translation_tx);
        actor.await.expect("actor join");

        let captured = payloads.lock().unwrap();
        assert_eq!(captured.last().map(String::as_str), Some("live"));
    }
}
