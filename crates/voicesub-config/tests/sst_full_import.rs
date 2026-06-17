use std::fs;
use std::path::PathBuf;

use voicesub_config::ConfigStore;

fn sst_example_config_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("data")
        .join("config.example.json")
}

#[test]
fn imports_full_sst_example_config_json() {
    let json_path = sst_example_config_path();
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
