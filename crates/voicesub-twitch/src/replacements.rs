use std::collections::HashMap;

use crate::settings::{TwitchReplacement, TwitchTtsSettings};

pub fn resolve_spoken_nick(
    settings: &TwitchTtsSettings,
    login: &str,
    display_name: &str,
) -> String {
    let map = replacement_map(&settings.nick_replacements);
    if let Some(hit) = map.get(display_name.trim()) {
        return hit.clone();
    }
    if let Some(hit) = map.get(login.trim()) {
        return hit.clone();
    }
    let fallback = display_name.trim();
    if !fallback.is_empty() {
        return fallback.to_string();
    }
    login.trim().to_string()
}

fn replacement_map(entries: &[TwitchReplacement]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for entry in entries {
        let from = entry.from.trim();
        if from.is_empty() {
            continue;
        }
        map.insert(from.to_string(), entry.to.trim().to_string());
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::TwitchTtsSettings;

    #[test]
    fn nick_replacement_prefers_display_name() {
        let mut settings = TwitchTtsSettings::default();
        settings.nick_replacements.push(TwitchReplacement {
            from: "Alice".into(),
            to: "Алиса".into(),
        });
        assert_eq!(resolve_spoken_nick(&settings, "alice", "Alice"), "Алиса");
    }
}
