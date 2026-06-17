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

pub async fn run_session(
    live: Arc<RwLock<TwitchLiveState>>,
    mut stop_rx: watch::Receiver<bool>,
    on_status: StatusCallback,
    on_message: MessageCallback,
    emotes: Arc<EmoteRegistry>,
) -> Result<(), TwitchError> {
    let connect_settings = live
        .read()
        .map_err(|_| TwitchError::Irc("settings lock poisoned".into()))?
        .chat
        .clone();
    connect_settings
        .validate_for_connect()
        .map_err(TwitchError::InvalidSettings)?;

    let channels = connect_settings.normalized_channels();
    if channels.is_empty() {
        return Err(TwitchError::InvalidSettings(
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

    let tcp = TcpStream::connect((TWITCH_IRC_HOST, TWITCH_IRC_PORT))
        .await
        .map_err(|err| {
            trace::trace("irc", "tcp_failed", json!({ "error": err.to_string() }));
            TwitchError::Irc(format!("tcp connect failed: {err}"))
        })?;

    let connector = build_tls_connector()?;
    let server_name = ServerName::try_from(TWITCH_IRC_HOST.to_string())
        .map_err(|err| TwitchError::Tls(err.to_string()))?;
    let tls = connector.connect(server_name, tcp).await.map_err(|err| {
        trace::trace("irc", "tls_failed", json!({ "error": err.to_string() }));
        TwitchError::Tls(err.to_string())
    })?;

    let (reader, mut writer) = tokio::io::split(tls);
    let mut lines = BufReader::new(reader).lines();

    send_line(
        &mut writer,
        "CAP REQ :twitch.tv/membership twitch.tv/tags twitch.tv/commands",
    )
    .await?;
    send_line(&mut writer, &format!("PASS {pass}")).await?;
    send_line(&mut writer, &format!("NICK {nick}")).await?;
    send_line(&mut writer, &format!("JOIN {join_arg}")).await?;

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
                let Some(line) = line.map_err(|err| TwitchError::Irc(err.to_string()))? else {
                    warn!(target: "voicesub.twitch", "irc stream closed");
                    trace::trace("irc", "stream_closed", json!({}));
                    break;
                };
                if line.is_empty() {
                    continue;
                }

                if let Some(reason) = parse_login_failure(&line) {
                    warn!(target: "voicesub.twitch", reason = %reason, "twitch login failed");
                    trace::trace("irc", "login_failed", json!({ "reason": reason }));
                    on_status("error", Some(&reason));
                    return Err(TwitchError::Irc(reason));
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
                    send_line(&mut writer, &pong).await?;
                    trace::trace("irc", "pong", json!({}));
                    continue;
                }

                if let Some(notice) = parse_notice(&line) {
                    debug!(target: "voicesub.twitch", notice = %notice, "irc notice");
                    trace::trace("irc", "notice", json!({ "text": notice }));
                    continue;
                }

                if let Some(message) = parse_privmsg(&line) {
                    let live_state = live
                        .read()
                        .map_err(|_| TwitchError::Irc("settings lock poisoned".into()))?
                        .clone();
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

    on_status("disconnected", None);
    trace::trace("irc", "session_end", json!({ "channels": channels }));
    Ok(())
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
}
