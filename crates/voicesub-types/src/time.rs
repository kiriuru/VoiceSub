//! RFC 3339 UTC timestamps without a date library (Howard Hinnant civil-from-days).

use std::time::{SystemTime, UNIX_EPOCH};

/// Format a Unix timestamp (seconds) as an RFC 3339 UTC string.
pub fn epoch_secs_to_rfc3339(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let tod = (secs % 86_400) as i64;
    let (hour, minute, second) = (tod / 3600, (tod % 3600) / 60, tod % 60);

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Current wall time as RFC 3339 UTC.
pub fn utc_now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    epoch_secs_to_rfc3339(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_secs_to_rfc3339_formats_known_timestamps() {
        assert_eq!(epoch_secs_to_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(epoch_secs_to_rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(epoch_secs_to_rfc3339(1_709_209_096), "2024-02-29T12:18:16Z");
    }
}
