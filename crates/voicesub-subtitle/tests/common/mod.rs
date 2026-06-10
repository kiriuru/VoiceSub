use std::fs;
use std::path::PathBuf;

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

pub fn read_workspace_file(rel: &str) -> String {
    fs::read_to_string(workspace_root().join(rel)).unwrap_or_else(|e| {
        panic!("failed to read `{rel}`: {e}");
    })
}

pub fn assert_contains(haystack: &str, needle: &str, context: &str) {
    assert!(
        haystack.contains(needle),
        "{context}: expected substring `{needle}`"
    );
}

#[allow(dead_code)]
pub fn assert_not_contains(haystack: &str, needle: &str, context: &str) {
    assert!(
        !haystack.contains(needle),
        "{context}: unexpected substring `{needle}`"
    );
}

#[allow(dead_code)]
pub fn count_innerhtml_wipe_statements(source: &str) -> usize {
    source
        .lines()
        .filter(|line| line.trim() == r#"container.innerHTML = "";"#)
        .count()
}

#[allow(dead_code)]
pub fn slice_from_function(source: &str, name: &str, max_len: usize) -> String {
    let marker = format!("function {name}");
    let index = source
        .find(&marker)
        .unwrap_or_else(|| panic!("function `{name}` not found"));
    source[index..index.saturating_add(max_len).min(source.len())].to_string()
}
