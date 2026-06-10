use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::emotes::EmoteRegistry;
use crate::error::TwitchError;
use crate::irc::{run_session, MessageCallback, StatusCallback};
use crate::settings::{TwitchChatMessage, TwitchTtsSettings};
use crate::source_text_replacement::{
    profanity_settings_for_twitch, SourceTextReplacementSettings,
};
use crate::trace;

#[derive(Debug, Clone, Default)]
pub struct TwitchLiveState {
    pub chat: TwitchTtsSettings,
    pub source_replacement: SourceTextReplacementSettings,
}

pub type EventBroadcaster = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwitchConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitchConnectionStatus {
    pub state: TwitchConnectionState,
    pub channel: String,
    pub message: String,
}

impl Default for TwitchConnectionStatus {
    fn default() -> Self {
        Self {
            state: TwitchConnectionState::Disconnected,
            channel: String::new(),
            message: String::new(),
        }
    }
}

struct ActiveSession {
    stop_tx: watch::Sender<bool>,
    task: JoinHandle<()>,
}

struct Inner {
    broadcaster: EventBroadcaster,
    status: Mutex<TwitchConnectionStatus>,
    active: Mutex<Option<ActiveSession>>,
}

pub struct TwitchChatService {
    inner: Arc<Inner>,
    runtime: tokio::runtime::Handle,
    emotes: Arc<EmoteRegistry>,
    /// Live message-processing state; updated on connect and when UI saves.
    live: Arc<RwLock<TwitchLiveState>>,
}

