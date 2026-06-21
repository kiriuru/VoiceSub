use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::subtitle_speech::TtsSpeechSettings;
use voicesub_audio::clamp_speech_volume;
use voicesub_twitch::{TwitchTtsSettings, normalize_twitch_settings};

#[derive(Debug, Error)]
pub enum TtsConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml parse error: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("toml serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

pub const TTS_PROVIDER_BROWSER_GOOGLE: &str = "browser_google";
pub const TTS_PROVIDER_PYTHON_STDLIB: &str = "python_stdlib";
pub const PLAYBACK_MODE_NATIVE: &str = "native";
/// Pitch-preserving tempo via libsonic (replaces legacy browser HTMLAudio playback).
pub const PLAYBACK_MODE_SONIC: &str = "sonic";
/// Legacy config value; migrated to [`PLAYBACK_MODE_SONIC`] on load.
pub const PLAYBACK_MODE_BROWSER: &str = "browser";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// `browser_google` (HTTP proxy + browser audio) or `python_stdlib` (urllib script).
    #[serde(default = "default_tts_provider")]
    pub tts_provider: String,
    /// Browser `MediaDeviceInfo.deviceId` from `selectAudioOutput`; empty = default.
    #[serde(default)]
    pub audio_output_device_id: String,
    #[serde(default)]
    pub audio_output_device_label: String,
    /// `native` (Rust/cpal @ 1.0×) or `sonic` (Rust/cpal + libsonic tempo).
    #[serde(default = "default_playback_mode")]
    pub playback_mode: String,
    #[serde(default = "default_rate")]
    pub speech_rate: f32,
    #[serde(default = "default_volume")]
    pub speech_volume: f32,
    #[serde(default)]
    pub speech: TtsSpeechSettings,
    #[serde(default)]
    pub twitch: TwitchTtsSettings,
}

fn default_enabled() -> bool {
    false
}

fn default_tts_provider() -> String {
    TTS_PROVIDER_BROWSER_GOOGLE.to_string()
}

pub fn normalize_tts_provider(provider: &str) -> Option<String> {
    let trimmed = provider.trim();
    match trimmed {
        TTS_PROVIDER_BROWSER_GOOGLE | TTS_PROVIDER_PYTHON_STDLIB => Some(trimmed.to_string()),
        _ => None,
    }
}

fn default_playback_mode() -> String {
    PLAYBACK_MODE_NATIVE.to_string()
}

pub fn normalize_playback_mode(mode: &str) -> Option<String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        PLAYBACK_MODE_NATIVE => Some(PLAYBACK_MODE_NATIVE.to_string()),
        PLAYBACK_MODE_SONIC | PLAYBACK_MODE_BROWSER => Some(PLAYBACK_MODE_SONIC.to_string()),
        _ => None,
    }
}

/// Migrate legacy modes and clamp native playback to 1.0×.
pub fn normalize_tts_config(config: &mut TtsConfig) {
    if let Some(mode) = normalize_playback_mode(&config.playback_mode) {
        config.playback_mode = mode;
    } else {
        config.playback_mode = PLAYBACK_MODE_NATIVE.to_string();
    }
    if config.playback_mode == PLAYBACK_MODE_NATIVE {
        config.speech_rate = 1.0;
        if config.twitch.speech_rate > 0.0 {
            config.twitch.speech_rate = 0.0;
        }
    }
    normalize_speech_settings(&mut config.speech);
    normalize_twitch_settings(&mut config.twitch);
    config.speech_volume = clamp_speech_volume(config.speech_volume);
    if config.twitch.speech_volume >= 0.0 {
        config.twitch.speech_volume = clamp_speech_volume(config.twitch.speech_volume);
    }
}

/// Keep speech planner thresholds aligned with the TTS UI (`min` 1, `max` 32).
pub fn normalize_speech_settings(speech: &mut TtsSpeechSettings) {
    speech.min_chars = speech.min_chars.clamp(1, 32);
    speech.max_queue_items = speech.max_queue_items.clamp(1, 64);
    speech.translation_slots = speech
        .translation_slots
        .iter()
        .map(|slot| slot.trim().to_ascii_lowercase())
        .filter(|slot| !slot.is_empty())
        .collect();
}

