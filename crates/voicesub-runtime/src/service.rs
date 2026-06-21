use std::collections::VecDeque;
use std::net::SocketAddr;

use std::path::PathBuf;

use std::sync::{
    Arc, Condvar, Mutex, RwLock as StdRwLock, atomic::{AtomicBool, AtomicU64, Ordering},
};

use tokio::sync::RwLock as TokioRwLock;

use axum::Router;

use tokio::sync::RwLock;

use tracing::{info, instrument};

use voicesub_browser::{
    BrowserAsrGateway, BrowserAsrService, BrowserWorkerLauncher, StatusCallback,
    WorkerLifecycleCallback,
    structured_log_from_runtime_logger as browser_structured_log_from_runtime_logger,
};

use crate::http::{
    BackgroundTaskRegistry, HttpState, LoopbackAuth, PartialEmitCoordinator,
    RuntimeMetricsCollector, RuntimeStatusBroadcaster, StylePresetsFn, build_router,
    spawn_runtime_heartbeat, spawn_startup_check,
};
use voicesub_config::read_full_logging_enabled;
use voicesub_config::{
    AppConfig, ConfigStore, ProjectPaths, base_url_from_socket, default_config_payload,
    worker_url_for_payload,
};
use voicesub_export::ExportService;
use voicesub_logging::{
    SessionLogManager, StructuredRuntimeLogger, apply_logging_preferences, ensure_logs_dir,
};
use voicesub_obs::{
    ObsCaptionService, structured_log_from_runtime_logger as obs_structured_log_from_runtime_logger,
};

use voicesub_subtitle::{
    ConfigGetter, OverlayBroadcaster, PublishCallback, SubtitleLog, SubtitleRouter,
    structured_log_from_runtime_logger,
};
use voicesub_ws::{
    AsrWorkerHub, EventsHub, RuntimeEventBus, RuntimeStateSnapshot, WsEventPublisher, WsLog,
    shared_event_sequencer, ws_structured_log_from_runtime_logger,
};

use crate::trace::{
    RuntimePipelineLog, structured_log_from_runtime_logger as runtime_pipeline_structured_log,
};

use voicesub_translation::{TranslationRuntimeController, arc_publish, arc_relevance};

use crate::browser_event_builder::BrowserTranscriptEventBuilder;
use crate::browser_speech_source::{
    BrowserSpeechSource, OrderedBrowserSpeechIngest, SharedBrowserSpeechSource,
};
use crate::transcript_controller::TranscriptController;

use voicesub_tts::TwitchOAuthBridge;
use voicesub_types::PROJECT_VERSION;

pub type SubtitlePayloadListener = Arc<dyn Fn(serde_json::Value) + Send + Sync>;

/// Max queued subtitle payloads while the TTS planner listener is slow; oldest frames drop.
const SUBTITLE_PAYLOAD_QUEUE_MAX: usize = 64;

/// Ordered, non-blocking bridge between the subtitle router actor and the subtitle payload
/// listener (TTS planner). The router actor calls [`SubtitlePayloadForwarder::dispatch`],
/// which only enqueues onto a bounded queue; the dedicated worker thread invokes
/// the listener in arrival order. This keeps a slow listener off the actor's publish loop
/// (review §6) while preserving per-frame ordering.
struct SubtitlePayloadForwarder {
    listener: Arc<Mutex<Option<SubtitlePayloadListener>>>,
    queue: Arc<Mutex<VecDeque<serde_json::Value>>>,
    notify: Arc<Condvar>,
    dropped_oldest: Arc<AtomicU64>,
}

