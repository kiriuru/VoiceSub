//! TOML config, SST JSON import, and project paths.

mod atomic_io;
mod bind_policy;
mod defaults;
mod document;
mod fonts;
mod logging_preferences;
mod migrate;
mod obs_normalize;
mod normalize;
mod paths;
mod profile_store;
mod secrets;
mod store;
mod translation_normalize;
mod translation_provider_settings;

pub use bind_policy::{allow_lan_from_env, http_bind_from_env, resolve_bind_host};
pub use defaults::{default_config_payload, CURRENT_CONFIG_VERSION};
pub use document::ConfigDocument;
pub use fonts::{build_font_catalog, build_project_fonts_stylesheet, list_project_font_entries};
pub use logging_preferences::{
    normalize_logging_config, read_full_logging_enabled, read_full_logging_enabled_from_config_path,
    read_full_logging_enabled_from_user_data,
};
pub use migrate::{apply_voicesub_import_rules, import_sst_json_value, migrate_sst_payload};
pub use normalize::{normalize_config_payload, repair_legacy_keep_completed_false};
pub use paths::{
    base_url_from_socket, discover_project_root, ensure_runtime_data_dirs, install_root_from_bin_dir,
    locate_bin_dir, overlay_url, worker_url, worker_url_for_base, worker_url_for_payload, AppConfig,
    HttpBindConfig, ProjectPaths, LEGACY_SST_CONFIG_JSON, RUNTIME_CONFIG_TOML, WORKER_PATH,
};
pub use profile_store::{ProfileError, ProfileStore};
pub use secrets::{
    normalize_google_translate_api_key, normalize_provider_secret, normalize_provider_text_value,
};
pub use store::{ConfigError, ConfigStore};
pub use translation_normalize::{
    default_translation_provider_settings, normalize_translation_config,
    normalize_translation_provider_settings, SUPPORTED_TRANSLATION_PROVIDERS,
};
pub use voicesub_types::PROJECT_VERSION;

pub fn product_version() -> &'static str {
    PROJECT_VERSION
}
