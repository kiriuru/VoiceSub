use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};
use std::sync::OnceLock;
use whatlang::{Lang, Script};

/// Top-20 localization / Twitch chat languages (ISO 639-1).
/// zh covers both simplified (zh-Hans) and traditional (zh-Hant) via script heuristics + Lingua Chinese.
pub const TWITCH_TOP_LANGUAGE_CODES: &[&str] = &[
    "en", "zh", "ru", "es", "pt", "de", "ko", "fr", "ja", "tr", "hi", "it", "ar", "pl", "id", "sv",
    "nl", "vi", "th",
];

/// Minimum whatlang confidence for normal-length chat lines (fallback only).
const RELIABLE_CONFIDENCE: f64 = 0.75;
/// Relaxed confidence for very short Latin lines (2–3 chars).
const SHORT_LATIN_CONFIDENCE: f64 = 0.55;
#[derive(Debug, Default, Clone, Copy)]
struct LatinDiacriticScores {
    german: u32,
    french: u32,
    spanish: u32,
    portuguese: u32,
    italian: u32,
    polish: u32,
    turkish: u32,
    czech: u32,
    romanian: u32,
    vietnamese: u32,
    nordic: u32,
}

pub fn resolve_message_language(text: &str, min_chars: usize, fallback: &str) -> String {
    if !has_meaningful_linguistic_content(text) {
        let fb = fallback.trim().to_ascii_lowercase();
        return if fb.is_empty() {
            "en".to_string()
        } else {
            fb
        };
    }
    detect_language_code(text, min_chars)
        .filter(|code| code != "und")
        .unwrap_or_else(|| {
            let fb = fallback.trim().to_ascii_lowercase();
            if fb.is_empty() {
                "en".to_string()
            } else {
                fb
            }
        })
}

/// True when text still carries speakable natural language (not link-only / URL garbage).
pub fn has_meaningful_linguistic_content(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    let without_links = crate::links::strip_links_from_text(trimmed);
    let without_label = strip_leading_speaker_label(&without_links);
    let without_mentions = strip_twitch_mentions(&without_label);
    let normalized = crate::emoji::normalize_whitespace(&without_mentions);
    if normalized.is_empty() {
        return false;
    }

    let words: Vec<&str> = normalized
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .collect();
    if words.is_empty() {
        return false;
    }

    if words.len() == 1 {
        let word = words[0];
        if crate::links::looks_like_url_token(word) || looks_like_url_garbage(word) {
            return false;
        }
    }

    if words
        .iter()
        .all(|word| crate::emoji::is_plain_decimal_token(word))
    {
        return true;
    }

    let alpha_count = normalized.chars().filter(|ch| ch.is_alphabetic()).count();
    alpha_count >= 2
}

fn looks_like_url_garbage(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    lower.contains("watch?v=")
        || lower.contains("youtu")
        || lower.contains("list=rd")
        || lower.contains("index=")
        || (lower.contains('=') && lower.contains('/') && token.len() > 12)
}

pub fn detect_language_code(text: &str, min_chars: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = clean_for_detection(trimmed);
    let char_count = cleaned.chars().count();
    if char_count == 0 {
        return None;
    }

    if char_count == 1 {
        return detect_single_letter(cleaned.chars().next()?);
    }

    if let Some(code) = detect_by_unicode_heuristics(&cleaned) {
        return Some(code);
    }

    if should_skip_statistical_detection(&cleaned) {
        return detect_by_script(&cleaned);
    }

    let stats_min = min_chars.max(2);
    if char_count >= stats_min {
        if let Some(code) = detect_with_lingua(&cleaned) {
            return Some(code);
        }
        if let Some(code) = detect_with_whatlang(&cleaned, RELIABLE_CONFIDENCE) {
            return Some(code);
        }
    } else if char_count >= 2 {
        if let Some(code) = detect_with_whatlang(&cleaned, SHORT_LATIN_CONFIDENCE) {
            return Some(code);
        }
    }

    detect_by_script(&cleaned)
}

