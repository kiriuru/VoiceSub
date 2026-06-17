//! Main dashboard WebView entry URL — must match embedded Axum static host.

use std::net::SocketAddr;

/// HTTP URL the Tauri shell navigates to after loading bundled assets (`lib.rs` setup).
pub fn main_dashboard_http_url(bind_addr: SocketAddr) -> String {
    format!("http://{}:{}/", bind_addr.ip(), bind_addr.port())
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddrV4};

    use super::*;

    #[test]
    fn main_dashboard_url_uses_runtime_bind_addr() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8765).into();
        assert_eq!(main_dashboard_http_url(addr), "http://127.0.0.1:8765/");
    }

    #[test]
    fn main_dashboard_url_preserves_custom_port() {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 9123).into();
        assert_eq!(main_dashboard_http_url(addr), "http://127.0.0.1:9123/");
    }
}
