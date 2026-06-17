use std::sync::{Arc, Mutex, RwLock};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::emotes::EmoteRegistry;
use crate::error::TwitchError;
use crate::irc::{MessageCallback, StatusCallback, run_session_with_reconnect};
use crate::settings::{TwitchChatMessage, TwitchTtsSettings};
use crate::source_text_replacement::{
    SourceTextReplacementSettings, profanity_settings_for_twitch,
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
    /// Comma-separated `#channel` labels for compact display.
    pub channel: String,
    #[serde(default)]
    pub channels: Vec<String>,
    pub message: String,
}

impl Default for TwitchConnectionStatus {
    fn default() -> Self {
        Self {
            state: TwitchConnectionState::Disconnected,
            channel: String::new(),
            channels: Vec::new(),
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
                    "emote_sources": settings.emote_sources,
                }),
            );
            guard.chat = settings.clone();
            guard.source_replacement = profanity_settings_for_twitch(&settings);
        }
        if self.is_session_active() {
            self.spawn_emote_refresh(settings);
        }
    }

    fn is_session_active(&self) -> bool {
        self.inner
            .active
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    fn spawn_emote_refresh(&self, settings: TwitchTtsSettings) {
        if !settings.strip_emotes {
            return;
        }
        let sources = settings.emote_sources.clone();
        if !sources.twitch && !sources.bttv && !sources.seventv {
            return;
        }
        let logins = settings.channel_logins();
        if logins.is_empty() {
            return;
        }
        let emotes = self.emotes.clone();
        let client_id = settings.resolve_client_id();
        let oauth_token = settings.oauth_token.clone();
        self.runtime.spawn(async move {
            if let Err(err) = emotes
                .refresh_all(&logins, &client_id, &oauth_token, &sources)
                .await
            {
                warn!(
                    target: "voicesub.twitch.emotes",
                    error = %err,
                    "hot-apply emote refresh failed"
                );
            }
        });
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
        self.set_status(TwitchConnectionState::Disconnected, "", &[], "");
        self.broadcast_connection();
    }

    pub fn connect(
        &self,
        settings: TwitchTtsSettings,
    ) -> Result<TwitchConnectionStatus, TwitchError> {
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
                "channels": settings.normalized_channels(),
                "nick": settings.nick.trim(),
                "lang": settings.language,
            }),
        );

        self.disconnect();
        let refresh_settings = settings.clone();
        let connect_channels = refresh_settings.normalized_channels();
        let connect_label = refresh_settings.normalized_channels_label();
        self.apply_settings(settings);

        let (stop_tx, stop_rx) = watch::channel(false);
        let emotes_for_session = self.emotes.clone();
        let refresh_logins = refresh_settings.channel_logins();
        let refresh_client_id = refresh_settings.resolve_client_id();
        let refresh_token = refresh_settings.oauth_token.clone();
        let refresh_sources = refresh_settings.emote_sources.clone();
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
            let label = channel.unwrap_or("");
            let channels: Vec<String> = if label.is_empty() {
                Vec::new()
            } else {
                label
                    .split(", ")
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                    .map(str::to_string)
                    .collect()
            };
            inner.set_status(mapped, label, &channels, &message);
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
        let inner_for_cleanup = self.inner.clone();
        let task = self.runtime.spawn(async move {
            if refresh_settings.strip_emotes
                && (refresh_sources.twitch || refresh_sources.bttv || refresh_sources.seventv)
                && let Err(err) = emotes_for_session
                    .refresh_all(
                        &refresh_logins,
                        &refresh_client_id,
                        &refresh_token,
                        &refresh_sources,
                    )
                    .await
            {
                warn!(
                    target: "voicesub.twitch.emotes",
                    error = %err,
                    "pre-connect emote refresh failed"
                );
            }

            run_session_with_reconnect(
                live_for_task,
                stop_rx,
                on_status,
                on_message,
                emotes_for_session,
            )
            .await;

            if let Ok(mut guard) = inner_for_cleanup.active.lock() {
                guard.take();
            }
        });

        self.inner
            .active
            .lock()
            .map_err(|_| TwitchError::Irc("lock poisoned".into()))?
            .replace(ActiveSession { stop_tx, task });

        self.set_status(
            TwitchConnectionState::Connecting,
            &connect_label,
            &connect_channels,
            "",
        );
        self.broadcast_connection();
        Ok(self.status())
    }

    fn set_status(
        &self,
        state: TwitchConnectionState,
        channel: &str,
        channels: &[String],
        message: &str,
    ) {
        if let Ok(mut guard) = self.inner.status.lock() {
            guard.state = state;
            guard.channel = channel.to_string();
            guard.channels = channels.to_vec();
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
                "channels": status.channels,
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
    fn set_status(
        &self,
        state: TwitchConnectionState,
        channel: &str,
        channels: &[String],
        message: &str,
    ) {
        if let Ok(mut guard) = self.status.lock() {
            guard.state = state;
            guard.channel = channel.to_string();
            guard.channels = channels.to_vec();
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

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn apply_settings_updates_live_chat_without_reconnect() {
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let svc = TwitchChatService::new(Arc::new(|_| {}), rt.handle().clone());

        let mut first = TwitchTtsSettings::default();
        first.strip_symbols = vec!["@".into()];
        first.strip_links = true;
        svc.apply_settings(first);

        let live = svc.live.read().expect("live lock");
        assert_eq!(live.chat.strip_symbols, vec!["@".to_string()]);
        assert!(live.chat.strip_links);
        drop(live);

        let mut second = TwitchTtsSettings::default();
        second.strip_symbols = vec!["#".into(), "%".into()];
        second.strip_links = false;
        svc.apply_settings(second);

        let live = svc.live.read().expect("live lock");
        assert_eq!(
            live.chat.strip_symbols,
            vec!["#".to_string(), "%".to_string()]
        );
        assert!(!live.chat.strip_links);
    }
}
