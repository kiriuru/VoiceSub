use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::{Mutex, RwLock};

use super::partial_emit::PartialEmitCoordinator;
use serde_json::Value;

pub type StylePresetsFn = Arc<dyn Fn(Option<&Value>) -> Value + Send + Sync>;
use tokio::sync::RwLock as AsyncRwLock;
use voicesub_config::{AppConfig, ConfigStore, ProjectPaths};
use voicesub_export::ExportService;
use voicesub_logging::{SessionLogManager, StructuredRuntimeLogger};
use voicesub_obs::ObsCaptionService;
use voicesub_subtitle::{OverlayBroadcaster, SubtitleRouter};
use voicesub_translation::TranslationRuntimeController;
use voicesub_tts::TwitchOAuthBridge;
use voicesub_ws::{AsrWorkerHub, EventsHub, WsEventPublisher};

use crate::trace::RuntimePipelineLog;

use super::background_tasks::BackgroundTaskRegistry;
use super::loopback_auth::LoopbackAuth;
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
    pub ws_publisher: WsEventPublisher,
    pub overlay_broadcaster: Arc<OverlayBroadcaster>,
    pub last_subtitle_payload: Arc<Mutex<Option<Value>>>,
    pub loopback_auth: Arc<LoopbackAuth>,
    pub background_tasks: Arc<BackgroundTaskRegistry>,
    pub local_asr: Arc<voicesub_asr_local::LocalAsrModuleService>,
    pub local_asr_speech: Arc<crate::local_asr_speech_source::SharedLocalAsrSpeechSource>,
}

impl HttpState {
    /// Deliver the latest subtitle/overlay payload to WS clients before teardown.
    pub async fn flush_overlay_presentations_to_clients(&self) {
        self.overlay_broadcaster.reset_dedupe_state();
        let body = self
            .last_subtitle_payload
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(body) = body else {
            return;
        };
        self.ws_publisher
            .broadcast_overlay_body("overlay_update", "overlay_update", body)
            .await;
    }
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
        ws_publisher: WsEventPublisher,
        overlay_broadcaster: Arc<OverlayBroadcaster>,
        last_subtitle_payload: Arc<Mutex<Option<Value>>>,
        loopback_auth: Arc<LoopbackAuth>,
        background_tasks: Arc<BackgroundTaskRegistry>,
        local_asr: Arc<voicesub_asr_local::LocalAsrModuleService>,
        local_asr_speech: Arc<crate::local_asr_speech_source::SharedLocalAsrSpeechSource>,
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
            ws_publisher,
            overlay_broadcaster,
            last_subtitle_payload,
            loopback_auth,
            background_tasks,
            local_asr,
            local_asr_speech,
        })
    }
}
