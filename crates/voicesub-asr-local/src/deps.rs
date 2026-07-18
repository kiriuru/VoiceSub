use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;
use tracing::{info, warn};

pub const ORT_VERSION: &str = "1.24.2";

pub const ORT_CPU_ZIP_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-1.24.2.zip";

pub const ORT_GPU_ZIP_URL: &str = "https://github.com/microsoft/onnxruntime/releases/download/v1.24.2/onnxruntime-win-x64-gpu_cuda13-1.24.2.zip";

/// CPU ORT package ships shared provider glue alongside the core DLL.
pub const ORT_CPU_DLLS: &[&str] = &["onnxruntime.dll", "onnxruntime_providers_shared.dll"];

/// GPU ORT requires CUDA provider plug-ins; core `onnxruntime.dll` alone falls back to CPU.
pub const ORT_GPU_DLLS: &[&str] = &[
    "onnxruntime.dll",
    "onnxruntime_providers_cuda.dll",
    "onnxruntime_providers_shared.dll",
];

pub const CUDA_RUNTIME_DLLS: &[&str] = &["cudart64_13.dll", "cublas64_13.dll", "cublasLt64_13.dll"];

/// ONNX Runtime CUDA EP (1.24.x, cu13) requires cuDNN 9 redist on PATH.
pub const CUDNN_DLLS: &[&str] = &[
    "cudnn64_9.dll",
    "cudnn_adv64_9.dll",
    "cudnn_cnn64_9.dll",
    "cudnn_engines_precompiled64_9.dll",
    "cudnn_engines_runtime_compiled64_9.dll",
    "cudnn_engines_tensor_ir64_9.dll",
    "cudnn_ext64_9.dll",
    "cudnn_graph64_9.dll",
    "cudnn_heuristic64_9.dll",
    "cudnn_ops64_9.dll",
];

pub const CUDA_REDIST_DLLS: &[&str] = &[
    "cudart64_13.dll",
    "cublas64_13.dll",
    "cublasLt64_13.dll",
    "cudnn64_9.dll",
    "cudnn_adv64_9.dll",
    "cudnn_cnn64_9.dll",
    "cudnn_engines_precompiled64_9.dll",
    "cudnn_engines_runtime_compiled64_9.dll",
    "cudnn_engines_tensor_ir64_9.dll",
    "cudnn_ext64_9.dll",
    "cudnn_graph64_9.dll",
    "cudnn_heuristic64_9.dll",
    "cudnn_ops64_9.dll",
];

pub const VCRUNTIME_DLLS: &[&str] = &[
    "MSVCP140.dll",
    "VCRUNTIME140.dll",
    "VCRUNTIME140_1.dll",
    "VCOMP140.dll",
];

pub const CUDART_WHEEL_URL: &str = "https://files.pythonhosted.org/packages/d2/27/b53a5e0397842a5c11f0e1a39d4e5b2f22638a4126e83b3c4e196f62c969/nvidia_cuda_runtime-13.3.29-py3-none-win_amd64.whl";

pub const CUBLAS_WHEEL_URL: &str = "https://files.pythonhosted.org/packages/08/8f/890a96ea1ff615100296977cce23296052dcb8c114d4e451201ec39df9bf/nvidia_cublas-13.6.0.2-py3-none-win_amd64.whl";

pub const CUDNN_WHEEL_URL: &str = "https://files.pythonhosted.org/packages/31/23/1dd3aa15cc4ab62c8fc88f8049ef137bc44c17892f5577bc80d994941f77/nvidia_cudnn_cu13-9.24.0.43-py3-none-win_amd64.whl";

pub const CUDA_DOWNLOAD_MB: u64 = 790;
pub const ORT_CPU_DOWNLOAD_MB: u64 = 14;
pub const ORT_GPU_DOWNLOAD_MB: u64 = 180;

/// Official CUDA Toolkit 13.x download page (Windows x64 installer).
/// ORT GPU package is the `cuda13` build; Toolkit 12 is not compatible.
pub const CUDA_TOOLKIT_URL: &str = "https://developer.nvidia.com/cuda-13-0-0-download-archive?target_os=Windows&target_arch=x86_64&target_type=exe_local";

/// Marker that a CUDA 13.x toolkit `bin` directory is present.
pub const CUDA13_MARKER_DLL: &str = "cudart64_13.dll";

#[derive(Debug, Error)]
pub enum DepError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("download error: {0}")]
    Download(String),
    #[error("download cancelled")]
    Cancelled,
    #[error("archive error: {0}")]
    Archive(String),
    #[error("dependency check failed: {0}")]
    Check(String),
}

