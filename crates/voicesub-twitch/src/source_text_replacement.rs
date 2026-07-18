use std::sync::{Arc, Mutex, OnceLock};

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::Value;

use crate::settings::TwitchTtsSettings;

const BUILTIN_PAIRS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/source_text_builtin_pairs.json"
));
const BUILTIN_STEMS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../data/source_text_builtin_stems.json"
));

/// Builtin / default censor: `fuck` → `f*ck` (keep first char, star the second, keep the rest).
fn mask_profanity_token(token: &str) -> String {
    // Already masked forms like `f*ck` are left alone — that is the intended display, not an evasion.
    if token.contains('*') {
        return token.to_string();
    }
    let chars: Vec<char> = token.chars().collect();
    match chars.len() {
        0 => String::new(),
        1 => "*".to_string(),
        _ => {
            let mut out = String::with_capacity(token.len());
            out.push(chars[0]);
            out.push('*');
            out.extend(chars[2..].iter().copied());
            out
        }
    }
}

fn mask_profanity_span(span: &str) -> String {
    let tokens: Vec<&str> = span.split_whitespace().collect();
    // Spaced singles in the original (`х у й`) were joined for matching — mask as one word.
    if tokens.len() >= 2 && tokens.iter().all(|t| t.chars().count() == 1) {
        let joined: String = tokens.concat();
        return mask_profanity_token(&joined);
    }

    let mut out = String::with_capacity(span.len());
    let mut token = String::new();
    for ch in span.chars() {
        if ch.is_whitespace() {
            if !token.is_empty() {
                out.push_str(&mask_profanity_token(&token));
                token.clear();
            }
            out.push(ch);
        } else {
            token.push(ch);
        }
    }
    if !token.is_empty() {
        out.push_str(&mask_profanity_token(&token));
    }
    out
}

fn replacement_for_match(matched: &str, configured_target: &str) -> String {
    if configured_target.is_empty() || configured_target == "***" {
        mask_profanity_span(matched)
    } else {
        configured_target.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceTextReplacementPair {
    pub source: String,
    #[serde(default)]
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SourceTextReplacementSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true", alias = "include_builtin_profanity")]
    pub include_builtin: bool,
    #[serde(default = "default_true")]
    pub case_insensitive: bool,
    #[serde(default = "default_true", alias = "whole_word_only")]
    pub whole_words: bool,
    #[serde(default)]
    pub pairs: Vec<SourceTextReplacementPair>,
}

impl Default for SourceTextReplacementSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StemMode {
    Infix,
    Token,
}

#[derive(Debug, Clone, Deserialize)]
struct BuiltinStem {
    stem: String,
    mode: StemMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineKey {
    include_builtin: bool,
    case_insensitive: bool,
    whole_words: bool,
    pairs: Vec<(String, String)>,
}

struct CompiledPair {
    regex: Regex,
    target: String,
}

struct StemAutomaton {
    ac: AhoCorasick,
    modes: Vec<StemMode>,
}

struct LiteralAutomaton {
    ac: AhoCorasick,
    /// Parallel to AC patterns: true = require Unicode word boundaries (Latin/Cyrillic).
    need_boundary: Vec<bool>,
}

struct ReplacementEngine {
    custom: Vec<CompiledPair>,
    /// Multi-word phrases only (few patterns).
    builtin_phrases: Vec<CompiledPair>,
    /// Single-token / CJK literals matched via Aho-Corasick.
    builtin_literals: Option<LiteralAutomaton>,
    stems: Option<StemAutomaton>,
}

fn bool_field(obj: &serde_json::Map<String, Value>, keys: &[&str], default: bool) -> bool {
    for key in keys {
        if let Some(value) = obj.get(*key).and_then(|v| v.as_bool()) {
            return value;
        }
    }
    default
}

fn parse_pairs(value: Option<&Value>) -> Vec<SourceTextReplacementPair> {
    let Some(Value::Array(items)) = value else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in items.iter().take(100) {
        let Some(obj) = item.as_object() else {
            continue;
        };
        let source = obj
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if source.is_empty() {
            continue;
        }
        let target = obj
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        out.push(SourceTextReplacementPair {
            source: source.to_string(),
            target,
        });
    }
    out
}

/// Parse replacement settings from a config root (`source_text_replacement` section).
pub fn settings_from_config_value(config: &Value) -> SourceTextReplacementSettings {
    config
        .get("source_text_replacement")
        .map(settings_from_section_value)
        .unwrap_or_default()
}

/// Parse replacement settings from the `source_text_replacement` object.
pub fn settings_from_section_value(section: &Value) -> SourceTextReplacementSettings {
    let Some(obj) = section.as_object() else {
        return SourceTextReplacementSettings::default();
    };
    let whole_words = match (
        obj.get("whole_words").and_then(|v| v.as_bool()),
        obj.get("whole_word_only").and_then(|v| v.as_bool()),
    ) {
        (Some(false), _) | (_, Some(false)) => false,
        (Some(true), _) | (_, Some(true)) => true,
        _ => true,
    };
    SourceTextReplacementSettings {
        enabled: obj
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        include_builtin: bool_field(obj, &["include_builtin", "include_builtin_profanity"], true),
        case_insensitive: bool_field(obj, &["case_insensitive"], true),
        whole_words,
        pairs: parse_pairs(obj.get("pairs")),
    }
}

fn default_true() -> bool {
    true
}

fn builtin_pairs_raw() -> &'static [SourceTextReplacementPair] {
    static CACHE: OnceLock<Vec<SourceTextReplacementPair>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<Vec<SourceTextReplacementPair>>(BUILTIN_PAIRS_JSON)
            .unwrap_or_default()
            .into_iter()
            .filter(|pair| !pair.source.trim().is_empty())
            .collect()
    })
}

