use std::sync::{Arc, RwLock};

use crate::emotes::EmoteRegistry;
use rustls::pki_types::ServerName;
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio_rustls::TlsConnector;
use tracing::{debug, info, warn};

use crate::error::TwitchError;
use crate::filter::{ChatMessageInput, to_chat_message};
use crate::service::TwitchLiveState;
use crate::settings::TwitchChatMessage;
use crate::trace;

const TWITCH_IRC_HOST: &str = "irc.chat.twitch.tv";
const TWITCH_IRC_PORT: u16 = 6697;

pub type StatusCallback = Arc<dyn Fn(&str, Option<&str>) + Send + Sync>;
pub type MessageCallback = Arc<dyn Fn(TwitchChatMessage) + Send + Sync>;

#[derive(Debug, PartialEq, Eq)]
pub enum SessionOutcome {
    /// Stop requested via `stop_rx`.
    Stopped,
    /// IRC stream ended; `joined` is true when the channel JOIN completed.
    Disconnected { joined: bool },
    /// Session failed before or during IRC handling.
    Error(TwitchError),
}

pub async fn run_session(
    live: Arc<RwLock<TwitchLiveState>>,
    mut stop_rx: watch::Receiver<bool>,
    on_status: StatusCallback,
    on_message: MessageCallback,
    emotes: Arc<EmoteRegistry>,
) -> SessionOutcome {
    let connect_settings = match live.read() {
        Ok(guard) => guard.chat.clone(),
        Err(_) => {
            return SessionOutcome::Error(TwitchError::Irc("settings lock poisoned".into()));
        }
    };
    if let Err(err) = connect_settings.validate_for_connect() {
        return SessionOutcome::Error(TwitchError::InvalidSettings(err));
    }

    let channels = connect_settings.normalized_channels();
    if channels.is_empty() {
        return SessionOutcome::Error(TwitchError::InvalidSettings(
            "at least one channel is required".into(),
        ));
    }
    let channels_label = channels.join(", ");
    let join_arg = channels.join(",");
    let nick = connect_settings.nick.trim().to_string();
    let pass = normalize_oauth_token(&connect_settings.oauth_token);

    on_status("connecting", Some(&channels_label));
    trace::trace(
        "irc",
        "connect_start",
        json!({ "channels": channels, "nick": nick }),
    );
    info!(
        target: "voicesub.twitch",
        channels = %channels_label,
        nick = %nick,
        "connecting to twitch irc"
    );

    let tcp = match TcpStream::connect((TWITCH_IRC_HOST, TWITCH_IRC_PORT)).await {
        Ok(stream) => stream,
        Err(err) => {
            trace::trace("irc", "tcp_failed", json!({ "error": err.to_string() }));
            return SessionOutcome::Error(TwitchError::Irc(format!("tcp connect failed: {err}")));
        }
    };
    let _ = tcp.set_nodelay(true);

    let connector = match build_tls_connector() {
        Ok(connector) => connector,
        Err(err) => return SessionOutcome::Error(err),
    };
    let server_name = match ServerName::try_from(TWITCH_IRC_HOST.to_string()) {
        Ok(name) => name,
        Err(err) => return SessionOutcome::Error(TwitchError::Tls(err.to_string())),
    };
    let tls = match connector.connect(server_name, tcp).await {
        Ok(stream) => stream,
        Err(err) => {
            trace::trace("irc", "tls_failed", json!({ "error": err.to_string() }));
            return SessionOutcome::Error(TwitchError::Tls(err.to_string()));
        }
    };

    let (reader, mut writer) = tokio::io::split(tls);
    let mut lines = BufReader::new(reader).lines();

    if let Err(err) = send_line(
        &mut writer,
        "CAP REQ :twitch.tv/membership twitch.tv/tags twitch.tv/commands",
    )
    .await
    {
        return SessionOutcome::Error(err);
    }
    if let Err(err) = send_line(&mut writer, &format!("PASS {pass}")).await {
        return SessionOutcome::Error(err);
    }
    if let Err(err) = send_line(&mut writer, &format!("NICK {nick}")).await {
        return SessionOutcome::Error(err);
    }
    if let Err(err) = send_line(&mut writer, &format!("JOIN {join_arg}")).await {
        return SessionOutcome::Error(err);
    }

    trace::trace("irc", "handshake_sent", json!({ "channels": channels }));
    let mut joined = false;

    loop {
        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() {
                    debug!(target: "voicesub.twitch", "irc stop requested");
                    trace::trace("irc", "stop_requested", json!({}));
                    break;
                }
            }
            line = lines.next_line() => {
                let line = match line {
                    Ok(Some(line)) => line,
                    Ok(None) => {
                        warn!(target: "voicesub.twitch", "irc stream closed");
                        trace::trace("irc", "stream_closed", json!({ "joined": joined }));
                        break;
                    }
                    Err(err) => {
                        if should_end_session_after_io_error(&err, joined) {
                            warn!(
                                target: "voicesub.twitch",
                                error = %err,
                                joined,
                                "irc transport closed"
                            );
                            trace::trace(
                                "irc",
                                "transport_closed",
                                json!({ "joined": joined, "error": err.to_string() }),
                            );
                            break;
                        }
                        return SessionOutcome::Error(irc_error_from_io(err));
                    }
                };
                if line.is_empty() {
                    continue;
                }

                if let Some(reason) = parse_login_failure(&line) {
                    warn!(target: "voicesub.twitch", reason = %reason, "twitch login failed");
                    trace::trace("irc", "login_failed", json!({ "reason": reason }));
                    on_status("error", Some(&reason));
                    return SessionOutcome::Error(TwitchError::Irc(reason));
                }

                if !joined && line.contains(" JOIN ") {
                    joined = true;
                    on_status("connected", Some(&channels_label));
                    info!(
                        target: "voicesub.twitch",
                        channels = %channels_label,
                        "twitch irc joined channel(s)"
                    );
                    trace::trace("irc", "joined", json!({ "channels": channels }));
                }

                if let Some(ping_payload) = line.strip_prefix("PING") {
                    let pong = format!("PONG{ping_payload}");
                    if let Err(err) = send_line(&mut writer, &pong).await {
                        if joined || is_transient_irc_disconnect_message(&err.to_string()) {
                            warn!(
                                target: "voicesub.twitch",
                                error = %err,
                                "irc pong failed; treating as disconnect"
                            );
                            break;
                        }
                        return SessionOutcome::Error(err);
                    }
                    trace::trace("irc", "pong", json!({}));
                    continue;
                }

                if let Some(notice) = parse_notice(&line) {
                    debug!(target: "voicesub.twitch", notice = %notice, "irc notice");
                    trace::trace("irc", "notice", json!({ "text": notice }));
                    continue;
                }

                if let Some(message) = parse_privmsg(&line) {
                    let live_state = match live.read() {
                        Ok(guard) => guard.clone(),
                        Err(_) => {
                            return SessionOutcome::Error(TwitchError::Irc(
                                "settings lock poisoned".into(),
                            ));
                        }
                    };
                    let emotes_tag = if message.emotes_tag.is_empty() {
                        None
                    } else {
                        Some(message.emotes_tag.as_str())
                    };
                    let chat = to_chat_message(
                        &live_state.chat,
                        &live_state.source_replacement,
                        &emotes,
                        ChatMessageInput {
                            id: &message.msg_id,
                            user: &message.login,
                            display_name: &message.display_name,
                            text: &message.text,
                            channel: &message.channel,
                            is_mod: message.is_mod,
                            is_subscriber: message.is_subscriber,
                            irc_emotes_tag: emotes_tag,
                        },
                    );
                    if !chat.speakable {
                        trace::trace(
                            "filter",
                            "skip",
                            trace::with_text(
                                json!({
                                    "user": message.login,
                                    "channel": message.channel,
                                    "lang": chat.language,
                                }),
                                &message.text,
                            ),
                        );
                    }
                    trace::trace(
                        "irc",
                        "privmsg",
                        trace::with_text(
                            json!({
                                "user": chat.user,
                                "display_name": chat.display_name,
                                "channel": chat.channel,
                                "speakable": chat.speakable,
                                "lang": chat.language,
                            }),
                            &chat.text,
                        ),
                    );
                    on_message(chat);
                }
            }
        }
    }

    trace::trace(
        "irc",
        "session_end",
        json!({ "channels": channels, "joined": joined }),
    );
    if *stop_rx.borrow() {
        on_status("disconnected", None);
        SessionOutcome::Stopped
    } else {
        SessionOutcome::Disconnected { joined }
    }
}