fn clean_for_detection(text: &str) -> String {
    let without_links = crate::links::strip_links_from_text(text);
    let without_label = strip_leading_speaker_label(&without_links);
    let without_mentions = strip_twitch_mentions(&without_label);
    if !has_meaningful_linguistic_content(without_mentions.trim()) {
        return String::new();
    }
    let mut out = String::with_capacity(without_mentions.len());
    let mut prev_space = false;
    for ch in without_mentions.chars() {
        if ch.is_alphabetic() {
            out.push(ch);
            prev_space = false;
        } else if ch.is_whitespace() && !prev_space && !out.is_empty() {
            out.push(' ');
            prev_space = true;
        }
    }
    out.trim().to_string()
}

/// Strip `Speaker:` prefix common in chat lines that only share a link.
pub fn strip_leading_speaker_label(text: &str) -> String {
    let trimmed = text.trim();
    let Some((prefix, rest)) = trimmed.split_once(':') else {
        return trimmed.to_string();
    };
    let label = prefix.trim();
    if label.is_empty()
        || !label.chars().any(|ch| ch.is_alphabetic())
        || !label
            .chars()
            .all(|ch| ch.is_alphanumeric() || matches!(ch, '_' | '-'))
    {
        return trimmed.to_string();
    }
    rest.trim().to_string()
}

fn should_skip_statistical_detection(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.contains(' ') {
        return false;
    }
    trimmed.chars().all(|ch| ch.is_ascii_alphabetic())
}

/// Remove `@username` tokens so mentions do not skew language detection or TTS.
pub fn strip_twitch_mentions(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '@' {
            while matches!(chars.peek(), Some(next) if next.is_alphanumeric() || *next == '_') {
                chars.next();
            }
            continue;
        }
        out.push(ch);
    }
    crate::emoji::normalize_whitespace(&out)
}

fn detect_single_letter(ch: char) -> Option<String> {
    if is_cyrillic(ch) {
        return Some(if is_ukrainian_cyrillic(ch) {
            "uk".to_string()
        } else {
            "ru".to_string()
        });
    }
    if is_hiragana_or_katakana(ch) {
        return Some("ja".to_string());
    }
    if is_hangul_syllable(ch) {
        return Some("ko".to_string());
    }
    if is_kanji(ch) {
        return Some("zh".to_string());
    }
    if is_arabic(ch) {
        return Some("ar".to_string());
    }
    if is_hebrew(ch) {
        return Some("he".to_string());
    }
    if is_thai(ch) {
        return Some("th".to_string());
    }
    if is_devanagari(ch) {
        return Some("hi".to_string());
    }
    if score_latin_diacritic(ch) > 0 {
        return pick_latin_language(&LatinDiacriticScores {
            german: score_latin_diacritic(ch) * u32::from(matches_german(ch)),
            french: score_latin_diacritic(ch) * u32::from(matches_french(ch)),
            spanish: score_latin_diacritic(ch) * u32::from(matches_spanish(ch)),
            portuguese: score_latin_diacritic(ch) * u32::from(matches_portuguese(ch)),
            italian: score_latin_diacritic(ch) * u32::from(matches_italian(ch)),
            polish: score_latin_diacritic(ch) * u32::from(matches_polish(ch)),
            turkish: score_latin_diacritic(ch) * u32::from(matches_turkish(ch)),
            czech: score_latin_diacritic(ch) * u32::from(matches_czech(ch)),
            romanian: score_latin_diacritic(ch) * u32::from(matches_romanian(ch)),
            vietnamese: score_latin_diacritic(ch) * u32::from(matches_vietnamese(ch)),
            nordic: score_latin_diacritic(ch) * u32::from(matches_nordic(ch)),
        }, 1);
    }
    if ch.is_ascii_alphabetic() {
        return Some("en".to_string());
    }
    None
}

