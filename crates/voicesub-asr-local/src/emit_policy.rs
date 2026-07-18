//! SST `realtime_transcript_emit_policy` — downstream emit filtering.
//!
//! SST Local ASR gates partials only via `PartialEmitCoordinator` / `word_growth`.
//! This policy is a thin duplicate suppressor (identical text) — **not** a second
//! monotonic gate. A strict prefix-lock blocked legitimate Parakeet revisions
//! (`"Какие вы?"` → `"Какие выводы…"`) and froze UI until Final.

use std::collections::HashMap;

use voicesub_partial_emit::normalize_transcript_text;

use crate::pipeline::PipelineEmit;

#[derive(Debug, Default)]
pub struct RealtimeEmitPolicy {
    last_partial_by_segment: HashMap<String, String>,
}

impl RealtimeEmitPolicy {
    pub fn reset(&mut self) {
        self.last_partial_by_segment.clear();
    }

    pub fn clear_segment(&mut self, segment_id: &str) {
        self.last_partial_by_segment.remove(segment_id);
    }

    pub fn last_partial(&self, segment_id: &str) -> &str {
        self.last_partial_by_segment
            .get(segment_id)
            .map(String::as_str)
            .unwrap_or("")
    }

    pub fn apply(&mut self, emits: Vec<PipelineEmit>) -> Vec<PipelineEmit> {
        let mut out = Vec::with_capacity(emits.len());
        for emit in emits {
            if emit.is_final {
                self.last_partial_by_segment.remove(&emit.segment_id);
                out.push(emit);
                continue;
            }
            let duplicate = self
                .last_partial_by_segment
                .get(&emit.segment_id)
                .is_some_and(|prev| prev == &emit.text);
            if duplicate {
                continue;
            }
            self.last_partial_by_segment
                .insert(emit.segment_id.clone(), emit.text.clone());
            out.push(emit);
        }
        out
    }
}

/// Collapse exact duplicated halves (common when streaming final re-feeds audio).
pub fn dedupe_repeated_transcript(text: &str) -> String {
    let norm = normalize_transcript_text(text);
    let words: Vec<&str> = norm.split(' ').filter(|w| !w.is_empty()).collect();
    if words.len() >= 2 && words.len() % 2 == 0 {
        let mid = words.len() / 2;
        if words[..mid] == words[mid..] {
            return words[..mid].join(" ");
        }
    }
    norm
}

/// Prefer a longer last partial when Final is a shorter rewrite (C1 — don't clip the phrase).
pub fn prefer_final_text(last_partial: Option<&str>, final_text: &str) -> String {
    let final_norm = normalize_transcript_text(final_text);
    let Some(prev) = last_partial.map(normalize_transcript_text) else {
        return final_norm;
    };
    if prev.is_empty() {
        return final_norm;
    }
    if final_norm.starts_with(&prev) || final_norm.len() >= prev.len() {
        return final_norm;
    }
    prev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn partial(segment: &str, text: &str) -> PipelineEmit {
        PipelineEmit {
            segment_id: segment.into(),
            revision: 1,
            text: text.into(),
            is_final: false,
            is_speech: true,
        }
    }

    fn fin(segment: &str, text: &str) -> PipelineEmit {
        PipelineEmit {
            segment_id: segment.into(),
            revision: 1,
            text: text.into(),
            is_final: true,
            is_speech: false,
        }
    }

    #[test]
    fn dedups_identical_partials() {
        let mut policy = RealtimeEmitPolicy::default();
        let first = policy.apply(vec![partial("s1", "hello")]);
        assert_eq!(first.len(), 1);
        let second = policy.apply(vec![partial("s1", "hello")]);
        assert!(second.is_empty());
    }

    #[test]
    fn final_clears_partial_cache() {
        let mut policy = RealtimeEmitPolicy::default();
        let _ = policy.apply(vec![partial("s1", "hello world")]);
        let _ = policy.apply(vec![fin("s1", "hello world.")]);
        let again = policy.apply(vec![partial("s1", "hello")]);
        assert_eq!(again.len(), 1);
    }

    #[test]
    fn allows_hypothesis_revision_during_growth() {
        let mut policy = RealtimeEmitPolicy::default();
        let _ = policy.apply(vec![partial("s1", "Какие вы?")]);
        let grow = policy.apply(vec![partial("s1", "Какие выводы можно")]);
        assert_eq!(grow.len(), 1);
        assert_eq!(grow[0].text, "Какие выводы можно");
    }

    #[test]
    fn allows_non_prefix_rewrite() {
        let mut policy = RealtimeEmitPolicy::default();
        let _ = policy.apply(vec![partial("s1", "hello world")]);
        let rewrite = policy.apply(vec![partial("s1", "yeah")]);
        assert_eq!(rewrite.len(), 1);
    }

    #[test]
    fn allows_monotonic_growth() {
        let mut policy = RealtimeEmitPolicy::default();
        let _ = policy.apply(vec![partial("s1", "hello")]);
        let grow = policy.apply(vec![partial("s1", "hello world")]);
        assert_eq!(grow.len(), 1);
    }

    #[test]
    fn allows_case_only_prefix_growth() {
        let mut policy = RealtimeEmitPolicy::default();
        let _ = policy.apply(vec![partial("s1", "он все")]);
        let grow = policy.apply(vec![partial("s1", "Он все равно")]);
        assert_eq!(grow.len(), 1);
        assert_eq!(grow[0].text, "Он все равно");
    }

    #[test]
    fn dedupes_repeated_final_halves() {
        let dup = "Он все равно не хочет такой уводить. Он все равно не хочет такой уводить.";
        let once = dedupe_repeated_transcript(dup);
        assert_eq!(once, "Он все равно не хочет такой уводить.");
    }

    #[test]
    fn prefer_final_keeps_longer_partial() {
        assert_eq!(
            prefer_final_text(Some("hello world"), "yeah"),
            "hello world"
        );
        assert_eq!(
            prefer_final_text(Some("hello"), "hello world"),
            "hello world"
        );
    }
}