fn builtin_stems_raw() -> &'static [BuiltinStem] {
    static CACHE: OnceLock<Vec<BuiltinStem>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str::<Vec<BuiltinStem>>(BUILTIN_STEMS_JSON)
            .unwrap_or_default()
            .into_iter()
            .filter(|stem| !stem.stem.trim().is_empty())
            .collect()
    })
}

fn is_cjk(c: char) -> bool {
    matches!(
        c,
        '\u{3040}'..='\u{30FF}' // Hiragana / Katakana
            | '\u{3400}'..='\u{4DBF}' // CJK extension A
            | '\u{4E00}'..='\u{9FFF}' // CJK unified
            | '\u{F900}'..='\u{FAFF}' // CJK compatibility
            | '\u{AC00}'..='\u{D7AF}' // Hangul syllables
            | '\u{1100}'..='\u{11FF}' // Hangul jamo
            | '\u{3130}'..='\u{318F}' // Hangul compatibility jamo
    )
}

fn source_uses_cjk(source: &str) -> bool {
    source.chars().any(is_cjk)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || is_cjk(c)
}

fn fold_leet(c: char) -> Option<char> {
    let lower = c.to_lowercase().next().unwrap_or(c);
    match lower {
        '@' | '4' => Some('a'),
        '1' | '!' => Some('i'),
        '3' => Some('e'),
        '0' => Some('o'),
        '5' | '$' => Some('s'),
        '7' => Some('t'),
        // Keep `*` — `f*ck` is normal masking output, not an obfuscation to unwrap.
        '*' => Some('*'),
        '#' | '.' | ',' | '-' | '_' | '/' | '\\' | '\'' | '"' | '`' | '~' | '^' | '+' | '='
        | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ':' | ';' | '?' => None,
        '9' => Some('я'),
        c if c.is_alphabetic() || c.is_numeric() || is_cjk(c) => Some(c),
        c if c.is_whitespace() => Some(' '),
        _ => None,
    }
}

#[derive(Debug, Clone)]
struct NormalizedText {
    /// Obfuscation-folded text used for matching.
    text: String,
    /// For each char in `text`, byte range in the original string.
    orig_byte_ranges: Vec<(usize, usize)>,
}

