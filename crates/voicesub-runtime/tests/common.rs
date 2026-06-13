use std::sync::{Arc, Mutex, Once, OnceLock};

use tempfile::TempDir;
use voicesub_config::{AppConfig, HttpBindConfig};
use voicesub_runtime::{RuntimeHandle, RuntimeService};
use voicesub_tts::TwitchOAuthBridge;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .to_path_buf()
}

static INTEGRATION_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static ENSURE_SKIP_BROWSER: Once = Once::new();

fn ensure_skip_browser_worker_in_tests() {
    ENSURE_SKIP_BROWSER.call_once(|| {
        if matches!(
            std::env::var("VOICESUB_FORCE_BROWSER_WORKER").ok().as_deref(),
            Some("1") | Some("true") | Some("yes")
        ) {
            return;
        }
        unsafe {
            std::env::set_var("VOICESUB_SKIP_BROWSER_WORKER", "1");
        }
    });
}

pub fn integration_lock() -> std::sync::MutexGuard<'static, ()> {
    ensure_skip_browser_worker_in_tests();
    INTEGRATION_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

pub struct EphemeralRuntime {
    service: RuntimeService,
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
