use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tracing::{debug, info};
use voicesub_audio::{CHANNEL_SPEECH, CHANNEL_TWITCH, PlaybackFinished, PlaybackHub};
use voicesub_twitch::TwitchChatMessage;

use crate::channel_orchestrator::{ChannelOrchestrator, CompletionWaiter};
use crate::queue::SpeechQueueItem;
use crate::service::TtsModuleService;

pub type SpeechPlannedListener = Arc<dyn Fn(&[SpeechQueueItem]) + Send + Sync>;

pub struct TtsSpeechPipeline {
    service: Arc<TtsModuleService>,
    speech: Arc<ChannelOrchestrator>,
    twitch: Arc<ChannelOrchestrator>,
    runtime_active: Arc<AtomicBool>,
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
            playback.clone(),
            module_dir,
            completion_waiter.clone(),
            runtime,
        );
        Self {
            service,
            speech,
            twitch,
            runtime_active: Arc::new(AtomicBool::new(false)),
            completion_waiter,
            on_speech_planned: Mutex::new(None),
        }
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
        let enabled = self
            .service
            .load_config()
            .map(|cfg| cfg.enabled)
            .unwrap_or(false);
        self.speech.set_enabled(enabled);
        self.twitch.set_enabled(enabled);
    }

    pub fn sync_enabled_from_config(&self) {
        let enabled = self
            .service
            .load_config()
            .map(|cfg| cfg.enabled)
            .unwrap_or(false);
        self.speech.set_enabled(enabled);
        self.twitch.set_enabled(enabled);
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
}