impl SubtitlePayloadForwarder {
    fn new(listener: Arc<Mutex<Option<SubtitlePayloadListener>>>) -> Arc<Self> {
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let notify = Arc::new(Condvar::new());
        let dropped_oldest = Arc::new(AtomicU64::new(0));
        let queue_for_thread = queue.clone();
        let notify_for_thread = notify.clone();
        let listener_for_thread = listener.clone();
        std::thread::Builder::new()
            .name("voicesub-subtitle-payload-forward".into())
            .spawn(move || {
                loop {
                    let body = {
                        let mut guard = queue_for_thread
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        while guard.is_empty() {
                            guard = notify_for_thread
                                .wait(guard)
                                .unwrap_or_else(|e| e.into_inner());
                        }
                        guard.pop_front()
                    };
                    let Some(body) = body else {
                        continue;
                    };
                    let callback = listener_for_thread
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    if let Some(callback) = callback {
                        // Isolate listener panics: a panic in the TTS planner must not kill
                        // the forwarder thread and stop all subsequent subtitle speech
                        // (review §6 / MED#6).
                        if let Err(panic) =
                            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| callback(body)))
                        {
                            let detail = panic
                                .downcast_ref::<&str>()
                                .map(|s| s.to_string())
                                .or_else(|| panic.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "unknown panic".to_string());
                            tracing::error!(
                                target: "voicesub.runtime",
                                error = %detail,
                                "subtitle payload listener panicked; forwarder thread continues"
                            );
                        }
                    }
                }
            })
            .expect("spawn subtitle payload forwarder thread");
        Arc::new(Self {
            listener,
            queue,
            notify,
            dropped_oldest,
        })
    }

    fn dispatch(&self, body: serde_json::Value) {
        let has_listener = self
            .listener
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false);
        if !has_listener {
            return;
        }
        let mut guard = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if guard.len() >= SUBTITLE_PAYLOAD_QUEUE_MAX {
            if drop_incoming_when_queue_full(&body) {
                // Never evict queued speakable frames to enqueue a partial/active update the
                // TTS planner would skip anyway.
                let dropped = self.dropped_oldest.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped == 1 || dropped.is_power_of_two() {
                    tracing::warn!(
                        target: "voicesub.runtime",
                        dropped,
                        queue_max = SUBTITLE_PAYLOAD_QUEUE_MAX,
                        "subtitle payload forwarder dropped incoming non-speakable frame (queue full)"
                    );
                }
                return;
            }
            if let Some(non_speakable_dropped) = evict_one_for_capacity(&mut guard) {
                let dropped = self.dropped_oldest.fetch_add(1, Ordering::Relaxed) + 1;
                if dropped == 1 || dropped.is_power_of_two() {
                    tracing::warn!(
                        target: "voicesub.runtime",
                        dropped,
                        queue_max = SUBTITLE_PAYLOAD_QUEUE_MAX,
                        non_speakable_dropped,
                        "subtitle payload forwarder dropped a queued frame"
                    );
                }
            }
        }
        guard.push_back(body);
        self.notify.notify_one();
    }
}

/// A subtitle payload is "speakable" when its lifecycle state is one the TTS planner will
/// actually voice (`completed_only` / `completed_with_partial`). Mirrors
/// `voicesub_tts::subtitle_speech::lifecycle_allows_planning` so the forwarder's backpressure
/// drop policy never discards a frame that would have produced speech (review HIGH#4).
fn payload_is_speakable(body: &serde_json::Value) -> bool {
    matches!(
        body.get("lifecycle_state").and_then(|v| v.as_str()),
        Some("completed_only") | Some("completed_with_partial")
    )
}

/// When the queue is at capacity, drop incoming partial/active frames instead of evicting
/// queued speakable frames the TTS planner still needs.
fn drop_incoming_when_queue_full(incoming: &serde_json::Value) -> bool {
    !payload_is_speakable(incoming)
}

/// Evict exactly one frame to make room when the queue is full. Prefers the oldest
/// NON-speakable frame (partial-only / active states the planner never voices) so speakable
/// completed frames survive a slow listener; only falls back to dropping the oldest frame
/// when every queued frame is speakable. Returns `Some(true)` if a non-speakable frame was
/// dropped, `Some(false)` if the oldest speakable frame was dropped, `None` if empty
/// (review HIGH#4).
fn evict_one_for_capacity(queue: &mut VecDeque<serde_json::Value>) -> Option<bool> {
    match queue.iter().position(|frame| !payload_is_speakable(frame)) {
        Some(index) => queue.remove(index).map(|_| true),
        None => queue.pop_front().map(|_| false),
    }
}