fn normalize_for_match(input: &str) -> NormalizedText {
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    let mut folded: Vec<(char, usize, usize)> = Vec::with_capacity(chars.len());
    for (idx, &(byte_start, ch)) in chars.iter().enumerate() {
        let byte_end = chars
            .get(idx + 1)
            .map(|(next, _)| *next)
            .unwrap_or(input.len());
        match fold_leet(ch) {
            Some(' ') => folded.push((' ', byte_start, byte_end)),
            Some(c) => folded.push((c, byte_start, byte_end)),
            None => {} // strip separators inside tokens
        }
    }

    // Join spaced single-letter runs: "х у й" / "f u c k" → "хуй" / "fuck".
    let mut joined: Vec<(char, usize, usize)> = Vec::with_capacity(folded.len());
    let mut i = 0;
    while i < folded.len() {
        let (ch, start, end) = folded[i];
        if ch != ' ' && is_word_char(ch) {
            let mut letters = vec![(ch, start, end)];
            let mut j = i + 1;
            while j + 1 < folded.len() {
                if folded[j].0 != ' ' {
                    break;
                }
                let (next, nstart, nend) = folded[j + 1];
                if next == ' ' || !is_word_char(next) {
                    break;
                }
                // Only glue when both sides are single-char "tokens".
                let prev_single = true;
                let next_is_single = j + 2 >= folded.len() || folded[j + 2].0 == ' ';
                if !(prev_single && next_is_single && letters.last().map(|l| l.0).is_some()) {
                    break;
                }
                // Require the letter before the space to be a singleton chunk.
                letters.push((next, nstart, nend));
                j += 2;
            }
            if letters.len() >= 3 {
                for item in letters {
                    joined.push(item);
                }
                i = j;
                continue;
            }
        }
        joined.push((ch, start, end));
        i += 1;
    }

    // Collapse runs of the same letter: "суууука" → "сука" (keep 1).
    let mut text = String::new();
    let mut orig_byte_ranges = Vec::new();
    let mut last: Option<char> = None;
    let mut space_pending: Option<(usize, usize)> = None;
    for (ch, start, end) in joined {
        if ch == ' ' {
            space_pending = Some((start, end));
            continue;
        }
        if let Some((s, e)) = space_pending.take()
            && !text.is_empty()
        {
            text.push(' ');
            orig_byte_ranges.push((s, e));
            last = Some(' ');
        }
        if last == Some(ch) && ch.is_alphabetic() {
            // skip repeat; extend last range
            if let Some(range) = orig_byte_ranges.last_mut() {
                range.1 = end;
            }
            continue;
        }
        text.push(ch);
        orig_byte_ranges.push((start, end));
        last = Some(ch);
    }

    NormalizedText {
        text,
        orig_byte_ranges,
    }
}