/// Fast Unicode pass inspired by XTTS Streamer.bot heuristics — O(n), no I/O.
fn detect_by_unicode_heuristics(text: &str) -> Option<String> {
    let total_chars = text.chars().count();
    if total_chars == 0 {
        return None;
    }

    let mut cyrillic_count = 0u32;
    let mut ukrainian_specific = 0u32;
    let mut kanji_count = 0u32;
    let mut korean_count = 0u32;
    let mut latin_count = 0u32;
    let mut arabic_count = 0u32;
    let mut thai_count = 0u32;
    let mut hebrew_count = 0u32;
    let mut devanagari_count = 0u32;
    let mut latin_scores = LatinDiacriticScores::default();

    for ch in text.chars() {
        if is_cyrillic(ch) {
            cyrillic_count += 1;
            if is_ukrainian_cyrillic(ch) {
                ukrainian_specific += 2;
            }
            let threshold = 3.max((total_chars as f32 * 0.3).ceil() as u32);
            if ukrainian_specific >= 2 {
                return Some("uk".to_string());
            }
            if cyrillic_count > threshold {
                return Some("ru".to_string());
            }
        } else if is_hiragana_or_katakana(ch) {
            return Some("ja".to_string());
        } else if is_kanji(ch) {
            kanji_count += 1;
        } else if is_hangul_syllable(ch) {
            korean_count += 1;
            if korean_count > 1 {
                return Some("ko".to_string());
            }
        } else if is_arabic(ch) {
            arabic_count += 1;
        } else if is_hebrew(ch) {
            hebrew_count += 1;
        } else if is_thai(ch) {
            thai_count += 1;
        } else if is_devanagari(ch) {
            devanagari_count += 1;
        } else if ch.is_ascii_alphabetic() {
            latin_count += 1;
            apply_latin_diacritic_scores(ch, &mut latin_scores);
        } else if ('\u{00C0}'..='\u{00FF}').contains(&ch) {
            latin_count += 1;
            apply_latin_diacritic_scores(ch, &mut latin_scores);
        } else if ('\u{0100}'..='\u{024F}').contains(&ch) {
            latin_count += 1;
            apply_latin_diacritic_scores(ch, &mut latin_scores);
        }
    }

    if korean_count > 0 {
        return Some("ko".to_string());
    }
    if kanji_count > 0 {
        return Some("zh".to_string());
    }
    if ukrainian_specific >= 2 || (ukrainian_specific > 0 && cyrillic_count > 0 && latin_count == 0) {
        return Some("uk".to_string());
    }
    if cyrillic_count > latin_count && cyrillic_count > 0 {
        return Some("ru".to_string());
    }
    if arabic_count > 0 && arabic_count >= latin_count {
        return Some("ar".to_string());
    }
    if hebrew_count > 0 && hebrew_count >= latin_count {
        return Some("he".to_string());
    }
    if thai_count > 0 {
        return Some("th".to_string());
    }
    if devanagari_count > 0 {
        return Some("hi".to_string());
    }

    if has_dominant_cyrillic_word(text, 4) {
        return Some("ru".to_string());
    }

    if latin_count == 0 {
        return None;
    }

    if contains_scandinavian_markers(text) {
        return Some("sv".to_string());
    }

    if latin_scores.german >= 2 && contains_german_markers(text) {
        return Some("de".to_string());
    }

    pick_latin_language(&latin_scores, total_chars)
}

fn contains_german_markers(text: &str) -> bool {
    text.chars()
        .any(|ch| matches!(ch, 'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü' | 'ß'))
}

fn contains_scandinavian_markers(text: &str) -> bool {
    text.chars()
        .any(|ch| matches!(ch, 'å' | 'Å' | 'æ' | 'Æ' | 'ø' | 'Ø'))
}

/// True when a whitespace token is mostly Cyrillic (e.g. `Привет` after a Latin @mention).
fn has_dominant_cyrillic_word(text: &str, min_chars: usize) -> bool {
    text.split_whitespace().any(|word| {
        let letters: Vec<char> = word.chars().filter(|ch| ch.is_alphabetic()).collect();
        if letters.len() < min_chars {
            return false;
        }
        let cyrillic = letters.iter().filter(|ch| is_cyrillic(**ch)).count();
        cyrillic >= min_chars && cyrillic * 2 >= letters.len()
    })
}