#[derive(Debug, thiserror::Error)]

pub enum RuntimeError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config error: {0}")]
    Config(#[from] voicesub_config::ConfigError),

    #[error("runtime server failed: {0}")]
    Server(String),
}

pub struct RuntimeHandle {
    pub bind_addr: SocketAddr,

    shutdown: Option<tokio::sync::oneshot::Sender<()>>,

    server_task: Option<tokio::task::JoinHandle<()>>,

    heartbeat_task: Option<tokio::task::JoinHandle<()>>,

    background_tasks: Option<Arc<BackgroundTaskRegistry>>,
}

impl RuntimeHandle {
    pub async fn shutdown(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let mut server_task = self
            .server_task
            .take()
            .expect("runtime server task missing");
        let abort_handle = server_task.abort_handle();
        match tokio::time::timeout(std::time::Duration::from_secs(5), &mut server_task).await {
            Ok(_) => {}
            Err(_) => {
                tracing::warn!("runtime server graceful shutdown timed out; aborting task");
                abort_handle.abort();
                let _ = server_task.await;
            }
        }
        if let Some(heartbeat_task) = self.heartbeat_task.take() {
            heartbeat_task.abort();
        }
        if let Some(tasks) = &self.background_tasks {
            tasks.set_runtime_heartbeat(false);
        }
    }
}

impl Drop for RuntimeHandle {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(heartbeat_task) = self.heartbeat_task.take() {
            heartbeat_task.abort();
        }
        if let Some(tasks) = &self.background_tasks {
            tasks.set_runtime_heartbeat(false);
        }
        if let Some(server_task) = self.server_task.take() {
            server_task.abort();
        }
    }
}

pub struct RuntimeService {
    pub config: AppConfig,

    pub paths: ProjectPaths,

    config_store: Arc<RwLock<ConfigStore>>,

    config_snapshot: Arc<StdRwLock<serde_json::Value>>,

    bind_addr: Arc<TokioRwLock<Option<SocketAddr>>>,

    events: EventsHub,

    ws_publisher: WsEventPublisher,

    runtime_broadcaster: Arc<RuntimeStatusBroadcaster>,

    pipeline_log: RuntimePipelineLog,

    subtitle: Arc<SubtitleRouter>,

    translation: Arc<tokio::sync::Mutex<TranslationRuntimeController>>,

    browser_asr: Arc<BrowserAsrService>,

    asr_worker: AsrWorkerHub,

    obs_captions: Arc<ObsCaptionService>,

    runtime_metrics: Arc<RuntimeMetricsCollector>,

    partial_emit: Arc<tokio::sync::Mutex<PartialEmitCoordinator>>,

    runtime_running: Arc<AtomicBool>,

    browser_speech: Arc<SharedBrowserSpeechSource>,

    twitch_oauth: Arc<TwitchOAuthBridge>,

    structured_runtime_logger: Arc<StructuredRuntimeLogger>,

    session_log: Arc<SessionLogManager>,

    runtime_event_bus: RuntimeEventBus,

    loopback_auth: Arc<LoopbackAuth>,

    background_tasks: Arc<BackgroundTaskRegistry>,

    subtitle_payload_listener: Arc<Mutex<Option<SubtitlePayloadListener>>>,

    overlay_broadcaster: Arc<OverlayBroadcaster>,

    last_subtitle_payload: Arc<Mutex<Option<serde_json::Value>>>,
}

