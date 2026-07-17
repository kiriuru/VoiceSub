use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::transfer::{TransferCancelled, TransferPhase, TransferReporter};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tracing::info;

use crate::model_family::{hf_file_url, FamilyVariantSpec, ModelFamily};

pub const MODEL_VARIANT_INT8: &str = "int8";
pub const MODEL_VARIANT_FP32: &str = "fp32";
pub const MODEL_VARIANT_INT8_SMOOTHQUANT: &str = "int8_smoothquant";

/// TDT-only variant enum kept for legacy call sites and tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelVariant {
    Int8,
    Fp32,
    Int8Smoothquant,
}

impl ModelVariant {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            MODEL_VARIANT_INT8 => Some(Self::Int8),
            MODEL_VARIANT_FP32 => Some(Self::Fp32),
            MODEL_VARIANT_INT8_SMOOTHQUANT => Some(Self::Int8Smoothquant),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Int8 => MODEL_VARIANT_INT8,
            Self::Fp32 => MODEL_VARIANT_FP32,
            Self::Int8Smoothquant => MODEL_VARIANT_INT8_SMOOTHQUANT,
        }
    }

    fn spec(self) -> &'static FamilyVariantSpec {
        ModelFamily::ParakeetTdt
            .parse_variant(self.as_str())
            .expect("tdt variant spec")
    }

    pub fn hf_repo(self) -> &'static str {
        self.spec().hf_repo
    }

    pub fn source_author(self) -> &'static str {
        self.spec().author
    }

    pub fn transfer_label(self) -> String {
        format!(
            "{} {} ({})",
            self.spec().display_name,
            self.as_str(),
            self.source_author()
        )
    }

    pub fn required_files(self) -> &'static [&'static str] {
        self.spec().required_files
    }

    pub fn size_mb(self) -> u32 {
        self.spec().size_mb
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntry {
    pub family: String,
    pub variant: String,
    pub installed: bool,
    pub size_mb: u32,
    pub active: bool,
    pub source_author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelManifestFile {
    pub name: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelManifest {
    pub family: String,
    pub variant: String,
    pub repo: String,
    pub files: Vec<ModelManifestFile>,
    pub folder_sha256: String,
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("unknown model variant: {0}")]
    UnknownVariant(String),
    #[error("download failed: {0}")]
    Download(String),
    #[error("download cancelled")]
    Cancelled,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
}

impl From<TransferCancelled> for ModelError {
    fn from(_: TransferCancelled) -> Self {
        Self::Cancelled
    }
}

const MANIFEST_FULL_HASH_MAX_BYTES: u64 = 4 * 1024 * 1024;

pub fn cleanup_pending_model_removals(module_dir: &Path) {
    let models_dir = module_dir.join("models");
    let Ok(entries) = fs::read_dir(&models_dir) else {
        return;
    };
    for family_entry in entries.flatten() {
        let family_path = family_entry.path();
        if !family_path.is_dir() {
            continue;
        }
        let Ok(variant_entries) = fs::read_dir(&family_path) else {
            continue;
        };
        for variant_entry in variant_entries.flatten() {
            let path = variant_entry.path();
            if path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".pending_delete"))
            {
                let _ = fs::remove_dir_all(path);
            }
        }
    }
    // Legacy flat layout under models/parakeet_tdt/*.pending_delete
    let legacy_root = models_root(module_dir, ModelFamily::ParakeetTdt);
    let Ok(entries) = fs::read_dir(&legacy_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".pending_delete"))
        {
            let _ = fs::remove_dir_all(path);
        }
    }
}

pub fn models_root(module_dir: &Path, family: ModelFamily) -> PathBuf {
    module_dir.join("models").join(family.as_str())
}

pub fn model_dir_for_family_variant(
    module_dir: &Path,
    family: ModelFamily,
    variant: &str,
) -> PathBuf {
    let folder = family
        .parse_variant(variant)
        .map(|spec| spec.variant)
        .unwrap_or_else(|| variant.trim());
    models_root(module_dir, family).join(folder)
}

