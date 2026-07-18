use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{Mutex as AsyncMutex, Notify, oneshot};
use tracing::{debug, warn};
use voicesub_audio::{PlaybackFinished, PlaybackHub};

use crate::config::PLAYBACK_MODE_NATIVE;
use crate::google_fetch::prefetch_tts_line;
use crate::playback_policy::{effective_playback_rate, queue_depth_for_playback};
use crate::queue::SpeechQueueItem;
use crate::service::TtsModuleService;

const PREFETCH_AHEAD_MAX: usize = 4;
const SPEAK_STUCK_TIMEOUT: Duration = Duration::from_secs(60);
/// Safety-net wakeup for the pump while it waits for a clip's audio to finish prefetching.
const PREFETCH_WAIT_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Default)]
pub struct CompletionWaiter {
    pending: Mutex<HashMap<(String, String), oneshot::Sender<bool>>>,
}

impl CompletionWaiter {
    pub fn wait(&self, channel: &str, item_id: &str) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("completion waiter lock")
            .insert((channel.to_string(), item_id.to_string()), tx);
        rx
    }

    pub fn notify(&self, finished: &PlaybackFinished) {
        let key = (finished.channel.clone(), finished.item_id.clone());
        if let Some(tx) = self
            .pending
            .lock()
            .expect("completion waiter lock")
            .remove(&key)
        {
            let _ = tx.send(finished.ok);
        }
    }

    pub fn cancel_channel(&self, channel: &str) {
        self.pending
            .lock()
            .expect("completion waiter lock")
            .retain(|(ch, _), _| ch != channel);
    }
}

#[derive(Default)]
struct PrefetchJob {
    audio: Option<Result<Vec<Vec<u8>>, String>>,
    /// A fetch task is currently running for this item (prevents duplicate fetches).
    in_flight: bool,
}

struct ChannelState {
    enabled: bool,
    speaking: bool,
    claim_in_progress: bool,
    current_item_id: Option<String>,
    speak_started_at: Option<Instant>,
    prefetched: HashMap<String, PrefetchJob>,
    prefetch_inflight: usize,
    epoch: u64,
}

impl ChannelState {
    fn new() -> Self {
        Self {
            enabled: true,
            speaking: false,
            claim_in_progress: false,
            current_item_id: None,
            speak_started_at: None,
            prefetched: HashMap::new(),
            prefetch_inflight: 0,
            epoch: 0,
        }
    }

    fn bump_epoch(&mut self) {
        self.epoch += 1;
        self.speaking = false;
        self.claim_in_progress = false;
        self.current_item_id = None;
        self.speak_started_at = None;
    }
}

pub struct ChannelOrchestrator {
    channel: String,
    service: Arc<TtsModuleService>,
    playback: Arc<PlaybackHub>,
    module_dir: PathBuf,
    completion_waiter: Arc<CompletionWaiter>,
    runtime: tokio::runtime::Handle,
    state: AsyncMutex<ChannelState>,
    pump_notify: Notify,
    prefetch_ready: Notify,
}

impl ChannelOrchestrator {
    pub fn new(
        channel: impl Into<String>,
        service: Arc<TtsModuleService>,
        playback: Arc<PlaybackHub>,
        module_dir: PathBuf,
        completion_waiter: Arc<CompletionWaiter>,
        runtime: tokio::runtime::Handle,
    ) -> Arc<Self> {
        Arc::new(Self {
            channel: channel.into(),
            service,
            playback,
            module_dir,
            completion_waiter,
            runtime,
            state: AsyncMutex::new(ChannelState::new()),
            pump_notify: Notify::new(),
            prefetch_ready: Notify::new(),
        })
    }

    pub fn set_enabled(self: &Arc<Self>, enabled: bool) {
        let orchestrator = Arc::clone(self);
        self.runtime.spawn(async move {
            let should_disable = {
                let mut state = orchestrator.state.lock().await;
                if state.enabled == enabled {
                    return;
                }
                state.enabled = enabled;
                if !enabled {
                    state.bump_epoch();
                    state.prefetched.clear();
                    state.prefetch_inflight = 0;
                }
                !enabled
            };
            if should_disable {
                orchestrator
                    .completion_waiter
                    .cancel_channel(&orchestrator.channel);
                let _ = orchestrator
                    .service
                    .queue_clear_channel(&orchestrator.channel);
                let _ = orchestrator.playback.stop_channel(&orchestrator.channel);
            }
            orchestrator.prefetch_ready.notify_one();
            orchestrator.pump_notify.notify_one();
        });
    }

