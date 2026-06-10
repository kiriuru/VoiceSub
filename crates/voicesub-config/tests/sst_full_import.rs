use std::fs;
use std::path::PathBuf;

use voicesub_config::ConfigStore;

#[test]
fn imports_full_sst_example_config_json() {
    let json_path =
        PathBuf::from(r"F:\AI\stream-sub-translator\backend\data\config.example.json");
    let raw = fs::read_to_string(&json_path).expect("read sst example");
    let dir = std::env::temp_dir().join(format!("voicesub-sst-full-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.json"), raw).unwrap();
    let toml_path = dir.join("config.toml");
    let mut store = ConfigStore::new(&toml_path);
    store.load_or_create().expect("import full sst example");
    assert!(toml_path.is_file());
    let _ = fs::remove_dir_all(dir);
}