fn compile_pair_regex(source: &str, case_insensitive: bool, whole_words: bool) -> Option<Regex> {
    if source.is_empty() {
        return None;
    }
    let escaped = regex::escape(source);
    // Rust `regex` lookbehind must be fixed-length, so `\p{L}` lookarounds cannot be used.
    // `\b` is Unicode-aware; CJK has no reliable word boundaries — match as substring.
    let pattern = if whole_words && !source_uses_cjk(source) {
        format!(r"\b{escaped}\b")
    } else {
        escaped
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(case_insensitive)
        .size_limit(1024 * 1024)
        .build()
        .ok()
}

fn compile_pairs(
    pairs: &[(String, String)],
    case_insensitive: bool,
    whole_words: bool,
) -> Vec<CompiledPair> {
    let mut out = Vec::with_capacity(pairs.len());
    for (source, target) in pairs {
        let Some(regex) = compile_pair_regex(source, case_insensitive, whole_words) else {
            continue;
        };
        out.push(CompiledPair {
            regex,
            target: target.clone(),
        });
    }
    out
}

fn normalize_custom_pairs(pairs: &[SourceTextReplacementPair]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for pair in pairs.iter().take(100) {
        let source = pair.source.trim();
        if source.is_empty() {
            continue;
        }
        out.push((source.to_string(), pair.target.clone()));
    }
    out
}

fn dedupe_key(source: &str, case_insensitive: bool) -> String {
    if case_insensitive {
        source.to_lowercase()
    } else {
        source.to_string()
    }
}

pub fn effective_replacement_pairs(
    settings: &SourceTextReplacementSettings,
) -> Vec<(String, String)> {
    if !settings.enabled {
        return Vec::new();
    }

    let mut by_key: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();
    for (source, target) in normalize_custom_pairs(&settings.pairs) {
        by_key.insert(
            dedupe_key(&source, settings.case_insensitive),
            (source, target),
        );
    }
    if settings.include_builtin {
        for pair in builtin_pairs_raw() {
            let source = pair.source.trim();
            if source.is_empty() {
                continue;
            }
            by_key
                .entry(dedupe_key(source, settings.case_insensitive))
                .or_insert_with(|| (source.to_string(), pair.target.clone()));
        }
    }

    let mut merged: Vec<(String, String)> = by_key.into_values().collect();
    merged.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
    merged
}

fn build_stem_automaton(case_insensitive: bool) -> Option<StemAutomaton> {
    let stems = builtin_stems_raw();
    if stems.is_empty() {
        return None;
    }
    let patterns: Vec<String> = stems
        .iter()
        .map(|s| {
            if case_insensitive {
                s.stem.to_lowercase()
            } else {
                s.stem.clone()
            }
        })
        .collect();
    let modes: Vec<StemMode> = stems.iter().map(|s| s.mode).collect();
    let ac = AhoCorasickBuilder::new()
        .ascii_case_insensitive(false)
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns)
        .ok()?;
    Some(StemAutomaton { ac, modes })
}

fn is_multi_word_phrase(source: &str) -> bool {
    source.split_whitespace().count() > 1
}

fn custom_overrides(source: &str, custom: &[(String, String)], case_insensitive: bool) -> bool {
    let key = dedupe_key(source, case_insensitive);
    custom
        .iter()
        .any(|(s, _)| dedupe_key(s, case_insensitive) == key)
}

fn build_literal_automaton(
    literals: &[(String, bool)],
    case_insensitive: bool,
) -> Option<LiteralAutomaton> {
    if literals.is_empty() {
        return None;
    }
    let patterns: Vec<String> = literals
        .iter()
        .map(|(source, _)| {
            if case_insensitive {
                source.to_lowercase()
            } else {
                source.clone()
            }
        })
        .collect();
    let need_boundary: Vec<bool> = literals.iter().map(|(_, b)| *b).collect();
    let ac = AhoCorasickBuilder::new()
        .ascii_case_insensitive(false)
        .match_kind(MatchKind::LeftmostLongest)
        .build(&patterns)
        .ok()?;
    Some(LiteralAutomaton { ac, need_boundary })
}

fn build_engine(settings: &SourceTextReplacementSettings) -> ReplacementEngine {
    let custom = normalize_custom_pairs(&settings.pairs);
    let mut custom_sorted = custom.clone();
    custom_sorted.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));

    let mut phrases = Vec::new();
    let mut literals = Vec::new();
    if settings.include_builtin {
        let mut seen = std::collections::HashSet::new();
        for pair in builtin_pairs_raw() {
            let source = pair.source.trim();
            if source.is_empty() {
                continue;
            }
            let key = dedupe_key(source, settings.case_insensitive);
            if !seen.insert(key) || custom_overrides(source, &custom, settings.case_insensitive) {
                continue;
            }
            if is_multi_word_phrase(source) {
                phrases.push((source.to_string(), pair.target.clone()));
            } else {
                let boundary = settings.whole_words && !source_uses_cjk(source);
                literals.push((source.to_string(), boundary));
            }
        }
        phrases.sort_by_key(|pair| std::cmp::Reverse(pair.0.len()));
        // Longer literals first for LeftmostLongest stability.
        literals.sort_by_key(|(source, _)| std::cmp::Reverse(source.len()));
    }

    ReplacementEngine {
        custom: compile_pairs(
            &custom_sorted,
            settings.case_insensitive,
            settings.whole_words,
        ),
        builtin_phrases: compile_pairs(&phrases, settings.case_insensitive, settings.whole_words),
        builtin_literals: if settings.include_builtin {
            build_literal_automaton(&literals, settings.case_insensitive)
        } else {
            None
        },
        stems: if settings.include_builtin {
            build_stem_automaton(settings.case_insensitive)
        } else {
            None
        },
    }
}