fn twitch_language_detector() -> &'static LanguageDetector {
    static DETECTOR: OnceLock<LanguageDetector> = OnceLock::new();
    DETECTOR.get_or_init(|| {
        use Language::{
            Arabic, Chinese, Dutch, English, French, German, Hindi, Indonesian, Italian, Japanese,
            Korean, Polish, Portuguese, Russian, Spanish, Swedish, Thai, Turkish, Vietnamese,
        };
        LanguageDetectorBuilder::from_languages(&[
            English,
            Chinese,
            Russian,
            Spanish,
            Portuguese,
            German,
            Korean,
            French,
            Japanese,
            Turkish,
            Hindi,
            Italian,
            Arabic,
            Polish,
            Indonesian,
            Swedish,
            Dutch,
            Vietnamese,
            Thai,
        ])
        .with_preloaded_language_models()
        .build()
    })
}

fn detect_with_lingua(text: &str) -> Option<String> {
    let detector = twitch_language_detector();
    let values = detector.compute_language_confidence_values(text);
    let (lang, top_conf) = values.first().copied()?;
    let second_conf = values.get(1).map(|(_, confidence)| *confidence).unwrap_or(0.0);
    let gap = top_conf - second_conf;
    let char_count = text.chars().count();

    let (min_conf, min_gap) = match char_count {
        0..=4 => (0.55, 0.22),
        5..=10 => (0.42, 0.16),
        11..=24 => (0.32, 0.10),
        _ => (0.22, 0.06),
    };

    if top_conf < min_conf || gap < min_gap {
        return None;
    }

    let code = lingua_to_iso639_1(lang);
    if code == "und" {
        None
    } else {
        Some(code)
    }
}

fn lingua_to_iso639_1(lang: Language) -> String {
    lang.iso_code_639_1().to_string()
}

fn pick_latin_language(scores: &LatinDiacriticScores, total_chars: usize) -> Option<String> {
    let entries = [
        ("pl", scores.polish),
        ("de", scores.german),
        ("pt", scores.portuguese),
        ("es", scores.spanish),
        ("fr", scores.french),
        ("it", scores.italian),
        ("tr", scores.turkish),
        ("vi", scores.vietnamese),
        ("cs", scores.czech),
        ("ro", scores.romanian),
        ("sv", scores.nordic),
    ];

    let mut ranked: Vec<(&str, u32)> = entries.into_iter().filter(|(_, count)| *count > 0).collect();
    if ranked.is_empty() {
        return None;
    }
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));

    let (best_code, best_count) = ranked[0];
    let second_count = ranked.get(1).map(|(_, count)| *count).unwrap_or(0);
    if best_count <= second_count {
        return None;
    }

    let share = best_count as f64 / total_chars.max(1) as f64;
    let decisive = matches!(best_code, "pl" | "tr" | "cs" | "ro");

    if decisive && best_count >= 2 {
        return Some(best_code.to_string());
    }
    if best_count >= 2 && share >= 0.25 && best_count > second_count {
        return Some(best_code.to_string());
    }

    None
}

fn apply_latin_diacritic_scores(ch: char, scores: &mut LatinDiacriticScores) {
    if ch == 'ß' {
        scores.german += 4;
    }
    if matches_german(ch) {
        scores.german += score_latin_diacritic(ch);
    }
    if matches_french(ch) {
        scores.french += score_latin_diacritic(ch);
    }
    if matches_spanish(ch) {
        let weight = score_latin_diacritic(ch);
        scores.spanish += if ch == 'ñ' || ch == 'Ñ' { weight + 2 } else { weight };
    }
    if matches_portuguese(ch) {
        scores.portuguese += if ch == 'ã' || ch == 'õ' || ch == 'Ã' || ch == 'Õ' {
            score_latin_diacritic(ch) + 2
        } else {
            score_latin_diacritic(ch)
        };
    }
    if matches_italian(ch) {
        scores.italian += score_latin_diacritic(ch);
    }
    if matches_polish(ch) {
        scores.polish += score_latin_diacritic(ch) + 1;
    }
    if matches_turkish(ch) {
        scores.turkish += score_latin_diacritic(ch) + 1;
    }
    if matches_czech(ch) {
        scores.czech += score_latin_diacritic(ch) + 1;
    }
    if matches_romanian(ch) {
        scores.romanian += score_latin_diacritic(ch) + 1;
    }
    if matches_vietnamese(ch) {
        scores.vietnamese += score_latin_diacritic(ch) + 1;
    }
    if matches_nordic(ch) {
        scores.nordic += score_latin_diacritic(ch) + 1;
    }
}

