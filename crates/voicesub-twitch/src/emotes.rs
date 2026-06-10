use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::emoji::{normalize_whitespace, strip_unicode_emoji};
use crate::settings::TwitchEmoteSources;

pub const DEFAULT_TWITCH_CLIENT_ID: &str = "oraf2d29s9mm8kxq4xx97zo28xaj7b";

#[derive(Debug, Default, Clone)]
pub struct EmoteSets {
    pub twitch: HashSet<String>,
    pub bttv: HashSet<String>,
    pub seventv: HashSet<String>,
    pub channel_login: String,
    pub twitch_count: usize,
    pub bttv_count: usize,
    pub seventv_count: usize,
    pub last_refresh: Option<Instant>,
}

#[derive(Debug, Default)]
pub struct EmoteRegistry {
    inner: RwLock<EmoteSets>,
}

impl EmoteRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn snapshot(&self) -> EmoteSets {
        self.inner
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn clean_message_text(
        &self,
        text: &str,
        irc_emotes_tag: Option<&str>,
        sources: &TwitchEmoteSources,
        strip_emoji: bool,
    ) -> String {
        let mut working = text.trim().to_string();
        if let Some(tag) = irc_emotes_tag.filter(|value| !value.is_empty()) {
            working = strip_irc_emotes(&working, tag);
        }
        self.remove_emotes_from_text(&working, sources, strip_emoji)
    }

    pub fn remove_emotes_from_text(
        &self,
        text: &str,
        sources: &TwitchEmoteSources,
        strip_emoji: bool,
    ) -> String {
        let sets = self.snapshot();
        let mut working = if strip_emoji {
            strip_unicode_emoji(text)
        } else {
            text.to_string()
        };
        if !sources.twitch && !sources.bttv && !sources.seventv {
            return normalize_whitespace(&working);
        }
        let words: Vec<String> = working
            .split_whitespace()
            .map(|word| {
                let no_emoji = if strip_emoji {
                    strip_unicode_emoji(word)
                } else {
                    word.to_string()
                };
                if no_emoji.is_empty() {
                    return String::new();
                }
                let lc = no_emoji.to_ascii_lowercase();
                if sources.twitch && sets.twitch.contains(&lc) {
                    return String::new();
                }
                if sources.bttv && emote_set_contains(&sets.bttv, &no_emoji) {
                    return String::new();
                }
                if sources.seventv && emote_set_contains(&sets.seventv, &no_emoji) {
                    return String::new();
                }
                no_emoji
            })
            .filter(|word| !word.is_empty())
            .collect();
        working = words.join(" ");
        normalize_whitespace(&working)
    }

    pub async fn refresh(
        &self,
        channel_login: &str,
        client_id: &str,
        oauth_token: &str,
        sources: &TwitchEmoteSources,
    ) -> Result<EmoteSets, String> {
        let login = channel_login.trim().trim_start_matches('#').to_lowercase();
        if login.is_empty() {
            return Err("channel login is empty".into());
        }

        let client = reqwest::Client::new();
        let mut twitch = HashSet::new();
        let mut bttv = HashSet::new();
        let mut seventv = HashSet::new();

        let bearer = normalize_bearer(oauth_token);
        let mut broadcaster_id: Option<String> = None;

        if sources.twitch || sources.bttv || sources.seventv {
            broadcaster_id = fetch_broadcaster_id(&client, &login, client_id, &bearer).await;
            if broadcaster_id.is_none() {
                broadcaster_id = fetch_broadcaster_id_fallback(&client, &login).await;
            }
            if broadcaster_id.is_none() {
                warn!(
                    target: "voicesub.twitch.emotes",
                    channel = %login,
                    "broadcaster id lookup failed — channel BTTV/7TV emotes unavailable"
                );
            }
        }

        if sources.twitch {
            if let Err(err) = fetch_twitch_emotes(
                &client,
                client_id,
                &bearer,
                broadcaster_id.as_deref(),
                &mut twitch,
            )
            .await
            {
                warn!(target: "voicesub.twitch.emotes", error = %err, "twitch emote fetch failed");
            }
        }

        if sources.bttv {
            if let Some(id) = broadcaster_id.as_deref() {
                if let Err(err) =
                    fetch_bttv_emotes(&client, id, &mut bttv).await
                {
                    warn!(target: "voicesub.twitch.emotes", error = %err, "bttv emote fetch failed");
                }
            }
        }

        if sources.seventv {
            if let Some(id) = broadcaster_id.as_deref() {
                if let Err(err) =
                    fetch_seventv_emotes(&client, id, &mut seventv).await
                {
                    warn!(target: "voicesub.twitch.emotes", error = %err, "7tv emote fetch failed");
                }
            }
        }

        let snapshot = EmoteSets {
            twitch,
            bttv,
            seventv,
            channel_login: login.clone(),
            twitch_count: 0,
            bttv_count: 0,
            seventv_count: 0,
            last_refresh: Some(Instant::now()),
        };
        let mut snapshot = snapshot;
        snapshot.twitch_count = snapshot.twitch.len();
        snapshot.bttv_count = snapshot.bttv.len();
        snapshot.seventv_count = snapshot.seventv.len();

        if let Ok(mut guard) = self.inner.write() {
            *guard = snapshot.clone();
        }

        info!(
            target: "voicesub.twitch.emotes",
            channel = %login,
            twitch = snapshot.twitch_count,
            bttv = snapshot.bttv_count,
            seventv = snapshot.seventv_count,
            "emote cache refreshed"
        );
        Ok(snapshot)
    }
}