type EngineCacheEntry = Option<(EngineKey, Arc<ReplacementEngine>)>;
type EngineCache = Mutex<EngineCacheEntry>;

fn engine_cache() -> &'static EngineCache {
    static CACHE: OnceLock<EngineCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn engine_for(settings: &SourceTextReplacementSettings) -> Arc<ReplacementEngine> {
    let key = EngineKey {
        include_builtin: settings.include_builtin,
        case_insensitive: settings.case_insensitive,
        whole_words: settings.whole_words,
        pairs: normalize_custom_pairs(&settings.pairs),
    };
    let mut guard = engine_cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some((cached_key, engine)) = guard.as_ref()
        && cached_key == &key
    {
        return Arc::clone(engine);
    }
    let engine = Arc::new(build_engine(settings));
    *guard = Some((key, Arc::clone(&engine)));
    engine
}

fn apply_compiled_pairs(text: &str, pairs: &[CompiledPair]) -> String {
    let mut result = text.to_string();
    for pair in pairs {
        let target = pair.target.clone();
        result = pair
            .regex
            .replace_all(&result, |caps: &regex::Captures| {
                replacement_for_match(&caps[0], &target)
            })
            .into_owned();
    }
    result
}

fn norm_index_to_orig_bytes(
    norm: &NormalizedText,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    if start >= end || end > norm.orig_byte_ranges.len() {
        return None;
    }
    let from = norm.orig_byte_ranges[start].0;
    let to = norm.orig_byte_ranges[end - 1].1;
    Some((from, to))
}

fn expand_token_char_range(text: &str, start: usize, end: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    if start >= chars.len() {
        return (start, end);
    }
    let mut lo = start;
    let mut hi = end.min(chars.len());
    while lo > 0 && is_word_char(chars[lo - 1]) {
        lo -= 1;
    }
    while hi < chars.len() && is_word_char(chars[hi]) {
        hi += 1;
    }
    (lo, hi)
}

fn is_token_bounded(text: &str, start: usize, end: usize) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if start >= chars.len() || end > chars.len() || start >= end {
        return false;
    }
    let left_ok = start == 0 || !is_word_char(chars[start - 1]);
    let right_ok = end == chars.len() || !is_word_char(chars[end]);
    left_ok && right_ok
}

fn merge_byte_spans(mut spans: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if spans.is_empty() {
        return spans;
    }
    spans.sort_by_key(|span| span.0);
    let mut out = Vec::with_capacity(spans.len());
    let mut cur = spans[0];
    for span in spans.into_iter().skip(1) {
        if span.0 <= cur.1 {
            cur.1 = cur.1.max(span.1);
        } else {
            out.push(cur);
            cur = span;
        }
    }
    out.push(cur);
    out
}

fn apply_literals(text: &str, literals: &LiteralAutomaton, case_insensitive: bool) -> String {
    let haystack = if case_insensitive {
        text.to_lowercase()
    } else {
        text.to_string()
    };
    let chars: Vec<char> = text.chars().collect();
    let mut byte_of_char: Vec<usize> = Vec::with_capacity(chars.len() + 1);
    let mut byte = 0;
    for ch in &chars {
        byte_of_char.push(byte);
        byte += ch.len_utf8();
    }
    byte_of_char.push(byte);

    let mut spans = Vec::new();
    for mat in literals.ac.find_iter(&haystack) {
        let need_boundary = literals
            .need_boundary
            .get(mat.pattern().as_usize())
            .copied()
            .unwrap_or(false);
        let start_char = haystack[..mat.start()].chars().count();
        let end_char = start_char + haystack[mat.start()..mat.end()].chars().count();
        if need_boundary && !is_token_bounded(text, start_char, end_char) {
            continue;
        }
        if start_char >= end_char || end_char > chars.len() {
            continue;
        }
        spans.push((byte_of_char[start_char], byte_of_char[end_char]));
    }
    if spans.is_empty() {
        return text.to_string();
    }
    replace_byte_spans(text, merge_byte_spans(spans))
}