const RECONNECT_INITIAL_BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);
const RECONNECT_MAX_BACKOFF: std::time::Duration = std::time::Duration::from_secs(30);

/// Peer closed the IRC TLS socket without a clean shutdown — treat as disconnect, not fatal.
pub(crate) fn is_transient_irc_disconnect_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("close_notify")
        || lower.contains("unexpected eof")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("connection aborted")
        || lower.contains("forcibly closed")
        || lower.contains("wsaeconnreset")
}

fn reconnect_delay(base: std::time::Duration, attempt: u32) -> std::time::Duration {
    let jitter_ms = (attempt.wrapping_mul(7919) % 400) as u64;
    base + std::time::Duration::from_millis(jitter_ms)
}

fn irc_error_from_io(err: std::io::Error) -> TwitchError {
    TwitchError::Irc(err.to_string())
}

fn should_end_session_after_io_error(err: &std::io::Error, joined: bool) -> bool {
    joined || is_transient_irc_disconnect_message(&err.to_string())
}

pub async fn run_session_with_reconnect(
    live: Arc<RwLock<TwitchLiveState>>,
    mut stop_rx: watch::Receiver<bool>,
    on_status: StatusCallback,
    on_message: MessageCallback,
    emotes: Arc<EmoteRegistry>,
) {
    let mut backoff = RECONNECT_INITIAL_BACKOFF;
    let mut attempt = 0u32;

    loop {
        if *stop_rx.borrow() {
            break;
        }

        let outcome = run_session(
            live.clone(),
            stop_rx.clone(),
            on_status.clone(),
            on_message.clone(),
            emotes.clone(),
        )
        .await;

        match outcome {
            SessionOutcome::Stopped => break,
            SessionOutcome::Disconnected { joined } => {
                if *stop_rx.borrow() {
                    break;
                }
                if joined {
                    backoff = RECONNECT_INITIAL_BACKOFF;
                }
                attempt += 1;
                let channels_label = live
                    .read()
                    .ok()
                    .map(|guard| guard.chat.normalized_channels_label())
                    .unwrap_or_default();
                warn!(
                    target: "voicesub.twitch",
                    attempt,
                    backoff_secs = backoff.as_secs(),
                    joined,
                    "twitch irc disconnected, reconnecting"
                );
                trace::trace(
                    "irc",
                    "reconnect_scheduled",
                    json!({
                        "attempt": attempt,
                        "backoff_ms": backoff.as_millis(),
                        "joined": joined,
                        "channels": channels_label,
                    }),
                );
                on_status("connecting", Some(&channels_label));
                if sleep_or_stop(&mut stop_rx, reconnect_delay(backoff, attempt)).await {
                    break;
                }
                backoff = (backoff * 2).min(RECONNECT_MAX_BACKOFF);
            }
            SessionOutcome::Error(err) => {
                if *stop_rx.borrow() {
                    break;
                }
                if !err.is_retryable() {
                    warn!(target: "voicesub.twitch", error = %err, "twitch irc session failed (non-retryable)");
                    trace::trace(
                        "service",
                        "session_error",
                        json!({ "error": err.to_string(), "retryable": false }),
                    );
                    on_status("error", Some(&err.to_string()));
                    break;
                }
                attempt += 1;
                let channels_label = live
                    .read()
                    .ok()
                    .map(|guard| guard.chat.normalized_channels_label())
                    .unwrap_or_default();
                warn!(
                    target: "voicesub.twitch",
                    attempt,
                    error = %err,
                    backoff_secs = backoff.as_secs(),
                    "twitch irc session failed, retrying"
                );
                trace::trace(
                    "irc",
                    "reconnect_scheduled",
                    json!({
                        "attempt": attempt,
                        "backoff_ms": backoff.as_millis(),
                        "error": err.to_string(),
                    }),
                );
                on_status("connecting", Some(&channels_label));
                if sleep_or_stop(&mut stop_rx, reconnect_delay(backoff, attempt)).await {
                    break;
                }
                backoff = (backoff * 2).min(RECONNECT_MAX_BACKOFF);
            }
        }
    }
}

