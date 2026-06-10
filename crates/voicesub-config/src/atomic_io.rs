use std::fs;
use std::io;
use std::path::Path;

/// Best-effort atomic replace (temp file in the same directory, then rename).
pub fn atomic_write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config.tmp".into());
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    fs::write(&tmp, contents)?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(err) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(&tmp, path).map_err(|_| err)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn atomic_write_replaces_existing_file() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("voicesub-atomic-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        atomic_write(&path, "first=true\n").unwrap();
        atomic_write(&path, "second=true\n").unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert_eq!(text, "second=true\n");
        let _ = fs::remove_dir_all(dir);
    }
}
