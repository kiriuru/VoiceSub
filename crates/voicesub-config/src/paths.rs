use std::ffi::OsStr;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde_json::Value;

/// Canonical browser worker page (SST-compatible path).
pub const WORKER_PATH: &str = "/google-asr";

/// SST 0.4.x persisted settings file inside `user-data/`.
pub const LEGACY_SST_CONFIG_JSON: &str = "config.json";

/// VoiceSub runtime settings file inside `user-data/`.
pub const RUNTIME_CONFIG_TOML: &str = "config.toml";

fn has_bin_layout(bin_dir: &Path) -> bool {
    bin_dir.join("overlay").join("overlay.html").is_file()
}

fn scan_for_bin_dir(mut start: PathBuf) -> Option<PathBuf> {
    for _ in 0..12 {
        for candidate in [start.join("bin"), start.join("resources").join("bin")] {
            if has_bin_layout(&candidate) {
                return Some(candidate);
            }
        }
        if !start.pop() {
            break;
        }
    }
    None
}

/// Locate shipped static tree (`bin/` or Tauri MSI `resources/bin/`).
pub fn locate_bin_dir() -> Option<PathBuf> {
    if let Ok(cwd) = std::env::current_dir()
        && let Some(bin_dir) = scan_for_bin_dir(cwd)
    {
        return Some(bin_dir);
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
        && let Some(bin_dir) = scan_for_bin_dir(parent.to_path_buf())
    {
        return Some(bin_dir);
    }

    None
}

/// Install / workspace root for `user-data/` and `logs/` (parent of `bin/`, skipping `resources/`).
pub fn install_root_from_bin_dir(bin_dir: &Path) -> PathBuf {
    let parent = bin_dir.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name() == Some(OsStr::new("resources")) {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

/// Resolve install/dev project root: MSI install dir, workspace (walk up from cwd/exe).
pub fn discover_project_root() -> PathBuf {
    locate_bin_dir()
        .map(|bin| install_root_from_bin_dir(&bin))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub project_root: PathBuf,
    pub bin_dir: PathBuf,
    pub overlay_root: PathBuf,
    pub worker_dist: PathBuf,
    pub tts_dist: PathBuf,
    pub local_asr_dist: PathBuf,
    pub dashboard_dist: PathBuf,
    pub fonts_dir: PathBuf,
    pub modules_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub logs_dir: PathBuf,
}

impl ProjectPaths {
    pub fn discover(project_root: impl Into<PathBuf>) -> Self {
        if let Some(bin_dir) = locate_bin_dir() {
            let project_root = install_root_from_bin_dir(&bin_dir);
            return Self::from_roots(project_root, bin_dir);
        }

        let project_root = project_root.into();
        Self::from_roots(project_root.clone(), project_root.join("bin"))
    }

    fn from_roots(project_root: PathBuf, bin_dir: PathBuf) -> Self {
        Self {
            overlay_root: bin_dir.join("overlay"),
            worker_dist: bin_dir.join("worker"),
            tts_dist: bin_dir.join("tts"),
            local_asr_dist: bin_dir.join("local-asr"),
            dashboard_dist: bin_dir.join("dashboard"),
            fonts_dir: bin_dir.join("fonts"),
            modules_dir: bin_dir.join("modules"),
            user_data_dir: project_root.join("user-data"),
            logs_dir: project_root.join("logs"),
            bin_dir,
            project_root,
        }
    }

    pub fn tts_module_dir(&self) -> PathBuf {
        self.modules_dir.join("tts")
    }

    pub fn local_asr_module_dir(&self) -> PathBuf {
        self.modules_dir.join("local-asr")
    }

    pub fn local_asr_user_module_dir(&self) -> PathBuf {
        self.user_data_dir.join("modules").join("local-asr")
    }

    pub fn config_toml_path(&self) -> PathBuf {
        self.user_data_dir.join(RUNTIME_CONFIG_TOML)
    }

    pub fn legacy_sst_config_json_path(&self) -> PathBuf {
        self.user_data_dir.join(LEGACY_SST_CONFIG_JSON)
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.user_data_dir.join("profiles")
    }
}

/// Create runtime-only folders on application start. Installers must not ship or manage these.
pub fn ensure_runtime_data_dirs(paths: &ProjectPaths) -> std::io::Result<()> {
    std::fs::create_dir_all(&paths.user_data_dir)?;
    std::fs::create_dir_all(&paths.logs_dir)?;
    std::fs::create_dir_all(&paths.local_asr_user_module_dir())?;
    Ok(())
}

pub fn worker_url_for_base(base: &str) -> String {
    format!("{}{}", base.trim_end_matches('/'), WORKER_PATH)
}

pub fn worker_url_for_payload(base: &str, payload: &Value) -> String {
    let mut url = worker_url_for_base(base);
    let mut params = vec!["autostart=1".to_string()];
    if let Some(lang) = payload
        .get("ui")
        .and_then(|ui| ui.get("language"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let normalized = lang.to_ascii_lowercase();
        if ["en", "ru", "ja", "ko", "zh"].contains(&normalized.as_str()) {
            params.push(format!("locale={normalized}"));
        }
    }
    url.push('?');
    url.push_str(&params.join("&"));
    url
}

#[derive(Debug, Clone)]
pub struct HttpBindConfig {
    pub host: IpAddr,
    pub port: u16,
}

impl Default for HttpBindConfig {
    fn default() -> Self {
        Self {
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8765,
        }
    }
}

impl HttpBindConfig {
    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    pub fn base_url(&self) -> String {
        format!("http://{}:{}", self.host, self.port)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub http: HttpBindConfig,
}

pub fn worker_url(config: &AppConfig) -> String {
    worker_url_for_base(&config.http.base_url())
}

pub fn overlay_url(base: &str) -> String {
    format!("{}/overlay", base.trim_end_matches('/'))
}

pub fn base_url_from_socket(addr: std::net::SocketAddr) -> String {
    format!("http://{addr}")
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::path::Path;

    use super::*;

    #[test]
    fn default_bind_is_localhost_8765() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.http.socket_addr().port(), 8765);
        assert_eq!(cfg.http.host, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn worker_url_uses_canonical_google_asr_path() {
        let cfg = AppConfig::default();
        assert!(worker_url(&cfg).ends_with("/google-asr"));
    }

    #[test]
    fn worker_url_for_payload_includes_autostart() {
        let payload = serde_json::json!({
            "asr": { "browser": { "recognition_language": "ru-RU" } }
        });
        let url = worker_url_for_payload("http://127.0.0.1:9123", &payload);
        assert_eq!(url, "http://127.0.0.1:9123/google-asr?autostart=1");
    }

    #[test]
    fn worker_url_for_payload_includes_locale_from_ui_language() {
        let payload = serde_json::json!({
            "ui": { "language": "ru" },
            "asr": { "browser": { "recognition_language": "ru-RU" } }
        });
        let url = worker_url_for_payload("http://127.0.0.1:9123", &payload);
        assert_eq!(
            url,
            "http://127.0.0.1:9123/google-asr?autostart=1&locale=ru"
        );
    }

    #[test]
    fn discover_project_root_finds_workspace_when_cwd_has_bin_overlay() {
        let root = discover_project_root();
        assert!(
            has_bin_layout(&root.join("bin")),
            "expected bin/overlay/overlay.html under {}",
            root.join("bin").display()
        );
    }

    #[test]
    fn install_root_from_tauri_resources_bin() {
        let bin = Path::new("C:/Program Files/VoiceSub/resources/bin");
        let root = install_root_from_bin_dir(bin);
        assert_eq!(root, Path::new("C:/Program Files/VoiceSub"));
    }

    #[test]
    fn install_root_from_program_files_x86_resources_bin() {
        let bin = Path::new("C:/Program Files (x86)/VoiceSub/resources/bin");
        let root = install_root_from_bin_dir(bin);
        assert_eq!(root, Path::new("C:/Program Files (x86)/VoiceSub"));
    }

    #[test]
    fn ensure_runtime_data_dirs_creates_user_data_and_logs() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("voicesub-runtime-dirs-{nanos}"));
        let paths = ProjectPaths {
            project_root: root.clone(),
            bin_dir: root.join("bin"),
            overlay_root: root.join("bin/overlay"),
            worker_dist: root.join("bin/worker"),
            tts_dist: root.join("bin/tts"),
            local_asr_dist: root.join("bin/local-asr"),
            dashboard_dist: root.join("bin/dashboard"),
            fonts_dir: root.join("bin/fonts"),
            modules_dir: root.join("bin/modules"),
            user_data_dir: root.join("user-data"),
            logs_dir: root.join("logs"),
        };
        ensure_runtime_data_dirs(&paths).expect("ensure dirs");
        assert!(paths.user_data_dir.is_dir());
        assert!(paths.logs_dir.is_dir());
        assert!(paths.local_asr_user_module_dir().is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn paths_resolve_under_project_root() {
        let paths = ProjectPaths::discover(Path::new("F:/AI/VoiceSub"));
        assert!(paths.bin_dir.ends_with("bin"));
        assert!(
            paths.overlay_root.ends_with("bin/overlay")
                || paths.overlay_root.ends_with("bin\\overlay")
        );
        assert!(
            paths.worker_dist.ends_with("bin/worker") || paths.worker_dist.ends_with("bin\\worker")
        );
        assert!(
            paths.tts_module_dir().ends_with("bin/modules/tts")
                || paths.tts_module_dir().ends_with("bin\\modules\\tts")
        );
        assert!(paths.config_toml_path().ends_with("user-data/config.toml"));
    }
}
