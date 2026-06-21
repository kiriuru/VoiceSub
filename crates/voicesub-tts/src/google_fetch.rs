use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header;
use tracing::warn;
use urlencoding::encode;

use crate::config::TTS_PROVIDER_PYTHON_STDLIB;
use crate::python_runtime::{normalize_tts_lang, run_google_tts_fetch};
use crate::upstream_retry::{
    MAX_UPSTREAM_ATTEMPTS, is_retryable_upstream_error, upstream_retry_delay,
};

pub const GOOGLE_TTS_MAX_CHARS: usize = 200;

/// Max concurrent upstream chunk fetches for a single multi-chunk line. Keeps prefetch
/// fast without bursting many parallel requests that invite 429/503 throttling.
const GOOGLE_TTS_PREFETCH_MAX_CONCURRENCY: usize = 3;

fn google_tts_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            )
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
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
            if let Some(at) = space_at
                && at > start
            {
                end = at;
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

async fn fetch_google_tts_browser_once(lang: &str, text: &str) -> Result<Vec<u8>, String> {
    let upstream = google_upstream_url(text, lang);
    let response = google_tts_http_client()
        .get(&upstream)
        .header(header::REFERER, "https://translate.google.com/")
        .send()
        .await
        .map_err(|err| format!("upstream request failed: {err}"))?;

    if !response.status().is_success() {
        return Err(format!("upstream HTTP {}", response.status().as_u16()));
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

    let mut last_error = String::from("upstream request failed");
    for attempt in 1..=MAX_UPSTREAM_ATTEMPTS {
        match fetch_google_tts_browser_once(lang, trimmed).await {
            Ok(bytes) => return Ok(bytes),
            Err(err) => {
                last_error = err;
                if attempt >= MAX_UPSTREAM_ATTEMPTS || !is_retryable_upstream_error(&last_error) {
                    break;
                }
                warn!(
                    target: "voicesub.tts.fetch",
                    attempt,
                    error = %last_error,
                    "google tts fetch retrying"
                );
                upstream_retry_delay(attempt).await;
            }
        }
    }
    Err(last_error)
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

    let total = chunks.len();
    let tl = normalize_tts_lang(lang);
    if total == 1 {
        return Ok(vec![
            fetch_tts_chunk(module_dir, provider, &tl, &chunks[0]).await?,
        ]);
    }

    // Fetch the first chunk synchronously (fail fast / warm the connection),
    // then fetch the rest concurrently. `JoinSet` yields in completion order,
    // so each task carries its index to let us restore the original text order.
    let mut parts: Vec<(usize, Vec<u8>)> = vec![(
        0,
        fetch_tts_chunk(module_dir, provider, &tl, &chunks[0]).await?,
    )];

    // Cap concurrent upstream fetches so a long line does not burst many parallel requests
    // at Google Translate TTS and trigger 429/503 throttling (review MED#11).
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(
        GOOGLE_TTS_PREFETCH_MAX_CONCURRENCY,
    ));
    let mut rest = tokio::task::JoinSet::new();
    for (index, chunk) in chunks.into_iter().enumerate().skip(1) {
        let module_dir = module_dir.to_path_buf();
        let provider = provider.to_string();
        let tl = tl.clone();
        let semaphore = semaphore.clone();
        rest.spawn(async move {
            let _permit = semaphore.acquire_owned().await.ok();
            (
                index,
                fetch_tts_chunk(&module_dir, &provider, &tl, &chunk).await,
            )
        });
    }
    while let Some(joined) = rest.join_next().await {
        let (index, result) = joined.map_err(|err| format!("prefetch task failed: {err}"))?;
        parts.push((index, result?));
    }

    assemble_ordered_chunks(parts, total)
}

/// Reassemble concurrently fetched `(index, audio)` pairs back into text order.
fn assemble_ordered_chunks(
    parts: Vec<(usize, Vec<u8>)>,
    total: usize,
) -> Result<Vec<Vec<u8>>, String> {
    let mut ordered: Vec<Option<Vec<u8>>> = (0..total).map(|_| None).collect();
    for (index, bytes) in parts {
        let slot = ordered
            .get_mut(index)
            .ok_or_else(|| format!("prefetch chunk index {index} out of range {total}"))?;
        *slot = Some(bytes);
    }
    let mut out = Vec::with_capacity(total);
    for (index, slot) in ordered.into_iter().enumerate() {
        out.push(slot.ok_or_else(|| format!("prefetch chunk {index} missing"))?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TTS_PROVIDER_BROWSER_GOOGLE;

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

    #[test]
    fn retryable_upstream_errors_cover_reqwest_transport() {
        assert!(crate::upstream_retry::is_retryable_upstream_error(
            "upstream request failed: error sending request for url (https://example)"
        ));
    }

    #[test]
    fn assemble_ordered_chunks_restores_text_order_from_completion_order() {
        // Simulate JoinSet completion order (2, 0, 1) for a 3-chunk line.
        let parts = vec![
            (2usize, b"third".to_vec()),
            (0usize, b"first".to_vec()),
            (1usize, b"second".to_vec()),
        ];
        let ordered = assemble_ordered_chunks(parts, 3).expect("assemble");
        assert_eq!(
            ordered,
            vec![b"first".to_vec(), b"second".to_vec(), b"third".to_vec()]
        );
    }

    #[test]
    fn assemble_ordered_chunks_reports_missing_and_out_of_range() {
        assert!(assemble_ordered_chunks(vec![(0, vec![1])], 2).is_err());
        assert!(assemble_ordered_chunks(vec![(5, vec![1])], 2).is_err());
    }
}
