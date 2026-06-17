use std::net::{IpAddr, Ipv4Addr};

/// Port of SST `backend/core/bind_policy.py`.
pub fn resolve_bind_host(explicit_host: Option<&str>, allow_lan: bool) -> IpAddr {
    if let Some(host) = explicit_host.map(str::trim).filter(|h| !h.is_empty()) {
        return host.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    if allow_lan {
        return IpAddr::V4(Ipv4Addr::UNSPECIFIED);
    }
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

/// Reads `VOICESUB_ALLOW_LAN` (same truthy set as SST `SST_ALLOW_LAN`).
///
/// When enabled, HTTP binds `0.0.0.0`. Protected `/api/*` still requires
/// `x-voicesub-token`, but `/ws/events` and `/ws/asr_worker` accept any LAN client.
pub fn allow_lan_from_env() -> bool {
    match std::env::var("VOICESUB_ALLOW_LAN") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => false,
    }
}

pub fn http_bind_from_env(explicit_host: Option<&str>) -> IpAddr {
    resolve_bind_host(explicit_host, allow_lan_from_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_localhost_without_lan() {
        assert_eq!(
            resolve_bind_host(None, false),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
    }

    #[test]
    fn allow_lan_uses_wildcard_bind() {
        assert_eq!(
            resolve_bind_host(None, true),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED)
        );
    }

    #[test]
    fn explicit_host_wins() {
        assert_eq!(
            resolve_bind_host(Some("192.168.1.10"), true),
            "192.168.1.10".parse::<IpAddr>().expect("ip")
        );
    }

    #[test]
    fn empty_explicit_host_falls_back() {
        assert_eq!(
            resolve_bind_host(Some("   "), false),
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        );
    }
}
