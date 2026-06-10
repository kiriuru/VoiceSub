use whatlang::{Lang, Script};

/// Minimum whatlang confidence before we trust trigram detection on short chat lines.
const RELIABLE_CONFIDENCE: f64 = 0.75;

pub fn resolve_message_language(text: &str, min_chars: usize, fallback: &str) -> String {
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

pub fn detect_language_code(text: &str, min_chars: usize) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(code) = detect_by_script(trimmed) {
        return Some(code);
    }

    if trimmed.chars().count() < min_chars {
        return None;
    }

    let info = whatlang::detect(trimmed)?;
    if !info.is_reliable() && info.confidence() < RELIABLE_CONFIDENCE {
        return detect_by_script(trimmed);
    }
    let code = lang_to_iso639_1(info.lang());
    if code == "und" {
        detect_by_script(trimmed)
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
        Script::Arabic | Script::Hebrew => "ar",
        Script::Devanagari => "hi",
        Script::Thai => "th",
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
}
