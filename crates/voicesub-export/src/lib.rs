//! Diagnostics export helpers (ZIP bundle).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;
use voicesub_config::ProjectPaths;
use voicesub_logging::{
    BackboneLogs, DEEP_TRACE_LOG_FILES, backbone_log_paths, deep_trace_log_paths, redact_data,
};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

#[derive(Debug, Clone, Serialize)]
pub struct ExportFileInfo {
    pub name: String,
    pub size_bytes: u64,
    pub modified_utc: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportsListResponse {
    pub exports: Vec<String>,
    pub files: Vec<ExportFileInfo>,
}

pub struct ExportService {
    export_dir: PathBuf,
    backbone: BackboneLogs,
    version: &'static str,
}

impl ExportService {
    pub fn new(export_dir: PathBuf, project_root: &Path, version: &'static str) -> Self {
        let _ = fs::create_dir_all(&export_dir);
        Self {
            export_dir,
            backbone: backbone_log_paths(project_root),
            version,
        }
    }

    pub fn from_paths(paths: &ProjectPaths, version: &'static str) -> Self {
        Self::new(
            paths.user_data_dir.join("exports"),
            &paths.project_root,
            version,
        )
    }

    pub fn list_exports(&self) -> io::Result<ExportsListResponse> {
        fs::create_dir_all(&self.export_dir)?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(&self.export_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                entries.push(path);
            }
        }
        entries.sort_by_key(|path| {
            path.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        });
        entries.reverse();

        let files: Vec<ExportFileInfo> = entries
            .iter()
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?.to_string();
                let metadata = path.metadata().ok()?;
                Some(ExportFileInfo {
                    name: name.clone(),
                    size_bytes: metadata.len(),
                    modified_utc: metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_else(|| "0".into()),
                })
            })
            .collect();
        let exports = files.iter().map(|f| f.name.clone()).collect();
        Ok(ExportsListResponse { exports, files })
    }

    pub fn export_diagnostics_bundle(
        &self,
        runtime_status: Value,
        config_payload: Value,
        paths: &ProjectPaths,
        base_url: &str,
        include_deep_traces: bool,
    ) -> io::Result<PathBuf> {
        fs::create_dir_all(&self.export_dir)?;
        let bundle_name = format!("diagnostics-{}.zip", utc_stamp());
        let bundle_path = self.export_dir.join(&bundle_name);

        let file = File::create(&bundle_path)?;
        let mut zip = ZipWriter::new(file);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        write_json_entry(&mut zip, "runtime_status.json", &runtime_status, options)?;
        write_json_entry(
            &mut zip,
            "config_redacted.json",
            &redact_data(&config_payload),
            options,
        )?;
        write_json_entry(
            &mut zip,
            "diagnostics-manifest.json",
            &self.build_manifest(include_deep_traces),
            options,
        )?;
        zip.start_file("environment.txt", options)?;
        zip.write_all(self.build_environment_text(paths, base_url).as_bytes())?;

        write_file_if_present(
            &mut zip,
            &self.backbone.session_latest,
            "latest_session.jsonl",
            options,
        )?;
        write_file_if_present(&mut zip, &self.backbone.core, "core.log", options)?;
        write_file_if_present(
            &mut zip,
            &self.backbone.runtime_events,
            "runtime-events.log",
            options,
        )?;

        if include_deep_traces {
            for path in deep_trace_log_paths(&paths.logs_dir) {
                let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                write_file_if_present(&mut zip, &path, name, options)?;
            }
        }

        zip.finish()?;
        Ok(bundle_path)
    }

    fn build_manifest(&self, include_deep_traces: bool) -> Value {
        let mut files = serde_json::json!({
            "runtime_status.json": "runtime status snapshot at export time",
            "config_redacted.json": "redacted config payload",
            "environment.txt": "paths, bind URL, platform",
            "diagnostics-manifest.json": "this file",
            "latest_session.jsonl": "client event session log",
            "core.log": "backbone application log",
            "runtime-events.log": "runtime events log"
        });
        if include_deep_traces && let Some(map) = files.as_object_mut() {
            for name in DEEP_TRACE_LOG_FILES {
                map.insert(
                    (*name).into(),
                    Value::String(format!("deep diagnostic JSONL trace ({name})")),
                );
            }
        }
        serde_json::json!({
            "app_version": self.version,
            "files": files,
        })
    }

    fn build_environment_text(&self, paths: &ProjectPaths, base_url: &str) -> String {
        format!(
            "product=VoiceSub\nversion={}\nbase_url={}\nproject_root={}\nuser_data_dir={}\nlogs_dir={}\nexport_dir={}\n",
            self.version,
            base_url,
            paths.project_root.display(),
            paths.user_data_dir.display(),
            paths.logs_dir.display(),
            self.export_dir.display(),
        )
    }
}

fn write_json_entry<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    value: &Value,
    options: SimpleFileOptions,
) -> io::Result<()> {
    zip.start_file(name, options)?;
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".into());
    zip.write_all(text.as_bytes())?;
    Ok(())
}

fn write_file_if_present<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    source: &Path,
    archive_name: &str,
    options: SimpleFileOptions,
) -> io::Result<()> {
    if !source.is_file() {
        zip.start_file(archive_name, options)?;
        return Ok(());
    }
    zip.start_file(archive_name, options)?;
    let bytes = fs::read(source)?;
    zip.write_all(&bytes)?;
    Ok(())
}

fn utc_stamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_exports_empty_dir() {
        let dir = std::env::temp_dir().join(format!("vs-exports-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let service = ExportService::new(dir.join("exports"), &dir, "0.5.1");
        let list = service.list_exports().unwrap();
        assert!(list.exports.is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