async fn sleep_or_stop(stop_rx: &mut watch::Receiver<bool>, delay: std::time::Duration) -> bool {
    if *stop_rx.borrow() {
        return true;
    }
    tokio::select! {
        () = tokio::time::sleep(delay) => *stop_rx.borrow(),
        changed = stop_rx.changed() => {
            changed.is_ok() && *stop_rx.borrow()
        }
    }
}

fn build_tls_connector() -> Result<TlsConnector, TwitchError> {
    crate::init_crypto_provider();
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(TlsConnector::from(Arc::new(config)))
}

fn normalize_oauth_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.to_ascii_lowercase().starts_with("oauth:") {
        trimmed.to_string()
    } else {
        format!("oauth:{trimmed}")
    }
}

async fn send_line(
    writer: &mut tokio::io::WriteHalf<tokio_rustls::client::TlsStream<TcpStream>>,
    line: &str,
) -> Result<(), TwitchError> {
    debug!(
        target: "voicesub.twitch",
        direction = "out",
        line = %redact_outbound_line(line),
    );
    writer
        .write_all(format!("{line}\r\n").as_bytes())
        .await
        .map_err(|err| TwitchError::Irc(err.to_string()))?;
    writer
        .flush()
        .await
        .map_err(|err| TwitchError::Irc(err.to_string()))
}