pub fn model_dir_for_variant(module_dir: &Path, variant: &str) -> PathBuf {
    model_dir_for_family_variant(module_dir, ModelFamily::ParakeetTdt, variant)
}

pub fn resolve_model_dir(
    config_model_path: &str,
    family_raw: &str,
    config_variant: &str,
    module_dir: &Path,
) -> PathBuf {
    let family = ModelFamily::parse(family_raw).unwrap_or(ModelFamily::ParakeetTdt);
    let canonical = model_dir_for_family_variant(module_dir, family, config_variant);
    let trimmed = config_model_path.trim();
    if trimmed.is_empty() {
        return canonical;
    }
    let configured = PathBuf::from(trimmed);
    // Trust configured path only when it actually contains this family/variant.
    // Stale paths (e.g. variant changed but path left pointing at another folder)
    // must fall back to the canonical install location.
    if is_model_installed_for(&configured, family, config_variant) {
        configured
    } else {
        canonical
    }
}

pub fn is_model_installed_for(
    model_dir: &Path,
    family: ModelFamily,
    variant: &str,
) -> bool {
    let Some(spec) = family.parse_variant(variant) else {
        return false;
    };
    model_dir_looks_complete(model_dir, spec)
}

/// True when every required weight/vocab file exists, is non-empty, meets a
/// minimum size floor, and there are no in-progress `.part` downloads.
/// A valid `manifest.json` (when present) must list the same required set.
fn model_dir_looks_complete(model_dir: &Path, spec: &FamilyVariantSpec) -> bool {
    if !model_dir.is_dir() {
        return false;
    }
    let dir_name = model_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if dir_name.ends_with(".pending_delete") {
        return false;
    }
    if has_incomplete_part_files(model_dir) {
        return false;
    }
    for name in spec.required_files {
        let path = model_dir.join(name);
        if !file_meets_min_size(&path, required_file_min_bytes(name)) {
            return false;
        }
    }
    if let Some(manifest) = load_manifest(model_dir) {
        if !manifest_matches_spec(&manifest, spec) {
            return false;
        }
    }
    true
}

fn has_incomplete_part_files(model_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(model_dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .ends_with(".part")
    })
}

fn required_file_min_bytes(name: &str) -> u64 {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".onnx.data") {
        1_000_000
    } else if lower.ends_with(".onnx") {
        1_000
    } else {
        32
    }
}

fn file_meets_min_size(path: &Path, min_bytes: u64) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && meta.len() >= min_bytes
}

fn manifest_matches_spec(manifest: &ModelManifest, spec: &FamilyVariantSpec) -> bool {
    if !manifest.variant.eq_ignore_ascii_case(spec.variant) {
        return false;
    }
    for name in spec.required_files {
        let Some(entry) = manifest.files.iter().find(|f| f.name == *name) else {
            return false;
        };
        if entry.size_bytes < required_file_min_bytes(name) {
            return false;
        }
    }
    true
}

pub fn is_model_installed_at(model_dir: &Path, variant: ModelVariant) -> bool {
    is_model_installed_for(model_dir, ModelFamily::ParakeetTdt, variant.as_str())
}

pub fn build_model_catalog(
    module_dir: &Path,
    family_raw: &str,
    active_variant: &str,
) -> Vec<ModelCatalogEntry> {
    let family = ModelFamily::parse(family_raw).unwrap_or(ModelFamily::ParakeetTdt);
    let active = active_variant.trim();
    family
        .variants()
        .iter()
        .map(|spec| {
            let model_dir = model_dir_for_family_variant(module_dir, family, spec.variant);
            ModelCatalogEntry {
                family: family.as_str().into(),
                variant: spec.variant.into(),
                installed: is_model_installed_for(&model_dir, family, spec.variant),
                size_mb: spec.size_mb,
                active: active.eq_ignore_ascii_case(spec.variant),
                source_author: spec.author.into(),
            }
        })
        .collect()
}