    pub fn enqueue(self: &Arc<Self>, item: SpeechQueueItem) {
        let channel = self.channel.clone();
        let service = self.service.clone();
        let orchestrator = Arc::clone(self);
        self.runtime.spawn(async move {
            match service.enqueue_channel(&channel, item.clone()) {
                Ok(result) => {
                    orchestrator
                        .apply_enqueue_result(item.id.clone(), result.dropped_ids)
                        .await;
                }
                Err(err) => {
                    warn!(
                        target: "voicesub.tts.orchestrator",
                        channel = %channel,
                        error = %err,
                        "enqueue failed"
                    );
                }
            }
            orchestrator.schedule_prefetch().await;
            orchestrator.pump_notify.notify_one();
        });
    }

    /// Resume prefetch/pump after a synchronous `enqueue_channel` (manual sample test IPC).
    pub fn wake_after_enqueue(self: &Arc<Self>, item_id: String, dropped_ids: Vec<String>) {
        let orchestrator = Arc::clone(self);
        self.runtime.spawn(async move {
            orchestrator
                .apply_enqueue_result(item_id, dropped_ids)
                .await;
            orchestrator.schedule_prefetch().await;
            orchestrator.pump_notify.notify_one();
        });
    }

    async fn apply_enqueue_result(self: &Arc<Self>, item_id: String, dropped_ids: Vec<String>) {
        let mut state = self.state.lock().await;
        for dropped_id in dropped_ids {
            state.prefetched.remove(&dropped_id);
        }
        state.prefetched.entry(item_id).or_default();
    }

    pub fn clear(self: &Arc<Self>) {
        let orchestrator = Arc::clone(self);
        self.runtime.spawn(async move {
            {
                let mut state = orchestrator.state.lock().await;
                state.bump_epoch();
                state.prefetched.clear();
                state.prefetch_inflight = 0;
            }
            orchestrator
                .completion_waiter
                .cancel_channel(&orchestrator.channel);
            let _ = orchestrator
                .service
                .queue_clear_channel(&orchestrator.channel);
            let _ = orchestrator.playback.stop_channel(&orchestrator.channel);
            orchestrator.prefetch_ready.notify_one();
            orchestrator.pump_notify.notify_one();
        });
    }

