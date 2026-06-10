use std::net::SocketAddr;
use std::sync::Arc;

use std::sync::RwLock;

use super::partial_emit::PartialEmitCoordinator;
use serde_json::Value;

pub type StylePresetsFn = Arc<dyn Fn(Option<&Value>) -> Value + Send + Sync>;
use tokio::sync::RwLock as AsyncRwLock;
use voicesub_config::{AppConfig, ConfigStore, ProjectPaths};
use voicesub_export::ExportService;
use voicesub_logging::{SessionLogManager, StructuredRuntimeLogger};
use voicesub_obs::ObsCaptionService;
use voicesub_subtitle::SubtitleRouter;
use voicesub_translation::TranslationRuntimeController;
use voicesub_tts::TwitchOAuthBridge;
use voicesub_ws::{AsrWorkerHub, EventsHub};

use crate::trace::RuntimePipelineLog;

use super::metrics::RuntimeMetricsCollector;
use super::runtime::RuntimeOrchestrator;
use super::runtime_state::RuntimeStatusBroadcaster;

#[derive(Clone)]
pub struct HttpState {
    pub paths: ProjectPaths,
    pub events: EventsHub,
    pub runtime_broadcaster: Arc<RuntimeStatusBroadcaster>,
    pub pipeline_log: RuntimePipelineLog,
    pub asr_worker: AsrWorkerHub,
    pub config: Arc<AsyncRwLock<ConfigStore>>,
    pub config_snapshot: Arc<RwLock<Value>>,
    pub app_config: AppConfig,
    pub bind_addr: Arc<AsyncRwLock<Option<SocketAddr>>>,
    pub orchestrator: RuntimeOrchestrator,
    pub session_log: Arc<SessionLogManager>,
    pub structured_runtime_logger: Arc<StructuredRuntimeLogger>,
    pub export_service: Arc<ExportService>,
    pub translation: Arc<tokio::sync::Mutex<TranslationRuntimeController>>,
    pub subtitle: Arc<SubtitleRouter>,
    pub obs_captions: Arc<ObsCaptionService>,
    pub runtime_metrics: Arc<RuntimeMetricsCollector>,
    pub partial_emit: Arc<tokio::sync::Mutex<PartialEmitCoordinator>>,
    pub runtime_running: Arc<std::sync::atomic::AtomicBool>,
    pub browser_speech: Arc<crate::browser_speech_source::SharedBrowserSpeechSource>,
    pub twitch_oauth: Arc<TwitchOAuthBridge>,
    pub style_presets: StylePresetsFn,
    pub version: &'static str,
}

impl HttpState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        paths: ProjectPaths,
        events: EventsHub,
        runtime_broadcaster: Arc<RuntimeStatusBroadcaster>,
        pipeline_log: RuntimePipelineLog,
        asr_worker: AsrWorkerHub,
        config: Arc<AsyncRwLock<ConfigStore>>,
        config_snapshot: Arc<RwLock<Value>>,
        app_config: AppConfig,
        bind_addr: Arc<AsyncRwLock<Option<SocketAddr>>>,
        session_log: Arc<SessionLogManager>,
        structured_runtime_logger: Arc<StructuredRuntimeLogger>,
        export_service: Arc<ExportService>,
        translation: Arc<tokio::sync::Mutex<TranslationRuntimeController>>,
        subtitle: Arc<SubtitleRouter>,
        obs_captions: Arc<ObsCaptionService>,
        runtime_metrics: Arc<RuntimeMetricsCollector>,
        partial_emit: Arc<tokio::sync::Mutex<PartialEmitCoordinator>>,
        runtime_running: Arc<std::sync::atomic::AtomicBool>,
        browser_speech: Arc<crate::browser_speech_source::SharedBrowserSpeechSource>,
        twitch_oauth: Arc<TwitchOAuthBridge>,
        style_presets: StylePresetsFn,
        version: &'static str,
    ) -> Arc<Self> {
        Arc::new(Self {
            paths,
            events,
            runtime_broadcaster,
            pipeline_log,
            asr_worker,
            config,
            config_snapshot,
            app_config,
            bind_addr,
            orchestrator: RuntimeOrchestrator::default(),
            session_log,
            structured_runtime_logger,
            export_service,
            translation,
            subtitle,
            obs_captions,
            runtime_metrics,
            partial_emit,
            runtime_running,
            browser_speech,
            twitch_oauth,
            style_presets,
            version,
        })
    }
}