/// Catalog for every supported family. `active_*` marks the currently selected model.
pub fn build_all_model_catalogs(
    module_dir: &Path,
    active_family_raw: &str,
    active_variant: &str,
) -> Vec<ModelCatalogEntry> {
    let active_family =
        ModelFamily::parse(active_family_raw).unwrap_or(ModelFamily::ParakeetTdt);
    let active_variant = active_variant.trim();
    ModelFamily::ParakeetTdt
        .variants()
        .iter()
        .map(|spec| {
            let family = ModelFamily::ParakeetTdt;
            let model_dir = model_dir_for_family_variant(module_dir, family, spec.variant);
            ModelCatalogEntry {
                family: family.as_str().into(),
                variant: spec.variant.into(),
                installed: is_model_installed_for(&model_dir, family, spec.variant),
                size_mb: spec.size_mb,
                active: family == active_family
                    && active_variant.eq_ignore_ascii_case(spec.variant),
                source_author: spec.author.into(),
            }
        })
        .collect()
}

pub fn manifest_path(module_dir: &Path) -> PathBuf {
    module_dir.join("manifest.json")
}

pub async fn download_model(
    module_dir: &Path,
    family_raw: &str,
    variant_raw: &str,
    reporter: &mut TransferReporter,
) -> Result<PathBuf, ModelError> {
    let family = ModelFamily::parse(family_raw).unwrap_or(ModelFamily::ParakeetTdt);
    let spec = family
        .parse_variant(variant_raw)
        .ok_or_else(|| ModelError::UnknownVariant(variant_raw.to_string()))?;
    let model_dir = model_dir_for_family_variant(module_dir, family, spec.variant);
    fs::create_dir_all(&model_dir)?;
    cleanup_pending_model_removals(module_dir);
    remove_part_files(&model_dir);

    reporter.begin(
        format!("model:{}:{}", family.as_str(), spec.variant),
        format!("{} ({})", spec.display_name, spec.author),
    );

    if is_model_installed_for(&model_dir, family, spec.variant) {
        info!(
            target: "voicesub.asr_local.model",
            path = %model_dir.display(),
            family = family.as_str(),
            variant = spec.variant,
            "ASR model already installed — skipping download"
        );
        reporter.set_phase(TransferPhase::Finalizing);
        let manifest = write_manifest(&model_dir, family, spec)?;
        write_manifest_file(&model_dir, &manifest)?;
        reporter.finish_ok();
        return Ok(model_dir);
    }

    reporter.register_cleanup_dir(model_dir.clone());

    let client = reqwest::Client::builder()
        .user_agent("VoiceSub-LocalAsr/0.6.0")
        .build()
        .map_err(|e| ModelError::Download(e.to_string()))?;

    let catalog_bytes = u64::from(spec.size_mb).saturating_mul(1024 * 1024);
    if catalog_bytes > 0 {
        reporter.set_total(Some(catalog_bytes));
    }

    let mut measured_total = 0u64;
    for file in spec.required_files {
        let dest = model_dir.join(file);
        if dest.is_file() {
            continue;
        }
        let url = hf_file_url(spec, file);
        if let Some(len) = hf_content_length(&client, &url).await? {
            measured_total = measured_total.saturating_add(len);
        }
    }
    if measured_total > catalog_bytes {
        reporter.set_total(Some(measured_total));
    }

    for file in spec.required_files {
        let dest = model_dir.join(file);
        if dest.is_file() {
            continue;
        }
        let url = hf_file_url(spec, file);
        reporter.set_phase(TransferPhase::Downloading);
        download_hf_file(&client, &url, &dest, reporter).await?;
    }

    reporter.set_phase(TransferPhase::Finalizing);
    let manifest = write_manifest(&model_dir, family, spec)?;
    write_manifest_file(&model_dir, &manifest)?;
    reporter.finish_ok();

    info!(
        target: "voicesub.asr_local.model",
        path = %model_dir.display(),
        family = family.as_str(),
        variant = spec.variant,
        folder_sha256 = %manifest.folder_sha256,
        "ASR model download complete"
    );
    Ok(model_dir)
}

