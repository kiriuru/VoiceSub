use std::time::Duration;

use tokio::time::sleep;

pub const MAX_UPSTREAM_ATTEMPTS: usize = 3;

const UPSTREAM_HTTP_PREFIX: &str = "upstream http ";

/// Parse status code from canonical fetch errors (`upstream HTTP 502`, etc.).
fn upstream_http_status_code(message: &str) -> Option<u16> {
    let lower = message.to_ascii_lowercase();
    let start = lower.find(UPSTREAM_HTTP_PREFIX)?;
    let digits: String = lower[start + UPSTREAM_HTTP_PREFIX.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn is_retryable_upstream_http_status(status: u16) -> bool {
    status == 408 || status == 429 || (500..=599).contains(&status)
}

fn is_retryable_transport_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    const TRANSPORT_MARKERS: &[&str] = &[
        "close_notify",
        "unexpected eof",
        "unexpected_eof",
        "connection reset",
        "connection aborted",
        "temporarily unavailable",
        "error sending request",
        "error trying to connect",
        "operation timed out",
        "timed out while",
        "request timed out",
    ];
    if TRANSPORT_MARKERS
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return true;
    }
    // TLS / SSL protocol flakes — avoid bare `ssl` (false positives in unrelated text).
    lower.contains("ssl:")
        || lower.contains("ssl, ")
        || lower.contains("tls handshake")
        || lower.contains("ssl handshake")
}

/// Transient network/TLS failures worth retrying for Google TTS HTTP and similar fetches.
pub fn is_retryable_upstream_error(message: &str) -> bool {
    if let Some(status) = upstream_http_status_code(message) {
        return is_retryable_upstream_http_status(status);
    }
    is_retryable_transport_error(message)
}

pub async fn upstream_retry_delay(attempt: usize) {
    sleep(Duration::from_millis(300 * attempt as u64)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retryable_upstream_errors_include_tls_eof_and_reqwest_transport() {
        assert!(is_retryable_upstream_error(
            "upstream request failed: error sending request for url (https://example)"
        ));
        assert!(is_retryable_upstream_error(
            "peer closed connection without sending TLS close_notify"
        ));
        assert!(is_retryable_upstream_error(
            "SSL: UNEXPECTED_EOF_WHILE_READING EOF occurred in violation of protocol"
        ));
        assert!(!is_retryable_upstream_error("empty text"));
        assert!(!is_retryable_upstream_error("upstream HTTP 400"));
        assert!(!is_retryable_upstream_error("grassland routing classless"));
        assert!(is_retryable_upstream_error("upstream HTTP 502"));
        assert!(is_retryable_upstream_error("upstream HTTP 429"));
        assert!(is_retryable_upstream_error("upstream HTTP 503"));
        assert!(!is_retryable_upstream_error("upstream HTTP 404"));
    }
}
