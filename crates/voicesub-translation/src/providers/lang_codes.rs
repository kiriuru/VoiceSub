//! Map VoiceSub UI language codes to provider-specific API codes.

/// DeepL target codes (uppercase, regional where required).
pub fn deepl_target_lang(code: &str) -> String {
    match normalize_ui_lang(code).as_str() {
        "en" | "en-us" => "EN-US".into(),
        "en-gb" => "EN-GB".into(),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "ZH-HANS".into(),
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "ZH-HANT".into(),
        "pt" | "pt-br" => "PT-BR".into(),
        "pt-pt" => "PT-PT".into(),
        other => other.to_ascii_uppercase(),
    }
}

/// DeepL source codes (base uppercase; no regional variants for source).
pub fn deepl_source_lang(code: &str) -> Option<String> {
    let normalized = normalize_ui_lang(code);
    if normalized.is_empty() || normalized == "auto" {
        return None;
    }
    let mapped = match normalized.as_str() {
        "en" | "en-us" | "en-gb" => "EN",
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" | "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "ZH",
        "pt" | "pt-br" | "pt-pt" => "PT",
        other => other,
    };
    Some(mapped.to_ascii_uppercase())
}

/// Azure Translator prefers BCP-47 script tags for Chinese.
pub fn azure_lang(code: &str) -> String {
    match normalize_ui_lang(code).as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh-Hans".into(),
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "zh-Hant".into(),
        other => other.to_string(),
    }
}

/// LibreTranslate / public mirrors commonly use `zh` / `zt`.
pub fn libretranslate_lang(code: &str) -> String {
    match normalize_ui_lang(code).as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh".into(),
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "zt".into(),
        other => other.to_string(),
    }
}

/// Baidu Translate open platform language codes.
pub fn baidu_lang(code: &str) -> String {
    match normalize_ui_lang(code).as_str() {
        "auto" => "auto".into(),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh".into(),
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "cht".into(),
        "ja" => "jp".into(),
        "ko" => "kor".into(),
        "fr" => "fra".into(),
        "es" => "spa".into(),
        "ar" => "ara".into(),
        "vi" => "vie".into(),
        // Baidu uses ISO-ish codes for most langs, but Swedish is `swe` (not `sv`).
        "sv" => "swe".into(),
        other => other.to_string(),
    }
}

/// Youdao Zhiyun language codes.
pub fn youdao_lang(code: &str) -> String {
    match normalize_ui_lang(code).as_str() {
        "auto" => "auto".into(),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh-CHS".into(),
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "zh-CHT".into(),
        other => other.to_string(),
    }
}

/// Tencent Cloud TMT language codes.
pub fn tencent_lang(code: &str) -> String {
    match normalize_ui_lang(code).as_str() {
        "auto" => "auto".into(),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh".into(),
        "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => "zh-TW".into(),
        other => other.to_string(),
    }
}

/// Caiyun Xiaoyi supports a small set (zh/en/ja + auto source).
pub fn caiyun_lang(code: &str) -> Result<String, String> {
    match normalize_ui_lang(code).as_str() {
        "auto" => Ok("auto".into()),
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" | "zh-tw" | "zh-hant" | "zh-hk" | "zh-mo" => {
            Ok("zh".into())
        }
        "en" | "en-us" | "en-gb" => Ok("en".into()),
        "ja" => Ok("ja".into()),
        other => Err(format!(
            "Caiyun Translator only supports zh/en/ja (got '{other}')."
        )),
    }
}

fn normalize_ui_lang(code: &str) -> String {
    code.trim().to_ascii_lowercase().replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepl_maps_ui_targets() {
        assert_eq!(deepl_target_lang("en"), "EN-US");
        assert_eq!(deepl_target_lang("zh-cn"), "ZH-HANS");
        assert_eq!(deepl_target_lang("zh-tw"), "ZH-HANT");
        assert_eq!(deepl_target_lang("pt"), "PT-BR");
        assert_eq!(deepl_target_lang("ru"), "RU");
    }

    #[test]
    fn deepl_maps_source_base_codes() {
        assert_eq!(deepl_source_lang("en-US").as_deref(), Some("EN"));
        assert_eq!(deepl_source_lang("zh-cn").as_deref(), Some("ZH"));
        assert_eq!(deepl_source_lang("auto"), None);
    }

    #[test]
    fn azure_and_libre_map_chinese() {
        assert_eq!(azure_lang("zh-cn"), "zh-Hans");
        assert_eq!(azure_lang("zh-tw"), "zh-Hant");
        assert_eq!(libretranslate_lang("zh-cn"), "zh");
        assert_eq!(libretranslate_lang("zh-tw"), "zt");
    }

    #[test]
    fn china_provider_lang_maps() {
        assert_eq!(baidu_lang("ja"), "jp");
        assert_eq!(baidu_lang("zh-tw"), "cht");
        assert_eq!(baidu_lang("sv"), "swe");
        assert_eq!(youdao_lang("zh-cn"), "zh-CHS");
        assert_eq!(tencent_lang("zh-tw"), "zh-TW");
        assert_eq!(caiyun_lang("en").as_deref(), Ok("en"));
        assert!(caiyun_lang("ru").is_err());
    }
}
