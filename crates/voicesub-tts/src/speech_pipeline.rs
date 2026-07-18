use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tracing::{debug, info};
use voicesub_audio::{CHANNEL_SPEECH, CHANNEL_TWITCH, PlaybackFinished, PlaybackHub};
use voicesub_twitch::TwitchChatMessage;

use crate::channel_orchestrator::{ChannelOrchestrator, CompletionWaiter};
use crate::ipc::speech_queue_item_id;
use crate::queue::{ChannelEnqueueResult, SpeechQueueItem};
use crate::service::{TtsModuleService, TtsServiceError};

pub type SpeechPlannedListener = Arc<dyn Fn(&[SpeechQueueItem]) + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum SpeechPipelineError {
    #[error("TTS module window is not open")]
    WindowClosed,
    #[error("TTS module is disabled")]
    Disabled,
    #[error("empty sample text")]
    EmptyText,
    #[error("{0}")]
    Service(#[from] TtsServiceError),
}

pub struct TtsSpeechPipeline {
    service: Arc<TtsModuleService>,
    speech: Arc<ChannelOrchestrator>,
    twitch: Arc<ChannelOrchestrator>,
    runtime_active: Arc<AtomicBool>,
    /// TTS orchestrators run only while the module WebView window is open.
    window_open: Arc<AtomicBool>,
    completion_waiter: Arc<CompletionWaiter>,
    on_speech_planned: Mutex<Option<SpeechPlannedListener>>,
}

impl TtsSpeechPipeline {
    pub fn new(
        service: Arc<TtsModuleService>,
        playback: Arc<PlaybackHub>,
        runtime_module_dir: PathBuf,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        // Shipped fetcher lives under bin/modules/tts (config stays in user-data).
        let module_dir = runtime_module_dir;
        let completion_waiter = Arc::new(CompletionWaiter::default());
        let speech = ChannelOrchestrator::new(
            CHANNEL_SPEECH,
            service.clone(),
            playback.clone(),
            module_dir.clone(),
            completion_waiter.clone(),
            runtime.clone(),
        );
        let twitch = ChannelOrchestrator::new(
            CHANNEL_TWITCH,
            service.clone(),
            playback,
            module_dir,
            completion_waiter.clone(),
            runtime,
        );
        Self {
            service,
            speech,
            twitch,
            runtime_active: Arc::new(AtomicBool::new(false)),
            window_open: Arc::new(AtomicBool::new(false)),
            completion_waiter,
            on_speech_planned: Mutex::new(None),
        }
    }

    fn effective_enabled(&self) -> bool {
        if !self.window_open.load(Ordering::Relaxed) {
            return false;
        }
        self.service
            .load_config()
            .map(|cfg| cfg.enabled)
            .unwrap_or(false)
    }

    fn apply_effective_enabled(&self) {
        let enabled = self.effective_enabled();
        self.speech.set_enabled(enabled);
        self.twitch.set_enabled(enabled);
    }

    /// Suspend or resume Rust-side speech when the TTS module window closes or opens.
    pub fn set_window_open(&self, open: bool) {
        let was_open = self.window_open.swap(open, Ordering::Relaxed);
        if was_open == open {
            return;
        }
        if open {
            info!(target: "voicesub.tts.pipeline", "tts module window opened — resuming pipeline");
        } else {
            info!(
                target: "voicesub.tts.pipeline",
                "tts module window closed — suspending pipeline"
            );
            self.service.reset_subtitle_planner();
            self.service.twitch_disconnect();
        }
        self.apply_effective_enabled();
    }

    pub fn set_speech_planned_listener(&self, listener: SpeechPlannedListener) {
        *self
            .on_speech_planned
            .lock()
            .expect("tts speech planned listener lock") = Some(listener);
    }

    fn notify_speech_planned(&self, planned: &[SpeechQueueItem]) {
        if planned.is_empty() {
            return;
        }
        if let Some(listener) = self
            .on_speech_planned
            .lock()
            .expect("tts speech planned listener lock")
            .as_ref()
        {
            listener(planned);
        }
    }

    pub fn start(self: Arc<Self>) {
        ChannelOrchestrator::spawn_pump_loop(self.speech.clone());
        ChannelOrchestrator::spawn_pump_loop(self.twitch.clone());
    }

    pub fn on_playback_finished(&self, finished: &PlaybackFinished) {
        self.completion_waiter.notify(finished);
        if finished.channel == CHANNEL_SPEECH {
            self.speech.notify_pump();
        } else if finished.channel == CHANNEL_TWITCH {
            self.twitch.notify_pump();
        }
    }

    pub fn handle_subtitle_payload(&self, payload: Value) {
        if !self.runtime_active.load(Ordering::Relaxed) {
            return;
        }
        if !self.window_open.load(Ordering::Relaxed) {
            return;
        }
        let config = match self.service.load_config() {
            Ok(config) => config,
            Err(_) => return,
        };
        if !config.enabled {
            return;
        }

        let planned = self.service.plan_subtitle_speech(&payload);
        if planned.is_empty() {
            return;
        }
        debug!(
            target: "voicesub.tts.pipeline",
            count = planned.len(),
            sequence = payload.get("sequence").and_then(|v| v.as_u64()).unwrap_or(0),
            "subtitle speech planned in rust"
        );
        self.notify_speech_planned(&planned);
        for item in planned {
            self.speech.enqueue(item);
        }
    }

    pub fn handle_twitch_chat_message(&self, message: &TwitchChatMessage) {
        if !self.window_open.load(Ordering::Relaxed) {
            return;
        }
        let config = match self.service.load_config() {
            Ok(config) => config,
            Err(_) => return,
        };
        if !config.enabled || !config.twitch.enabled || !message.speakable {
            return;
        }
        let id = format!("twitch-{}", message.id);
        self.twitch.enqueue(SpeechQueueItem {
            id,
            text: message.speak_text.clone(),
            source: CHANNEL_TWITCH.to_string(),
            lang: message.language.clone(),
            dedupe_key: None,
        });
    }

    pub fn set_runtime_active(&self, active: bool) {
        let was_active = self.runtime_active.swap(active, Ordering::Relaxed);
        if was_active && !active {
            info!(target: "voicesub.tts.pipeline", "runtime stopped — clearing speech pipeline");
            self.service.reset_subtitle_planner();
            self.speech.clear();
            self.twitch.clear();
        }
        if was_active == active {
            return;
        }
        self.apply_effective_enabled();
    }

    pub fn sync_enabled_from_config(&self) {
        self.apply_effective_enabled();
    }

    /// Manual Speak test from the TTS UI — does not require translation runtime.
    pub fn enqueue_speech_test(
        &self,
        text: String,
        lang: String,
    ) -> Result<ChannelEnqueueResult, SpeechPipelineError> {
        if !self.window_open.load(Ordering::Relaxed) {
            return Err(SpeechPipelineError::WindowClosed);
        }
        let config = self.service.load_config()?;
        if !config.enabled {
            return Err(SpeechPipelineError::Disabled);
        }
        let text = text.trim().to_string();
        if text.is_empty() {
            return Err(SpeechPipelineError::EmptyText);
        }
        let lang = lang.trim();
        let lang = if lang.is_empty() {
            "en".to_string()
        } else {
            lang.to_string()
        };
        let item = SpeechQueueItem {
            id: speech_queue_item_id(),
            text,
            source: "test".to_string(),
            lang: lang.clone(),
            dedupe_key: None,
        };
        let result = self.service.enqueue_channel(CHANNEL_SPEECH, item.clone())?;
        self.speech
            .wake_after_enqueue(item.id.clone(), result.dropped_ids.clone());
        debug!(
            target: "voicesub.tts.pipeline",
            id = %item.id,
            lang = %lang,
            queue_len = result.queue_len,
            "speech sample test enqueued"
        );
        self.notify_speech_planned(&[item]);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;
    use voicesub_twitch::{TwitchChatMessage, TwitchTtsSettings};

    #[test]
    fn pipeline_skips_subtitle_when_runtime_inactive() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );
        pipeline.handle_subtitle_payload(json!({
            "sequence": 1,
            "lifecycle_state": "completed_only",
            "visible_items": [{"text": "hello", "kind": "source"}]
        }));
        assert!(service.queue_snapshot(CHANNEL_SPEECH).unwrap().is_empty());
    }

    #[tokio::test]
    async fn runtime_heartbeat_does_not_reapply_enabled() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service.set_enabled(false).expect("disable");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service,
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );

        pipeline.set_runtime_active(true);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let epoch_after_first = pipeline.speech.test_epoch().await;

        pipeline.set_runtime_active(true);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let epoch_after_heartbeat = pipeline.speech.test_epoch().await;

        assert_eq!(epoch_after_first, epoch_after_heartbeat);
    }

    #[tokio::test]
    async fn pipeline_skips_subtitle_when_window_closed() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service.set_enabled(true).expect("enable");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );
        pipeline.set_runtime_active(true);
        pipeline.handle_subtitle_payload(json!({
            "sequence": 1,
            "lifecycle_state": "completed_only",
            "visible_items": [{"text": "hello", "kind": "source"}]
        }));
        assert!(service.queue_snapshot(CHANNEL_SPEECH).unwrap().is_empty());
    }

    #[tokio::test]
    async fn pipeline_plans_subtitle_when_window_open() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service.set_enabled(true).expect("enable");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );
        pipeline.set_window_open(true);
        pipeline.set_runtime_active(true);
        pipeline.handle_subtitle_payload(json!({
            "sequence": 1,
            "lifecycle_state": "completed_only",
            "visible_items": [{"text": "hello", "kind": "source"}]
        }));
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(
            service
                .queue_snapshot(CHANNEL_SPEECH)
                .expect("snapshot")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn twitch_chat_enqueued_without_translation_runtime() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service.set_enabled(true).expect("enable");
        service
            .update_twitch_settings(TwitchTtsSettings {
                enabled: true,
                ..Default::default()
            })
            .expect("twitch settings");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );
        pipeline.set_window_open(true);
        pipeline.sync_enabled_from_config();
        pipeline.handle_twitch_chat_message(&TwitchChatMessage {
            id: "msg-1".into(),
            user: "viewer".into(),
            display_name: "viewer".into(),
            text: "hello chat".into(),
            speak_text: "hello chat".into(),
            clean_text: "hello chat".into(),
            spoken_nick: "viewer".into(),
            channel: "#evikey".into(),
            language: "en".into(),
            is_mod: false,
            is_subscriber: false,
            speakable: true,
        });
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(
            service
                .queue_snapshot(CHANNEL_TWITCH)
                .expect("snapshot")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn speech_test_enqueued_when_window_open_without_runtime() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service.set_enabled(true).expect("enable");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service.clone(),
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );
        pipeline.set_window_open(true);
        pipeline.sync_enabled_from_config();
        let result = pipeline
            .enqueue_speech_test("hello sample".into(), "en".into())
            .expect("enqueue sample");
        assert_eq!(result.queue_len, 1);
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let snapshot = service.queue_snapshot(CHANNEL_SPEECH).expect("snapshot");
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].source, "test");
    }

    #[test]
    fn speech_test_rejected_when_window_closed() {
        let dir = tempdir().unwrap();
        let service = Arc::new(TtsModuleService::new(dir.path()));
        service.set_enabled(true).expect("enable");
        let (playback, _rx) = PlaybackHub::spawn();
        let runtime_module_dir = dir.path().join("runtime").join("win-x64");
        std::fs::create_dir_all(&runtime_module_dir).unwrap();
        let pipeline = TtsSpeechPipeline::new(
            service,
            Arc::new(playback),
            dir.path().to_path_buf(),
            crate::async_runtime::shared_handle(),
        );
        let err = pipeline
            .enqueue_speech_test("hello".into(), "en".into())
            .expect_err("window closed");
        assert!(matches!(err, SpeechPipelineError::WindowClosed));
    }
}