impl From<TransferCancelled> for DepError {
    fn from(_: TransferCancelled) -> Self {
        Self::Cancelled
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DepDownloadKind {
    OrtCpu,
    OrtGpu,
    CudaRedist,
}

impl DepDownloadKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ort_cpu" | "onnxruntime_cpu" => Some(Self::OrtCpu),
            "ort_gpu" | "onnxruntime_gpu" => Some(Self::OrtGpu),
            "cuda_redist" | "cuda" => Some(Self::CudaRedist),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FoundDll {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DllGroupStatus {
    pub ok: bool,
    pub missing: Vec<String>,
    pub found: Vec<FoundDll>,
    pub download_mb: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CudaToolkitStatus {
    pub ok: bool,
    pub version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LocalAsrEnvCheck {
    pub vcruntime: DllGroupStatus,
    pub ort_cpu: DllGroupStatus,
    pub ort_gpu: DllGroupStatus,
    pub cuda_redist: DllGroupStatus,
    pub cuda_toolkit: CudaToolkitStatus,
    pub cpu_deps_ready: bool,
    pub cuda_deps_ready: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeLayout {
    pub root: PathBuf,
    pub cpu: PathBuf,
    pub gpu: PathBuf,
    pub cuda: PathBuf,
    pub vcruntime: PathBuf,
}

pub fn runtime_layout(module_dir: &Path) -> RuntimeLayout {
    let root = module_dir.join("runtime");
    RuntimeLayout {
        cpu: root.join("cpu"),
        gpu: root.join("gpu"),
        cuda: root.join("cuda"),
        vcruntime: root.join("vcruntime"),
        root,
    }
}

pub fn ort_dll_path_for_provider(module_dir: &Path, provider: &str) -> PathBuf {
    preferred_ort_dll(module_dir).unwrap_or_else(|| {
        let layout = runtime_layout(module_dir);
        if provider.eq_ignore_ascii_case(crate::config::EXECUTION_PROVIDER_CUDA) {
            layout.gpu.join("onnxruntime.dll")
        } else {
            layout.cpu.join("onnxruntime.dll")
        }
    })
}

/// Prefer the GPU ORT build when installed — it includes CUDA EP and still supports CPU.
pub fn preferred_ort_dll(module_dir: &Path) -> Option<PathBuf> {
    let layout = runtime_layout(module_dir);
    let env = env_check(module_dir);
    if env.ort_gpu.ok {
        Some(layout.gpu.join("onnxruntime.dll"))
    } else if env.ort_cpu.ok {
        Some(layout.cpu.join("onnxruntime.dll"))
    } else {
        None
    }
}

pub fn prepare_ort_runtime(module_dir: &Path, provider: &str) -> Result<(), DepError> {
    let layout = runtime_layout(module_dir);
    let dll = ort_dll_path_for_provider(module_dir, provider);
    if !dll.is_file() {
        return Err(DepError::Check(format!(
            "ONNX Runtime DLL missing at {}",
            dll.display()
        )));
    }
    // SAFETY: Local ASR owns ORT dynamic loading during probe/load/transcribe.
    unsafe {
        std::env::set_var("ORT_DYLIB_PATH", dll.as_os_str());
    }
    let env = env_check(module_dir);
    if env.ort_gpu.ok {
        prepend_path_dir(&layout.cuda);
        prepend_path_dir(&layout.gpu);
        prepend_system_cuda_search_paths();
    } else {
        prepend_path_dir(&layout.cpu);
    }
    Ok(())
}

/// Fallback CUDA/cuDNN discovery (same idea as Higgs-Ultimate `windows_dependency_search_dirs`).
#[cfg(windows)]
fn prepend_system_cuda_search_paths() {
    for bin in cuda13_toolkit_bin_dirs() {
        if find_dll(CUDA13_MARKER_DLL, &[bin.clone(), bin.join("x64")]).is_none() {
            continue;
        }
        prepend_path_dir(&bin);
        prepend_path_dir(&bin.join("x64"));
    }
}

#[cfg(not(windows))]
fn prepend_system_cuda_search_paths() {}

/// Candidate `bin` directories for an installed CUDA Toolkit 13.x.
#[cfg(windows)]
fn cuda13_toolkit_bin_dirs() -> Vec<PathBuf> {
    let mut bins = Vec::new();
    for (key, value) in std::env::vars_os() {
        let key_up = key.to_string_lossy().to_ascii_uppercase();
        if key_up == "CUDA_PATH" || key_up.starts_with("CUDA_PATH_V13") {
            let bin = PathBuf::from(&value).join("bin");
            // Marker DLL (cudart64_13) decides major version — CUDA_PATH may point at 12.x.
            if bin.is_dir() {
                bins.push(bin);
            }
        } else if key_up.starts_with("CUDA_PATH_V") {
            // Explicitly ignore CUDA_PATH_V12_* / older majors.
            continue;
        }
    }
    for root in ["ProgramFiles", "ProgramW6432"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
    {
        let cuda_root = root.join("NVIDIA GPU Computing Toolkit").join("CUDA");
        if !cuda_root.is_dir() {
            continue;
        }
        if let Ok(read) = fs::read_dir(&cuda_root) {
            for entry in read.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if !name.to_ascii_lowercase().starts_with("v13") {
                    continue;
                }
                let bin = entry.path().join("bin");
                if bin.is_dir() {
                    bins.push(bin);
                }
            }
        }
        // Stable preferred order for known patch folders.
        for version in ["v13.3", "v13.2", "v13.1", "v13.0", "v13"] {
            let bin = cuda_root.join(version).join("bin");
            if bin.is_dir() && !bins.iter().any(|existing| existing == &bin) {
                bins.push(bin);
            }
        }
    }
    bins
}

#[cfg(not(windows))]
fn cuda13_toolkit_bin_dirs() -> Vec<PathBuf> {
    Vec::new()
}

fn prepend_path_dir(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let dir_str = dir.to_string_lossy();
    let Ok(current) = std::env::var("PATH") else {
        unsafe {
            std::env::set_var("PATH", dir_str.as_ref());
        }
        return;
    };
    if current
        .split(';')
        .any(|entry| entry.eq_ignore_ascii_case(dir_str.as_ref()))
    {
        return;
    }
    unsafe {
        std::env::set_var("PATH", format!("{dir_str};{current}"));
    }
}

pub fn env_check(module_dir: &Path) -> LocalAsrEnvCheck {
    let layout = runtime_layout(module_dir);
    let vcruntime = check_vcruntime(&layout);
    let ort_cpu = check_group(
        ORT_CPU_DLLS,
        std::slice::from_ref(&layout.cpu),
        ORT_CPU_DOWNLOAD_MB,
    );
    let ort_gpu = check_group(
        ORT_GPU_DLLS,
        std::slice::from_ref(&layout.gpu),
        ORT_GPU_DOWNLOAD_MB,
    );
    let cuda_redist = check_group(
        CUDA_REDIST_DLLS,
        std::slice::from_ref(&layout.cuda),
        CUDA_DOWNLOAD_MB,
    );
    let cuda_toolkit = check_cuda_toolkit_13();
    // GPU ORT build still supports CPU EP — either package satisfies the CPU path.
    let cpu_deps_ready = vcruntime.ok && (ort_cpu.ok || ort_gpu.ok);
    let cuda_deps_ready = vcruntime.ok && ort_gpu.ok && cuda_redist.ok && cuda_toolkit.ok;
    LocalAsrEnvCheck {
        vcruntime,
        ort_cpu,
        ort_gpu,
        cuda_redist,
        cuda_toolkit,
        cpu_deps_ready,
        cuda_deps_ready,
    }
}

fn check_group(names: &[&str], dirs: &[PathBuf], download_mb: u64) -> DllGroupStatus {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    // Prefer exact `dir.join(name)` — CUDA Toolkit `bin/` can be huge; never scan it
    // unless a case-insensitive fallback is actually needed.
    let mut indexes: Option<Vec<DllDirIndex>> = None;
    for name in names {
        let path = find_dll_direct(name, dirs).or_else(|| {
            if indexes.is_none() {
                indexes = Some(dirs.iter().map(|dir| DllDirIndex::scan(dir)).collect());
            }
            find_dll_in_indexes(name, indexes.as_ref().unwrap())
        });
        if let Some(path) = path {
            found.push(FoundDll {
                name: (*name).to_string(),
                path: path.display().to_string(),
            });
        } else {
            missing.push((*name).to_string());
        }
    }
    DllGroupStatus {
        ok: missing.is_empty(),
        missing,
        found,
        download_mb,
    }
}

struct DllDirIndex {
    files: std::collections::HashMap<String, PathBuf>,
}

impl DllDirIndex {
    fn scan(dir: &Path) -> Self {
        let mut files = std::collections::HashMap::new();
        if !dir.is_dir() {
            return Self { files };
        }
        if let Ok(read) = fs::read_dir(dir) {
            for entry in read.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    files.insert(name.to_ascii_lowercase(), path);
                }
            }
        }
        Self { files }
    }

    fn get(&self, name: &str) -> Option<&PathBuf> {
        self.files.get(&name.to_ascii_lowercase())
    }
}

fn find_dll_in_indexes(name: &str, indexes: &[DllDirIndex]) -> Option<PathBuf> {
    for index in indexes {
        if let Some(path) = index.get(name) {
            return Some(path.clone());
        }
    }
    None
}

fn find_dll_direct(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for dir in dirs {
        let direct = dir.join(name);
        if direct.is_file() {
            return Some(direct);
        }
    }
    None
}

fn check_vcruntime(layout: &RuntimeLayout) -> DllGroupStatus {
    let mut dirs = vec![layout.vcruntime.clone()];
    if let Some(system32) = system32_dir() {
        dirs.push(system32);
    }
    check_group(VCRUNTIME_DLLS, &dirs, 1)
}

fn system32_dir() -> Option<PathBuf> {
    std::env::var_os("SystemRoot").map(|root| PathBuf::from(root).join("System32"))
}

fn find_dll(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    find_dll_direct(name, dirs).or_else(|| {
        let indexes: Vec<DllDirIndex> = dirs.iter().map(|dir| DllDirIndex::scan(dir)).collect();
        find_dll_in_indexes(name, &indexes)
    })
}

fn check_cuda_toolkit_13() -> CudaToolkitStatus {
    check_cuda_toolkit_13_in(&cuda13_toolkit_bin_dirs())
}

fn check_cuda_toolkit_13_in(bin_dirs: &[PathBuf]) -> CudaToolkitStatus {
    for bin in bin_dirs {
        if let Some(marker) = find_dll(CUDA13_MARKER_DLL, &[bin.clone(), bin.join("x64")]) {
            let version = cuda13_version_from_path(&marker);
            return CudaToolkitStatus {
                ok: true,
                version,
                message: Some("CUDA Toolkit 13 found".into()),
            };
        }
    }
    #[cfg(windows)]
    {
        CudaToolkitStatus {
            ok: false,
            version: None,
            message: Some("CUDA Toolkit 13 not found".into()),
        }
    }
    #[cfg(not(windows))]
    {
        CudaToolkitStatus {
            ok: false,
            version: None,
            message: Some("CUDA EP is Windows-only in this module".into()),
        }
    }
}

fn cuda13_version_from_path(marker: &Path) -> Option<String> {
    for component in marker.components().rev() {
        let name = component.as_os_str().to_string_lossy();
        let lower = name.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix('v')
            && rest.starts_with("13")
        {
            return Some(format!("v{rest}"));
        }
    }
    None
}

use crate::transfer::{TransferCancelled, TransferPhase, TransferReporter};

pub async fn download_dependency(
    module_dir: &Path,
    kind: DepDownloadKind,
    reporter: &mut TransferReporter,
) -> Result<(), DepError> {
    fs::create_dir_all(module_dir)?;
    match kind {
        DepDownloadKind::OrtCpu => {
            reporter.begin("ort_cpu", "ONNX Runtime (CPU)");
            download_ort_zip(module_dir, ORT_CPU_ZIP_URL, "cpu", reporter).await
        }
        DepDownloadKind::OrtGpu => {
            reporter.begin("ort_gpu", "ONNX Runtime (GPU)");
            download_ort_zip(module_dir, ORT_GPU_ZIP_URL, "gpu", reporter).await
        }
        DepDownloadKind::CudaRedist => {
            reporter.begin("cuda_redist", "CUDA runtime");
            download_cuda_redist(module_dir, reporter).await
        }
    }
}

pub fn delete_dependency(module_dir: &Path, kind: DepDownloadKind) -> Result<(), DepError> {
    let layout = runtime_layout(module_dir);
    match kind {
        DepDownloadKind::OrtCpu => remove_path_or_defer(&layout.cpu),
        DepDownloadKind::OrtGpu => remove_path_or_defer(&layout.gpu),
        DepDownloadKind::CudaRedist => remove_path_or_defer(&layout.cuda),
    }
}

/// Remove runtime trees left over from a prior session when DLL locks are gone.
pub fn cleanup_pending_runtime_removals(module_dir: &Path) {
    let layout = runtime_layout(module_dir);
    for name in ["cpu", "gpu", "cuda"] {
        let pending = pending_delete_path(&layout.root.join(name));
        if pending.is_dir() {
            let _ = fs::remove_dir_all(&pending);
        }
    }
}

pub fn remove_path_or_defer(path: &Path) -> Result<(), DepError> {
    if !path.exists() {
        return Ok(());
    }
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(err) if path_locked(&err) => {
            let pending = pending_delete_path(path);
            if pending.exists() {
                let _ = fs::remove_dir_all(&pending);
            }
            fs::rename(path, &pending)?;
            info!(
                target: "voicesub.asr_local.deps",
                from = %path.display(),
                to = %pending.display(),
                "dependency directory locked — deferred removal until next restart"
            );
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

fn pending_delete_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "runtime".into());
    path.with_file_name(format!("{name}.pending_delete"))
}

fn path_locked(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::Other
    ) || matches!(err.raw_os_error(), Some(5) | Some(32))
}

async fn download_ort_zip(
    module_dir: &Path,
    url: &str,
    subdir: &str,
    reporter: &mut TransferReporter,
) -> Result<(), DepError> {
    info!(
        target: "voicesub.asr_local.deps",
        url,
        subdir,
        "downloading ONNX Runtime"
    );
    let bytes = http_get_bytes(url, reporter, HttpTotalPolicy::SetFromResponse).await?;
    reporter.check_cancelled()?;
    reporter.set_phase(TransferPhase::Extracting);
    let layout = runtime_layout(module_dir);
    let dest_dir = layout.root.join(subdir);
    fs::create_dir_all(&dest_dir)?;
    reporter.register_cleanup_dir(dest_dir.clone());
    let dlls = if subdir == "gpu" {
        ORT_GPU_DLLS
    } else {
        ORT_CPU_DLLS
    };
    // Zip inflate + DLL writes are CPU/disk bound — keep them off the Tokio worker.
    let extract_dest = dest_dir.clone();
    tokio::task::spawn_blocking(move || extract_zip_lib_dlls(&bytes, &extract_dest, dlls))
        .await
        .map_err(|e| DepError::Archive(format!("extract task failed: {e}")))??;
    reporter.finish_ok();
    info!(
        target: "voicesub.asr_local.deps",
        path = %dest_dir.display(),
        count = dlls.len(),
        "installed ONNX Runtime DLLs"
    );
    Ok(())
}

async fn download_cuda_redist(
    module_dir: &Path,
    reporter: &mut TransferReporter,
) -> Result<(), DepError> {
    let layout = runtime_layout(module_dir);
    fs::create_dir_all(&layout.cuda)?;
    reporter.register_cleanup_dir(layout.cuda.clone());

    let need_cudart = find_dll("cudart64_13.dll", std::slice::from_ref(&layout.cuda)).is_none();
    let need_cublas = find_dll("cublas64_13.dll", std::slice::from_ref(&layout.cuda)).is_none()
        || find_dll("cublasLt64_13.dll", std::slice::from_ref(&layout.cuda)).is_none();
    let need_cudnn = find_dll("cudnn64_9.dll", std::slice::from_ref(&layout.cuda)).is_none();

    let mut planned_total = 0u64;
    let mut fallback_total = 0u64;
    if need_cudart {
        fallback_total = fallback_total.saturating_add(CUDART_WHEEL_BYTES);
        planned_total = planned_total.saturating_add(resolve_wheel_bytes(
            fetch_content_length(CUDART_WHEEL_URL).await,
            CUDART_WHEEL_BYTES,
        ));
    }
    if need_cublas {
        fallback_total = fallback_total.saturating_add(CUBLAS_WHEEL_BYTES);
        planned_total = planned_total.saturating_add(resolve_wheel_bytes(
            fetch_content_length(CUBLAS_WHEEL_URL).await,
            CUBLAS_WHEEL_BYTES,
        ));
    }
    if need_cudnn {
        fallback_total = fallback_total.saturating_add(CUDNN_WHEEL_BYTES);
        planned_total = planned_total.saturating_add(resolve_wheel_bytes(
            fetch_content_length(CUDNN_WHEEL_URL).await,
            CUDNN_WHEEL_BYTES,
        ));
    }
    if planned_total == 0 {
        planned_total = fallback_total;
    }
    if planned_total > 0 {
        reporter.set_total(Some(planned_total));
    }

    if need_cudart {
        reporter.set_phase(TransferPhase::Downloading);
        reporter.set_label("CUDA runtime (cudart)");
        extract_wheel_dlls(
            CUDART_WHEEL_URL,
            &layout.cuda,
            &["cudart64_13.dll"],
            reporter,
            HttpTotalPolicy::AccumulateOnly,
        )
        .await?;
    }
    if need_cublas {
        reporter.set_phase(TransferPhase::Downloading);
        reporter.set_label("CUDA runtime (cuBLAS)");
        extract_wheel_dlls(
            CUBLAS_WHEEL_URL,
            &layout.cuda,
            CUDA_RUNTIME_DLLS,
            reporter,
            HttpTotalPolicy::AccumulateOnly,
        )
        .await?;
    }
    if need_cudnn {
        reporter.set_phase(TransferPhase::Downloading);
        reporter.set_label("CUDA runtime (cuDNN 9)");
        extract_wheel_dlls(
            CUDNN_WHEEL_URL,
            &layout.cuda,
            CUDNN_DLLS,
            reporter,
            HttpTotalPolicy::AccumulateOnly,
        )
        .await?;
    }
    reporter.finish_ok();
    Ok(())
}

/// Fallback sizes when Content-Length is unavailable (bytes).
const CUDART_WHEEL_BYTES: u64 = 3 * 1024 * 1024;
const CUBLAS_WHEEL_BYTES: u64 = 394 * 1024 * 1024;
const CUDNN_WHEEL_BYTES: u64 = 393 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HttpTotalPolicy {
    /// Single-file download: replace total with this response Content-Length.
    SetFromResponse,
    /// Multi-part download: keep preset aggregate total, only add received bytes.
    AccumulateOnly,
}

fn resolve_wheel_bytes(result: Result<u64, DepError>, fallback: u64) -> u64 {
    match result {
        Ok(bytes) if bytes > 0 => bytes,
        _ => fallback,
    }
}

async fn fetch_content_length(url: &str) -> Result<u64, DepError> {
    let client = reqwest::Client::builder()
        .user_agent("VoiceSub-LocalAsr/0.6.0")
        .build()
        .map_err(|e| DepError::Download(e.to_string()))?;
    let response = client
        .head(url)
        .send()
        .await
        .map_err(|e| DepError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(DepError::Download(format!(
            "HTTP {} for HEAD {url}",
            response.status()
        )));
    }
    response
        .content_length()
        .ok_or_else(|| DepError::Download(format!("Content-Length missing for {url}")))
}

async fn http_get_bytes(
    url: &str,
    reporter: &mut TransferReporter,
    total_policy: HttpTotalPolicy,
) -> Result<Vec<u8>, DepError> {
    let client = reqwest::Client::builder()
        .user_agent("VoiceSub-LocalAsr/0.6.0")
        .build()
        .map_err(|e| DepError::Download(e.to_string()))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| DepError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(DepError::Download(format!(
            "HTTP {} for {url}",
            response.status()
        )));
    }
    let total = response.content_length().filter(|bytes| *bytes > 0);
    if total_policy == HttpTotalPolicy::SetFromResponse {
        reporter.set_total(total);
    }
    reporter.set_phase(TransferPhase::Downloading);
    let mut buffer = Vec::new();
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        reporter.check_cancelled()?;
        let chunk = chunk.map_err(|e| DepError::Download(e.to_string()))?;
        buffer.extend_from_slice(&chunk);
        reporter.add_bytes(chunk.len() as u64);
    }
    Ok(buffer)
}

fn extract_zip_lib_dlls(bytes: &[u8], dest_dir: &Path, dll_names: &[&str]) -> Result<(), DepError> {
    use std::collections::HashSet;

    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| DepError::Archive(e.to_string()))?;
    let targets: HashSet<String> = dll_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect();
    let mut written = HashSet::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| DepError::Archive(e.to_string()))?;
        let name = entry.name().replace('\\', "/");
        if !name.contains("/lib/") || !name.ends_with(".dll") {
            continue;
        }
        let base = name
            .rsplit('/')
            .next()
            .unwrap_or(&name)
            .to_ascii_lowercase();
        if !targets.contains(&base) {
            continue;
        }
        let out_name = dll_names
            .iter()
            .find(|candidate| candidate.eq_ignore_ascii_case(&base))
            .copied()
            .unwrap_or(base.as_str());
        let dest = dest_dir.join(out_name);
        let tmp = dest.with_extension("dll.tmp");
        if let Some(parent) = tmp.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = fs::File::create(&tmp).map_err(DepError::Io)?;
        std::io::copy(&mut entry, &mut out).map_err(DepError::Io)?;
        out.flush().ok();
        if dest.exists() {
            fs::remove_file(&dest).ok();
        }
        fs::rename(&tmp, &dest).map_err(DepError::Io)?;
        written.insert(base);
    }
    let missing: Vec<&str> = dll_names
        .iter()
        .copied()
        .filter(|name| !written.contains(&name.to_ascii_lowercase()))
        .collect();
    if !missing.is_empty() {
        return Err(DepError::Archive(format!(
            "missing ONNX Runtime DLLs in archive: {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

#[allow(dead_code)]
fn extract_zip_entry_suffix(bytes: &[u8], suffix: &str, dest: &Path) -> Result<(), DepError> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| DepError::Archive(e.to_string()))?;
    let entry_name = (0..archive.len())
        .filter_map(|i| {
            archive
                .by_index(i)
                .ok()
                .map(|f| f.name().replace('\\', "/"))
        })
        .find(|name| name.ends_with(suffix))
        .ok_or_else(|| DepError::Archive(format!("{suffix} missing in archive")))?;
    let mut entry = archive
        .by_name(&entry_name)
        .map_err(|e| DepError::Archive(e.to_string()))?;
    let tmp = dest.with_extension("dll.tmp");
    if let Some(parent) = tmp.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = fs::File::create(&tmp).map_err(DepError::Io)?;
    std::io::copy(&mut entry, &mut out).map_err(DepError::Io)?;
    out.flush().ok();
    if dest.exists() {
        fs::remove_file(dest).ok();
    }
    fs::rename(&tmp, dest).map_err(DepError::Io)?;
    Ok(())
}

async fn extract_wheel_dlls(
    url: &str,
    dest_dir: &Path,
    dll_names: &[&str],
    reporter: &mut TransferReporter,
    total_policy: HttpTotalPolicy,
) -> Result<(), DepError> {
    let bytes = http_get_bytes(url, reporter, total_policy).await?;
    reporter.check_cancelled()?;
    reporter.set_phase(TransferPhase::Extracting);
    let dest_dir = dest_dir.to_path_buf();
    let dll_names: Vec<String> = dll_names.iter().map(|n| (*n).to_string()).collect();
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        extract_wheel_dlls_sync(&bytes, &dest_dir, &dll_names, &url)
    })
    .await
    .map_err(|e| DepError::Archive(format!("extract task failed: {e}")))?
}