impl TwitchChatService {
    pub fn new(broadcaster: EventBroadcaster, runtime: tokio::runtime::Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                broadcaster,
                status: Mutex::new(TwitchConnectionStatus::default()),
                active: Mutex::new(None),
            }),
            runtime,
            emotes: EmoteRegistry::new(),
            live: Arc::new(RwLock::new(TwitchLiveState::default())),
        }
    }

    /// Apply Twitch tab settings to the active IRC session without reconnecting.
    pub fn apply_settings(&self, settings: TwitchTtsSettings) {
        if let Ok(mut guard) = self.live.write() {
            trace::trace(
                "service",
                "settings_applied",
                json!({
                    "include_username": settings.include_username,
                    "strip_emotes": settings.strip_emotes,
                    "block_commands": settings.block_commands,
                }),
            );
            guard.chat = settings.clone();
            guard.source_replacement = profanity_settings_for_twitch(&settings);
        }
    }

    /// Legacy IPC hook; profanity for Twitch chat lives in `TwitchTtsSettings.include_builtin_profanity`.
    pub fn apply_source_text_replacement(&self, settings: SourceTextReplacementSettings) {
        if let Ok(mut guard) = self.live.write() {
            trace::trace(
                "service",
                "source_replacement_applied",
                json!({
                    "enabled": settings.enabled,
                    "include_builtin": settings.include_builtin,
                    "pairs": settings.pairs.len(),
                }),
            );
            guard.source_replacement = settings;
        }
    }

    pub fn emote_registry(&self) -> Arc<EmoteRegistry> {
        self.emotes.clone()
    }

    pub fn status(&self) -> TwitchConnectionStatus {
        self.inner
            .status
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn disconnect(&self) {
        let session = self
            .inner
            .active
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(session) = session {
            let _ = session.stop_tx.send(true);
            session.task.abort();
            info!(target: "voicesub.twitch", "twitch irc disconnect requested");
            trace::trace("service", "disconnect", json!({}));
        }
        self.set_status(TwitchConnectionState::Disconnected, "", "");
        self.broadcast_connection();
    }

    pub fn connect(&self, settings: TwitchTtsSettings) -> Result<TwitchConnectionStatus, TwitchError> {
        settings
            .validate_for_connect()
            .map_err(TwitchError::InvalidSettings)?;

        if !settings.enabled {
            return Err(TwitchError::InvalidSettings(
                "twitch chat TTS is disabled in module settings".into(),
            ));
        }

        trace::trace(
            "service",
            "connect_requested",
            json!({
                "channel": settings.normalized_channel(),
                "nick": settings.nick.trim(),
                "lang": settings.language,
            }),
        );

        self.disconnect();
        let refresh_settings = settings.clone();
        let connect_channel = refresh_settings.normalized_channel();
        self.apply_settings(settings);

        let emotes = self.emotes.clone();
        let refresh_runtime = self.runtime.clone();
        refresh_runtime.spawn(async move {
            let _ = emotes
                .refresh(
                    &refresh_settings.channel_login(),
                    &refresh_settings.resolve_client_id(),
                    &refresh_settings.oauth_token,
                    &refresh_settings.emote_sources,
                )
                .await;
        });

        let (stop_tx, stop_rx) = watch::channel(false);
        let emotes_for_session = self.emotes.clone();
        let inner = self.inner.clone();
        let on_status: StatusCallback = Arc::new(move |state, channel| {
            let mapped = match state {
                "connecting" => TwitchConnectionState::Connecting,
                "connected" => TwitchConnectionState::Connected,
                "disconnected" => TwitchConnectionState::Disconnected,
                _ => TwitchConnectionState::Error,
            };
            let message = if mapped == TwitchConnectionState::Error {
                state.to_string()
            } else {
                String::new()
            };
            inner.set_status(mapped, channel.unwrap_or(""), &message);
            inner.broadcast_connection();
            trace::trace(
                "service",
                "status",
                json!({
                    "state": format!("{:?}", mapped),
                    "channel": channel.unwrap_or(""),
                    "message": message,
                }),
            );
        });
        let on_message: MessageCallback = {
            let broadcaster = self.inner.broadcaster.clone();
            Arc::new(move |message: TwitchChatMessage| {
                trace::trace(
                    "service",
                    "broadcast_message",
                    trace::with_text(
                        json!({
                            "id": message.id,
                            "user": message.user,
                            "speakable": message.speakable,
                            "lang": message.language,
                        }),
                        &message.text,
                    ),
                );
                broadcaster(json!({
                    "type": "twitch_chat_message",
                    "payload": message,
                }));
            })
        };

        let live_for_task = self.live.clone();
        let on_status_for_err = on_status.clone();
        let task = self.runtime.spawn(async move {
            if let Err(err) = run_session(
                live_for_task,
                stop_rx,
                on_status,
                on_message,
                emotes_for_session,
            )
            .await
            {
                warn!(target: "voicesub.twitch", error = %err, "twitch irc session ended with error");
                trace::trace("service", "session_error", json!({ "error": err.to_string() }));
                on_status_for_err("error", Some(&err.to_string()));
            }
        });

        self.inner
            .active
            .lock()
            .map_err(|_| TwitchError::Irc("lock poisoned".into()))?
            .replace(ActiveSession { stop_tx, task });

        self.set_status(TwitchConnectionState::Connecting, &connect_channel, "");
        self.broadcast_connection();
        Ok(self.status())
    }

    fn set_status(&self, state: TwitchConnectionState, channel: &str, message: &str) {
        if let Ok(mut guard) = self.inner.status.lock() {
            guard.state = state;
            guard.channel = channel.to_string();
            guard.message = message.to_string();
        }
    }

    fn broadcast_connection(&self) {
        let status = self.status();
        trace::trace(
            "service",
            "broadcast_connection",
            json!({
                "state": format!("{:?}", status.state),
                "channel": status.channel,
                "message": status.message,
            }),
        );
        (self.inner.broadcaster)(json!({
            "type": "twitch_connection_update",
            "payload": status,
        }));
    }
}

impl Inner {
    fn set_status(&self, state: TwitchConnectionState, channel: &str, message: &str) {
        if let Ok(mut guard) = self.status.lock() {
            guard.state = state;
            guard.channel = channel.to_string();
            guard.message = message.to_string();
        }
    }

    fn broadcast_connection(&self) {
        let status = self
            .status
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        (self.broadcaster)(json!({
            "type": "twitch_connection_update",
            "payload": status,
        }));
    }
}
