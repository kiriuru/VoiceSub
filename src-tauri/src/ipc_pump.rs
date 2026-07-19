//! Bus → Tauri `runtime-event` pump with overlay IPC coalescing and lag-resync debounce.
//!
//! OBS overlay still receives full-rate `overlay_update` on `/ws/events`; only the main
//! dashboard IPC path is coalesced to protect the WebView2 UI thread.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::sync::broadcast;
use tokio::time::{Instant, Sleep};
use tracing::warn;
use voicesub_asr_local::LOCAL_ASR_WINDOW_LABEL;
use voicesub_runtime::RuntimeService;
use voicesub_tts::{TTS_WINDOW_LABEL, TtsSpeechPipeline};
use voicesub_twitch::TwitchChatMessage;

use crate::event_routing;

pub const DEFAULT_OVERLAY_IPC_COALESCE_MS: u64 = 90;
pub const LAG_RESYNC_DEBOUNCE_MS: u64 = 200;

fn overlay_ipc_coalesce_interval() -> Duration {
    let ms = std::env::var("VOICESUB_OVERLAY_IPC_MIN_INTERVAL_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_OVERLAY_IPC_COALESCE_MS);
    Duration::from_millis(ms)
}

/// Event types that bypass overlay coalescing and flush any pending overlay frame first.
pub fn flushes_overlay_ipc_coalesce(event_type: &str) -> bool {
    !matches!(event_type, "overlay_update" | "transcript_update")
}

pub fn overlay_ipc_coalesce_event(event_type: &str) -> bool {
    event_type == "overlay_update"
}

fn emit_to_main(app: &AppHandle, payload: &Value) {
    if let Err(err) = app.emit_to(event_routing::MAIN_WINDOW_LABEL, "runtime-event", payload) {
        warn!(
            target: "voicesub.ipc_pump",
            error = %err,
            "runtime-event emit_to(main) failed"
        );
    }
}

fn emit_to_tts_if_wanted(app: &AppHandle, event_type: &str, payload: &Value) {
    if event_routing::tts_window_wants(event_type)
        && let Err(err) = app.emit_to(TTS_WINDOW_LABEL, "runtime-event", payload)
    {
        warn!(
            target: "voicesub.ipc_pump",
            error = %err,
            event_type,
            "runtime-event emit_to(tts) failed"
        );
    }
}

fn emit_to_local_asr_if_wanted(app: &AppHandle, event_type: &str, payload: &Value) {
    if event_routing::local_asr_window_wants(event_type)
        && let Err(err) = app.emit_to(LOCAL_ASR_WINDOW_LABEL, "runtime-event", payload)
    {
        warn!(
            target: "voicesub.ipc_pump",
            error = %err,
            event_type,
            "runtime-event emit_to(local-asr) failed"
        );
    }
}

fn emit_runtime_event_immediate(app: &AppHandle, event_type: &str, payload: &Value) {
    emit_to_main(app, payload);
    emit_to_tts_if_wanted(app, event_type, payload);
    emit_to_local_asr_if_wanted(app, event_type, payload);
}

struct IpcPumpState {
    overlay_pending: Option<Arc<Value>>,
    overlay_timer: Pin<Box<Sleep>>,
    overlay_timer_active: bool,
    last_lag_resync: Option<Instant>,
    lag_resync_in_flight: Arc<AtomicBool>,
    lag_resync_pending: Arc<AtomicBool>,
}

impl IpcPumpState {
    fn new() -> Self {
        let mut overlay_timer = Box::pin(tokio::time::sleep(Duration::from_secs(3600)));
        overlay_timer
            .as_mut()
            .reset(Instant::now() + Duration::from_secs(3600));
        Self {
            overlay_pending: None,
            overlay_timer,
            overlay_timer_active: false,
            last_lag_resync: None,
            lag_resync_in_flight: Arc::new(AtomicBool::new(false)),
            lag_resync_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    fn queue_overlay(&mut self, message: Arc<Value>, runtime: &RuntimeService, coalesce: Duration) {
        if self.overlay_pending.is_some() {
            runtime.record_overlay_ipc_coalesced();
        }
        self.overlay_pending = Some(message);
        // Trailing-edge: each new frame pushes the flush deadline forward.
        self.overlay_timer.as_mut().reset(Instant::now() + coalesce);
        self.overlay_timer_active = true;
    }

    fn flush_overlay(&mut self, app: &AppHandle) {
        if let Some(message) = self.overlay_pending.take() {
            emit_to_main(app, message.as_ref());
        }
        self.deactivate_overlay_timer();
    }

    /// Drop a pending coalesced frame without emitting — used on bus lag so a
    /// later timer cannot overwrite a fresher snapshot resync.
    fn discard_overlay(&mut self) {
        self.overlay_pending = None;
        self.deactivate_overlay_timer();
    }

    fn deactivate_overlay_timer(&mut self) {
        self.overlay_timer
            .as_mut()
            .reset(Instant::now() + Duration::from_secs(3600));
        self.overlay_timer_active = false;
    }

    fn handle_immediate(&mut self, app: &AppHandle, event_type: &str, payload: &Value) {
        if self.overlay_pending.is_some() {
            self.flush_overlay(app);
        }
        emit_runtime_event_immediate(app, event_type, payload);
    }
}

fn apply_pipeline_side_effects(pipeline: &TtsSpeechPipeline, event_type: &str, message: &Value) {
    if event_type == "runtime_update" {
        let running = message
            .pointer("/payload/running")
            .or_else(|| message.pointer("/payload/is_running"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        pipeline.set_runtime_active(running);
    } else if event_type == "twitch_chat_message"
        && let Ok(chat) = serde_json::from_value::<TwitchChatMessage>(
            message.get("payload").cloned().unwrap_or_default(),
        )
    {
        pipeline.handle_twitch_chat_message(&chat);
    }
}

fn spawn_lag_snapshot_resync(
    app: AppHandle,
    runtime: Arc<RuntimeService>,
    pipeline: Arc<TtsSpeechPipeline>,
    lag_resync_in_flight: Arc<AtomicBool>,
    lag_resync_pending: Arc<AtomicBool>,
    skipped: u64,
) {
    tokio::spawn(async move {
        let mut skipped = skipped;
        loop {
            // Consume the demand we are about to serve.
            lag_resync_pending.store(false, Ordering::Release);
            warn!(skipped, "runtime event bus lagged; resyncing snapshot");
            let snapshot = runtime.runtime_state_snapshot().await;
            if let Some(running) = snapshot
                .runtime
                .get("running")
                .or_else(|| snapshot.runtime.get("is_running"))
                .and_then(|value| value.as_bool())
            {
                pipeline.set_runtime_active(running);
            }
            for envelope in event_routing::snapshot_to_envelopes(&snapshot) {
                let event_type = envelope
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                emit_runtime_event_immediate(&app, event_type, &envelope);
            }
            if lag_resync_pending.load(Ordering::Acquire) {
                // Coalesce stampeding lags into one follow-up snapshot.
                tokio::time::sleep(Duration::from_millis(LAG_RESYNC_DEBOUNCE_MS)).await;
                skipped = 0;
                continue;
            }
            lag_resync_in_flight.store(false, Ordering::Release);
            // Recover the race where Lagged set pending after our load and before
            // in_flight was cleared (or while CAS on the pump side failed).
            if lag_resync_pending.load(Ordering::Acquire)
                && lag_resync_in_flight
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
            {
                skipped = 0;
                continue;
            }
            break;
        }
    });
}

fn request_lag_resync(
    state: &mut IpcPumpState,
    app: &AppHandle,
    runtime: &Arc<RuntimeService>,
    pipeline: &Arc<TtsSpeechPipeline>,
    skipped: u64,
) {
    // Always remember that a resync is required; never drop the last needed sync.
    state.lag_resync_pending.store(true, Ordering::Release);
    if state
        .lag_resync_in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        warn!(
            skipped,
            "runtime event bus lagged; resync queued (in flight)"
        );
        return;
    }
    state.last_lag_resync = Some(Instant::now());
    spawn_lag_snapshot_resync(
        app.clone(),
        runtime.clone(),
        pipeline.clone(),
        state.lag_resync_in_flight.clone(),
        state.lag_resync_pending.clone(),
        skipped,
    );
}

pub async fn run_runtime_event_ipc_pump(
    app: AppHandle,
    runtime: Arc<RuntimeService>,
    pipeline: Arc<TtsSpeechPipeline>,
) {
    let coalesce = overlay_ipc_coalesce_interval();
    let coalesce_disabled = coalesce.is_zero();
    let mut bus_rx = runtime.runtime_event_bus().subscribe();
    let mut state = IpcPumpState::new();

    loop {
        tokio::select! {
            biased;

            result = bus_rx.recv() => {
                match result {
                    Ok(message) => {
                        let event_type = message
                            .get("type")
                            .and_then(|value| value.as_str())
                            .unwrap_or("");
                        apply_pipeline_side_effects(&pipeline, event_type, message.as_ref());

                        if overlay_ipc_coalesce_event(event_type) {
                            if coalesce_disabled {
                                state.handle_immediate(&app, event_type, message.as_ref());
                            } else {
                                state.queue_overlay(message, runtime.as_ref(), coalesce);
                            }
                        } else if flushes_overlay_ipc_coalesce(event_type) {
                            state.handle_immediate(&app, event_type, message.as_ref());
                        } else {
                            emit_runtime_event_immediate(&app, event_type, message.as_ref());
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        runtime.record_event_bus_consumer_lagged(skipped);
                        // Never keep a coalesced frame across lag: its timer could fire
                        // after snapshot resync and regress the dashboard overlay.
                        state.discard_overlay();
                        request_lag_resync(&mut state, &app, &runtime, &pipeline, skipped);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        state.flush_overlay(&app);
                        break;
                    }
                }
            }

            () = state.overlay_timer.as_mut(), if state.overlay_timer_active => {
                state.flush_overlay(&app);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_update_is_coalesced_for_ipc() {
        assert!(overlay_ipc_coalesce_event("overlay_update"));
        assert!(!overlay_ipc_coalesce_event("runtime_update"));
    }

    #[test]
    fn important_events_flush_overlay_coalesce() {
        assert!(flushes_overlay_ipc_coalesce("runtime_update"));
        assert!(flushes_overlay_ipc_coalesce("translation_update"));
        assert!(!flushes_overlay_ipc_coalesce("overlay_update"));
        assert!(!flushes_overlay_ipc_coalesce("transcript_update"));
    }

    #[test]
    fn zero_coalesce_interval_disables_batching() {
        assert!(Duration::from_millis(0).is_zero());
    }
}