pub fn delete_model_variant(
    module_dir: &Path,
    family_raw: &str,
    variant_raw: &str,
) -> Result<(), ModelError> {
    let family = ModelFamily::parse(family_raw).unwrap_or(ModelFamily::ParakeetTdt);
    let spec = family
        .parse_variant(variant_raw)
        .ok_or_else(|| ModelError::UnknownVariant(variant_raw.to_string()))?;
    let model_dir = model_dir_for_family_variant(module_dir, family, spec.variant);
    cleanup_pending_model_removals(module_dir);

    // Drop incomplete download leftovers before/while deleting the tree.
    if model_dir.is_dir() {
        remove_part_files(&model_dir);
        crate::deps::remove_path_or_defer(&model_dir).map_err(|err| match err {
            crate::deps::DepError::Io(io) => ModelError::Io(io),
            other => ModelError::Manifest(other.to_string()),
        })?;
    }

    // Deferred rename leaves `*.pending_delete` — try to finish cleanup now.
    let pending = model_dir.with_file_name(format!(
        "{}.pending_delete",
        model_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| spec.variant.to_string())
    ));
    if pending.is_dir() {
        let _ = fs::remove_dir_all(&pending);
    }

    if is_model_installed_for(&model_dir, family, spec.variant) {
        return Err(ModelError::Manifest(format!(
            "model still present after delete: {}",
            model_dir.display()
        )));
    }
    Ok(())
}

fn remove_part_files(model_dir: &Path) {
    let Ok(entries) = fs::read_dir(model_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with(".part"))
        {
            let _ = fs::remove_file(&path);
        }
    }
}

pub fn write_manifest(
    model_dir: &Path,
    family: ModelFamily,
    spec: &FamilyVariantSpec,
) -> Result<ModelManifest, ModelError> {
    let mut files = Vec::new();
    for name in spec.required_files {
        let path = model_dir.join(name);
        if !path.is_file() {
            return Err(ModelError::Manifest(format!("missing file: {name}")));
        }
        let entry = file_manifest_entry(&path, name)?;
        files.push(entry);
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    let folder_sha256 = folder_digest(&files);
    Ok(ModelManifest {
        family: family.as_str().into(),
        variant: spec.variant.into(),
        repo: spec.hf_repo.into(),
        files,
        folder_sha256,
    })
}

fn write_manifest_file(model_dir: &Path, manifest: &ModelManifest) -> Result<(), ModelError> {
    let body =
        serde_json::to_string_pretty(manifest).map_err(|e| ModelError::Manifest(e.to_string()))?;
    fs::write(manifest_path(model_dir), body)?;
    Ok(())
}

fn file_manifest_entry(path: &Path, name: &str) -> Result<ModelManifestFile, ModelError> {
    let size_bytes = fs::metadata(path)?.len();
    let sha256 = if size_bytes > MANIFEST_FULL_HASH_MAX_BYTES {
        format!("size:{size_bytes}")
    } else {
        file_sha256(path)?.0
    };
    Ok(ModelManifestFile {
        name: name.to_string(),
        sha256,
        size_bytes,
    })
}

pub fn load_manifest(module_dir: &Path) -> Option<ModelManifest> {
    let path = manifest_path(module_dir);
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

async fn hf_content_length(
    client: &reqwest::Client,
    url: &str,
) -> Result<Option<u64>, ModelError> {
    let response = client
        .head(url)
        .send()
        .await
        .map_err(|e| ModelError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ModelError::Download(format!(
            "HTTP {} for HEAD {url}",
            response.status()
        )));
    }
    Ok(response.content_length().filter(|bytes| *bytes > 0))
}

async fn download_hf_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    reporter: &mut TransferReporter,
) -> Result<(), ModelError> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    reporter.register_cleanup_file(tmp.clone());
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| ModelError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(ModelError::Download(format!(
            "HTTP {} for {url}",
            response.status()
        )));
    }
    let mut file = File::create(&tmp)?;
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        reporter.check_cancelled()?;
        let chunk = chunk.map_err(|e| ModelError::Download(e.to_string()))?;
        file.write_all(&chunk)?;
        reporter.add_bytes(chunk.len() as u64);
    }
    file.sync_all()?;
    fs::rename(tmp, dest)?;
    Ok(())
}