impl RuntimeService {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self::with_config(
            project_root,
            AppConfig::default(),
            Arc::new(TwitchOAuthBridge::default()),
        )
    }

    pub fn with_config(
        project_root: impl Into<PathBuf>,
        config: AppConfig,
        twitch_oauth: Arc<TwitchOAuthBridge>,
    ) -> Self {
        Self::build(ProjectPaths::discover(project_root), config, twitch_oauth)
    }

    /// Workspace assets with an isolated `user-data` + `logs` tree (integration tests).
    pub fn with_config_isolated_user_data(
        project_root: impl Into<PathBuf>,
        user_data_dir: impl Into<PathBuf>,
        config: AppConfig,
        twitch_oauth: Arc<TwitchOAuthBridge>,
    ) -> Self {
        let project_root = project_root.into();
        let user_data_dir = user_data_dir.into();
        let mut paths = ProjectPaths::discover(&project_root);
        paths.user_data_dir = user_data_dir.clone();
        paths.logs_dir = user_data_dir.join("logs");
        Self::build(paths, config, twitch_oauth)
    }

    fn build(paths: ProjectPaths, config: AppConfig, twitch_oauth: Arc<TwitchOAuthBridge>) -> Self {
        let config_store = Arc::new(RwLock::new(ConfigStore::new(paths.config_toml_path())));

        let config_snapshot = Arc::new(StdRwLock::new(default_config_payload()));

        let structured_runtime_logger = Arc::new(StructuredRuntimeLogger::new(&paths.logs_dir));
        let session_log = Arc::new(SessionLogManager::new(&paths.logs_dir));
        let runtime_metrics = Arc::new(RuntimeMetricsCollector::new());
        let pipeline_log = RuntimePipelineLog::new(Some(runtime_pipeline_structured_log(
            structured_runtime_logger.clone(),
        )));
        let ws_log = WsLog::new(Some(ws_structured_log_from_runtime_logger(
            structured_runtime_logger.clone(),
        )));
        let events = EventsHub::with_log(ws_log);
        let runtime_event_bus = RuntimeEventBus::new();
        let loopback_auth = Arc::new(LoopbackAuth::generate());
        let background_tasks = Arc::new(BackgroundTaskRegistry::default());
        let ws_publisher = WsEventPublisher::with_event_bus(
            events.clone(),
            shared_event_sequencer(),
            Some(runtime_event_bus.clone()),
        );
        let subtitle_payload_listener: Arc<Mutex<Option<SubtitlePayloadListener>>> =
            Arc::new(Mutex::new(None));
        let runtime_broadcaster = Arc::new(RuntimeStatusBroadcaster::new(
            ws_publisher.clone(),
            1_000,
            pipeline_log.clone(),
            runtime_metrics.clone(),
        ));
        let subtitle_structured_log =
            structured_log_from_runtime_logger(structured_runtime_logger.clone());
        let subtitle_log = SubtitleLog::new(Some(subtitle_structured_log.clone()));

        let publisher_for_overlay = ws_publisher.clone();
        let last_subtitle_payload: Arc<Mutex<Option<serde_json::Value>>> =
            Arc::new(Mutex::new(None));
        let last_subtitle_payload_for_publish = last_subtitle_payload.clone();
        let overlay_broadcaster = Arc::new(OverlayBroadcaster::new(
            Arc::new({
                let publisher = publisher_for_overlay.clone();
                move |message| {
                    let channel = message
                        .get("type")
                        .and_then(|value| value.as_str())
                        .unwrap_or("overlay_update")
                        .to_string();
                    let body = message.get("payload").cloned().unwrap_or(message);
                    publisher.broadcast_overlay_body_now(&channel, "overlay_update", body);
                }
            }),
            subtitle_log,
        ));
        let overlay_broadcaster_for_publish = overlay_broadcaster.clone();

        // Offload the subtitle payload listener (TTS planner) onto its own ordered worker
        // thread so a slow listener never blocks the subtitle router actor's publish loop
        // (review §6). Order is preserved by the single-consumer channel.
        let subtitle_payload_forwarder =
            SubtitlePayloadForwarder::new(subtitle_payload_listener.clone());
        let forwarder_for_publish = subtitle_payload_forwarder.clone();
        let publish: PublishCallback = Arc::new(move |payload| {
            let overlay = overlay_broadcaster_for_publish.clone();
            let subtitle_body = serde_json::to_value(&payload).unwrap_or_default();
            *last_subtitle_payload_for_publish
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(subtitle_body.clone());
            // Overlay applies its own time/signature dedupe; the subtitle listener
            // (TTS planner) must see every payload independently because it has its
            // own per-sequence dedupe and lifecycle gating. Gating it on the overlay
            // broadcast result would silently drop speakable frames if overlay dedupe
            // ever widened.
            let _overlay_broadcast = overlay.publish(&payload);
            forwarder_for_publish.dispatch(subtitle_body);
        });

        let config_getter: ConfigGetter = {
            let snapshot = config_snapshot.clone();

            Arc::new(move || snapshot.read().unwrap_or_else(|e| e.into_inner()).clone())
        };

        let obs_structured_log =
            obs_structured_log_from_runtime_logger(structured_runtime_logger.clone());
        let obs_captions = ObsCaptionService::new(config_getter.clone(), Some(obs_structured_log));

        let obs_for_publish = obs_captions.clone();
        let base_publish = publish.clone();
        let publish_with_obs: PublishCallback = Arc::new(move |payload| {
            obs_for_publish.publish_payload(payload.clone());
            base_publish(payload);
        });

        let subtitle = SubtitleRouter::new(
            config_getter.clone(),
            publish_with_obs,
            Some(subtitle_structured_log),
        );

        let subtitle_for_translation_publish = subtitle.clone();

        let publisher_for_translation = ws_publisher.clone();

        let translation_publish = arc_publish(move |event| {
            let subtitle = subtitle_for_translation_publish.clone();

            let publisher = publisher_for_translation.clone();

            async move {
                let relevant = subtitle
                    .is_sequence_relevant_for_presentation(event.sequence)
                    .await;
                subtitle.handle_translation(event.clone()).await;

                if relevant {
                    let body = serde_json::to_value(&event).unwrap_or_default();

                    publisher.broadcast_channel_now(
                        "translation_update",
                        "translation_update",
                        body,
                    );
                }
            }
        });

        let subtitle_for_relevance = subtitle.clone();

        let translation_relevance = arc_relevance(move |sequence| {
            let subtitle = subtitle_for_relevance.clone();

            async move {
                subtitle
                    .is_sequence_relevant_for_translation(sequence)
                    .await
            }
        });

        let translation_cache_dir = paths.user_data_dir.join("cache");
        let translation = Arc::new(tokio::sync::Mutex::new(TranslationRuntimeController::new(
            config_getter,
            translation_publish,
            translation_relevance,
            Some(translation_cache_dir),
        )));

        let partial_emit = Arc::new(tokio::sync::Mutex::new(PartialEmitCoordinator::default()));

        let runtime_running = Arc::new(AtomicBool::new(false));

        let transcript_controller = Arc::new(TranscriptController::new(
            subtitle.clone(),
            translation.clone(),
            obs_captions.clone(),
            ws_publisher.clone(),
            config_snapshot.clone(),
            pipeline_log.clone(),
            runtime_metrics.clone(),
        ));

        let event_builder = Arc::new(BrowserTranscriptEventBuilder::new(
            runtime_running.clone(),
            partial_emit.clone(),
        ));

        let browser_structured_log =
            browser_structured_log_from_runtime_logger(structured_runtime_logger.clone());
        let browser_asr_gateway = Arc::new(std::sync::Mutex::new(BrowserAsrGateway::new(Some(
            browser_structured_log,
        ))));

        let browser_speech = SharedBrowserSpeechSource::new(BrowserSpeechSource::new(
            runtime_running.clone(),
            event_builder,
            transcript_controller,
            config_snapshot.clone(),
            browser_asr_gateway.clone(),
            runtime_metrics.clone(),
        ));

        let gateway_for_status = browser_asr_gateway.clone();
        let on_status_update: StatusCallback = Arc::new(move |payload| {
            if let Ok(mut gateway) = gateway_for_status.lock() {
                gateway.update_status(&payload);
            }
        });

        let gateway_for_connected = browser_asr_gateway.clone();
        let on_worker_connected: WorkerLifecycleCallback = Arc::new(move || {
            if let Ok(mut gateway) = gateway_for_connected.lock() {
                gateway.worker_connected();
            }
        });

        let gateway_for_disconnect = browser_asr_gateway.clone();
        let subtitle_for_disconnect = subtitle.clone();
        let partial_emit_for_disconnect = partial_emit.clone();
        let on_worker_disconnected: WorkerLifecycleCallback = Arc::new(move || {
            if let Ok(mut gateway) = gateway_for_disconnect.lock() {
                gateway.worker_disconnected();
            }
            let subtitle = subtitle_for_disconnect.clone();
            let partial_emit = partial_emit_for_disconnect.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    subtitle.clear_active_partial().await;
                    partial_emit
                        .lock()
                        .await
                        .segment_state
                        .cleanup_on_browser_worker_disconnect();
                });
            }
        });

        let browser_speech_for_ingest =
            Arc::new(OrderedBrowserSpeechIngest::new(browser_speech.clone()));
        let browser_asr = Arc::new(BrowserAsrService::with_hooks(
            Arc::new(move |update| {
                browser_speech_for_ingest.enqueue(update);
            }),
            Some(on_worker_connected),
            Some(on_worker_disconnected),
            Some(on_status_update),
        ));

        let asr_worker = AsrWorkerHub::new(browser_asr.clone());

        Self {
            config,

            paths,

            config_store,

            config_snapshot,

            bind_addr: Arc::new(TokioRwLock::new(None)),

            events,

            ws_publisher,

            runtime_broadcaster,
            pipeline_log,

            subtitle,

            translation,

            browser_asr,

            asr_worker,

            obs_captions,

            runtime_metrics,

            partial_emit,
            runtime_running,
            browser_speech,
            twitch_oauth,
            structured_runtime_logger,
            session_log,
            runtime_event_bus,
            loopback_auth,
            background_tasks,
            subtitle_payload_listener,
            overlay_broadcaster,
            last_subtitle_payload,
        }
    }

    pub fn set_subtitle_payload_listener(&self, listener: SubtitlePayloadListener) {
        if let Ok(mut guard) = self.subtitle_payload_listener.lock() {
            *guard = Some(listener);
        }
    }

    pub fn runtime_event_bus(&self) -> RuntimeEventBus {
        self.runtime_event_bus.clone()
    }

    pub fn loopback_api_token(&self) -> &str {
        self.loopback_auth.token()
    }

    pub async fn runtime_state_snapshot(&self) -> RuntimeStateSnapshot {
        let subtitle = self
            .last_subtitle_payload
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        RuntimeStateSnapshot {
            rev: self.runtime_event_bus.revision(),
            runtime: self
                .events
                .last_message("runtime_update")
                .await
                .map(|message| message.get("payload").cloned().unwrap_or_default())
                .unwrap_or(serde_json::Value::Null),
            subtitle,
            overlay: self
                .events
                .last_message("overlay_update")
                .await
                .map(|message| message.get("payload").cloned().unwrap_or_default()),
            translation: self
                .events
                .last_message("translation_update")
                .await
                .map(|message| message.get("payload").cloned().unwrap_or_default()),
            diagnostics: self
                .events
                .last_message("diagnostics_update")
                .await
                .map(|message| message.get("payload").cloned().unwrap_or_default()),
            twitch_connection: self
                .events
                .last_message("twitch_connection_update")
                .await
                .map(|message| message.get("payload").cloned().unwrap_or_default()),
        }
    }

    pub fn config_store(&self) -> Arc<RwLock<ConfigStore>> {
        self.config_store.clone()
    }

    pub fn asr_worker(&self) -> AsrWorkerHub {
        self.asr_worker.clone()
    }

    pub fn browser_asr_service(&self) -> Arc<BrowserAsrService> {
        self.browser_asr.clone()
    }

    pub fn subtitle_router(&self) -> Arc<SubtitleRouter> {
        self.subtitle.clone()
    }

    pub fn translation_controller(&self) -> Arc<tokio::sync::Mutex<TranslationRuntimeController>> {
        self.translation.clone()
    }

    pub fn obs_captions(&self) -> Arc<ObsCaptionService> {
        self.obs_captions.clone()
    }

    pub fn events_hub(&self) -> EventsHub {
        self.events.clone()
    }

    pub fn ws_publisher(&self) -> WsEventPublisher {
        self.ws_publisher.clone()
    }

    pub fn http_state(&self) -> Arc<HttpState> {
        let session_log = self.session_log.clone();
        let structured_runtime_logger = self.structured_runtime_logger.clone();
        let export_service = Arc::new(ExportService::from_paths(&self.paths, PROJECT_VERSION));

        let style_presets: StylePresetsFn = Arc::new(voicesub_subtitle::subtitle_style_presets);
        HttpState::new(
            self.paths.clone(),
            self.events.clone(),
            self.runtime_broadcaster.clone(),
            self.pipeline_log.clone(),
            self.asr_worker.clone(),
            self.config_store.clone(),
            self.config_snapshot.clone(),
            self.config.clone(),
            self.bind_addr.clone(),
            session_log,
            structured_runtime_logger,
            export_service,
            self.translation.clone(),
            self.subtitle.clone(),
            self.obs_captions.clone(),
            self.runtime_metrics.clone(),
            self.partial_emit.clone(),
            self.runtime_running.clone(),
            self.browser_speech.clone(),
            self.twitch_oauth.clone(),
            style_presets,
            PROJECT_VERSION,
            self.ws_publisher.clone(),
            self.overlay_broadcaster.clone(),
            self.last_subtitle_payload.clone(),
            self.loopback_auth.clone(),
            self.background_tasks.clone(),
        )
    }

    pub fn router(state: Arc<HttpState>) -> Router {
        build_router(state)
    }

    #[instrument(skip(self))]

    pub async fn start(&self) -> Result<RuntimeHandle, RuntimeError> {
        voicesub_config::ensure_runtime_data_dirs(&self.paths)
            .map_err(|err| RuntimeError::Server(err.to_string()))?;
        let _ = ensure_logs_dir(&self.paths.project_root)?;

        // Reap a high-priority browser worker left over from a previous crashed session
        // before we accept new sessions (review §8).
        voicesub_browser::reap_orphan_worker(&self.paths.user_data_dir);

        {
            let mut store = self.config_store.write().await;

            store.load_or_create()?;

            if let Ok(mut snapshot) = self.config_snapshot.write() {
                *snapshot = store.payload().clone();
            }
            apply_logging_preferences(
                &self.paths.logs_dir,
                read_full_logging_enabled(store.payload()),
            );
        }

        let addr = self.config.http.socket_addr();

        let state = self.http_state();
        self.background_tasks.set_http_server(true);
        self.background_tasks.set_runtime_heartbeat(true);
        self.background_tasks.set_startup_check(true);
        let heartbeat_task =
            spawn_runtime_heartbeat(self.runtime_broadcaster.clone(), state.clone());
        spawn_startup_check(state.clone());
        let background_tasks = self.background_tasks.clone();
        let router = Self::router(state);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|err| RuntimeError::Server(err.to_string()))?;

        let bound = listener
            .local_addr()
            .map_err(|err| RuntimeError::Server(err.to_string()))?;

        *self.bind_addr.write().await = Some(bound);

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let server_task = tokio::spawn(async move {
            let shutdown_tasks = background_tasks.clone();
            let server = axum::serve(listener, router).with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
                shutdown_tasks.set_http_server(false);
                info!("http server shutdown requested");
            });

            if let Err(err) = server.await {
                tracing::error!(error = %err, "http server exited with error");
            }
            background_tasks.set_http_server(false);
        });

        info!(%bound, "VoiceSub runtime listening");

        Ok(RuntimeHandle {
            bind_addr: bound,

            shutdown: Some(shutdown_tx),

            server_task: Some(server_task),

            heartbeat_task: Some(heartbeat_task),

            background_tasks: Some(self.background_tasks.clone()),
        })
    }

    pub async fn launch_browser_worker(
        &self,
    ) -> Result<voicesub_browser::LaunchResult, voicesub_browser::BrowserLaunchError> {
        let base = if let Some(addr) = *self.bind_addr.read().await {
            base_url_from_socket(addr)
        } else {
            self.config.http.base_url()
        };

        let store = self.config_store.read().await;
        let payload = store.payload().clone();
        let url = worker_url_for_payload(&base, &payload);
        let chrome_launch = voicesub_browser::chrome_launch_from_config(&payload);

        let launcher = BrowserWorkerLauncher::new(&self.paths.user_data_dir);

        let result = launcher.launch_worker(&url, &chrome_launch)?;
        // Persist the PID so a crash before graceful stop can reap this worker on the next
        // startup, matching the HTTP `/api/runtime/start` path (review HIGH#3). Without this
        // an IPC-launched worker is invisible to `reap_orphan_worker`.
        voicesub_browser::record_worker_pid(&self.paths.user_data_dir, result.pid);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]

    fn runtime_service_builds_router() {
        let service = RuntimeService::new(".");

        let _router = RuntimeService::router(service.http_state());
    }

    #[test]
    fn payload_is_speakable_matches_planner_lifecycle_states() {
        assert!(payload_is_speakable(
            &serde_json::json!({ "lifecycle_state": "completed_only" })
        ));
        assert!(payload_is_speakable(
            &serde_json::json!({ "lifecycle_state": "completed_with_partial" })
        ));
        assert!(!payload_is_speakable(
            &serde_json::json!({ "lifecycle_state": "partial_only" })
        ));
        assert!(!payload_is_speakable(&serde_json::json!({})));
    }

    #[test]
    fn evict_prefers_non_speakable_frame_over_speakable() {
        let mut queue = VecDeque::new();
        // Oldest is speakable; a later partial-only frame is the better drop candidate.
        queue.push_back(serde_json::json!({ "lifecycle_state": "completed_only", "id": 1 }));
        queue.push_back(serde_json::json!({ "lifecycle_state": "partial_only", "id": 2 }));
        queue.push_back(serde_json::json!({ "lifecycle_state": "completed_only", "id": 3 }));

        assert_eq!(evict_one_for_capacity(&mut queue), Some(true));
        let ids: Vec<u64> = queue
            .iter()
            .filter_map(|f| f.get("id").and_then(|v| v.as_u64()))
            .collect();
        // The speakable completed frames are preserved; the partial-only frame was dropped.
        assert_eq!(ids, vec![1, 3]);
    }

    #[test]
    fn evict_falls_back_to_oldest_when_all_speakable() {
        let mut queue = VecDeque::new();
        queue.push_back(serde_json::json!({ "lifecycle_state": "completed_only", "id": 1 }));
        queue.push_back(serde_json::json!({ "lifecycle_state": "completed_with_partial", "id": 2 }));

        assert_eq!(evict_one_for_capacity(&mut queue), Some(false));
        let ids: Vec<u64> = queue
            .iter()
            .filter_map(|f| f.get("id").and_then(|v| v.as_u64()))
            .collect();
        assert_eq!(ids, vec![2]);
    }

    #[test]
    fn drop_incoming_when_queue_full_rejects_partial_only() {
        assert!(drop_incoming_when_queue_full(
            &serde_json::json!({ "lifecycle_state": "partial_only" })
        ));
        assert!(!drop_incoming_when_queue_full(
            &serde_json::json!({ "lifecycle_state": "completed_only" })
        ));
    }

    #[test]
    fn evict_returns_none_when_empty() {
        let mut queue: VecDeque<serde_json::Value> = VecDeque::new();
        assert_eq!(evict_one_for_capacity(&mut queue), None);
    }
}
