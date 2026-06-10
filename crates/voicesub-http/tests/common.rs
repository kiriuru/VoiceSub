use std::sync::{Arc, Mutex, OnceLock};

use tempfile::TempDir;
use voicesub_config::{AppConfig, HttpBindConfig};
use voicesub_runtime::{RuntimeHandle, RuntimeService};
use voicesub_tts::TwitchOAuthBridge;

static INTEGRATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Serializes HTTP integration tests — they previously shared `user-data/config.toml`.
pub fn integration_lock() -> std::sync::MutexGuard<'static, ()> {
    INTEGRATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

pub fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

pub struct EphemeralRuntime {
    pub service: RuntimeService,
    _temp: TempDir,
}

impl Default for EphemeralRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl EphemeralRuntime {
    pub fn new() -> Self {
        let temp = TempDir::new().expect("tempdir");
        let user_data = temp.path().join("user-data");
        std::fs::create_dir_all(&user_data).expect("user-data dir");
        let mut config = AppConfig::default();
        config.http = HttpBindConfig {
            host: config.http.host,
            port: 0,
        };
        let service = RuntimeService::with_config_isolated_user_data(
            workspace_root(),
            user_data,
            config,
            Arc::new(TwitchOAuthBridge::default()),
        );
        Self {
            service,
            _temp: temp,
        }
    }

    pub async fn start(&self) -> RuntimeHandle {
        self.service.start().await.expect("start runtime")
    }
}
