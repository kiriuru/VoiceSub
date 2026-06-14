use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header;
use tracing::warn;
use urlencoding::encode;

use crate::config::TTS_PROVIDER_PYTHON_STDLIB;
use crate::python_runtime::{normalize_tts_lang, run_google_tts_fetch};

pub const GOOGLE_TTS_MAX_CHARS: usize = 200;

fn google_tts_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .expect("google tts reqwest client")
    })
}

fn google_upstream_url(text: &str, lang: &str) -> String {
    let tl = normalize_tts_lang(lang);
    let textlen = text.chars().count();
    format!(
        "https://translate.google.com/translate_tts?ie=UTF-8&client=tw-ob&tl={tl}&q={q}&total=1&idx=0&textlen={textlen}",
        tl = tl,
        q = encode(text),
        textlen = textlen
    )
}

pub fn chunk_text_for_google_tts(text: &str, max_chars: usize) -> Vec<String> {
    let normalized = text.trim();
    if normalized.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = normalized.chars().collect();
    if chars.len() <= max_chars {
        return vec![normalized.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let mut end = (start + max_chars).min(chars.len());
        if end < chars.len() {
            let min_break = start + (max_chars * 4 / 10);
            let mut space_at = None;
            for index in (min_break..end).rev() {
                if chars[index] == ' ' {
                    space_at = Some(index);
                    break;
                }
            }
            if let Some(at) = space_at {
                if at > start {
                    end = at;
                }
            }
        }
        let piece: String = chars[start..end].iter().collect();
        let trimmed = piece.trim();
        if !trimmed.is_empty() {
            chunks.push(trimmed.to_string());
        }
        start = end;
        while start < chars.len() && chars[start] == ' ' {
            start += 1;
        }
    }
    chunks
}

fn looks_like_mpeg_audio(bytes: &[u8]) -> bool {
    if bytes.len() < 2 {
        return false;
    }
    if bytes[0] == 0x49 && bytes[1] == 0x44 && bytes.get(2) == Some(&0x33) {
        return true;
    }
    bytes[0] == 0xff && (bytes[1] & 0xe0) == 0xe0
}

pub async fn fetch_google_tts_browser(lang: &str, text: &str) -> Result<Vec<u8>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("empty text".into());
    }
    if trimmed.chars().count() > GOOGLE_TTS_MAX_CHARS {
        return Err(format!(
            "text exceeds {GOOGLE_TTS_MAX_CHARS} chars; chunk before fetch"
        ));
    }

    let upstream = google_upstream_url(trimmed, lang);
    let response = google_tts_http_client()
        .get(&upstream)
        .header(header::REFERER, "https://translate.google.com/")
        .send()
        .await
        .map_err(|err| format!("upstream request failed: {err}"))?;

    if !response.status().is_success() {
        return Err(format!("upstream HTTP {}", response.status()));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|err| format!("upstream body read failed: {err}"))?
        .to_vec();

    if bytes.is_empty() {
        return Err("upstream empty audio body".into());
    }
    if !looks_like_mpeg_audio(&bytes) {
        warn!(
            target: "voicesub.tts.fetch",
            bytes = bytes.len(),
            "response is not MPEG audio"
        );
        return Err(format!(
            "response is not MPEG audio ({} bytes)",
            bytes.len()
        ));
    }
    Ok(bytes)
}

pub async fn fetch_tts_chunk(
    module_dir: &Path,
    provider: &str,
    lang: &str,
    text: &str,
) -> Result<Vec<u8>, String> {
    match provider {
        TTS_PROVIDER_PYTHON_STDLIB => run_google_tts_fetch(module_dir, lang, text)
            .await
            .map(|(bytes, _kind)| bytes),
        _ => fetch_google_tts_browser(lang, text).await,
    }
}

pub async fn prefetch_tts_line(
    module_dir: &Path,
    provider: &str,
    lang: &str,
    text: &str,
) -> Result<Vec<Vec<u8>>, String> {
    let chunks = chunk_text_for_google_tts(text, GOOGLE_TTS_MAX_CHARS);
    if chunks.is_empty() {
        return Ok(Vec::new());
    }

    let tl = normalize_tts_lang(lang);
    let mut out = Vec::with_capacity(chunks.len());
    out.push(
        fetch_tts_chunk(module_dir, provider, &tl, &chunks[0])
            .await?,
    );
    if chunks.len() == 1 {
        return Ok(out);
    }

    let mut rest = tokio::task::JoinSet::new();
    for chunk in chunks.into_iter().skip(1) {
        let module_dir = module_dir.to_path_buf();
        let provider = provider.to_string();
        let tl = tl.clone();
        rest.spawn(async move { fetch_tts_chunk(&module_dir, &provider, &tl, &chunk).await });
    }
    while let Some(result) = rest.join_next().await {
        out.push(result.map_err(|err| format!("prefetch task failed: {err}"))??);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TTS_PROVIDER_BROWSER_GOOGLE;

    #[test]
    fn chunk_text_splits_long_lines_on_word_boundary() {
        let text = "alpha ".repeat(40).trim().to_string();
        let chunks = chunk_text_for_google_tts(&text, 50);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.chars().count() <= GOOGLE_TTS_MAX_CHARS);
        }
    }

    #[test]
    fn chunk_text_keeps_short_line_intact() {
        let chunks = chunk_text_for_google_tts("hello world", GOOGLE_TTS_MAX_CHARS);
        assert_eq!(chunks, vec!["hello world".to_string()]);
    }

    #[test]
    fn provider_constants_are_stable() {
        assert_eq!(TTS_PROVIDER_BROWSER_GOOGLE, "browser_google");
        assert_eq!(TTS_PROVIDER_PYTHON_STDLIB, "python_stdlib");
    }
}
