use std::time::Instant;

#[derive(Debug, Clone, Copy)]
pub struct PartialEmitInput<'a> {
    pub new_text: &'a str,
    pub previous_text: &'a str,
    pub mode: &'a str,
    pub min_new_words: u32,
    pub min_delta_chars: u32,
    pub coalescing_ms: u32,
    pub previous_emit: Option<Instant>,
    pub now: Instant,
}

pub fn normalize_transcript_text(value: &str) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    collapse_space_before_ascii_punct(&collapsed)
}

/// Collapse `word .` / `word ,` style gaps that break word_growth diffs.
fn collapse_space_before_ascii_punct(value: &str) -> String {
    let mut out = value.to_string();
    for (spaced, bare) in [
        (" .", "."),
        (" ,", ","),
        (" !", "!"),
        (" ?", "?"),
        (" ;", ";"),
        (" :", ":"),
        (" …", "…"),
    ] {
        while out.contains(spaced) {
            out = out.replace(spaced, bare);
        }
    }
    out
}

pub fn split_words(text: &str) -> Vec<String> {
    let normalized = normalize_transcript_text(text);
    if normalized.is_empty() {
        return Vec::new();
    }
    normalized.split(' ').map(str::to_string).collect()
}

pub fn should_emit_partial(input: PartialEmitInput<'_>) -> bool {
    let PartialEmitInput {
        new_text,
        previous_text,
        mode,
        min_new_words,
        min_delta_chars,
        coalescing_ms,
        previous_emit,
        now,
    } = input;
    let new_norm = normalize_transcript_text(new_text);
    if new_norm.is_empty() {
        return false;
    }
    let prev_norm = normalize_transcript_text(previous_text);
    if new_norm == prev_norm {
        return false;
    }

    let emit_mode = mode.trim().to_ascii_lowercase();
    if emit_mode == "char_delta" {
        return should_emit_char_delta(
            &new_norm,
            &prev_norm,
            min_delta_chars,
            coalescing_ms,
            previous_emit,
            now,
        );
    }

    if !should_emit_word_growth(&new_norm, &prev_norm, min_new_words.max(1)) {
        return false;
    }
    // Quality-style coalesce: at most one word_growth emit per window.
    // First partial (empty previous) always passes.
    if coalescing_ms == 0 || prev_norm.is_empty() {
        return true;
    }
    match previous_emit {
        None => true,
        Some(prev) => now.duration_since(prev).as_millis() as u32 >= coalescing_ms,
    }
}

fn should_emit_char_delta(
    new_norm: &str,
    prev_norm: &str,
    min_delta_chars: u32,
    coalescing_ms: u32,
    previous_emit: Option<Instant>,
    now: Instant,
) -> bool {
    let growth_chars = new_norm
        .chars()
        .count()
        .saturating_sub(prev_norm.chars().count()) as i64;
    if prev_norm.is_empty() {
        return true;
    }
    if min_delta_chars == 0 && coalescing_ms == 0 {
        return true;
    }
    if growth_chars < 0 {
        return true;
    }
    if min_delta_chars > 0 && growth_chars >= min_delta_chars as i64 {
        return true;
    }
    if coalescing_ms > 0
        && let Some(previous_emit) = previous_emit
        && growth_chars >= 0
        && (min_delta_chars == 0 || growth_chars < min_delta_chars as i64)
    {
        let elapsed_ms = now.duration_since(previous_emit).as_millis() as u32;
        return elapsed_ms >= coalescing_ms;
    }
    growth_chars > 0
}

fn should_emit_word_growth(new_norm: &str, prev_norm: &str, min_new_words: u32) -> bool {
    // SST `realtime_transcript_emit_policy._should_emit_word_growth` parity.
    let new_words = split_words(new_norm);
    let prev_words = split_words(prev_norm);

    if prev_words.is_empty() {
        return new_words.len() >= min_new_words as usize;
    }
    if new_words.len() < prev_words.len() {
        return false;
    }
    if new_words.len() == prev_words.len() {
        return new_words != prev_words;
    }
    // Prefix break (hypothesis rewrite) — SST still emits.
    if new_words[..prev_words.len()] != prev_words[..] {
        return true;
    }
    let added = new_words.len() - prev_words.len();
    added >= min_new_words as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emit_input<'a>(
        new_text: &'a str,
        previous_text: &'a str,
        mode: &'a str,
        min_new_words: u32,
    ) -> PartialEmitInput<'a> {
        PartialEmitInput {
            new_text,
            previous_text,
            mode,
            min_new_words,
            min_delta_chars: 0,
            coalescing_ms: 0,
            previous_emit: None,
            now: Instant::now(),
        }
    }

    #[test]
    fn word_growth_requires_new_words() {
        assert!(!should_emit_partial(emit_input(
            "hello",
            "hello",
            "word_growth",
            1
        )));
        assert!(should_emit_partial(emit_input(
            "hello world",
            "",
            "word_growth",
            1
        )));
        assert!(!should_emit_partial(emit_input(
            "hello there",
            "hello",
            "word_growth",
            2,
        )));
    }

    #[test]
    fn word_growth_allows_prefix_break_like_sst() {
        // SST: when len grows and shared prefix breaks → emit True.
        // Shortening is still rejected.
        assert!(!should_emit_partial(emit_input(
            "yeah",
            "hello world",
            "word_growth",
            1
        )));
        assert!(should_emit_partial(emit_input(
            "something else entirely",
            "hello world",
            "word_growth",
            1
        )));
        assert!(should_emit_partial(emit_input(
            "Какие выводы можно",
            "Какие вы?",
            "word_growth",
            1
        )));
    }

    #[test]
    fn word_growth_allows_last_token_revision() {
        assert!(should_emit_partial(emit_input(
            "hello there",
            "hello world",
            "word_growth",
            1
        )));
    }

    #[test]
    fn char_delta_emits_on_growth() {
        let mut input = emit_input("hello", "", "char_delta", 1);
        input.min_delta_chars = 1;
        assert!(should_emit_partial(input));
    }

    #[test]
    fn word_growth_respects_coalescing_window() {
        let t0 = Instant::now();
        let mut held = emit_input("hello world", "hello", "word_growth", 1);
        held.coalescing_ms = 80;
        held.previous_emit = Some(t0);
        held.now = t0;
        assert!(!should_emit_partial(held));

        let mut ready = emit_input("hello world", "hello", "word_growth", 1);
        ready.coalescing_ms = 80;
        ready.previous_emit = Some(t0);
        ready.now = t0 + std::time::Duration::from_millis(80);
        assert!(should_emit_partial(ready));
    }

    #[test]
    fn word_growth_first_partial_ignores_coalescing() {
        let t0 = Instant::now();
        let mut input = emit_input("hello", "", "word_growth", 1);
        input.coalescing_ms = 80;
        input.previous_emit = Some(t0);
        input.now = t0;
        assert!(should_emit_partial(input));
    }

    #[test]
    fn normalize_collapses_space_before_punct() {
        assert_eq!(normalize_transcript_text("hello  ,  world ?"), "hello, world?");
        assert_eq!(normalize_transcript_text("  a   b  "), "a b");
    }
}
