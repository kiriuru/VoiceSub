use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .expect("workspace root")
}

fn worker_src_root() -> PathBuf {
    workspace_root().join("src-worker")
}

fn read_manager_ts_bundle() -> String {
    let root = worker_src_root();
    let manager =
        fs::read_to_string(root.join("lib/asr/session-manager.ts")).expect("session-manager");
    let logic_dir = root.join("lib/asr");
    let mut logic = String::new();
    let mut paths: Vec<PathBuf> = fs::read_dir(&logic_dir)
        .expect("asr dir")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "ts")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !name.ends_with(".test.ts"))
        })
        .collect();
    paths.sort();
    for path in paths {
        logic.push_str(&fs::read_to_string(path).expect("logic ts"));
        logic.push('\n');
    }
    format!("{manager}\n{logic}")
}

fn read_policy_ts() -> String {
    fs::read_to_string(worker_src_root().join("lib/asr/web-speech-policy.ts")).expect("policy ts")
}

fn read_worker_app() -> String {
    fs::read_to_string(worker_src_root().join("WorkerApp.svelte")).expect("WorkerApp.svelte")
}

fn assert_contains(haystack: &str, needle: &str, context: &str) {
    assert!(
        haystack.contains(needle),
        "{context}: expected substring `{needle}`"
    );
}

#[test]
fn worker_app_visibility_warning_present() {
    let app = read_worker_app();
    assert_contains(&app, "visibilityWarning", "visibility warning binding");
    assert_contains(&app, "worker.warning.title", "visibility warning title");
}

#[test]
fn session_manager_supports_overlap_and_watchdog_defaults() {
    let manager = read_manager_ts_bundle();
    let policy = read_policy_ts();
    assert_contains(&manager, "createOverlapRecognitionPair", "overlap");
    assert_contains(&policy, "shouldEnableRecognitionOverlap", "overlap policy");
    assert_contains(&manager, "watchdog_stall: 750", "watchdog timing");
    assert_contains(&manager, "maxBrowserSessionAgeMs", "session age");
    assert_contains(&manager, "180000", "session age limit");
    assert_contains(&manager, "acquireWakeLock", "wake lock");
    assert_contains(&manager, "wake-lock-bridge", "wake lock bridge");
}

#[test]
fn session_manager_terminal_errors_block_automatic_restart() {
    let manager = read_manager_ts_bundle();
    for needle in [
        "\"not-allowed\"",
        "\"service-not-allowed\"",
        "phrases-not-supported",
        "language-not-supported",
    ] {
        assert_contains(&manager, needle, "terminal errors");
    }
}

#[test]
fn session_manager_retries_audio_capture_with_backoff() {
    let manager = read_manager_ts_bundle();
    for needle in [
        "\"audio_capture\"",
        "audioCaptureRetryHintMessage",
        "releaseMicrophoneMonitor",
    ] {
        assert_contains(&manager, needle, "audio capture retry");
    }
}

#[test]
fn built_worker_index_uses_worker_assets_prefix() {
    let index_path = workspace_root().join("bin/worker/index.html");
    if !index_path.is_file() {
        return;
    }
    let index = fs::read_to_string(index_path).expect("bin/worker index");
    assert_contains(&index, "/worker-assets/", "vite base path");
    assert!(
        !index.contains("/worker-svelte-assets/"),
        "legacy worker asset prefix must be gone"
    );
}
