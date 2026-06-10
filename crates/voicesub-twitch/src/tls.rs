use std::sync::Once;

static INIT: Once = Once::new();

/// Select rustls crypto backend before any TLS client is built.
/// Required when both `ring` (Twitch IRC) and `aws-lc-rs` (reqwest) are linked.
pub fn init_crypto_provider() {
    INIT.call_once(|| {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install rustls ring crypto provider");
    });
}
