use std::path::PathBuf;
use std::sync::{Arc, Mutex};



use serde_json::{json, Value};

use tracing::{debug, info, warn};

use voicesub_audio::{list_output_devices, set_process_output_device, AudioOutputDevice};
use voicesub_twitch::{
    EventBroadcaster, SourceTextReplacementSettings, TwitchChatService, TwitchConnectionStatus,
    TwitchTtsSettings,
};



use crate::async_runtime::shared_handle;
use crate::config::{
    normalize_playback_mode, normalize_tts_provider, TtsConfig, TtsConfigStore,
    PLAYBACK_MODE_NATIVE,
};
use crate::oauth_bridge::TwitchOAuthBridge;

use crate::channel_queue::{DualChannelSpeechQueue, CHANNEL_SPEECH, CHANNEL_TWITCH};
use crate::queue::{SpeechQueueItem};

use crate::subtitle_speech::{SubtitleSpeechPlanner, TtsSpeechSettings};

use crate::trace;



#[derive(Debug, thiserror::Error)]

pub enum TtsServiceError {

    #[error("config error: {0}")]

    Config(#[from] crate::config::TtsConfigError),



    #[error("audio error: {0}")]

    Audio(#[from] voicesub_audio::AudioError),

    #[error("invalid tts provider: {0}")]
    InvalidProvider(String),

    #[error("twitch error: {0}")]
    Twitch(#[from] voicesub_twitch::TwitchError),

}



pub struct TtsModuleService {

    config_store: TtsConfigStore,

    queues: DualChannelSpeechQueue,

    bound_pid: Mutex<Option<u32>>,

    subtitle_planner: Mutex<SubtitleSpeechPlanner>,

    twitch: TwitchChatService,

    oauth_bridge: Arc<TwitchOAuthBridge>,

}



impl TtsModuleService {

    pub fn new(user_data_dir: impl Into<PathBuf>) -> Self {
        Self::with_broadcaster(
            user_data_dir,
            Arc::new(|_| {}),
            Arc::new(TwitchOAuthBridge::default()),
            shared_handle(),
        )
    }

    pub fn with_broadcaster(
        user_data_dir: impl Into<PathBuf>,
        broadcaster: EventBroadcaster,
        oauth_bridge: Arc<TwitchOAuthBridge>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        let module_dir = user_data_dir.into().join("modules").join("tts");

        info!(
            target: "voicesub.tts",
            path = %module_dir.display(),
            "tts module service initialized"
        );

        Self {
            config_store: TtsConfigStore::new(module_dir),
            queues: DualChannelSpeechQueue::new(),
            bound_pid: Mutex::new(None),
            subtitle_planner: Mutex::new(SubtitleSpeechPlanner::new()),
            twitch: TwitchChatService::new(broadcaster, runtime),
            oauth_bridge,
        }
    }

    pub fn oauth_bridge(&self) -> Arc<TwitchOAuthBridge> {
        self.oauth_bridge.clone()
    }

    pub fn store_pending_oauth_token(&self, token: &str) {
        self.oauth_bridge.store(token.to_string());
    }

    pub fn take_pending_oauth_token(&self) -> Option<String> {
        self.oauth_bridge.take()
    }

    pub fn config_path(&self) -> &std::path::Path {

        self.config_store.path()

    }



    pub fn load_config(&self) -> Result<TtsConfig, TtsServiceError> {

        let config = self.config_store.load()?;

        debug!(

            target: "voicesub.tts",

            enabled = config.enabled,

            device_id = if config.audio_output_device_id.is_empty() {

                "default"

            } else {

                config.audio_output_device_id.as_str()

            },

            "config loaded"

        );

        Ok(config)

    }



    pub fn save_config(&self, config: &TtsConfig) -> Result<(), TtsServiceError> {

        Ok(self.config_store.save(config)?)

    }



    pub fn list_output_devices(&self) -> Result<Vec<AudioOutputDevice>, TtsServiceError> {

        debug!(target: "voicesub.tts", "list_output_devices requested");

        Ok(list_output_devices()?)

    }



    pub fn set_tts_provider(&self, provider: &str) -> Result<TtsConfig, TtsServiceError> {
        let provider = normalize_tts_provider(provider)
            .ok_or_else(|| TtsServiceError::InvalidProvider(provider.to_string()))?;
        info!(target: "voicesub.tts", provider = %provider, "tts provider updated");
        trace::trace("service", "set_tts_provider", json!({ "provider": provider }));
        Ok(self.config_store.update(|cfg| cfg.tts_provider = provider)?)
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<TtsConfig, TtsServiceError> {

        info!(target: "voicesub.tts", enabled, "tts enabled flag updated");

        trace::trace("service", "set_enabled", json!({ "enabled": enabled }));

        Ok(self.config_store.update(|cfg| cfg.enabled = enabled)?)

    }



    pub fn update_speech_settings(

        &self,

        settings: TtsSpeechSettings,

    ) -> Result<TtsConfig, TtsServiceError> {

        info!(

            target: "voicesub.tts",

            speak_source = settings.speak_source,

            speak_translations = settings.speak_translations,

            min_chars = settings.min_chars,

            max_queue_items = settings.max_queue_items,

            "speech settings updated"

        );

        trace::trace("service", "update_speech_settings", json!({

            "speak_source": settings.speak_source,

            "speak_translations": settings.speak_translations,

            "translation_slots": settings.translation_slots,

            "min_chars": settings.min_chars,

            "max_queue_items": settings.max_queue_items,

        }));

        Ok(self.config_store.update(|cfg| cfg.speech = settings)?)

    }



    pub fn update_voice_settings(
        &self,
        rate: f32,
        volume: f32,
    ) -> Result<TtsConfig, TtsServiceError> {
        info!(
            target: "voicesub.tts",
            speech_rate = rate,
            speech_volume = volume,
            "voice settings updated"
        );
        trace::trace("service", "update_voice_settings", json!({
            "speech_rate": rate,
            "speech_volume": volume,
        }));
        Ok(self.config_store.update(|cfg| {
            cfg.speech_rate = rate;
            cfg.speech_volume = volume;
        })?)
    }



    pub fn plan_subtitle_speech(&self, payload: &Value) -> Vec<SpeechQueueItem> {

        let config = self.load_config().unwrap_or_default();

        let sequence = payload

            .get("sequence")

            .and_then(|v| v.as_u64())

            .unwrap_or(0);

        let lifecycle = payload

            .get("lifecycle_state")

            .and_then(|v| v.as_str())

            .unwrap_or("idle");



        if !config.enabled {

            debug!(

                target: "voicesub.tts",

                sequence,

                lifecycle,

                "subtitle speech skipped: tts disabled"

            );

            return Vec::new();

        }



        let mut planner = self.subtitle_planner.lock().expect("tts subtitle planner lock");

        let planned = planner.plan(payload, &config.speech);



        if planned.is_empty() {

            trace::trace(

                "planner",

                "plan_empty",

                json!({

                    "sequence": sequence,

                    "lifecycle": lifecycle,

                }),

            );

        } else {

            info!(

                target: "voicesub.tts",

                sequence,

                count = planned.len(),

                "subtitle speech planned"

            );

            trace::trace(

                "planner",

                "plan_ready",

                json!({

                    "sequence": sequence,

                    "lifecycle": lifecycle,

                    "count": planned.len(),

                    "items": planned.iter().map(|item| json!({

                        "id": item.id,

                        "source": item.source,

                        "text": trace::text_fields(&item.text),

                    })).collect::<Vec<_>>(),

                }),

            );

        }



        planned

    }



    pub fn reset_subtitle_planner(&self) {

        info!(target: "voicesub.tts", "subtitle speech planner reset");

        trace::trace("planner", "reset", json!({}));

        self.subtitle_planner

            .lock()

            .expect("tts subtitle planner lock")

            .reset();

    }



    pub fn set_audio_device(
        &self,
        device_id: &str,
        device_label: Option<&str>,
    ) -> Result<TtsConfig, TtsServiceError> {
        info!(
            target: "voicesub.tts",
            device_id = if device_id.is_empty() { "default" } else { device_id },
            device_label = device_label.unwrap_or(""),
            "audio output device updated"
        );
        trace::trace(
            "service",
            "set_audio_device",
            json!({
                "device_id": if device_id.is_empty() { "default" } else { device_id },
                "device_label": device_label.unwrap_or(""),
                "routing": if voicesub_audio::is_per_process_routing_enabled() {
                    "winapi"
                } else {
                    "browser"
                },
            }),
        );

        let config = self.config_store.update(|cfg| {
            cfg.audio_output_device_id = device_id.to_string();
            match device_label {
                Some(label) => cfg.audio_output_device_label = label.to_string(),
                None if device_id.is_empty() => cfg.audio_output_device_label.clear(),
                None => {}
            }
        })?;

        if voicesub_audio::is_per_process_routing_enabled() {
            if let Some(pid) = *self.bound_pid.lock().expect("tts pid lock") {
                self.apply_audio_route(pid, &config.audio_output_device_id)?;
            }
        }

        Ok(config)
    }

    pub fn set_channel_audio_device(
        &self,
        channel: &str,
        device_id: &str,
        device_label: Option<&str>,
    ) -> Result<TtsConfig, TtsServiceError> {
        match channel {
            CHANNEL_SPEECH => self.set_audio_device(device_id, device_label),
            CHANNEL_TWITCH => {
                info!(
                    target: "voicesub.tts",
                    device_id = if device_id.is_empty() { "default" } else { device_id },
                    device_label = device_label.unwrap_or(""),
                    "twitch audio output device updated"
                );
                trace::trace(
                    "service",
                    "set_twitch_audio_device",
                    json!({
                        "device_id": if device_id.is_empty() { "default" } else { device_id },
                        "device_label": device_label.unwrap_or(""),
                    }),
                );
                Ok(self.config_store.update(|cfg| {
                    cfg.twitch.audio_output_device_id = device_id.to_string();
                    match device_label {
                        Some(label) => cfg.twitch.audio_output_device_label = label.to_string(),
                        None if device_id.is_empty() => cfg.twitch.audio_output_device_label.clear(),
                        None => {}
                    }
                })?)
            }
            other => Err(TtsServiceError::InvalidProvider(format!(
                "unknown audio channel: {other}"
            ))),
        }
    }

    pub fn set_playback_mode(&self, mode: &str) -> Result<TtsConfig, TtsServiceError> {
        let mode = normalize_playback_mode(mode)
            .ok_or_else(|| TtsServiceError::InvalidProvider(mode.to_string()))?;
        info!(target: "voicesub.tts", playback_mode = %mode, "playback mode updated");
        trace::trace("service", "set_playback_mode", json!({ "mode": mode }));
        Ok(self.config_store.update(|cfg| cfg.playback_mode = mode)?)
    }

    pub fn playback_mode_is_native(config: &TtsConfig) -> bool {
        config.playback_mode == PLAYBACK_MODE_NATIVE
    }

    pub fn device_label_for_channel(config: &TtsConfig, channel: &str) -> String {
        let (device_id, label) = match channel {
            CHANNEL_TWITCH => (
                config.twitch.audio_output_device_id.as_str(),
                config.twitch.audio_output_device_label.as_str(),
            ),
            _ => (
                config.audio_output_device_id.as_str(),
                config.audio_output_device_label.as_str(),
            ),
        };
        if !label.trim().is_empty() {
            return label.to_string();
        }
        if device_id.trim().is_empty() {
            return String::new();
        }
        if let Ok(devices) = list_output_devices() {
            if let Some(device) = devices.iter().find(|entry| entry.id == device_id) {
                return device.label.clone();
            }
        }
        String::new()
    }

    pub fn bound_process_id(&self) -> Option<u32> {
        *self.bound_pid.lock().expect("tts pid lock")
    }

    pub fn register_window_process(&self, pid: u32) -> Result<(), TtsServiceError> {
        {
            let mut guard = self.bound_pid.lock().expect("tts pid lock");
            *guard = Some(pid);
        }
        info!(target: "voicesub.tts", pid, "tts window process registered");
        trace::trace("service", "register_window_process", json!({ "pid": pid }));
        Ok(())
    }

    pub fn bind_window_process(&self, pid: u32) -> Result<TtsConfig, TtsServiceError> {
        self.register_window_process(pid)?;
        let config = self.load_config()?;
        if !config.audio_output_device_id.is_empty() {
            self.apply_audio_route(pid, &config.audio_output_device_id)?;
        } else {
            debug!(
                target: "voicesub.tts",
                pid,
                "audio routing deferred: default device selected"
            );
        }
        trace::trace(
            "service",
            "bind_window_process",
            json!({
                "pid": pid,
                "device_id": if config.audio_output_device_id.is_empty() {
                    "default"
                } else {
                    config.audio_output_device_id.as_str()
                },
            }),
        );
        Ok(config)
    }



    pub fn apply_audio_route(&self, pid: u32, device_id: &str) -> Result<(), TtsServiceError> {

        debug!(

            target: "voicesub.tts",

            pid,

            device_id = if device_id.is_empty() { "default" } else { device_id },

            "apply audio route"

        );

        set_process_output_device(pid, device_id)?;

        Ok(())

    }



    fn max_queue_items_for_channel(&self, channel: &str) -> u32 {
        let Ok(cfg) = self.config_store.load() else {
            return 8;
        };
        match channel {
            CHANNEL_TWITCH => cfg.twitch.effective_max_queue_items(),
            _ => cfg.speech.max_queue_items.max(1),
        }
    }

    pub fn enqueue_channel(
        &self,
        channel: &str,
        item: SpeechQueueItem,
    ) -> Result<usize, TtsServiceError> {
        let max_items = self.max_queue_items_for_channel(channel);
        let len = self
            .queues
            .enqueue(channel, item.clone(), max_items)
            .map_err(|e| TtsServiceError::InvalidProvider(e.to_string()))?;

        info!(
            target: "voicesub.tts",
            channel,
            id = %item.id,
            queue_len = len,
            source = %item.source,
            "channel speech item enqueued"
        );

        trace::trace(
            "queue",
            "enqueue",
            json!({
                "channel": channel,
                "id": item.id,
                "source": item.source,
                "queue_len": len,
                "text": trace::text_fields(&item.text),
            }),
        );

        Ok(len)
    }

    /// Deprecated: use [`Self::enqueue_channel`] with `speech`.
    pub fn enqueue_speech(&self, item: SpeechQueueItem) -> usize {
        self.enqueue_channel(CHANNEL_SPEECH, item)
            .unwrap_or(0)
    }

    pub fn queue_begin_next(&self, channel: &str) -> Result<Option<SpeechQueueItem>, TtsServiceError> {
        let next = self
            .queues
            .begin_next(channel)
            .map_err(|e| TtsServiceError::InvalidProvider(e.to_string()))?;

        if let Some(item) = &next {
            debug!(target: "voicesub.tts", channel, id = %item.id, "queue begin next");

            trace::trace(
                "queue",
                "begin_next",
                json!({
                    "channel": channel,
                    "id": item.id,

                    "text": trace::text_fields(&item.text),

                }),

            );

        }

        Ok(next)
    }

    pub fn queue_mark_finished(
        &self,
        channel: &str,
        item_id: &str,
    ) -> Result<(), TtsServiceError> {
        debug!(target: "voicesub.tts", channel, item_id, "queue mark finished");
        trace::trace("queue", "mark_finished", json!({ "channel": channel, "id": item_id }));
        self.queues
            .mark_finished(channel, item_id)
            .map_err(|e| TtsServiceError::InvalidProvider(e.to_string()))
    }

    pub fn queue_clear_channel(&self, channel: &str) -> Result<(), TtsServiceError> {
        info!(target: "voicesub.tts", channel, "channel queue cleared");
        trace::trace("queue", "clear", json!({ "channel": channel }));
        self.queues
            .clear(channel)
            .map_err(|e| TtsServiceError::InvalidProvider(e.to_string()))
    }

    pub fn queue_clear_all(&self) {
        info!(target: "voicesub.tts", "all channel queues cleared");
        trace::trace("queue", "clear_all", json!({}));
        self.queues.clear_all();
    }

    pub fn queue_snapshot(
        &self,
        channel: &str,
    ) -> Result<Vec<SpeechQueueItem>, TtsServiceError> {
        self.queues
            .snapshot(channel)
            .map_err(|e| TtsServiceError::InvalidProvider(e.to_string()))
    }

    /// Deprecated: clears speech channel only.
    pub fn queue_clear(&self) {
        let _ = self.queue_clear_channel(CHANNEL_SPEECH);
    }



    pub fn validate_device_id(&self, device_id: &str) -> bool {

        if device_id.is_empty() {

            return true;

        }

        if !voicesub_audio::is_per_process_routing_enabled() {
            return true;
        }

        match list_output_devices() {

            Ok(devices) => {

                let valid = devices.iter().any(|d| !d.id.is_empty() && d.id == device_id);

                if !valid {

                    warn!(target: "voicesub.tts", device_id, "device validation failed");

                }

                valid

            }

            Err(err) => {

                warn!(target: "voicesub.tts", error = %err, "device validation skipped");

                true

            }

        }

    }

    pub fn twitch_status(&self) -> TwitchConnectionStatus {
        self.twitch.status()
    }

    pub fn twitch_disconnect(&self) {
        info!(target: "voicesub.tts", "twitch disconnect via service");
        trace::trace("twitch", "disconnect", json!({}));
        self.twitch.disconnect();
    }

    pub fn twitch_connect(&self) -> Result<TwitchConnectionStatus, TtsServiceError> {
        let settings = self.load_config()?.twitch;
        trace::trace(
            "twitch",
            "connect",
            json!({
                "channel": settings.normalized_channel(),
                "enabled": settings.enabled,
                "lang": settings.language,
            }),
        );
        let status = self.twitch.connect(settings)?;
        info!(
            target: "voicesub.tts",
            state = ?status.state,
            channel = %status.channel,
            "twitch connect requested"
        );
        Ok(status)
    }

    pub fn update_twitch_settings(
        &self,
        twitch: TwitchTtsSettings,
    ) -> Result<TtsConfig, TtsServiceError> {
        info!(
            target: "voicesub.tts",
            channel = %twitch.normalized_channel(),
            enabled = twitch.enabled,
            "twitch settings updated"
        );
        trace::trace(
            "twitch",
            "settings_updated",
            json!({
                "channel": twitch.normalized_channel(),
                "enabled": twitch.enabled,
                "nick": twitch.nick.trim(),
                "lang": twitch.language,
                "min_chars": twitch.min_chars,
                "block_commands": twitch.block_commands,
                "ignore_users": twitch.ignore_users.len(),
            }),
        );
        let config = self.config_store.update(|cfg| cfg.twitch = twitch)?;
        self.twitch.apply_settings(config.twitch.clone());
        if !config.twitch.enabled {
            self.twitch.disconnect();
        }
        Ok(config)
    }

    pub fn sync_source_text_replacement(
        &self,
        settings: SourceTextReplacementSettings,
    ) -> Result<(), TtsServiceError> {
        trace::trace(
            "twitch",
            "source_replacement_sync",
            serde_json::json!({
                "enabled": settings.enabled,
                "include_builtin": settings.include_builtin,
                "pairs": settings.pairs.len(),
            }),
        );
        self.twitch.apply_source_text_replacement(settings);
        Ok(())
    }

    pub fn shutdown(&self) {
        info!(target: "voicesub.tts", "tts module shutdown");
        trace::trace("service", "shutdown", json!({}));
        self.queues.clear_all();
        self.twitch.disconnect();
    }

}