fn replace_byte_spans(text: &str, spans: Vec<(usize, usize)>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    for (start, end) in spans {
        if start < cursor || end > text.len() || start >= end {
            continue;
        }
        let matched = &text[start..end];
        out.push_str(&text[cursor..start]);
        // Leave intentional masks like `f*ck` untouched.
        if matched.contains('*') {
            out.push_str(matched);
        } else {
            out.push_str(&mask_profanity_span(matched));
        }
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    out
}

fn apply_stems(text: &str, stems: &StemAutomaton, case_insensitive: bool) -> String {
    let norm = normalize_for_match(text);
    if norm.text.is_empty() {
        return text.to_string();
    }
    let haystack = if case_insensitive {
        norm.text.to_lowercase()
    } else {
        norm.text.clone()
    };

    let mut spans = Vec::new();
    for mat in stems.ac.find_iter(&haystack) {
        let mode = stems
            .modes
            .get(mat.pattern().as_usize())
            .copied()
            .unwrap_or(StemMode::Infix);
        // Byte offsets → char indices (Unicode lowercase can change byte length).
        let start_char = haystack[..mat.start()].chars().count();
        let end_char = start_char + haystack[mat.start()..mat.end()].chars().count();
        let (tok_start, tok_end) = match mode {
            StemMode::Infix => expand_token_char_range(&norm.text, start_char, end_char),
            StemMode::Token => {
                if !is_token_bounded(&norm.text, start_char, end_char) {
                    continue;
                }
                (start_char, end_char)
            }
        };
        if let Some(byte_span) = norm_index_to_orig_bytes(&norm, tok_start, tok_end) {
            spans.push(byte_span);
        }
    }

    if spans.is_empty() {
        return text.to_string();
    }
    replace_byte_spans(text, merge_byte_spans(spans))
}

impl ReplacementEngine {
    fn apply(&self, text: &str, case_insensitive: bool) -> String {
        if text.is_empty() {
            return text.to_string();
        }
        let mut result = apply_compiled_pairs(text, &self.custom);
        result = apply_compiled_pairs(&result, &self.builtin_phrases);
        if let Some(literals) = &self.builtin_literals {
            result = apply_literals(&result, literals, case_insensitive);
        }
        if let Some(stems) = &self.stems {
            result = apply_stems(&result, stems, case_insensitive);
        }
        result
    }
}

/// Apply find/replace rules. Prefer [`apply_source_text_replacement`] which caches compiles.
pub fn apply_replacement_rules(
    text: &str,
    pairs: &[(String, String)],
    case_insensitive: bool,
    whole_words: bool,
) -> String {
    if text.is_empty() || pairs.is_empty() {
        return text.to_string();
    }
    let compiled = compile_pairs(pairs, case_insensitive, whole_words);
    apply_compiled_pairs(text, &compiled)
}

pub fn apply_source_text_replacement(
    text: &str,
    settings: &SourceTextReplacementSettings,
) -> String {
    if !settings.enabled {
        return text.to_string();
    }
    if !settings.include_builtin && settings.pairs.is_empty() {
        return text.to_string();
    }
    engine_for(settings).apply(text, settings.case_insensitive)
}

pub fn profanity_settings_for_twitch(chat: &TwitchTtsSettings) -> SourceTextReplacementSettings {
    SourceTextReplacementSettings {
        enabled: true,
        include_builtin: chat.include_builtin_profanity,
        case_insensitive: true,
        whole_words: true,
        pairs: Vec::new(),
    }
}

/// Builtin profanity list only (`include_builtin_profanity` on Twitch settings).
pub fn apply_builtin_profanity(text: &str, settings: &SourceTextReplacementSettings) -> String {
    if !settings.include_builtin {
        return text.to_string();
    }
    let effective = SourceTextReplacementSettings {
        enabled: true,
        include_builtin: true,
        case_insensitive: settings.case_insensitive,
        whole_words: settings.whole_words,
        pairs: Vec::new(),
    };
    engine_for(&effective).apply(text, effective.case_insensitive)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn settings_with_pairs(pairs: Vec<(&str, &str)>) -> SourceTextReplacementSettings {
        SourceTextReplacementSettings {
            enabled: true,
            include_builtin: false,
            case_insensitive: true,
            whole_words: true,
            pairs: pairs
                .into_iter()
                .map(|(source, target)| SourceTextReplacementPair {
                    source: source.into(),
                    target: target.into(),
                })
                .collect(),
        }
    }

    fn builtin_on() -> SourceTextReplacementSettings {
        SourceTextReplacementSettings {
            enabled: true,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: Vec::new(),
        }
    }

    #[test]
    fn whole_word_pattern_compiles_and_matches() {
        let rx = RegexBuilder::new(r"\bbad\b")
            .case_insensitive(true)
            .build()
            .expect("whole-word pattern should compile");
        assert_eq!(rx.replace("BAD word", "X"), "X word");
    }

    #[test]
    fn disabled_returns_original() {
        let settings = SourceTextReplacementSettings {
            enabled: false,
            ..Default::default()
        };
        assert_eq!(
            apply_source_text_replacement("bad word", &settings),
            "bad word"
        );
    }

    #[test]
    fn replaces_whole_word_case_insensitive() {
        let settings = settings_with_pairs(vec![("bad", "X")]);
        assert_eq!(
            apply_source_text_replacement("BAD word", &settings),
            "X word"
        );
    }

    #[test]
    fn builtin_profanity_replaces_known_term() {
        let out = apply_source_text_replacement("what the fuck", &builtin_on());
        assert_eq!(out, "what the f*ck");
    }

    #[test]
    fn mask_keeps_first_stars_second() {
        assert_eq!(mask_profanity_token("fuck"), "f*ck");
        assert_eq!(mask_profanity_token("FUCK"), "F*CK");
        assert_eq!(mask_profanity_token("хуйня"), "х*йня");
        assert_eq!(mask_profanity_token("f*ck"), "f*ck");
        assert_eq!(mask_profanity_span("kiss my ass"), "k*ss m* a*s");
    }

    #[test]
    fn settings_from_ui_aliases_enable_builtin_and_custom_pairs() {
        let settings = settings_from_config_value(&serde_json::json!({
            "source_text_replacement": {
                "enabled": true,
                "include_builtin_profanity": true,
                "whole_word_only": true,
                "pairs": [{"source": "bad", "target": "X"}]
            }
        }));
        assert!(settings.enabled);
        assert!(settings.include_builtin);
        assert!(settings.whole_words);
        assert_eq!(
            apply_source_text_replacement("BAD word", &settings),
            "X word"
        );
    }

    #[test]
    fn settings_from_section_respects_disabled_builtin_flag() {
        let settings = settings_from_config_value(&serde_json::json!({
            "source_text_replacement": {
                "enabled": true,
                "include_builtin_profanity": false,
                "pairs": [{"source": "bad", "target": "X"}]
            }
        }));
        assert!(!settings.include_builtin);
        assert_eq!(
            apply_source_text_replacement("what the fuck", &settings),
            "what the fuck"
        );
        assert_eq!(apply_source_text_replacement("bad", &settings), "X");
    }

    #[test]
    fn builtin_cjk_profanity_replaced_with_default_whole_words() {
        let settings = builtin_on();
        assert_eq!(
            apply_source_text_replacement("これは くそ だ", &settings),
            "これは く* だ"
        );
        assert_eq!(
            apply_source_text_replacement("これはくそだ", &settings),
            "これはく*だ"
        );
        // Two-syllable Hangul → first + '*' (same rule as `くそ` → `く*`).
        assert_eq!(
            apply_source_text_replacement("아 씨발 진짜", &settings),
            "아 씨* 진짜"
        );
        assert_eq!(
            apply_source_text_replacement("你他妈的", &settings),
            "你他*的"
        );
    }

    #[test]
    fn deserializes_ui_field_aliases() {
        let settings: SourceTextReplacementSettings = serde_json::from_value(serde_json::json!({
            "enabled": true,
            "include_builtin_profanity": true,
            "case_insensitive": true,
            "whole_word_only": true,
            "pairs": [{"source": "bad", "target": "X"}]
        }))
        .expect("ui-shaped config should deserialize");
        assert!(settings.enabled);
        assert!(settings.include_builtin);
        assert!(settings.whole_words);
        assert_eq!(
            apply_source_text_replacement("BAD word", &settings),
            "X word"
        );
    }

    #[test]
    fn custom_overrides_builtin() {
        let settings = SourceTextReplacementSettings {
            enabled: true,
            include_builtin: true,
            case_insensitive: true,
            whole_words: true,
            pairs: vec![SourceTextReplacementPair {
                source: "fuck".into(),
                target: "duck".into(),
            }],
        };
        assert_eq!(apply_source_text_replacement("fuck", &settings), "duck");
    }

    #[test]
    fn russian_stems_catch_derivatives() {
        let settings = builtin_on();
        assert_eq!(
            apply_source_text_replacement("переебался уже", &settings),
            "п*реебался уже"
        );
        assert_eq!(
            apply_source_text_replacement("какая хуйня", &settings),
            "какая х*йня"
        );
        assert_eq!(
            apply_source_text_replacement("спиздил всё", &settings),
            "с*издил всё"
        );
    }

    #[test]
    fn russian_false_positives_kept() {
        let settings = builtin_on();
        assert_eq!(
            apply_source_text_replacement("синее небо", &settings),
            "синее небо"
        );
        assert_eq!(
            apply_source_text_replacement("сто рублей", &settings),
            "сто рублей"
        );
        assert_eq!(
            apply_source_text_replacement("всё хорошо", &settings),
            "всё хорошо"
        );
    }

    #[test]
    fn english_token_ass_does_not_hit_class() {
        let settings = builtin_on();
        assert_eq!(
            apply_source_text_replacement("first class pass", &settings),
            "first class pass"
        );
        assert_eq!(
            apply_source_text_replacement("kiss my ass", &settings),
            "k*ss m* a*s"
        );
    }

    #[test]
    fn already_masked_fstarck_is_left_alone() {
        let settings = builtin_on();
        // `f*ck` is the normal censor form — do not treat `*` as an evasion to unwrap.
        assert_eq!(
            apply_source_text_replacement("f*ck this", &settings),
            "f*ck this"
        );
        assert_eq!(
            apply_source_text_replacement("what the f*ck", &settings),
            "what the f*ck"
        );
    }

    #[test]
    fn other_obfuscation_still_masked() {
        let settings = builtin_on();
        assert_eq!(
            apply_source_text_replacement("sh1t happens", &settings),
            "s*1t happens"
        );
        assert_eq!(apply_source_text_replacement("х у й", &settings), "х*й");
        assert_eq!(
            apply_source_text_replacement("суууука", &settings),
            "с*ууука"
        );
    }

    #[test]
    fn cached_engine_stays_fast_on_hot_path() {
        let settings = builtin_on();
        // Warm cache.
        let _ = apply_source_text_replacement("warmup fuck", &settings);
        let t0 = Instant::now();
        for _ in 0..200 {
            let _ = apply_source_text_replacement("what the fuck bad word hello world", &settings);
        }
        let elapsed = t0.elapsed();
        // Debug builds are slower; release is ~10× faster. Guard against regex-per-call regression.
        assert!(
            elapsed.as_millis() < 800,
            "expected <800ms for 200 cached applies, got {elapsed:?}"
        );
    }
}
