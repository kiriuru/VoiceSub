use std::sync::OnceLock;

/// Shared tokio runtime for unit tests and non-Tauri callers.
pub fn shared_handle() -> tokio::runtime::Handle {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("voicesub-tts-shared")
                .build()
                .expect("voicesub-tts shared runtime")
        })
        .handle()
        .clone()
}