fn normalize_bearer(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.to_ascii_lowercase().starts_with("oauth:") {
        trimmed[6..].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

async fn fetch_broadcaster_id(
    client: &reqwest::Client,
    login: &str,
    client_id: &str,
    bearer: &str,
) -> Option<String> {
    let url = format!("https://api.twitch.tv/helix/users?login={login}");
    let mut req = client.get(&url).header("Client-Id", client_id);
    if !bearer.is_empty() {
        req = req.header("Authorization", format!("Bearer {bearer}"));
    }
    let response = req.send().await.ok()?;
    if !response.status().is_success() {
        debug!(
            target: "voicesub.twitch.emotes",
            status = %response.status(),
            "helix users lookup failed"
        );
        return None;
    }
    let body: HelixUsersResponse = response.json().await.ok()?;
    body.data.into_iter().next().map(|user| user.id)
}

async fn fetch_twitch_emotes(
    client: &reqwest::Client,
    client_id: &str,
    bearer: &str,
    broadcaster_id: Option<&str>,
    out: &mut HashSet<String>,
) -> Result<(), String> {
    let headers = |req: reqwest::RequestBuilder| {
        let mut req = req.header("Client-Id", client_id);
        if !bearer.is_empty() {
            req = req.header("Authorization", format!("Bearer {bearer}"));
        }
        req
    };

    let global: HelixEmotesResponse = headers(client.get("https://api.twitch.tv/helix/chat/emotes/global"))
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;
    for emote in global.data {
        out.insert(emote.name.to_ascii_lowercase());
    }

    if let Some(id) = broadcaster_id {
        let url = format!("https://api.twitch.tv/helix/chat/emotes?broadcaster_id={id}");
        let channel: HelixEmotesResponse = headers(client.get(url))
            .send()
            .await
            .map_err(|err| err.to_string())?
            .json()
            .await
            .map_err(|err| err.to_string())?;
        for emote in channel.data {
            out.insert(emote.name.to_ascii_lowercase());
        }
    }
    Ok(())
}

async fn fetch_bttv_emotes(
    client: &reqwest::Client,
    broadcaster_id: &str,
    out: &mut HashSet<String>,
) -> Result<(), String> {
    let global: Vec<BttvEmote> = client
        .get("https://api.betterttv.net/3/cached/emotes/global")
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;
    for emote in global {
        insert_third_party_code(out, &emote.code);
    }

    let url = format!("https://api.betterttv.net/3/cached/users/twitch/{broadcaster_id}");
    let user: BttvUser = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;
    for emote in user.channel_emotes {
        insert_third_party_code(out, &emote.code);
    }
    for emote in user.shared_emotes {
        insert_third_party_code(out, &emote.code);
    }
    Ok(())
}

async fn fetch_seventv_emotes(
    client: &reqwest::Client,
    broadcaster_id: &str,
    out: &mut HashSet<String>,
) -> Result<(), String> {
    let global: SevenTvEmoteSet = client
        .get("https://7tv.io/v3/emote-sets/global")
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;
    for emote in global.emotes {
        if let Some(name) = emote.name {
            insert_third_party_code(out, &name);
        }
    }

    let url = format!("https://7tv.io/v3/users/twitch/{broadcaster_id}");
    let user: SevenTvUser = client
        .get(url)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .json()
        .await
        .map_err(|err| err.to_string())?;
    if let Some(set) = user.emote_set {
        for emote in set.emotes {
            if let Some(name) = emote.name {
                insert_third_party_code(out, &name);
            }
        }
    }
    Ok(())
}

/// Strip Twitch IRC `emotes` tag ranges (UTF-16 indices, inclusive).
pub fn strip_irc_emotes(text: &str, emotes_tag: &str) -> String {
    let ranges = parse_irc_emote_ranges(emotes_tag);
    if ranges.is_empty() {
        return text.to_string();
    }
    let mut units: Vec<u16> = text.encode_utf16().collect();
    for (start, end) in ranges {
        if start < units.len() && end < units.len() && start <= end {
            units.drain(start..=end);
        }
    }
    String::from_utf16_lossy(&units)
}

fn parse_irc_emote_ranges(emotes_tag: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for segment in emotes_tag.split('/') {
        let Some((_, ranges_part)) = segment.split_once(':') else {
            continue;
        };
        for range in ranges_part.split(',') {
            let Some((start, end)) = range.split_once('-') else {
                continue;
            };
            if let (Ok(start), Ok(end)) = (start.parse::<usize>(), end.parse::<usize>()) {
                ranges.push((start, end));
            }
        }
    }
    ranges.sort_by_key(|range| std::cmp::Reverse(range.0));
    ranges
}

fn insert_third_party_code(set: &mut HashSet<String>, code: &str) {
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return;
    }
    set.insert(trimmed.to_string());
    let lower = trimmed.to_ascii_lowercase();
    if lower != trimmed {
        set.insert(lower);
    }
}

fn emote_set_contains(set: &HashSet<String>, word: &str) -> bool {
    set.contains(word) || set.contains(&word.to_ascii_lowercase())
}

async fn fetch_broadcaster_id_fallback(client: &reqwest::Client, login: &str) -> Option<String> {
    #[derive(Debug, Deserialize)]
    struct IvrUser {
        id: Option<u64>,
    }
    let url = format!("https://api.ivr.fi/v2/twitch/user?login={login}");
    let response = client.get(&url).send().await.ok()?;
    if !response.status().is_success() {
        debug!(
            target: "voicesub.twitch.emotes",
            status = %response.status(),
            "ivr.fi user lookup failed"
        );
        return None;
    }
    let body: IvrUser = response.json().await.ok()?;
    body.id.map(|id| id.to_string())
}

#[derive(Debug, Deserialize)]
struct HelixUsersResponse {
    data: Vec<HelixUser>,
}

#[derive(Debug, Deserialize)]
struct HelixUser {
    id: String,
}

#[derive(Debug, Deserialize)]
struct HelixEmotesResponse {
    data: Vec<HelixEmote>,
}

#[derive(Debug, Deserialize)]
struct HelixEmote {
    name: String,
}

#[derive(Debug, Deserialize)]
struct BttvEmote {
    code: String,
}

#[derive(Debug, Deserialize)]
struct BttvUser {
    #[serde(default)]
    channel_emotes: Vec<BttvEmote>,
    #[serde(default)]
    shared_emotes: Vec<BttvEmote>,
}

#[derive(Debug, Deserialize)]
struct SevenTvUser {
    emote_set: Option<SevenTvEmoteSet>,
}

#[derive(Debug, Deserialize)]
struct SevenTvEmoteSet {
    #[serde(default)]
    emotes: Vec<SevenTvEmote>,
}

#[derive(Debug, Deserialize)]
struct SevenTvEmote {
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_known_emotes_lexically() {
        let registry = EmoteRegistry::new();
        {
            let mut guard = registry.inner.write().unwrap();
            guard.twitch.insert("kappa".into());
            insert_third_party_code(&mut guard.bttv, "OMEGALUL");
        }
        let sources = TwitchEmoteSources::default();
        let out = registry.remove_emotes_from_text("hello Kappa OMEGALUL world", &sources, true);
        assert_eq!(out, "hello world");
    }

    #[test]
    fn strips_irc_emote_positions() {
        let out = strip_irc_emotes("baleGIGA", "25:0-7");
        assert_eq!(out, "");
    }

    #[test]
    fn bttv_case_insensitive_match() {
        let mut set = HashSet::new();
        insert_third_party_code(&mut set, "baleGIGA");
        assert!(emote_set_contains(&set, "baleGIGA"));
        assert!(emote_set_contains(&set, "BaleGIGA"));
    }

    #[test]
    fn clean_message_text_removes_cached_emote_without_irc_tag() {
        let registry = EmoteRegistry::new();
        {
            let mut guard = registry.inner.write().unwrap();
            insert_third_party_code(&mut guard.bttv, "baleGIGA");
        }
        let sources = TwitchEmoteSources::default();
        let out = registry.clean_message_text("baleGIGA", None, &sources, false);
        assert_eq!(out, "");
    }
}