fn extract_wheel_dlls_sync(
    bytes: &[u8],
    dest_dir: &Path,
    dll_names: &[String],
    url: &str,
) -> Result<(), DepError> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| DepError::Archive(e.to_string()))?;
    let targets: Vec<String> = dll_names.iter().map(|n| n.to_ascii_lowercase()).collect();
    let mut written = 0usize;
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| DepError::Archive(e.to_string()))?;
        let name = file.name().replace('\\', "/");
        let base = name
            .rsplit('/')
            .next()
            .unwrap_or(&name)
            .to_ascii_lowercase();
        if !targets.contains(&base) {
            continue;
        }
        let out_name = dll_names
            .iter()
            .find(|n| n.eq_ignore_ascii_case(&base))
            .map(String::as_str)
            .unwrap_or(base.as_str());
        let out_path = dest_dir.join(out_name);
        let mut out = fs::File::create(&out_path).map_err(DepError::Io)?;
        std::io::copy(&mut file, &mut out).map_err(DepError::Io)?;
        written += 1;
    }
    if written == 0 {
        warn!(
            target: "voicesub.asr_local.deps",
            url,
            "no matching DLLs found in wheel"
        );
        return Err(DepError::Archive(format!(
            "no requested DLLs in wheel: {url}"
        )));
    }
    Ok(())
}

