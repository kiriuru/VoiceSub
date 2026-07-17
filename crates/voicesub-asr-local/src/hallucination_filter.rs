//! SST `local_asr_hallucination_filter` — reject spurious short text in silence.

use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use voicesub_partial_emit::normalize_transcript_text;

use crate::local_asr_constants::SHORT_HALLUCINATION_TOKENS;

static SHORT_HALLUCINATION_TOKEN_SET: LazyLock<HashSet<String>> = LazyLock::new(|| {
    SHORT_HALLUCINATION_TOKENS
        .iter()
        .map(|token| canonicalize_hallucination_token(token))
        .collect()
});

/// Strip punctuation/noise so `yeah!` / `Yeah.` match the SST token list.
fn canonicalize_hallucination_token(text: &str) -> String {
    normalize_transcript_text(text)
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '\'')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// SST `should_drop_short_hallucination` — drop typical Parakeet silence tokens.
pub fn should_drop_short_hallucination(text: &str, duration_ms: u32, is_final: bool) -> bool {
    let normalized = normalize_transcript_text(text);
    if normalized.is_empty() {
        return true;
    }
    let key = canonicalize_hallucination_token(&normalized);
    if key.is_empty() || !SHORT_HALLUCINATION_TOKEN_SET.contains(&key) {
        return false;
    }
    let word_count = normalized.split_whitespace().count();
    if word_count > 2 {
        return false;
    }
    // Finals: stricter window — silence tokens after a pause should not become subtitles.
    let short_duration_limit_ms = if is_final { 900 } else { 1100 };
    duration_ms <= short_duration_limit_ms
}

#[derive(Debug, Clone, PartialEq)]
pub struct HallucinationFilterConfig {
    pub enabled: bool,
    pub min_chars_when_silent: u32,
    pub cooldown_ms: u32,
}

impl Default for HallucinationFilterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_chars_when_silent: 0,
            cooldown_ms: 500,
        }
    }
}

#[derive(Debug, Default)]
pub struct HallucinationFilter {
    config: HallucinationFilterConfig,
    last_reject: Option<Instant>,
}

impl HallucinationFilter {
    pub fn new(config: HallucinationFilterConfig) -> Self {
        Self {
            config,
            last_reject: None,
        }
    }

    pub fn reset(&mut self) {
        self.last_reject = None;
    }

    pub fn accept_transcript(
        &mut self,
        text: &str,
        duration_ms: u32,
        is_final: bool,
        speech_active: bool,
    ) -> bool {
        // Short silence tokens: drop even mid-utterance when duration is tiny (Parakeet noise).
        // During active speech, keep non-token short text (C2 — do not swallow real brief words).
        if should_drop_short_hallucination(text, duration_ms, is_final) {
            return false;
        }
        self.accept(text, speech_active)
    }

    pub fn accept(&mut self, text: &str, speech_active: bool) -> bool {
        if !self.config.enabled {
            return true;
        }
        let normalized = normalize_transcript_text(text);
        if normalized.is_empty() {
            return false;
        }
        if speech_active {
            return true;
        }
        if normalized.chars().count() >= self.config.min_chars_when_silent as usize {
            return true;
        }
        if let Some(last) = self.last_reject
            && last.elapsed() < Duration::from_millis(self.config.cooldown_ms as u64)
        {
            return false;
        }
        self.last_reject = Some(Instant::now());
        false
    }
}

impl From<&crate::config::LocalAsrRecognitionConfig> for HallucinationFilterConfig {
    fn from(value: &crate::config::LocalAsrRecognitionConfig) -> Self {
        Self {
            enabled: value.hallucination_filter_enabled,
            min_chars_when_silent: value.hallucination_min_chars,
            cooldown_ms: value.hallucination_cooldown_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_asr_constants::SHORT_HALLUCINATION_TOKENS;

    #[test]
    fn rejects_short_text_in_silence() {
        let mut filter = HallucinationFilter::new(HallucinationFilterConfig {
            min_chars_when_silent: 5,
            ..HallucinationFilterConfig::default()
        });
        assert!(!filter.accept("hi", false));
        assert!(filter.accept("hello world", false));
    }

    #[test]
    fn allows_short_text_during_speech() {
        let mut filter = HallucinationFilter::new(HallucinationFilterConfig::default());
        assert!(filter.accept("ok", true));
    }

    #[test]
    fn drops_empty_short_hallucination() {
        assert!(should_drop_short_hallucination("", 100, false));
    }

    #[test]
    fn drops_short_yeah_within_limit() {
        assert!(should_drop_short_hallucination("yeah", 500, false));
        assert!(should_drop_short_hallucination("Yeah.", 500, false));
    }

    #[test]
    fn keeps_yeah_when_duration_too_long() {
        assert!(!should_drop_short_hallucination("yeah", 2000, false));
    }

    #[test]
    fn keeps_non_token_text() {
        assert!(!should_drop_short_hallucination("hello world", 200, true));
    }

    #[test]
    fn accept_transcript_drops_yeah_in_silence() {
        let mut filter = HallucinationFilter::default();
        assert!(!filter.accept_transcript("yeah", 500, false, false));
        assert!(filter.accept_transcript("hello world", 500, false, false));
    }

    #[test]
    fn every_hallucination_token_drops_within_partial_limit() {
        for token in SHORT_HALLUCINATION_TOKENS {
            assert!(
                should_drop_short_hallucination(token, 500, false),
                "expected drop for token {token:?}"
            );
        }
    }

    #[test]
    fn every_hallucination_token_keeps_beyond_partial_limit() {
        for token in SHORT_HALLUCINATION_TOKENS {
            assert!(
                !should_drop_short_hallucination(token, 2000, false),
                "expected keep for token {token:?}"
            );
        }
    }

    #[test]
    fn every_hallucination_token_drops_within_final_limit() {
        for token in SHORT_HALLUCINATION_TOKENS {
            assert!(
                should_drop_short_hallucination(token, 800, true),
                "expected final drop for token {token:?}"
            );
        }
    }

    #[test]
    fn token_match_is_case_insensitive() {
        assert!(should_drop_short_hallucination("OK", 400, false));
        assert!(should_drop_short_hallucination("  Yeah.  ", 400, false));
    }

    #[test]
    fn token_match_strips_exclamation() {
        assert!(should_drop_short_hallucination("yeah!", 400, false));
        assert!(should_drop_short_hallucination("Okay…", 400, true));
    }
}
