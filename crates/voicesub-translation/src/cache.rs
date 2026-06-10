use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tracing::warn;

pub(crate) const DEFAULT_MAX_ENTRIES: usize = 5000;
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug)]
struct CacheState {
    entries: HashMap<String, String>,
    order: VecDeque<String>,
    max_entries: usize,
    enabled: bool,
    persist: bool,
    dirty: bool,
    loaded: bool,
    cache_file: Option<PathBuf>,
    last_flush_scheduled: Option<Instant>,
}

impl CacheState {
    fn new(cache_dir: Option<PathBuf>, max_entries: usize) -> Self {
        let cache_file = cache_dir.map(|dir| dir.join("translation_cache.json"));
        if let Some(ref path) = cache_file {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if !path.exists() {
                let _ = fs::write(path, "{}");
            }
        }
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
            enabled: true,
            persist: cache_file.is_some(),
            dirty: false,
            loaded: false,
            cache_file,
            last_flush_scheduled: None,
        }
    }

    fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let Some(path) = self.cache_file.clone() else {
            return;
        };
        let raw = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return,
        };
        let payload: HashMap<String, String> = match serde_json::from_str(&raw) {
            Ok(map) => map,
            Err(_) => {
                let _ = fs::write(&path, "{}");
                return;
            }
        };
        for (key, value) in payload {
            self.insert_locked(key, value, false);
        }
        self.dirty = false;
    }

    fn touch_locked(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|item| item == key) {
            self.order.remove(pos);
        }
        self.order.push_back(key.to_string());
    }

    fn evict_locked(&mut self) {
        if self.max_entries == 0 {
            self.entries.clear();
            self.order.clear();
            self.dirty = true;
            return;
        }
        while self.entries.len() > self.max_entries {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
                self.dirty = true;
            } else {
                break;
            }
        }
    }

    fn insert_locked(&mut self, key: String, value: String, mark_dirty: bool) {
        if self.max_entries == 0 {
            return;
        }
        if self.entries.contains_key(&key) {
            if self.entries.get(&key) == Some(&value) {
                self.touch_locked(&key);
                return;
            }
            self.entries.insert(key.clone(), value);
            self.touch_locked(&key);
        } else {
            self.entries.insert(key.clone(), value);
            self.order.push_back(key);
            self.evict_locked();
        }
        if mark_dirty {
            self.dirty = true;
        }
    }

    fn get_locked(&mut self, key: &str) -> Option<String> {
        let value = self.entries.get(key).cloned()?;
        self.touch_locked(key);
        Some(value)
    }

    fn clear_locked(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.dirty = false;
        if self.persist {
            if let Some(path) = self.cache_file.clone() {
                let _ = Self::write_atomic(&path, &HashMap::new());
            }
        }
    }

    fn write_atomic(path: &Path, payload: &HashMap<String, String>) -> std::io::Result<()> {
        let temp = path.with_extension("tmp");
        let body = serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".into());
        fs::write(&temp, body)?;
        fs::rename(temp, path)?;
        Ok(())
    }

    fn maybe_flush(&mut self) {
        if !self.persist || !self.dirty {
            return;
        }
        let now = Instant::now();
        if let Some(last) = self.last_flush_scheduled {
            if now.duration_since(last) < FLUSH_INTERVAL {
                return;
            }
        }
        self.last_flush_scheduled = Some(now);
        let Some(path) = self.cache_file.clone() else {
            return;
        };
        let snapshot = self.entries.clone();
        self.dirty = false;
        if let Err(err) = Self::write_atomic(&path, &snapshot) {
            warn!(?err, "translation cache flush failed");
            self.dirty = true;
        }
    }
}

#[derive(Debug)]
pub struct TranslationCache {
    state: Mutex<CacheState>,
}

impl TranslationCache {
    pub fn with_dir(cache_dir: Option<PathBuf>, max_entries: usize) -> Self {
        Self {
            state: Mutex::new(CacheState::new(cache_dir, max_entries)),
        }
    }

    pub fn clear(&self) {
        let mut state = self.state.lock().expect("cache lock");
        state.ensure_loaded();
        state.clear_locked();
    }

    pub fn get(&self, key: &str) -> Option<String> {
        let mut state = self.state.lock().expect("cache lock");
        if !state.enabled {
            return None;
        }
        state.ensure_loaded();
        state.get_locked(key)
    }

    pub fn insert(&self, key: String, value: String) {
        let mut state = self.state.lock().expect("cache lock");
        if !state.enabled || state.max_entries == 0 {
            return;
        }
        state.ensure_loaded();
        state.insert_locked(key, value, true);
        state.maybe_flush();
    }

    pub fn update_settings(&self, enabled: bool, persist: bool, max_entries: Option<usize>) {
        let mut state = self.state.lock().expect("cache lock");
        if !enabled {
            state.enabled = false;
            state.clear_locked();
            return;
        }
        state.enabled = true;
        if let Some(max) = max_entries {
            state.max_entries = max.max(1);
            state.evict_locked();
        }
        if !persist {
            state.persist = false;
        } else if state.cache_file.is_some() {
            state.persist = true;
        }
        state.maybe_flush();
    }

    #[cfg(test)]
    fn flush_now(&self) {
        let mut state = self.state.lock().expect("cache lock");
        if !state.persist || !state.dirty {
            return;
        }
        let Some(path) = state.cache_file.clone() else {
            return;
        };
        let snapshot = state.entries.clone();
        state.dirty = false;
        if let Err(err) = CacheState::write_atomic(&path, &snapshot) {
            warn!(?err, "translation cache flush_now failed");
            state.dirty = true;
        }
    }

    pub fn enabled(&self) -> bool {
        self.state.lock().expect("cache lock").enabled
    }
}

pub fn cache_key(
    provider: &str,
    source_lang: &str,
    target_lang: &str,
    source_text: &str,
) -> String {
    format!("{provider}::{source_lang}::{target_lang}::{source_text}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_cache_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("vs-cache-test-{stamp}"))
    }

    #[test]
    fn lru_evicts_oldest_entry() {
        let cache = TranslationCache::with_dir(None, 2);
        cache.insert("a".into(), "1".into());
        cache.insert("b".into(), "2".into());
        cache.insert("c".into(), "3".into());
        assert_eq!(cache.get("a"), None);
        assert_eq!(cache.get("b"), Some("2".into()));
        assert_eq!(cache.get("c"), Some("3".into()));
    }

    #[test]
    fn touch_moves_entry_to_recent() {
        let cache = TranslationCache::with_dir(None, 2);
        cache.insert("a".into(), "1".into());
        cache.insert("b".into(), "2".into());
        assert_eq!(cache.get("a"), Some("1".into()));
        cache.insert("c".into(), "3".into());
        assert_eq!(cache.get("b"), None);
        assert_eq!(cache.get("a"), Some("1".into()));
    }

    #[test]
    fn persist_roundtrip() {
        let dir = temp_cache_dir();
        let cache = TranslationCache::with_dir(Some(dir.clone()), DEFAULT_MAX_ENTRIES);
        cache.insert(
            cache_key("google_translate_v2", "en", "fr", "hello"),
            "bonjour".into(),
        );
        cache.flush_now();
        let reloaded = TranslationCache::with_dir(Some(dir.clone()), DEFAULT_MAX_ENTRIES);
        assert_eq!(
            reloaded.get(&cache_key("google_translate_v2", "en", "fr", "hello")),
            Some("bonjour".into())
        );
        let _ = fs::remove_dir_all(dir);
    }
}
