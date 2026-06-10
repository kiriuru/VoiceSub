//! Developer tasks: `cargo run -p xtask -- export-golden`, `prune-target`.

mod prune_target;

use std::fs;
use std::path::{Path, PathBuf};

use prune_target::{prune_dev_caches, PruneOptions};

use serde_json::json;
use voicesub_config::{import_sst_json_value, migrate_sst_payload};
use voicesub_types::{AsrWorkerHello, ExternalAsrUpdate, WsMessage};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn export_golden(root: &Path) -> std::io::Result<()> {
    let out = root.join("tests/golden");
    fs::create_dir_all(&out)?;

    write_fixture(
        &out.join("ws_events_hello.json"),
        &json!({
            "source_test": "test_ws_manager.py::hello_contract",
            "input": {},
            "expected": WsMessage::hello_events(),
        }),
    )?;

    write_fixture(
        &out.join("ws_asr_worker_hello.json"),
        &json!({
            "source_test": "test_browser_asr_service.py::send_hello",
            "input": { "transport_id": 1 },
            "expected": AsrWorkerHello::new(1),
        }),
    )?;

    write_fixture(
        &out.join("external_asr_update_partial.json"),
        &json!({
            "source_test": "test_browser_asr_service.py::handle_external_update",
            "input": {
                "type": "external_asr_update",
                "partial": "hello",
                "final": "",
                "is_final": false,
                "generation_id": 1,
                "session_id": "sess-1"
            },
            "expected": {
                "transcript_text": "hello",
                "is_final": false
            }
        }),
    )?;

    write_fixture(
        &out.join("external_asr_update_final.json"),
        &json!({
            "source_test": "test_browser_asr_service.py::handle_external_update",
            "input": {
                "type": "external_asr_update",
                "partial": "",
                "final": "hello world",
                "is_final": true,
                "generation_id": 1,
                "session_id": "sess-1"
            },
            "expected": {
                "transcript_text": "hello world",
                "is_final": true
            }
        }),
    )?;

    write_fixture(
        &out.join("config_migrate_legacy_targets.json"),
        &json!({
            "source_test": "test_config_migrations.py::test_old_config_without_version_migrates_to_current_schema",
            "input": {
                "targets": ["en", "ja"],
                "translation": {
                    "enabled": true,
                    "provider": "google_translate_v2"
                }
            },
            "expected": migrate_sst_payload(json!({
                "targets": ["en", "ja"],
                "translation": {
                    "enabled": true,
                    "provider": "google_translate_v2"
                }
            }))
        }),
    )?;

    write_fixture(
        &out.join("config_import_local_asr.json"),
        &json!({
            "source_test": "voicesub roadmap §9",
            "input": {
                "config_version": 3,
                "asr": { "mode": "local" },
                "remote": { "enabled": true }
            },
            "expected": import_sst_json_value(json!({
                "config_version": 3,
                "asr": { "mode": "local" },
                "remote": { "enabled": true }
            }))
        }),
    )?;

    // Semantic check fixture for ExternalAsrUpdate parser
    let partial: ExternalAsrUpdate = serde_json::from_value(json!({
        "type": "external_asr_update",
        "partial": "hel",
        "final": "",
        "is_final": false,
        "generation_id": 2
    }))
    .expect("partial fixture parses");
    assert_eq!(partial.transcript_text(), "hel");

    println!("exported golden fixtures to {}", out.display());
    Ok(())
}

fn write_fixture(path: &Path, value: &serde_json::Value) -> std::io::Result<()> {
    let pretty = serde_json::to_string_pretty(value).expect("serialize fixture");
    fs::write(path, pretty)?;
    Ok(())
}

fn parse_prune_args(args: &[String]) -> PruneOptions {
    let mut options = PruneOptions {
        if_needed_bytes: None,
        dry_run: false,
    };
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--if-needed" => {
                let gb: f64 = args
                    .get(i + 1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5.0);
                options.if_needed_bytes = Some((gb * 1_073_741_824.0) as u64);
                i += 2;
            }
            "--dry-run" => {
                options.dry_run = true;
                i += 1;
            }
            other => {
                eprintln!("unknown prune-target flag: {other}");
                std::process::exit(2);
            }
        }
    }
    options
}

fn run_prune_target(root: &Path, args: &[String]) -> std::io::Result<()> {
    let options = parse_prune_args(args);
    let target = root.join("target");
    let target = target.is_dir().then_some(target.as_path());
    let report = prune_dev_caches(target, options)?;
    if report.removed_dirs == 0 {
        println!(
            "dev caches: {} bytes incremental across {} dirs; nothing stale to remove",
            report.incremental_bytes_before, report.scanned_dirs
        );
        return Ok(());
    }
    println!(
        "pruned {} stale dirs ({} bytes freed; incremental was {} bytes)",
        report.removed_dirs, report.freed_bytes, report.incremental_bytes_before
    );
    Ok(())
}

fn main() {
    let root = workspace_root();
    let cmd = std::env::args().nth(1).unwrap_or_default();
    let extra: Vec<String> = std::env::args().skip(2).collect();
    let result = match cmd.as_str() {
        "export-golden" => export_golden(&root),
        "prune-target" => run_prune_target(&root, &extra),
        other => {
            eprintln!("usage:");
            eprintln!("  cargo run -p xtask -- export-golden");
            eprintln!("  cargo run -p xtask -- prune-target [--if-needed GB] [--dry-run]");
            eprintln!("unknown command: {other}");
            std::process::exit(2);
        }
    };
    if let Err(err) = result {
        eprintln!("xtask failed: {err}");
        std::process::exit(1);
    }
}