fn score_latin_diacritic(ch: char) -> u32 {
    if ch.is_ascii_alphabetic() {
        1
    } else {
        2
    }
}

fn matches_german(ch: char) -> bool {
    matches!(ch, 'ä' | 'ö' | 'ü' | 'Ä' | 'Ö' | 'Ü' | 'ß')
}

fn matches_french(ch: char) -> bool {
    matches!(
        ch,
        'é' | 'è' | 'ê' | 'ë' | 'à' | 'ù' | 'ç' | 'î' | 'ô' | 'û' | 'â' | 'ï' | 'œ' | 'æ'
    )
}

fn matches_spanish(ch: char) -> bool {
    matches!(ch, 'ñ' | 'Ñ' | 'á' | 'í' | 'ó' | 'ú' | 'ü' | '¿' | '¡')
}

fn matches_portuguese(ch: char) -> bool {
    matches!(ch, 'ã' | 'õ' | 'Ã' | 'Õ' | 'á' | 'â' | 'ê' | 'ô' | 'ç')
}

fn matches_italian(ch: char) -> bool {
    matches!(ch, 'ì' | 'ò' | 'à' | 'è' | 'é' | 'ù')
}

fn matches_polish(ch: char) -> bool {
    matches!(
        ch,
        'ą' | 'ć' | 'ę' | 'ł' | 'ń' | 'ó' | 'ś' | 'ź' | 'ż' | 'Ą' | 'Ć' | 'Ę' | 'Ł' | 'Ń' | 'Ó' | 'Ś' | 'Ź' | 'Ż'
    )
}

fn matches_turkish(ch: char) -> bool {
    matches!(ch, 'ğ' | 'ı' | 'İ' | 'ş' | 'Ğ' | 'Ş')
}

fn matches_czech(ch: char) -> bool {
    matches!(
        ch,
        'č' | 'ď' | 'ě' | 'ň' | 'ř' | 'š' | 'ť' | 'ů' | 'ž' | 'Č' | 'Ď' | 'Ě' | 'Ň' | 'Ř' | 'Š' | 'Ť'
            | 'Ů' | 'Ž'
    )
}

fn matches_romanian(ch: char) -> bool {
    matches!(ch, 'ă' | 'â' | 'î' | 'ș' | 'ț' | 'Ă' | 'Â' | 'Î' | 'Ș' | 'Ț')
}

fn matches_vietnamese(ch: char) -> bool {
    matches!(
        ch,
        'ă' | 'â' | 'đ' | 'ê' | 'ô' | 'ơ' | 'ư' | 'Ă' | 'Â' | 'Đ' | 'Ê' | 'Ô' | 'Ơ' | 'Ư'
    ) || ('\u{1EA0}'..='\u{1EF9}').contains(&ch)
}

fn matches_nordic(ch: char) -> bool {
    matches!(ch, 'å' | 'æ' | 'ø' | 'Å' | 'Æ' | 'Ø')
}

fn is_cyrillic(ch: char) -> bool {
    ('\u{0400}'..='\u{04FF}').contains(&ch)
}

fn is_ukrainian_cyrillic(ch: char) -> bool {
    matches!(ch, 'і' | 'ї' | 'є' | 'ґ' | 'І' | 'Ї' | 'Є' | 'Ґ')
}

fn is_hiragana_or_katakana(ch: char) -> bool {
    ('\u{3040}'..='\u{309F}').contains(&ch) || ('\u{30A0}'..='\u{30FF}').contains(&ch)
}

fn is_kanji(ch: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&ch) || ('\u{3400}'..='\u{4DBF}').contains(&ch)
}

fn is_hangul_syllable(ch: char) -> bool {
    ('\u{AC00}'..='\u{D7AF}').contains(&ch)
}

fn is_arabic(ch: char) -> bool {
    ('\u{0600}'..='\u{06FF}').contains(&ch) || ('\u{0750}'..='\u{077F}').contains(&ch)
}

fn is_hebrew(ch: char) -> bool {
    ('\u{0590}'..='\u{05FF}').contains(&ch)
}

fn is_thai(ch: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&ch)
}