fn file_sha256(path: &Path) -> Result<(String, u64), ModelError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    let mut size_bytes = 0u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size_bytes += read as u64;
    }
    Ok((hex_digest(hasher.finalize()), size_bytes))
}

fn folder_digest(files: &[ModelManifestFile]) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        hasher.update(file.name.as_bytes());
        hasher.update(file.sha256.as_bytes());
    }
    hex_digest(hasher.finalize())
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_stub_files(dir: &Path, files: &[&str]) {
        for name in files {
            let min = required_file_min_bytes(name);
            fs::write(dir.join(name), vec![b'x'; min as usize]).unwrap();
        }
    }

    #[test]
    fn parses_model_variants() {
        assert_eq!(ModelVariant::parse("INT8"), Some(ModelVariant::Int8));
        assert_eq!(ModelVariant::parse("fp32"), Some(ModelVariant::Fp32));
        assert_eq!(
            ModelVariant::parse("int8_smoothquant"),
            Some(ModelVariant::Int8Smoothquant)
        );
        assert!(ModelVariant::parse("unknown").is_none());
    }

    #[test]
    fn smoothquant_variant_uses_olicorne_repo() {
        let variant = ModelVariant::Int8Smoothquant;
        assert_eq!(variant.hf_repo(), "Olicorne/parakeet-tdt-0.6b-v3-smoothquant-onnx");
        assert_eq!(variant.source_author(), "Olicorne");
    }

    #[test]
    fn istupakov_variants_use_primary_repo() {
        assert_eq!(ModelVariant::Int8.hf_repo(), "istupakov/parakeet-tdt-0.6b-v3-onnx");
        assert_eq!(ModelVariant::Fp32.hf_repo(), "istupakov/parakeet-tdt-0.6b-v3-onnx");
        assert_eq!(ModelVariant::Int8.source_author(), "istupakov");
    }

    #[test]
    fn model_installed_requires_all_int8_files_with_min_size() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_model_installed_at(dir.path(), ModelVariant::Int8));
        for name in ModelVariant::Int8.required_files() {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        assert!(
            !is_model_installed_at(dir.path(), ModelVariant::Int8),
            "tiny stubs must not count as installed"
        );
        write_stub_files(dir.path(), ModelVariant::Int8.required_files());
        assert!(is_model_installed_at(dir.path(), ModelVariant::Int8));
    }

    #[test]
    fn model_installed_rejects_part_files() {
        let dir = tempfile::tempdir().unwrap();
        write_stub_files(dir.path(), ModelVariant::Int8.required_files());
        fs::write(dir.path().join("encoder-model.int8.onnx.part"), b"partial").unwrap();
        assert!(!is_model_installed_at(dir.path(), ModelVariant::Int8));
    }

    #[test]
    fn delete_model_variant_removes_install() {
        let module = tempfile::tempdir().unwrap();
        let family = ModelFamily::ParakeetTdt;
        let variant = "int8";
        let model_dir = model_dir_for_family_variant(module.path(), family, variant);
        fs::create_dir_all(&model_dir).unwrap();
        write_stub_files(&model_dir, ModelVariant::Int8.required_files());
        assert!(is_model_installed_for(&model_dir, family, variant));
        delete_model_variant(module.path(), family.as_str(), variant).unwrap();
        assert!(!is_model_installed_for(&model_dir, family, variant));
        assert!(!model_dir.is_dir());
    }

    #[test]
    fn all_catalogs_include_tdt_variants() {
        let module = tempfile::tempdir().unwrap();
        let catalog = build_all_model_catalogs(module.path(), "parakeet_tdt", "int8");
        assert_eq!(catalog.len(), 3);
        assert!(catalog
            .iter()
            .any(|e| e.family == "parakeet_tdt" && e.variant == "int8" && e.active));
        assert!(catalog
            .iter()
            .any(|e| e.family == "parakeet_tdt" && e.variant == "fp32"));
        assert!(catalog.iter().all(|e| !e.installed));
    }

    #[test]
    fn manifest_uses_size_fingerprint_for_large_files() {
        let dir = tempfile::tempdir().unwrap();
        for name in ModelVariant::Int8.required_files() {
            let path = dir.path().join(name);
            if name.contains("encoder") {
                let mut file = File::create(path).unwrap();
                file.write_all(&vec![0u8; (MANIFEST_FULL_HASH_MAX_BYTES + 1) as usize])
                    .unwrap();
            } else {
                fs::write(path, vec![b's'; required_file_min_bytes(name) as usize]).unwrap();
            }
        }
        let spec = ModelFamily::ParakeetTdt.parse_variant("int8").unwrap();
        let manifest = write_manifest(dir.path(), ModelFamily::ParakeetTdt, spec).unwrap();
        let encoder = manifest
            .files
            .iter()
            .find(|file| file.name.contains("encoder"))
            .expect("encoder entry");
        assert!(encoder.sha256.starts_with("size:"));
    }

    #[test]
    fn manifest_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        write_stub_files(dir.path(), ModelVariant::Int8.required_files());
        let spec = ModelFamily::ParakeetTdt.parse_variant("int8").unwrap();
        let manifest = write_manifest(dir.path(), ModelFamily::ParakeetTdt, spec).unwrap();
        assert_eq!(manifest.variant, MODEL_VARIANT_INT8);
        assert_eq!(manifest.family, ModelFamily::ParakeetTdt.as_str());
        assert_eq!(
            manifest.files.len(),
            ModelVariant::Int8.required_files().len()
        );
        assert!(!manifest.folder_sha256.is_empty());
        write_manifest_file(dir.path(), &manifest).unwrap();
        let loaded = load_manifest(dir.path()).expect("manifest");
        assert_eq!(loaded, manifest);
        assert!(is_model_installed_at(dir.path(), ModelVariant::Int8));
    }

    #[test]
    fn model_catalog_reports_install_state() {
        let dir = tempfile::tempdir().unwrap();
        let catalog = build_model_catalog(dir.path(), ModelFamily::ParakeetTdt.as_str(), "int8");
        assert_eq!(catalog.len(), 3);
        assert!(!catalog[0].installed);
        assert_eq!(catalog[2].variant, MODEL_VARIANT_INT8_SMOOTHQUANT);
        assert_eq!(catalog[2].source_author, "Olicorne");
        let int8_dir = model_dir_for_family_variant(dir.path(), ModelFamily::ParakeetTdt, "int8");
        fs::create_dir_all(&int8_dir).unwrap();
        write_stub_files(&int8_dir, ModelVariant::Int8.required_files());
        let catalog = build_model_catalog(dir.path(), ModelFamily::ParakeetTdt.as_str(), "int8");
        assert!(catalog
            .iter()
            .any(|entry| entry.variant == "int8" && entry.installed && entry.family == "parakeet_tdt"));
        assert!(catalog
            .iter()
            .any(|entry| entry.variant == "fp32" && !entry.installed));
    }

    #[test]
    fn resolve_model_dir_ignores_stale_path_for_other_variant() {
        let module = tempfile::tempdir().unwrap();
        let family = ModelFamily::ParakeetTdt;
        let fp32_dir = model_dir_for_family_variant(module.path(), family, "fp32");
        let sq_dir = model_dir_for_family_variant(module.path(), family, "int8_smoothquant");
        fs::create_dir_all(&fp32_dir).unwrap();
        fs::create_dir_all(&sq_dir).unwrap();
        write_stub_files(&fp32_dir, ModelVariant::Fp32.required_files());
        write_stub_files(&sq_dir, ModelVariant::Int8Smoothquant.required_files());

        // Config points at fp32 folder while active variant is smoothquant.
        let resolved = resolve_model_dir(
            &fp32_dir.display().to_string(),
            family.as_str(),
            "int8_smoothquant",
            module.path(),
        );
        assert_eq!(resolved, sq_dir);
        assert!(is_model_installed_for(&resolved, family, "int8_smoothquant"));
    }
}