fn redact_outbound_line(line: &str) -> String {
    if line.to_ascii_uppercase().starts_with("PASS ") {
        "PASS oauth:***".to_string()
    } else {
        line.to_string()
    }
}

fn parse_login_failure(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    if lower.contains("login authentication failed")
        || lower.contains("improperly formatted auth")
        || lower.contains("invalid nick")
    {
        return Some("Twitch login authentication failed — check nick and OAuth token".into());
    }
    None
}

fn parse_notice(line: &str) -> Option<String> {
    if line.contains(" NOTICE ") {
        return line.split_once(" :").map(|(_, msg)| msg.to_string());
    }
    None
}

struct ParsedPrivmsg {
    login: String,
    display_name: String,
    channel: String,
    text: String,
    msg_id: String,
    is_mod: bool,
    is_subscriber: bool,
    emotes_tag: String,
}

fn parse_privmsg(line: &str) -> Option<ParsedPrivmsg> {
    let (tags, rest) = if let Some(stripped) = line.strip_prefix('@') {
        let (tag_part, remainder) = stripped.split_once(' ')?;
        (tag_part, remainder)
    } else {
        ("", line)
    };

    if !rest.contains(" PRIVMSG ") {
        return None;
    }

    let (prefix, message) = rest.split_once(" :")?;
    let mut prefix_parts = prefix.split_whitespace();
    let user_part = prefix_parts.next()?;
    let command = prefix_parts.next()?;
    let channel = prefix_parts.next()?;
    if command != "PRIVMSG" {
        return None;
    }

    let login = user_part
        .trim_start_matches(':')
        .split('!')
        .next()
        .unwrap_or("unknown")
        .to_string();

    let tag_map = parse_tags(tags);
    let display_name = tag_map
        .get("display-name")
        .cloned()
        .unwrap_or_else(|| login.clone());
    let msg_id = tag_map.get("id").cloned().unwrap_or_default();
    let is_mod = tag_map.get("mod").is_some_and(|v| v == "1");
    let is_subscriber = tag_map.get("subscriber").is_some_and(|v| v == "1");
    let emotes_tag = tag_map.get("emotes").cloned().unwrap_or_default();

    Some(ParsedPrivmsg {
        login,
        display_name,
        channel: channel.to_string(),
        text: message.to_string(),
        msg_id,
        is_mod,
        is_subscriber,
        emotes_tag,
    })
}

fn parse_tags(tags: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for part in tags.split(';') {
        if let Some((key, value)) = part.split_once('=') {
            out.insert(key.to_string(), value.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tagged_privmsg() {
        let line = "@badge-info=;badges=;color=#FF0000;display-name=Alice;emotes=;first-msg=0;flags=;id=abc-123;mod=0;room-id=1;subscriber=0;tmi-sent-ts=1;turbo=0;user-id=2;user-type= :alice!alice@alice.tmi.twitch.tv PRIVMSG #channel :Hello chat";
        let parsed = parse_privmsg(line).expect("privmsg");
        assert_eq!(parsed.login, "alice");
        assert_eq!(parsed.display_name, "Alice");
        assert_eq!(parsed.channel, "#channel");
        assert_eq!(parsed.text, "Hello chat");
        assert_eq!(parsed.msg_id, "abc-123");
    }

    #[test]
    fn parses_emotes_tag_from_privmsg() {
        let line = "@display-name=User;emotes=25:0-7;id=msg-1;mod=0;subscriber=0 :user!user@user.tmi.twitch.tv PRIVMSG #channel :baleGIGA";
        let parsed = parse_privmsg(line).expect("privmsg");
        assert_eq!(parsed.text, "baleGIGA");
        assert_eq!(parsed.emotes_tag, "25:0-7");
    }

    #[test]
    fn detects_login_failure_notice() {
        let line = ":tmi.twitch.tv NOTICE * :Login authentication failed";
        assert!(parse_login_failure(line).is_some());
    }

    #[test]
    fn redacts_pass_line() {
        assert_eq!(redact_outbound_line("PASS oauth:secret"), "PASS oauth:***");
    }

    #[test]
    fn transient_disconnect_includes_tls_close_notify() {
        assert!(is_transient_irc_disconnect_message(
            "peer closed connection without sending TLS close_notify"
        ));
        assert!(is_transient_irc_disconnect_message(
            "connection reset by peer"
        ));
        assert!(!is_transient_irc_disconnect_message(
            "login authentication failed"
        ));
    }
}