fn is_devanagari(ch: char) -> bool {
    ('\u{0900}'..='\u{097F}').contains(&ch)
}

fn detect_with_whatlang(text: &str, min_confidence: f64) -> Option<String> {
    let info = whatlang::detect(text)?;
    let char_count = text.chars().count();
    let effective_min = match char_count {
        0..=8 => 0.28,
        9..=16 => 0.22,
        17..=32 => 0.18,
        _ => min_confidence,
    }
    .min(min_confidence.max(0.18));
    if info.confidence() < effective_min {
        return None;
    }
    let code = lang_to_iso639_1(info.lang());
    if code == "und" {
        None
    } else {
        Some(code)
    }
}

fn detect_by_script(text: &str) -> Option<String> {
    let script = whatlang::detect_script(text)?;
    let code = match script {
        Script::Cyrillic => "ru",
        Script::Latin => "en",
        Script::Hiragana | Script::Katakana => "ja",
        Script::Hangul => "ko",
        Script::Mandarin => "zh",
        Script::Arabic => "ar",
        Script::Hebrew => "he",
        Script::Devanagari => "hi",
        Script::Thai => "th",
        Script::Bengali => "bn",
        Script::Georgian => "ka",
        Script::Armenian => "hy",
        Script::Greek => "el",
        Script::Gurmukhi => "pa",
        Script::Tamil => "ta",
        Script::Telugu => "te",
        Script::Kannada => "kn",
        Script::Gujarati => "gu",
        Script::Khmer => "km",
        Script::Myanmar => "my",
        Script::Sinhala => "si",
        Script::Ethiopic => "am",
        _ => return None,
    };
    Some(code.to_string())
}

pub fn lang_to_iso639_1(lang: Lang) -> String {
    let code = match lang {
        Lang::Rus => "ru",
        Lang::Eng => "en",
        Lang::Jpn => "ja",
        Lang::Kor => "ko",
        Lang::Cmn => "zh",
        Lang::Deu => "de",
        Lang::Fra => "fr",
        Lang::Spa => "es",
        Lang::Por => "pt",
        Lang::Ita => "it",
        Lang::Ukr => "uk",
        Lang::Pol => "pl",
        Lang::Tur => "tr",
        Lang::Ara => "ar",
        Lang::Hin => "hi",
        Lang::Tha => "th",
        Lang::Vie => "vi",
        Lang::Ind => "id",
        Lang::Nld => "nl",
        Lang::Swe => "sv",
        Lang::Dan => "da",
        Lang::Nob => "no",
        Lang::Fin => "fi",
        Lang::Hun => "hu",
        Lang::Ces => "cs",
        Lang::Ron => "ro",
        Lang::Ell => "el",
        Lang::Heb => "he",
        Lang::Bul => "bg",
        Lang::Bel => "be",
        Lang::Hrv => "hr",
        Lang::Slv => "sl",
        Lang::Srp => "sr",
        Lang::Mkd => "mk",
        Lang::Lit => "lt",
        Lang::Lav => "lv",
        Lang::Est => "et",
        Lang::Ben => "bn",
        Lang::Tam => "ta",
        Lang::Tel => "te",
        Lang::Kan => "kn",
        Lang::Guj => "gu",
        Lang::Pes => "fa",
        Lang::Urd => "ur",
        Lang::Aze => "az",
        Lang::Uzb => "uz",
        Lang::Amh => "am",
        Lang::Kat => "ka",
        Lang::Mya => "my",
        Lang::Khm => "km",
        Lang::Sin => "si",
        Lang::Nep => "ne",
        Lang::Jav => "jv",
        Lang::Epo => "eo",
        _ => "und",
    };
    code.to_string()
}