    pub fn spawn_pump_loop(orchestrator: Arc<Self>) {
        let runtime = orchestrator.runtime.clone();
        let pump = orchestrator.clone();
        runtime.spawn(async move {
            loop {
                pump.pump_notify.notified().await;
                pump.pump().await;
            }
        });

        let watchdog_runtime = orchestrator.runtime.clone();
        let watchdog = orchestrator;
        watchdog_runtime.spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(5));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                watchdog.check_stuck().await;
            }
        });
    }

    pub fn notify_pump(self: &Arc<Self>) {
        self.pump_notify.notify_one();
    }

    #[cfg(test)]
    pub async fn test_epoch(&self) -> u64 {
        self.state.lock().await.epoch
    }

    async fn schedule_prefetch(self: &Arc<Self>) {
        let snapshot = match self.service.queue_snapshot(&self.channel) {
            Ok(items) => items,
            Err(_) => return,
        };

        for item in snapshot {
            let should_spawn = {
                let mut state = self.state.lock().await;
                if state.prefetch_inflight >= PREFETCH_AHEAD_MAX {
                    break;
                }
                let job = state.prefetched.entry(item.id.clone()).or_default();
                if job.audio.is_some() || job.in_flight {
                    continue;
                }
                job.in_flight = true;
                state.prefetch_inflight += 1;
                true
            };
            if !should_spawn {
                continue;
            }
            let orchestrator = Arc::clone(self);
            let item_id = item.id.clone();
            let text = item.text.clone();
            let lang = item.lang.clone();
            self.runtime.spawn(async move {
                orchestrator.run_prefetch(item_id, text, lang).await;
            });
        }
    }

    async fn run_prefetch(self: &Arc<Self>, item_id: String, text: String, lang: String) {
        let config = match self.service.load_config() {
            Ok(config) => config,
            Err(err) => {
                warn!(target: "voicesub.tts.orchestrator", error = %err, "prefetch config load failed");
                self.finish_prefetch(item_id, Err(err.to_string())).await;
                return;
            }
        };
        let result = prefetch_tts_line(&self.module_dir, &config.tts_provider, &lang, &text).await;
        self.finish_prefetch(item_id, result).await;
    }

    async fn finish_prefetch(
        self: &Arc<Self>,
        item_id: String,
        result: Result<Vec<Vec<u8>>, String>,
    ) {
        {
            let mut state = self.state.lock().await;
            state.prefetch_inflight = state.prefetch_inflight.saturating_sub(1);
            if let Some(job) = state.prefetched.get_mut(&item_id) {
                job.in_flight = false;
                job.audio = Some(result);
            }
        }
        // Wake both the idle pump (outer loop) and any in-flight wait for this clip.
        self.prefetch_ready.notify_one();
        self.pump_notify.notify_one();
    }

    async fn pump(self: &Arc<Self>) {
        loop {
            let epoch = {
                let state = self.state.lock().await;
                if !state.enabled || state.speaking || state.claim_in_progress {
                    return;
                }
                state.epoch
            };

            let item = match self.service.queue_begin_next(&self.channel) {
                Ok(item) => item,
                Err(err) => {
                    warn!(
                        target: "voicesub.tts.orchestrator",
                        channel = %self.channel,
                        error = %err,
                        "begin_next failed"
                    );
                    return;
                }
            };
            let Some(item) = item else {
                return;
            };

            {
                let mut state = self.state.lock().await;
                state.claim_in_progress = true;
                state.prefetched.entry(item.id.clone()).or_default();
            }

            self.schedule_prefetch().await;

            let audio_result = loop {
                // Create the wait future before checking so a concurrent
                // `finish_prefetch` notification is never lost (notify_one stores a permit).
                let notified = self.prefetch_ready.notified();
                {
                    let mut state = self.state.lock().await;
                    if state.epoch != epoch || !state.enabled {
                        state.claim_in_progress = false;
                        let _ = self.service.queue_mark_finished(&self.channel, &item.id);
                        state.prefetched.remove(&item.id);
                        return;
                    }
                    if let Some(job) = state.prefetched.get(&item.id)
                        && let Some(audio) = &job.audio
                    {
                        break audio.clone();
                    }
                }
                // Wake on the next prefetch completion, with a short timeout as a
                // safety net against epoch/enabled changes that bypass the notifier.
                tokio::select! {
                    _ = notified => {}
                    _ = tokio::time::sleep(PREFETCH_WAIT_TIMEOUT) => {}
                }
            };

            if let Err(message) = audio_result {
                warn!(
                    target: "voicesub.tts.orchestrator",
                    channel = %self.channel,
                    id = %item.id,
                    error = %message,
                    "prefetch failed"
                );
                let _ = self.service.queue_mark_finished(&self.channel, &item.id);
                {
                    let mut state = self.state.lock().await;
                    state.claim_in_progress = false;
                    state.prefetched.remove(&item.id);
                }
                self.schedule_prefetch().await;
                continue;
            }

            let chunks = audio_result.unwrap_or_default();
            if chunks.is_empty() {
                let _ = self.service.queue_mark_finished(&self.channel, &item.id);
                {
                    let mut state = self.state.lock().await;
                    state.claim_in_progress = false;
                    state.prefetched.remove(&item.id);
                }
                continue;
            }

            let (volume, rate) = self.playback_settings();
            {
                let mut state = self.state.lock().await;
                if state.epoch != epoch || !state.enabled {
                    state.claim_in_progress = false;
                    let _ = self.service.queue_mark_finished(&self.channel, &item.id);
                    state.prefetched.remove(&item.id);
                    return;
                }
                state.speaking = true;
                state.current_item_id = Some(item.id.clone());
                state.speak_started_at = Some(Instant::now());
                state.claim_in_progress = false;
            }

            let mut playback_ok = true;
            for (chunk_index, bytes) in chunks.into_iter().enumerate() {
                {
                    let state = self.state.lock().await;
                    if state.epoch != epoch || !state.enabled {
                        playback_ok = false;
                        break;
                    }
                }
                let chunk_item_id = format!("{}#{}", item.id, chunk_index);
                if let Err(err) =
                    self.playback
                        .play(&self.channel, chunk_item_id.clone(), bytes, volume, rate)
                {
                    warn!(
                        target: "voicesub.tts.orchestrator",
                        channel = %self.channel,
                        id = %chunk_item_id,
                        error = %err,
                        "playback play failed"
                    );
                    playback_ok = false;
                    break;
                }
                let wait = self.completion_waiter.wait(&self.channel, &chunk_item_id);
                match wait.await {
                    Ok(true) => {}
                    Ok(false) | Err(_) => {
                        playback_ok = false;
                        break;
                    }
                }
            }

            let _ = self.service.queue_mark_finished(&self.channel, &item.id);
            {
                let mut state = self.state.lock().await;
                state.speaking = false;
                state.current_item_id = None;
                state.speak_started_at = None;
                state.prefetched.remove(&item.id);
                if !playback_ok {
                    debug!(
                        target: "voicesub.tts.orchestrator",
                        channel = %self.channel,
                        id = %item.id,
                        "playback aborted before completion"
                    );
                }
            }
            self.schedule_prefetch().await;
        }
    }

    fn playback_settings(self: &Arc<Self>) -> (f32, f32) {
        let config = self.service.load_config().unwrap_or_default();
        let waiting = self
            .service
            .queue_snapshot(&self.channel)
            .map(|items| items.len())
            .unwrap_or(0);
        let (base_rate, volume) = if self.channel == voicesub_audio::CHANNEL_TWITCH {
            let twitch = &config.twitch;
            (
                twitch.effective_speech_rate(config.speech_rate),
                twitch.effective_speech_volume(config.speech_volume),
            )
        } else {
            (config.speech_rate, config.speech_volume)
        };

        let rate = if config.playback_mode == PLAYBACK_MODE_NATIVE {
            1.0
        } else {
            effective_playback_rate(base_rate, queue_depth_for_playback(waiting), true)
        };
        (volume, rate)
    }

    async fn check_stuck(self: &Arc<Self>) {
        let stuck_id = {
            let state = self.state.lock().await;
            if !state.speaking {
                return;
            }
            let Some(started) = state.speak_started_at else {
                return;
            };
            if started.elapsed() < SPEAK_STUCK_TIMEOUT {
                return;
            }
            state.current_item_id.clone()
        };
        let Some(item_id) = stuck_id else {
            return;
        };
        warn!(
            target: "voicesub.tts.orchestrator",
            channel = %self.channel,
            item_id = %item_id,
            "speaking watchdog forced recovery"
        );
        let _ = self.playback.stop_channel(&self.channel);
        self.completion_waiter.cancel_channel(&self.channel);
        let _ = self.service.queue_force_idle(&self.channel);
        {
            let mut state = self.state.lock().await;
            state.bump_epoch();
            state.prefetched.remove(&item_id);
        }
        self.schedule_prefetch().await;
        self.prefetch_ready.notify_one();
        self.pump_notify.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_runtime::shared_handle;
    use crate::queue::SpeechQueueItem;
    use std::time::Duration;
    use tempfile::tempdir;
    use voicesub_audio::CHANNEL_SPEECH;

    #[tokio::test]
    async fn completion_waiter_notifies_exact_match() {
        let waiter = CompletionWaiter::default();
        let rx = waiter.wait("speech", "abc#0");
        waiter.notify(&PlaybackFinished {
            channel: "speech".into(),
            item_id: "abc#0".into(),
            ok: true,
            error: None,
        });
        assert!(rx.await.unwrap());
    }

    #[tokio::test]
    async fn repeated_disable_is_noop() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime = shared_handle();
        let orch = ChannelOrchestrator::new(
            CHANNEL_SPEECH,
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            Arc::new(CompletionWaiter::default()),
            runtime,
        );

        orch.set_enabled(false);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let epoch_after_first = orch.test_epoch().await;

        orch.set_enabled(false);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let epoch_after_second = orch.test_epoch().await;

        assert_eq!(epoch_after_first, epoch_after_second);
    }

    #[tokio::test]
    async fn disable_clears_queue_once() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service
            .enqueue_channel(
                CHANNEL_SPEECH,
                SpeechQueueItem {
                    id: "line-1".into(),
                    text: "hello".into(),
                    source: "test".into(),
                    lang: "en".into(),
                    dedupe_key: None,
                },
            )
            .expect("enqueue");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime = shared_handle();
        let orch = ChannelOrchestrator::new(
            CHANNEL_SPEECH,
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            Arc::new(CompletionWaiter::default()),
            runtime,
        );

        orch.set_enabled(false);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            service
                .queue_snapshot(CHANNEL_SPEECH)
                .expect("snapshot")
                .is_empty()
        );

        orch.set_enabled(false);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            service
                .queue_snapshot(CHANNEL_SPEECH)
                .expect("snapshot")
                .is_empty()
        );
    }
}
