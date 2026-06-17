//! Localized WebView2-missing dialogs (dashboard locales: en, ru, ja, ko, zh).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebView2MissingDialogCopy {
    pub title: &'static str,
    pub body: &'static str,
}

pub fn normalize_supported_ui_language(raw: &str) -> &'static str {
    let current = raw.trim().to_ascii_lowercase();
    if current.is_empty() {
        return "en";
    }
    if ["en", "ru", "ja", "ko", "zh"].contains(&current.as_str()) {
        return match current.as_str() {
            "ru" => "ru",
            "ja" => "ja",
            "ko" => "ko",
            "zh" => "zh",
            _ => "en",
        };
    }
    if current.starts_with("ru") {
        return "ru";
    }
    if current.starts_with("zh") {
        return "zh";
    }
    if current.starts_with("ja") {
        return "ja";
    }
    if current.starts_with("ko") {
        return "ko";
    }
    if current.starts_with("en") {
        return "en";
    }
    "en"
}

/// Resolve UI language from the Windows display language (LANGID).
#[cfg(windows)]
pub fn system_supported_ui_language() -> &'static str {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }

    let lang_id = unsafe { GetUserDefaultUILanguage() };
    normalize_supported_ui_language(&map_windows_lang_id(lang_id))
}

#[cfg(not(windows))]
pub fn system_supported_ui_language() -> &'static str {
    "en"
}

#[cfg(windows)]
fn map_windows_lang_id(lang_id: u16) -> String {
    let primary = lang_id & 0x3FF;
    match primary {
        0x19 => "ru".into(),
        0x11 => "ja".into(),
        0x12 => "ko".into(),
        0x04 => "zh".into(),
        _ => "en".into(),
    }
}

pub fn webview2_missing_dialog_copy(locale: &str) -> WebView2MissingDialogCopy {
    match normalize_supported_ui_language(locale) {
        "ru" => WebView2MissingDialogCopy {
            title: "VoiceSub — требуется WebView2",
            body: concat!(
                "Для работы VoiceSub нужен Microsoft Edge WebView2 Runtime.\r\n\r\n",
                "Сейчас он не найден на этом компьютере. Установите WebView2 и запустите VoiceSub снова.\r\n\r\n",
                "Скачать:\r\n",
                "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
            ),
        },
        "ja" => WebView2MissingDialogCopy {
            title: "VoiceSub — WebView2 が必要です",
            body: concat!(
                "VoiceSub を使用するには Microsoft Edge WebView2 Runtime が必要です。\r\n\r\n",
                "このコンピューターに WebView2 が見つかりません。インストール後に VoiceSub を再起動してください。\r\n\r\n",
                "ダウンロード:\r\n",
                "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
            ),
        },
        "ko" => WebView2MissingDialogCopy {
            title: "VoiceSub — WebView2 필요",
            body: concat!(
                "VoiceSub를 실행하려면 Microsoft Edge WebView2 Runtime이 필요합니다.\r\n\r\n",
                "이 PC에서 WebView2를 찾을 수 없습니다. 설치한 뒤 VoiceSub를 다시 실행하세요.\r\n\r\n",
                "다운로드:\r\n",
                "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
            ),
        },
        "zh" => WebView2MissingDialogCopy {
            title: "VoiceSub — 需要 WebView2",
            body: concat!(
                "运行 VoiceSub 需要 Microsoft Edge WebView2 运行时。\r\n\r\n",
                "未在此计算机上检测到 WebView2。请安装后重新启动 VoiceSub。\r\n\r\n",
                "下载:\r\n",
                "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
            ),
        },
        _ => WebView2MissingDialogCopy {
            title: "VoiceSub — WebView2 required",
            body: concat!(
                "VoiceSub requires Microsoft Edge WebView2 Runtime.\r\n\r\n",
                "WebView2 was not found on this computer. Install it, then start VoiceSub again.\r\n\r\n",
                "Download:\r\n",
                "https://go.microsoft.com/fwlink/p/?LinkId=2124703"
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WEBVIEW2_DOWNLOAD_URL;

    #[test]
    fn normalizes_locale_aliases() {
        assert_eq!(normalize_supported_ui_language("ru-RU"), "ru");
        assert_eq!(normalize_supported_ui_language("ja-JP"), "ja");
        assert_eq!(normalize_supported_ui_language("ko-KR"), "ko");
        assert_eq!(normalize_supported_ui_language("zh-CN"), "zh");
        assert_eq!(normalize_supported_ui_language("fr-FR"), "en");
    }

    #[test]
    fn all_locales_have_copy() {
        for locale in ["en", "ru", "ja", "ko", "zh"] {
            let copy = webview2_missing_dialog_copy(locale);
            assert!(!copy.title.is_empty());
            assert!(copy.body.contains(WEBVIEW2_DOWNLOAD_URL));
        }
    }
}