pub fn language_allowed(detected: &str, enabled: &[String]) -> bool {
    if enabled.is_empty() {
        return true;
    }
    let key = detected.trim().to_ascii_lowercase();
    if key == "und" {
        return true;
    }
    enabled
        .iter()
        .any(|entry| entry.trim().eq_ignore_ascii_case(&key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_russian() {
        assert_eq!(detect_language_code("привет как дела", 4).as_deref(), Some("ru"));
    }

    #[test]
    fn detects_ukrainian() {
        assert_eq!(detect_language_code("привіт друже", 4).as_deref(), Some("uk"));
    }

    #[test]
    fn detects_english_hello() {
        assert_eq!(detect_language_code("hello", 4).as_deref(), Some("en"));
        assert_eq!(resolve_message_language("hello", 4, "ru"), "en");
    }

    #[test]
    fn short_latin_uses_script_not_und() {
        assert_eq!(resolve_message_language("hi", 4, "ru"), "en");
    }

    #[test]
    fn und_falls_back_to_settings_language() {
        assert_eq!(resolve_message_language("12345", 4, "ja"), "ja");
    }

    #[test]
    fn empty_enabled_allows_all() {
        assert!(language_allowed("ru", &[]));
    }

    #[test]
    fn filters_by_enabled() {
        assert!(language_allowed("en", &["en".into()]));
        assert!(!language_allowed("ru", &["en".into()]));
    }

    #[test]
    fn detects_polish_diacritics() {
        assert_eq!(detect_language_code("cześć", 4).as_deref(), Some("pl"));
        assert_eq!(detect_language_code("dziękuję", 4).as_deref(), Some("pl"));
    }

    #[test]
    fn detects_german_diacritics() {
        assert_eq!(detect_language_code("schön", 4).as_deref(), Some("de"));
        assert_eq!(detect_language_code("grüße", 4).as_deref(), Some("de"));
    }

    #[test]
    fn detects_spanish_diacritics() {
        assert_eq!(detect_language_code("español", 4).as_deref(), Some("es"));
    }

    #[test]
    fn detects_french_diacritics() {
        assert_eq!(
            detect_language_code("bonjour à tous", 4).as_deref(),
            Some("fr")
        );
    }

    #[test]
    fn detects_portuguese_diacritics() {
        assert_eq!(detect_language_code("não", 4).as_deref(), Some("pt"));
    }

    #[test]
    fn detects_italian_diacritics() {
        assert_eq!(detect_language_code("città", 4).as_deref(), Some("it"));
        assert_eq!(detect_language_code("così", 4).as_deref(), Some("it"));
    }

    #[test]
    fn detects_turkish_diacritics() {
        assert_eq!(detect_language_code("ışık", 4).as_deref(), Some("tr"));
        assert_eq!(detect_language_code("dağ", 4).as_deref(), Some("tr"));
    }

    #[test]
    fn detects_japanese_kana() {
        assert_eq!(detect_language_code("こんにちは", 4).as_deref(), Some("ja"));
        assert_eq!(detect_language_code("カタカナ", 4).as_deref(), Some("ja"));
    }

    #[test]
    fn detects_korean() {
        assert_eq!(detect_language_code("안녕", 4).as_deref(), Some("ko"));
    }

    #[test]
    fn detects_chinese_kanji() {
        assert_eq!(detect_language_code("你好", 4).as_deref(), Some("zh"));
    }

    #[test]
    fn detects_short_cyrillic_without_min_chars() {
        assert_eq!(detect_language_code("да", 4).as_deref(), Some("ru"));
    }

    #[test]
    fn detects_vietnamese_diacritics() {
        assert_eq!(
            detect_language_code("xin chào các bạn", 4).as_deref(),
            Some("vi")
        );
    }

    #[test]
    fn lingua_picks_dutch_on_phrase() {
        assert_eq!(
            detect_language_code("dit is een typische nederlandse zin", 4).as_deref(),
            Some("nl")
        );
    }

    #[test]
    fn detects_long_french_not_dutch() {
        let sample = "Bonjour à tous, je m'appelle Kiriuru. Je parle principalement russe, mais je connais un peu l'anglais. Mon discours est sous-titré en anglais et en japonais.";
        assert_eq!(detect_language_code(sample, 4).as_deref(), Some("fr"));
        assert_eq!(resolve_message_language(sample, 4, "en"), "fr");
    }

    #[test]
    fn detects_long_german_not_dutch() {
        let sample = "In früheren Windows-Versionen (vor 10/11) wurde direkt die Meldung „Nicht genügend virtueller Speicher“ angezeigt, in modernen Windows-Versionen ist die Speicherverwaltung intelligenter, sodass eine solche Meldung nicht mehr angezeigt wird.";
        assert_eq!(detect_language_code(sample, 4).as_deref(), Some("de"));
        assert_eq!(resolve_message_language(sample, 4, "en"), "de");
    }

    #[test]
    fn german_article_not_dutch() {
        assert_eq!(
            detect_language_code("die Meldung ist wichtig", 4).as_deref(),
            Some("de")
        );
    }

    #[test]
    fn twitch_top_language_codes_match_localization_top20() {
        assert_eq!(TWITCH_TOP_LANGUAGE_CODES.len(), 19);
        assert!(TWITCH_TOP_LANGUAGE_CODES.contains(&"en"));
        assert!(TWITCH_TOP_LANGUAGE_CODES.contains(&"zh"));
        assert!(TWITCH_TOP_LANGUAGE_CODES.contains(&"th"));
    }

    #[test]
    fn detects_indonesian() {
        assert_eq!(
            detect_language_code("selamat pagi semuanya", 4).as_deref(),
            Some("id")
        );
    }

    #[test]
    fn detects_hindi_devanagari() {
        assert_eq!(detect_language_code("नमस्ते दोस्त", 4).as_deref(), Some("hi"));
    }

    #[test]
    fn detects_arabic() {
        assert_eq!(
            detect_language_code("مرحبا بكم في البث", 4).as_deref(),
            Some("ar")
        );
    }

    #[test]
    fn detects_thai() {
        assert_eq!(
            detect_language_code("สวัสดีทุกคน", 4).as_deref(),
            Some("th")
        );
    }

    #[test]
    fn detects_swedish() {
        assert_eq!(
            detect_language_code("jag går hem nu, vi ses senare", 4).as_deref(),
            Some("sv")
        );
    }

    #[test]
    fn strips_punctuation_before_detection() {
        assert_eq!(detect_language_code("привет!!!", 4).as_deref(), Some("ru"));
        assert_eq!(detect_language_code("cześć!!!", 4).as_deref(), Some("pl"));
    }

    #[test]
    fn mention_does_not_override_cyrillic_language() {
        assert_eq!(
            detect_language_code("@KamakiriMeido Привет", 2).as_deref(),
            Some("ru")
        );
        assert_eq!(
            resolve_message_language("@KamakiriMeido Привет", 2, "en"),
            "ru"
        );
    }

    #[test]
    fn strips_speaker_label_before_detection() {
        assert_eq!(
            strip_leading_speaker_label("Wallenber: hello chat"),
            "hello chat"
        );
        assert_eq!(strip_leading_speaker_label("12:30 meeting"), "12:30 meeting");
    }

    #[test]
    fn link_only_line_does_not_detect_indonesian_from_name() {
        let sample = "Wallenber: https://www.youtube.com/watch?v=zqBnOfSmKQo";
        assert_eq!(detect_language_code(sample, 2), None);
        assert_eq!(resolve_message_language(sample, 2, "ru"), "ru");
        assert_eq!(
            detect_language_code("Wallenber", 2).as_deref(),
            Some("en")
        );
    }

    #[test]
    fn youtube_playlist_link_does_not_detect_dutch() {
        let sample =
            "Wallenber: https://www.youtube.com/watch?v=3VTkBuxU4yk&list=RDMM&index=5";
        assert_eq!(resolve_message_language(sample, 2, "ru"), "ru");
        assert_eq!(detect_language_code(sample, 2), None);
        assert!(!has_meaningful_linguistic_content(sample));
        assert!(!has_meaningful_linguistic_content(
            "https://www.youtube.com/watch?v=3VTkBuxU4yklist=RDMMindex=5"
        ));
    }

    #[test]
    fn strip_twitch_mentions_removes_at_tokens() {
        assert_eq!(
            strip_twitch_mentions("@KamakiriMeido Привет"),
            "Привет"
        );
        assert_eq!(strip_twitch_mentions("hi @friend there"), "hi there");
    }

    #[test]
    fn digit_only_lines_are_meaningful() {
        assert!(has_meaningful_linguistic_content("522"));
        assert!(has_meaningful_linguistic_content("123"));
        assert!(has_meaningful_linguistic_content("1 2 3"));
    }
}