pub fn validate_deps_for_provider(
    check: &LocalAsrEnvCheck,
    provider: &str,
) -> Result<(), DepError> {
    if !check.vcruntime.ok {
        return Err(DepError::Check(format!(
            "VC++ runtime missing: {}",
            check.vcruntime.missing.join(", ")
        )));
    }
    if provider.eq_ignore_ascii_case(crate::config::EXECUTION_PROVIDER_CUDA) {
        if !check.cuda_toolkit.ok {
            return Err(DepError::Check(
                check
                    .cuda_toolkit
                    .message
                    .clone()
                    .unwrap_or_else(|| "CUDA Toolkit 13 missing".into()),
            ));
        }
        if !check.cuda_redist.ok {
            return Err(DepError::Check(format!(
                "CUDA runtime missing: {}",
                check.cuda_redist.missing.join(", ")
            )));
        }
        if !check.ort_gpu.ok {
            return Err(DepError::Check(format!(
                "ONNX Runtime (GPU / CUDA EP) incomplete: {}",
                check.ort_gpu.missing.join(", ")
            )));
        }
    } else if !check.ort_cpu.ok && !check.ort_gpu.ok {
        return Err(DepError::Check(
            "ONNX Runtime is not installed (download CPU or GPU package)".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dep_kind_parse() {
        assert_eq!(
            DepDownloadKind::parse("ort_cpu"),
            Some(DepDownloadKind::OrtCpu)
        );
        assert_eq!(
            DepDownloadKind::parse("cuda_redist"),
            Some(DepDownloadKind::CudaRedist)
        );
        assert_eq!(DepDownloadKind::parse("nope"), None);
    }

    #[test]
    fn remove_path_or_defer_is_ok_for_missing_dir() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("runtime").join("cpu");
        remove_path_or_defer(&missing).expect("missing dir");
    }

    #[test]
    fn runtime_removal_drops_gpu_deps_from_env_check() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.gpu).unwrap();
        for name in ORT_GPU_DLLS {
            fs::write(layout.gpu.join(name), b"x").unwrap();
        }
        assert!(env_check(dir.path()).ort_gpu.ok);
        remove_path_or_defer(&layout.gpu).expect("remove gpu runtime");
        assert!(!layout.gpu.is_dir());
        assert!(!env_check(dir.path()).ort_gpu.ok);
    }

    #[test]
    fn env_check_empty_module_dir() {
        let dir = tempfile::tempdir().unwrap();
        let check = env_check(dir.path());
        assert!(!check.ort_cpu.ok);
        assert!(!check.cpu_deps_ready);
        assert!(!check.cuda_deps_ready);
    }

    #[test]
    fn env_check_cpu_ort_present() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.cpu).unwrap();
        for name in ORT_CPU_DLLS {
            fs::write(layout.cpu.join(name), b"fake").unwrap();
        }
        let check = env_check(dir.path());
        assert!(check.ort_cpu.ok);
    }

    #[test]
    fn env_check_gpu_requires_cuda_provider_dlls() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.gpu).unwrap();
        fs::write(layout.gpu.join("onnxruntime.dll"), b"fake").unwrap();
        let check = env_check(dir.path());
        assert!(!check.ort_gpu.ok);
        assert!(
            check
                .ort_gpu
                .missing
                .iter()
                .any(|name| name.contains("providers_cuda"))
        );
    }

    #[test]
    fn preferred_ort_dll_chooses_gpu_when_complete() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.gpu).unwrap();
        for name in ORT_GPU_DLLS {
            fs::write(layout.gpu.join(name), b"fake").unwrap();
        }
        let dll = preferred_ort_dll(dir.path()).expect("gpu ort");
        assert!(dll.ends_with("gpu\\onnxruntime.dll") || dll.ends_with("gpu/onnxruntime.dll"));
    }

    #[test]
    fn env_check_cuda_redist_requires_cudnn() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.cuda).unwrap();
        for name in CUDA_RUNTIME_DLLS {
            fs::write(layout.cuda.join(name), b"fake").unwrap();
        }
        let check = env_check(dir.path());
        assert!(!check.cuda_redist.ok);
        assert!(
            check
                .cuda_redist
                .missing
                .iter()
                .any(|name| name.contains("cudnn64_9"))
        );
    }

    #[test]
    fn resolve_wheel_bytes_uses_fallback_for_zero_or_error() {
        assert_eq!(
            resolve_wheel_bytes(Ok(0), CUDNN_WHEEL_BYTES),
            CUDNN_WHEEL_BYTES
        );
        assert_eq!(
            resolve_wheel_bytes(Err(DepError::Download("missing".into())), CUDNN_WHEEL_BYTES,),
            CUDNN_WHEEL_BYTES
        );
        assert_eq!(resolve_wheel_bytes(Ok(123), CUDNN_WHEEL_BYTES), 123);
    }

    #[test]
    fn validate_cpu_requires_ort_cpu() {
        let dir = tempfile::tempdir().unwrap();
        let check = env_check(dir.path());
        let err = validate_deps_for_provider(&check, "cpu").unwrap_err();
        assert!(err.to_string().contains("ONNX Runtime"));
    }

    #[test]
    fn cuda_deps_ready_without_cpu_ort_when_gpu_stack_complete() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.gpu).unwrap();
        for name in ORT_GPU_DLLS {
            fs::write(layout.gpu.join(name), b"fake").unwrap();
        }
        fs::create_dir_all(&layout.cuda).unwrap();
        for name in CUDA_REDIST_DLLS {
            fs::write(layout.cuda.join(name), b"fake").unwrap();
        }
        let check = env_check(dir.path());
        assert!(!check.ort_cpu.ok);
        assert!(check.ort_gpu.ok);
        assert!(check.cuda_redist.ok);
        // Toolkit 13 is a system install; cuda_deps_ready follows that gate.
        if check.cuda_toolkit.ok {
            assert!(check.cuda_deps_ready);
            validate_deps_for_provider(&check, "cuda").expect("gpu-only cuda path");
        } else {
            assert!(!check.cuda_deps_ready);
            let err = validate_deps_for_provider(&check, "cuda").unwrap_err();
            assert!(err.to_string().contains("CUDA Toolkit 13"));
        }
    }

    #[test]
    fn check_cuda_toolkit_13_detects_marker_dll() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("v13.3").join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join(CUDA13_MARKER_DLL), b"fake").unwrap();
        let status = check_cuda_toolkit_13_in(&[bin]);
        assert!(status.ok);
        assert_eq!(status.version.as_deref(), Some("v13.3"));
        assert!(
            status
                .message
                .as_deref()
                .is_some_and(|m| m == "CUDA Toolkit 13 found")
        );
    }

    #[test]
    fn check_cuda_toolkit_13_detects_marker_in_bin_x64() {
        // Real CUDA 13 Windows layout often places DLLs under bin\x64.
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("v13.3").join("bin");
        let x64 = bin.join("x64");
        fs::create_dir_all(&x64).unwrap();
        fs::write(x64.join(CUDA13_MARKER_DLL), b"fake").unwrap();
        let status = check_cuda_toolkit_13_in(&[bin]);
        assert!(status.ok);
        assert_eq!(status.version.as_deref(), Some("v13.3"));
    }

    #[test]
    fn check_cuda_toolkit_13_rejects_empty_bins() {
        let status = check_cuda_toolkit_13_in(&[]);
        assert!(!status.ok);
        assert_eq!(status.message.as_deref(), Some("CUDA Toolkit 13 not found"));
    }

    #[test]
    fn cuda_toolkit_url_targets_cuda_13() {
        assert!(CUDA_TOOLKIT_URL.contains("cuda-13"));
        assert!(CUDA_TOOLKIT_URL.contains("Windows"));
    }

    #[test]
    fn validate_cpu_accepts_gpu_ort_package() {
        let dir = tempfile::tempdir().unwrap();
        let layout = runtime_layout(dir.path());
        fs::create_dir_all(&layout.gpu).unwrap();
        for name in ORT_GPU_DLLS {
            fs::write(layout.gpu.join(name), b"fake").unwrap();
        }
        let check = env_check(dir.path());
        validate_deps_for_provider(&check, "cpu").expect("gpu ort satisfies cpu ep");
    }
}