fn default_rate() -> f32 {
    1.0
}

fn default_volume() -> f32 {
    1.0
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            tts_provider: default_tts_provider(),
            audio_output_device_id: String::new(),
            audio_output_device_label: String::new(),
            playback_mode: default_playback_mode(),
            speech_rate: default_rate(),
            speech_volume: default_volume(),
            speech: TtsSpeechSettings::default(),
            twitch: TwitchTtsSettings::default(),
        }
    }
}

#[derive(Clone)]
struct CachedConfig {
    config: TtsConfig,
    /// Modified time of `config.toml` when this snapshot was cached. A mismatch on the
    /// next `load()` means the file changed underneath us (manual edit / another instance)
    /// and the cache must be refreshed.
    mtime: Option<SystemTime>,
}

pub struct TtsConfigStore {
    path: PathBuf,
    /// In-memory copy of the persisted (normalized) config to avoid blocking disk
    /// reads on every hot-path call (prefetch / playback / enqueue). Validated against
    /// the file mtime so external edits are picked up without restarting.
    cache: RwLock<Option<CachedConfig>>,
}

impl TtsConfigStore {
    pub fn new(module_dir: impl Into<PathBuf>) -> Self {
        Self {
            path: module_dir.into().join("config.toml"),
            cache: RwLock::new(None),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Best-effort modified time of the backing file (a cheap stat, far cheaper than a
    /// full read+parse on the hot path).
    fn file_mtime(&self) -> Option<SystemTime> {
        fs::metadata(&self.path)
            .and_then(|meta| meta.modified())
            .ok()
    }

    pub fn load(&self) -> Result<TtsConfig, TtsConfigError> {
        let current_mtime = self.file_mtime();
        if let Some(cached) = self.cache.read().expect("tts config cache lock").as_ref()
            && cached.mtime == current_mtime
        {
            return Ok(cached.config.clone());
        }
        let config = self.load_from_disk()?;
        // Re-stat after `load_from_disk` so a defaults/migration write is reflected and the
        // next load hits the cache instead of re-parsing.
        let mtime = self.file_mtime();
        *self.cache.write().expect("tts config cache lock") = Some(CachedConfig {
            config: config.clone(),
            mtime,
        });
        Ok(config)
    }

    fn load_from_disk(&self) -> Result<TtsConfig, TtsConfigError> {
        if !self.path.is_file() {
            let config = TtsConfig::default();
            info!(
                target: "voicesub.tts",
                path = %self.path.display(),
                "tts config missing; writing defaults"
            );
            self.write_to_disk(&config)?;
            return Ok(config);
        }
        let text = fs::read_to_string(&self.path)?;
        let mut config: TtsConfig = toml::from_str(&text)?;
        let before = config.clone();
        normalize_tts_config(&mut config);
        if config != before {
            info!(
                target: "voicesub.tts",
                path = %self.path.display(),
                "tts config migrated (playback mode / native rate)"
            );
            self.write_to_disk(&config)?;
        }
        debug!(
            target: "voicesub.tts",
            path = %self.path.display(),
            enabled = config.enabled,
            "tts config loaded from disk"
        );
        Ok(config)
    }

    pub fn save(&self, config: &TtsConfig) -> Result<(), TtsConfigError> {
        // Persist a normalized copy so we never write an inconsistent config to disk.
        let mut normalized = config.clone();
        normalize_tts_config(&mut normalized);
        self.write_to_disk(&normalized)?;
        let mtime = self.file_mtime();
        *self.cache.write().expect("tts config cache lock") = Some(CachedConfig {
            config: normalized,
            mtime,
        });
        Ok(())
    }

    /// Atomically write the config: serialize to a temp file then rename over the
    /// target so a crash mid-write can never leave a truncated `config.toml`.
    fn write_to_disk(&self, config: &TtsConfig) -> Result<(), TtsConfigError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(config)?;
        let tmp = self.temp_path();
        fs::write(&tmp, text)?;
        if let Err(err) = fs::rename(&tmp, &self.path) {
            let _ = fs::remove_file(&tmp);
            return Err(err.into());
        }
        info!(path = %self.path.display(), "tts module config saved");
        Ok(())
    }

    fn temp_path(&self) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let base = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("config.toml");
        let mut tmp = self.path.clone();
        tmp.set_file_name(format!("{base}.{pid}.{seq}.tmp"));
        tmp
    }

    /// Preserve an unreadable config instead of silently overwriting user data.
    fn backup_corrupt_file(&self) {
        if !self.path.is_file() {
            return;
        }
        let backup = self.path.with_extension("toml.bak");
        match fs::rename(&self.path, &backup) {
            Ok(()) => info!(
                target: "voicesub.tts",
                backup = %backup.display(),
                "backed up unreadable tts config"
            ),
            Err(err) => warn!(
                target: "voicesub.tts",
                error = %err,
                "failed to back up unreadable tts config"
            ),
        }
    }

    pub fn update<F>(&self, mutate: F) -> Result<TtsConfig, TtsConfigError>
    where
        F: FnOnce(&mut TtsConfig),
    {
        let mut config = match self.load() {
            Ok(config) => config,
            Err(err) => {
                // Don't destroy a corrupt config blindly: back it up, then heal
                // forward from defaults so the user keeps a recoverable copy.
                warn!(
                    target: "voicesub.tts",
                    error = %err,
                    "tts config load failed; backing up and resetting to defaults"
                );
                self.backup_corrupt_file();
                TtsConfig::default()
            }
        };
        mutate(&mut config);
        normalize_tts_config(&mut config);
        self.save(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_disabled() {
        assert!(!TtsConfig::default().enabled);
    }

    #[test]
    fn roundtrip_twitch_ignore_users() {
        let dir = std::env::temp_dir().join(format!("voicesub-tts-ignore-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = TtsConfigStore::new(&dir);
        let mut twitch = TwitchTtsSettings::default();
        twitch.ignore_users = vec!["nightbot".into(), "streamelements".into()];
        let config = TtsConfig {
            twitch,
            ..TtsConfig::default()
        };
        store.save(&config).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(
            loaded.twitch.ignore_users,
            vec!["nightbot".to_string(), "streamelements".to_string()]
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_picks_up_external_edits_via_mtime() {
        let dir = std::env::temp_dir().join(format!("voicesub-tts-mtime-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = TtsConfigStore::new(&dir);
        let config = TtsConfig {
            enabled: false,
            ..TtsConfig::default()
        };
        store.save(&config).expect("save");
        assert!(!store.load().expect("load").enabled);

        // Simulate an external edit (manual change / another instance). Sleep past
        // second-granularity mtime resolution so the change is guaranteed observable.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let edited = TtsConfig {
            enabled: true,
            ..TtsConfig::default()
        };
        let text = toml::to_string_pretty(&edited).expect("serialize");
        fs::write(store.path(), text).expect("external write");

        // The cache must be invalidated by the mtime change, not return the stale copy.
        assert!(store.load().expect("reload").enabled);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_browser_to_sonic() {
        let dir = std::env::temp_dir().join(format!("voicesub-tts-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = TtsConfigStore::new(&dir);
        let mut twitch = TwitchTtsSettings::default();
        twitch.speech_rate = 1.5;
        let config = TtsConfig {
            playback_mode: PLAYBACK_MODE_BROWSER.to_string(),
            speech_rate: 1.25,
            twitch,
            ..TtsConfig::default()
        };
        store.save(&config).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded.playback_mode, PLAYBACK_MODE_SONIC);
        assert_eq!(loaded.speech_rate, 1.25);
        assert_eq!(loaded.twitch.speech_rate, 1.5);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn native_mode_clamps_rate_on_load() {
        let dir =
            std::env::temp_dir().join(format!("voicesub-tts-native-clamp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = TtsConfigStore::new(&dir);
        let mut twitch = TwitchTtsSettings::default();
        twitch.speech_rate = 1.5;
        let config = TtsConfig {
            playback_mode: PLAYBACK_MODE_NATIVE.to_string(),
            speech_rate: 1.25,
            twitch,
            ..TtsConfig::default()
        };
        store.save(&config).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded.playback_mode, PLAYBACK_MODE_NATIVE);
        assert_eq!(loaded.speech_rate, 1.0);
        assert_eq!(loaded.twitch.speech_rate, 0.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip_config() {
        let dir = std::env::temp_dir().join(format!("voicesub-tts-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = TtsConfigStore::new(&dir);
        let config = TtsConfig {
            enabled: false,
            tts_provider: TTS_PROVIDER_PYTHON_STDLIB.to_string(),
            playback_mode: PLAYBACK_MODE_NATIVE.to_string(),
            audio_output_device_id: "{test-device}".to_string(),
            audio_output_device_label: "Speakers".to_string(),
            speech_rate: 1.0,
            speech_volume: 0.8,
            speech: TtsSpeechSettings {
                speak_source: true,
                speak_translations: false,
                translation_slots: vec!["translation_1".into()],
                min_chars: 4,
                max_queue_items: 4,
            },
            twitch: TwitchTtsSettings::default(),
        };
        store.save(&config).expect("save");
        let loaded = store.load().expect("load");
        assert_eq!(loaded, config);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migrates_legacy_file_written_directly_to_disk() {
        let dir =
            std::env::temp_dir().join(format!("voicesub-tts-disk-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        // Simulate a legacy on-disk config (bypassing the normalizing save path).
        fs::write(
            dir.join("config.toml"),
            "playback_mode = \"browser\"\nspeech_rate = 1.25\n",
        )
        .expect("write legacy");
        let store = TtsConfigStore::new(&dir);
        let loaded = store.load().expect("load");
        assert_eq!(loaded.playback_mode, PLAYBACK_MODE_SONIC);
        assert_eq!(loaded.speech_rate, 1.25);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn update_backs_up_corrupt_config_instead_of_losing_it() {
        let dir = std::env::temp_dir().join(format!("voicesub-tts-corrupt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        let config_path = dir.join("config.toml");
        fs::write(&config_path, "not-valid-toml [[[").expect("write corrupt");

        let store = TtsConfigStore::new(&dir);
        let updated = store
            .update(|cfg| cfg.enabled = false)
            .expect("update heals forward");
        assert!(!updated.enabled);

        let backup = config_path.with_extension("toml.bak");
        assert!(
            backup.is_file(),
            "corrupt config should be preserved as .bak"
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("read backup"),
            "not-valid-toml [[["
        );
        // The healed config must be valid TOML on reload.
        let reloaded = TtsConfigStore::new(&dir).load().expect("reload");
        assert!(!reloaded.enabled);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_does_not_leave_temp_files() {
        let dir = std::env::temp_dir().join(format!("voicesub-tts-atomic-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = TtsConfigStore::new(&dir);
        store.save(&TtsConfig::default()).expect("save");
        let leftover_tmp = fs::read_dir(&dir)
            .expect("read dir")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".tmp"));
        assert!(!leftover_tmp, "atomic save must not leave .tmp files");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_speech_settings_clamps_min_chars() {
        let mut speech = TtsSpeechSettings {
            min_chars: 0,
            max_queue_items: 0,
            ..TtsSpeechSettings::default()
        };
        normalize_speech_settings(&mut speech);
        assert_eq!(speech.min_chars, 1);
        assert_eq!(speech.max_queue_items, 1);

        speech.min_chars = 99;
        speech.max_queue_items = 999;
        normalize_speech_settings(&mut speech);
        assert_eq!(speech.min_chars, 32);
        assert_eq!(speech.max_queue_items, 64);
    }

    #[test]
    fn normalize_clamps_speech_volume_to_one_hundred_fifty_percent() {
        let mut config = TtsConfig::default();
        config.speech_volume = 2.0;
        config.twitch.speech_volume = 3.0;
        normalize_tts_config(&mut config);
        assert_eq!(config.speech_volume, 1.5);
        assert_eq!(config.twitch.speech_volume, 1.5);
    }
}
