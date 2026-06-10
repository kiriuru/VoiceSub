use std::path::PathBuf;

use serde_json::{json, Value};

use crate::atomic_io::atomic_write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("invalid profile name")]
    InvalidName,
    #[error("profile not found: {0}")]
    NotFound(String),
    #[error("default profile cannot be deleted")]
    CannotDeleteDefault,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone)]
pub struct ProfileStore {
    profiles_dir: PathBuf,
}

impl ProfileStore {
    pub fn new(profiles_dir: impl Into<PathBuf>) -> Self {
        Self {
            profiles_dir: profiles_dir.into(),
        }
    }

    pub fn ensure_ready(&self) -> Result<(), ProfileError> {
        std::fs::create_dir_all(&self.profiles_dir)?;
        Ok(())
    }

    pub fn list_profiles(&self) -> Result<Vec<String>, ProfileError> {
        self.ensure_ready()?;
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.profiles_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn load_profile(&self, name: &str) -> Result<Value, ProfileError> {
        let path = self.profile_path(name)?;
        if !path.is_file() {
            return Err(ProfileError::NotFound(name.to_string()));
        }
        let raw = std::fs::read_to_string(path)?;
        let mut payload: Value = serde_json::from_str(&raw)?;
        if let Some(obj) = payload.as_object_mut() {
            obj.remove("name");
            obj.insert("profile".into(), json!(name));
        }
        Ok(payload)
    }

    pub fn save_profile(
        &self,
        name: &str,
        payload: &Value,
    ) -> Result<(PathBuf, Value), ProfileError> {
        let path = self.profile_path(name)?;
        self.ensure_ready()?;
        let mut stored = payload.clone();
        if let Some(obj) = stored.as_object_mut() {
            obj.remove("name");
            obj.insert("profile".into(), json!(name));
        }
        let text = serde_json::to_string_pretty(&stored)?;
        atomic_write(&path, &text)?;
        Ok((path, stored))
    }

    pub fn delete_profile(&self, name: &str) -> Result<bool, ProfileError> {
        if name == "default" {
            return Err(ProfileError::CannotDeleteDefault);
        }
        let path = self.profile_path(name)?;
        if !path.is_file() {
            return Ok(false);
        }
        std::fs::remove_file(path)?;
        Ok(true)
    }

    pub fn ensure_default_profile(&self) -> Result<(), ProfileError> {
        let path = self.profiles_dir.join("default.json");
        if path.is_file() {
            return Ok(());
        }
        self.save_profile(
            "default",
            &json!({
                "source_lang": "auto",
                "targets": ["en"]
            }),
        )?;
        Ok(())
    }

    fn profile_path(&self, name: &str) -> Result<PathBuf, ProfileError> {
        let raw = name.trim();
        if raw.is_empty()
            || raw == "."
            || raw == ".."
            || raw.contains("..")
            || raw.contains('/')
            || raw.contains('\\')
        {
            return Err(ProfileError::InvalidName);
        }
        Ok(self.profiles_dir.join(format!("{raw}.json")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_profiles_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("voicesub-profiles-{nanos}"))
    }

    #[test]
    fn roundtrip_save_load_delete() {
        let dir = temp_profiles_dir();
        let store = ProfileStore::new(&dir);
        store.ensure_default_profile().unwrap();
        let names = store.list_profiles().unwrap();
        assert!(names.contains(&"default".to_string()));

        store
            .save_profile("stream", &json!({ "translation": { "enabled": true } }))
            .unwrap();
        let loaded = store.load_profile("stream").unwrap();
        assert_eq!(loaded["profile"], "stream");
        assert_eq!(loaded["translation"]["enabled"], true);

        assert!(store.delete_profile("stream").unwrap());
        assert!(store.load_profile("stream").is_err());
        assert!(store.delete_profile("default").is_err());

        let _ = std::fs::remove_dir_all(dir);
    }
}
