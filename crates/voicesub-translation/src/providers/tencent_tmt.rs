use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde_json::{Value, json};

use std::sync::Arc;

use super::{
    ProviderError, ProviderInfo, TranslateRequest, TranslationProvider, base_diagnostics,
    crypto_util::{hmac_sha256, hmac_sha256_hex, sha256_hex},
    http, http::SharedHttpClient,
    lang_codes::tencent_lang,
    mask_secret, normalize_source_lang,
};

const SERVICE: &str = "tmt";
const HOST: &str = "tmt.tencentcloudapi.com";
const ACTION: &str = "TextTranslate";
const VERSION: &str = "2018-03-21";
const ALGORITHM: &str = "TC3-HMAC-SHA256";

pub struct TencentTmtProvider {
    transport: Arc<SharedHttpClient>,
}

impl TencentTmtProvider {
    pub fn new(transport: Arc<SharedHttpClient>) -> Self {
        Self { transport }
    }

    fn authorization(
        secret_id: &str,
        secret_key: &str,
        timestamp: u64,
        payload: &str,
    ) -> (String, String) {
        let date = chrono_date_utc(timestamp);
        let credential_scope = format!("{date}/{SERVICE}/tc3_request");
        let hashed_payload = sha256_hex(payload);
        let canonical_headers = format!(
            "content-type:application/json; charset=utf-8\nhost:{HOST}\nx-tc-action:{}\n",
            ACTION.to_ascii_lowercase()
        );
        let signed_headers = "content-type;host;x-tc-action";
        let canonical_request = format!(
            "POST\n/\n\n{canonical_headers}\n{signed_headers}\n{hashed_payload}"
        );
        let hashed_canonical = sha256_hex(&canonical_request);
        let string_to_sign =
            format!("{ALGORITHM}\n{timestamp}\n{credential_scope}\n{hashed_canonical}");

        let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes());
        let secret_service = hmac_sha256(&secret_date, SERVICE.as_bytes());
        let secret_signing = hmac_sha256(&secret_service, b"tc3_request");
        let signature = hmac_sha256_hex(&secret_signing, string_to_sign.as_bytes());

        let authorization = format!(
            "{ALGORITHM} Credential={secret_id}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature}"
        );
        (authorization, date)
    }
}

fn chrono_date_utc(timestamp: u64) -> String {
    // YYYY-MM-DD without pulling chrono dependency.
    const SECONDS_PER_DAY: u64 = 86_400;
    let days = timestamp / SECONDS_PER_DAY;
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Howard Hinnant civil_from_days (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

#[async_trait]
impl TranslationProvider for TencentTmtProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo {
            name: "tencent_tmt",
            group: "china",
            experimental: false,
            local_provider: false,
        }
    }

    async fn translate(&self, request: TranslateRequest<'_>) -> Result<String, ProviderError> {
        let secret_id = http::setting(request.settings, "secret_id");
        let secret_key = http::setting(request.settings, "secret_key");
        if secret_id.is_empty() || secret_key.is_empty() {
            return Err(ProviderError::Message(
                "Tencent TMT secret_id and secret_key are required.".into(),
            ));
        }
        let region = {
            let region = http::setting(request.settings, "region");
            if region.is_empty() {
                "ap-guangzhou".to_string()
            } else {
                region
            }
        };

        let source = normalize_source_lang(request.source_lang);
        let source_lang = if source == "auto" {
            "auto".to_string()
        } else {
            tencent_lang(&source)
        };
        let target_lang = tencent_lang(request.target_lang);
        let body = json!({
            "SourceText": request.text,
            "Source": source_lang,
            "Target": target_lang,
            "ProjectId": 0,
        });
        let payload = serde_json::to_string(&body).unwrap_or_else(|_| "{}".into());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (authorization, _date) =
            Self::authorization(&secret_id, &secret_key, timestamp, &payload);

        // Do not set `Host` manually — reqwest derives it from the URL; the signed
        // canonical request still includes `host:{HOST}` which must match that URL.
        let response = self
            .transport
            .client()
            .post(format!("https://{HOST}"))
            .timeout(http::effective_request_timeout(request.timeout_secs))
            .header("Authorization", authorization)
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-TC-Action", ACTION)
            .header("X-TC-Timestamp", timestamp.to_string())
            .header("X-TC-Version", VERSION)
            .header("X-TC-Region", region)
            .body(payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let detail = http::truncate_error_body(&body, 280);
            return Err(ProviderError::Message(format!(
                "Tencent TMT HTTP {status}: {detail}"
            )));
        }

        let payload: Value = response.json().await?;
        if let Some(error) = payload.pointer("/Response/Error") {
            let code = error
                .get("Code")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");
            let message = error
                .get("Message")
                .and_then(|v| v.as_str())
                .unwrap_or("request failed");
            return Err(ProviderError::Message(format!(
                "Tencent TMT error {code}: {message}"
            )));
        }

        let translated = payload
            .pointer("/Response/TargetText")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if translated.is_empty() {
            return Err(ProviderError::Message(
                "Tencent TMT returned an empty translation.".into(),
            ));
        }
        Ok(translated)
    }

    fn diagnostics(&self, settings: &HashMap<String, String>) -> Value {
        let secret_id = http::setting(settings, "secret_id");
        let secret_key = http::setting(settings, "secret_key");
        let region = http::setting(settings, "region");
        let mut diag = base_diagnostics(&self.info(), settings);
        if let Some(obj) = diag.as_object_mut() {
            obj.insert("secret_id_present".into(), json!(!secret_id.is_empty()));
            obj.insert(
                "secret_key_masked_preview".into(),
                json!(mask_secret(&secret_key)),
            );
            obj.insert(
                "region".into(),
                json!(if region.is_empty() {
                    "ap-guangzhou"
                } else {
                    region.as_str()
                }),
            );
            obj.insert(
                "status_message".into(),
                json!("Tencent Cloud Machine Translation (TextTranslate). Generous free monthly character quota after service activation."),
            );
        }
        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_known_epoch() {
        // 1970-01-01
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2024-01-01 = 19731 days since epoch? 1970 to 2024 = 54*365 + leap days
        assert_eq!(chrono_date_utc(1_704_067_200), "2024-01-01");
    }
}
