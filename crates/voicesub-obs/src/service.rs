use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use chrono::Utc;
use serde_json::{json, Value};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use tracing::debug;
use voicesub_subtitle::{ConfigGetter, SubtitlePayloadEvent};

use crate::client::{ObsClientError, ObsClientHandle, ObsWsClient};
#[cfg(test)]
use crate::client::MockObsClient;
use crate::diagnostics::{ConnectionState, ObsCaptionDiagnostics};
use crate::error_codes::{self, native_status};
use crate::settings::{ObsCaptionSettings, CONNECTABLE_OUTPUT_MODES, SOURCE_EVENT_OUTPUT_MODES};
use crate::text::{
    normalize_text, select_first_visible_text, select_payload_text, should_throttle_partial_update,
};
use crate::trace::{ObsCaptionLog, StructuredLogFn};

const QUEUE_MAX_SIZE: usize = 32;

enum QueueItem {
    SourcePartial(String),
    SourceFinal(String),
    Payload(Box<SubtitlePayloadEvent>),
    DelayedSend {
        text: String,
        send_stream: bool,
        mirror_debug: bool,
        delay_ms: u64,
        generation: u64,
    },
    DelayedClear {
        send_stream: bool,
        mirror_debug: bool,
        delay_ms: u64,
        generation: u64,
    },
}

pub struct ObsCaptionService {
    config_getter: ConfigGetter,
    inner: Arc<Inner>,
}

struct Inner {
    config_getter: ConfigGetter,
    log: ObsCaptionLog,
    queue: StdMutex<VecDeque<QueueItem>>,
    queue_notify: Notify,
    worker_task: Mutex<Option<JoinHandle<()>>>,
    connection_task: Mutex<Option<JoinHandle<()>>>,
    client: Mutex<Option<ObsClientHandle>>,
    diagnostics: Mutex<ObsCaptionDiagnostics>,
    desired_connection: Mutex<bool>,
    connected_notify: Notify,
    last_partial_text: Mutex<String>,
    last_partial_sent: Mutex<Option<Instant>>,
    last_payload_signature: StdMutex<Option<(u64, String, String)>>,
    connection_key: Mutex<Option<(String, u16, String)>>,
    delayed_generation: AtomicU64,
    clear_generation: AtomicU64,
}

#[cfg(test)]
impl ObsCaptionService {
    pub async fn install_mock_client(&self, mock: MockObsClient) {
        {
            let mut diag = self.inner.diagnostics.lock().await;
            diag.connected = true;
            diag.connection_state = ConnectionState::Connected;
            diag.stream_output_active = Some(true);
            diag.last_error = None;
        }
        *self.inner.client.lock().await = Some(ObsClientHandle::Mock(mock));
        self.inner.connected_notify.notify_waiters();
    }
}

impl ObsCaptionService {
    pub fn new(config_getter: ConfigGetter, structured_log: Option<StructuredLogFn>) -> Arc<Self> {
        let log = ObsCaptionLog::new(structured_log);
        Arc::new(Self {
            config_getter: config_getter.clone(),
            inner: Arc::new(Inner {
                config_getter,
                log: log.clone(),
                queue: StdMutex::new(VecDeque::new()),
                queue_notify: Notify::new(),
                worker_task: Mutex::new(None),
                connection_task: Mutex::new(None),
                client: Mutex::new(None),
                diagnostics: Mutex::new(ObsCaptionDiagnostics::default()),
                desired_connection: Mutex::new(false),
                connected_notify: Notify::new(),
                last_partial_text: Mutex::new(String::new()),
                last_partial_sent: Mutex::new(None),
                last_payload_signature: StdMutex::new(None),
                connection_key: Mutex::new(None),
                delayed_generation: AtomicU64::new(0),
                clear_generation: AtomicU64::new(0),
            }),
        })
    }

    pub async fn diagnostics(&self) -> Value {
        let settings = ObsCaptionSettings::from_config(&(self.config_getter)());
        let diag = self.inner.diagnostics.lock().await.clone();
        let mut value = diag.to_value(&settings);
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "active".into(),
                json!(
                    settings.native_enabled()
                        && diag.connected
                        && diag.stream_output_active == Some(true)
                ),
            );
            obj.insert(
                "native_caption_ready".into(),
                json!(
                    settings.native_enabled()
                        && diag.connected
                        && diag.stream_output_active == Some(true)
                ),
            );
        }
        value
    }

    pub async fn start(&self) {
        let mut worker = self.inner.worker_task.lock().await;
        if worker.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        let inner = self.inner.clone();
        self.inner.log.service_started();
        *worker = Some(tokio::spawn(async move {
            worker_loop(inner).await;
        }));
    }

    pub async fn stop(&self) {
        self.inner.log.service_stopped();
        bump_delayed_generation(&self.inner);
        bump_clear_generation(&self.inner);
        *self.inner.desired_connection.lock().await = false;
        drain_queue(&self.inner);
        let settings = ObsCaptionSettings::from_config(&(self.config_getter)());
        self.clear_remote_outputs_if_possible(&settings).await;
        if let Some(task) = self.inner.connection_task.lock().await.take() {
            task.abort();
        }
        if let Some(client) = self.inner.client.lock().await.take() {
            client.close().await;
        }
        {
            let mut diag = self.inner.diagnostics.lock().await;
            diag.connected = false;
            diag.stream_output_active = None;
            diag.stream_output_reconnecting = None;
            diag.native_caption_status = None;
        }
        set_connection_state(&self.inner, ConnectionState::Disconnected, None).await;
        self.inner.connected_notify.notify_waiters();
        if let Some(task) = self.inner.worker_task.lock().await.take() {
            task.abort();
        }
        drain_queue(&self.inner);
        *self.inner.last_partial_text.lock().await = String::new();
        *self.inner.last_partial_sent.lock().await = None;
        *self.inner.last_payload_signature.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    async fn clear_remote_outputs_if_possible(&self, settings: &ObsCaptionSettings) {
        let mut client_guard = self.inner.client.lock().await;
        let Some(client) = client_guard.as_mut() else {
            return;
        };

        let diag = self.inner.diagnostics.lock().await;
        let should_clear_native = (settings.enabled
            && CONNECTABLE_OUTPUT_MODES.contains(&settings.output_mode.as_str()))
            || diag.last_caption_text.is_some();
        let debug_input_name = if settings.debug_text_input_enabled() {
            Some(settings.debug_input_name.clone())
        } else {
            None
        };
        let last_debug_input_name = diag.last_debug_input_name.clone();
        let should_clear_debug =
            debug_input_name.is_some() || diag.last_debug_text.is_some() || last_debug_input_name.is_some();
        drop(diag);

        if should_clear_native
            && client
                .send_request("SendStreamCaption", json!({ "captionText": "" }))
                .await
                .is_ok()
        {
            let mut diag = self.inner.diagnostics.lock().await;
            diag.last_caption_text = Some(String::new());
            diag.last_caption_sent_at_utc = Some(utc_now_iso());
        }

        if should_clear_debug {
            if let Some(input_name) = debug_input_name.or(last_debug_input_name) {
                let _ = client
                    .send_request(
                        "SetInputSettings",
                        json!({
                            "inputName": input_name,
                            "inputSettings": { "text": "" },
                            "overlay": true
                        }),
                    )
                    .await;
                let mut diag = self.inner.diagnostics.lock().await;
                diag.last_debug_text = Some(String::new());
                diag.last_debug_input_name = Some(input_name);
            }
        }
    }

    pub async fn apply_live_settings(&self) {
        bump_delayed_generation(&self.inner);
        bump_clear_generation(&self.inner);
        let settings = ObsCaptionSettings::from_config(&(self.config_getter)());
        if !settings.should_connect() {
            *self.inner.desired_connection.lock().await = false;
            drain_queue(&self.inner);
            self.clear_remote_outputs_if_possible(&settings).await;
            if let Some(task) = self.inner.connection_task.lock().await.take() {
                task.abort();
            }
            if let Some(client) = self.inner.client.lock().await.take() {
                client.close().await;
            }
            {
                let mut diag = self.inner.diagnostics.lock().await;
                diag.connected = false;
                diag.stream_output_active = None;
                diag.stream_output_reconnecting = None;
                diag.native_caption_status = None;
            }
            set_connection_state(&self.inner, ConnectionState::Disabled, None).await;
            self.inner.log.live_settings_applied(
                settings.enabled,
                false,
                &settings.output_mode,
                false,
            );
            return;
        }

        let next_key = settings.connection_key();
        let mut connection_key = self.inner.connection_key.lock().await;
        let connection_key_changed = connection_key.as_ref() != Some(&next_key);
        if connection_key_changed {
            *connection_key = Some(next_key);
            if let Some(client) = self.inner.client.lock().await.take() {
                client.close().await;
            }
            {
                let mut diag = self.inner.diagnostics.lock().await;
                diag.connected = false;
            }
            set_connection_state(&self.inner, ConnectionState::Disconnected, None).await;
        }
        drop(connection_key);

        *self.inner.desired_connection.lock().await = true;
        self.inner.log.live_settings_applied(
            settings.enabled,
            true,
            &settings.output_mode,
            connection_key_changed,
        );
        ensure_connection_task(self.inner.clone()).await;
    }

    pub fn publish_source(&self, text: &str, is_final: bool) {
        if is_final {
            bump_delayed_generation(&self.inner);
            bump_clear_generation(&self.inner);
        }
        let item = if is_final {
            QueueItem::SourceFinal(text.to_string())
        } else {
            QueueItem::SourcePartial(text.to_string())
        };
        self.enqueue(item);
    }

    pub fn publish_payload(&self, payload: SubtitlePayloadEvent) {
        let settings = ObsCaptionSettings::from_config(&(self.config_getter)());
        if payload_will_supersede_caption(&self.inner, &settings, &payload) {
            bump_delayed_generation(&self.inner);
            bump_clear_generation(&self.inner);
        }
        self.enqueue(QueueItem::Payload(Box::new(payload)));
    }

    fn enqueue(&self, item: QueueItem) {
        let worker_running = self
            .inner
            .worker_task
            .try_lock()
            .ok()
            .and_then(|worker| {
                worker
                    .as_ref()
                    .map(|task| !task.is_finished())
            })
            .unwrap_or(false);
        if !worker_running {
            return;
        }
        if matches!(&item, QueueItem::SourcePartial(_)) {
            drop_queued_partials(&self.inner);
        }
        {
            let mut queue = self.inner.queue.lock().expect("obs queue lock");
            if queue.len() >= QUEUE_MAX_SIZE {
                queue.pop_front();
            }
            queue.push_back(item);
        }
        self.inner.queue_notify.notify_one();
    }
}

async fn ensure_connection_task(inner: Arc<Inner>) {
    let mut task_slot = inner.connection_task.lock().await;
    if task_slot.as_ref().is_some_and(|task| !task.is_finished()) {
        return;
    }
    let inner_clone = inner.clone();
    *task_slot = Some(tokio::spawn(async move {
        connection_loop(inner_clone).await;
    }));
}

async fn connection_loop(inner: Arc<Inner>) {
    let mut backoff = Duration::from_secs(1);
    loop {
        if !*inner.desired_connection.lock().await {
            break;
        }
        let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
        if !settings.should_connect() {
            set_connection_state(&inner, ConnectionState::Disabled, None).await;
            break;
        }

        if inner.client.lock().await.is_some() {
            tokio::time::sleep(Duration::from_secs(15)).await;
            if !*inner.desired_connection.lock().await {
                break;
            }
            let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
            let mut client_guard = inner.client.lock().await;
            if let Some(client) = client_guard.as_mut() {
                if let Err(err) = client.ping().await {
                    let code = error_codes::error::CONNECTION_LOST;
                    let detail = err.to_string();
                    {
                        let mut diag = inner.diagnostics.lock().await;
                        diag.last_error = Some(code.into());
                        diag.connected = false;
                    }
                    inner.log.connection_lost(&detail);
                    set_connection_state(&inner, ConnectionState::Error, Some(code)).await;
                    drop(client_guard);
                    if let Some(client) = inner.client.lock().await.take() {
                        client.close().await;
                    }
                    inner.connected_notify.notify_waiters();
                    continue;
                }
                if let Err(err) = refresh_stream_status(&inner, client, &settings).await {
                    debug!(error = %err, "obs stream status refresh failed");
                }
            }
            continue;
        }

        set_connection_state(&inner, ConnectionState::Connecting, None).await;

        match ObsWsClient::connect(&settings.host, settings.port, &settings.password).await {
            Ok(client) => {
                let (studio, ws_ver) = client.versions();
                {
                    let mut diag = inner.diagnostics.lock().await;
                    diag.connected = true;
                    diag.last_error = None;
                    diag.reconnect_attempt_count = 0;
                    diag.obs_studio_version = studio.map(str::to_string);
                    diag.obs_websocket_version = ws_ver.map(str::to_string);
                }
                set_connection_state(&inner, ConnectionState::Connected, None).await;
                *inner.connection_key.lock().await = Some(settings.connection_key());
                *inner.client.lock().await = Some(ObsClientHandle::WebSocket(Box::new(client)));
                inner.connected_notify.notify_waiters();
                backoff = Duration::from_secs(1);
                if let Some(client) = inner.client.lock().await.as_mut() {
                    let _ = refresh_stream_status(&inner, client, &settings).await;
                }
            }
            Err(ObsClientError::PasswordRequired) => {
                let code = error_codes::error::PASSWORD_REQUIRED;
                {
                    let mut diag = inner.diagnostics.lock().await;
                    diag.reconnect_attempt_count += 1;
                    diag.last_error = Some(code.into());
                    diag.connected = false;
                }
                set_connection_state(&inner, ConnectionState::AuthFailed, Some(code)).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(ObsClientError::AuthFailed) => {
                let code = error_codes::error::AUTH_FAILED;
                {
                    let mut diag = inner.diagnostics.lock().await;
                    diag.reconnect_attempt_count += 1;
                    diag.last_error = Some(code.into());
                    diag.connected = false;
                }
                set_connection_state(&inner, ConnectionState::AuthFailed, Some(code)).await;
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(err) => {
                let code = error_codes::obs_client_error_code(&err);
                {
                    let mut diag = inner.diagnostics.lock().await;
                    diag.reconnect_attempt_count += 1;
                    diag.last_error = Some(code.into());
                    diag.connected = false;
                }
                debug!(error = %err, code, "obs websocket connect failed");
                set_connection_state(&inner, ConnectionState::Error, Some(code)).await;
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(10));
            }
        }
    }
}

async fn refresh_stream_status(
    inner: &Inner,
    client: &mut ObsClientHandle,
    settings: &ObsCaptionSettings,
) -> Result<(), ObsClientError> {
    if !settings.native_enabled() {
        let mut diag = inner.diagnostics.lock().await;
        diag.stream_output_active = None;
        diag.stream_output_reconnecting = None;
        return Ok(());
    }
    let response = client.send_request("GetStreamStatus", json!({})).await?;
    let output_active = response
        .get("outputActive")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let output_reconnecting = response
        .get("outputReconnecting")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let mut diag = inner.diagnostics.lock().await;
    diag.stream_output_active = Some(output_active);
    diag.stream_output_reconnecting = Some(output_reconnecting);
    diag.native_caption_status = Some(if output_active {
        if output_reconnecting {
            native_status::STREAM_ACTIVE_RECONNECTING.into()
        } else {
            native_status::STREAM_ACTIVE.into()
        }
    } else {
        native_status::STREAM_INACTIVE.into()
    });
    Ok(())
}

async fn worker_loop(inner: Arc<Inner>) {
    loop {
        let item = {
            let mut queue = inner.queue.lock().expect("obs queue lock");
            queue.pop_front()
        };
        let Some(item) = item else {
            inner.queue_notify.notified().await;
            continue;
        };
        if let Err(err) = process_item(inner.clone(), item).await {
            let mut diag = inner.diagnostics.lock().await;
            diag.last_error = Some(err);
        }
    }
}

async fn process_item(inner: Arc<Inner>, item: QueueItem) -> Result<(), String> {
    match item {
        QueueItem::DelayedSend {
            text,
            send_stream,
            mirror_debug,
            delay_ms,
            generation,
        } => {
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            if inner.delayed_generation.load(Ordering::SeqCst) != generation {
                return Ok(());
            }
            let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
            send_text(
                inner.clone(),
                &text,
                &settings,
                send_stream,
                mirror_debug,
                false,
                true,
            )
            .await
        }
        QueueItem::DelayedClear {
            send_stream,
            mirror_debug,
            delay_ms,
            generation,
        } => {
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            if inner.clear_generation.load(Ordering::SeqCst) != generation {
                return Ok(());
            }
            let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
            send_text(inner, "", &settings, send_stream, mirror_debug, true, false).await
        }
        QueueItem::SourcePartial(text) => handle_source_partial(inner.clone(), &text).await,
        QueueItem::SourceFinal(text) => handle_source_final(inner.clone(), &text).await,
        QueueItem::Payload(payload) => handle_payload(inner, *payload).await,
    }
}

async fn handle_source_partial(inner: Arc<Inner>, text: &str) -> Result<(), String> {
    let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
    let mode = settings.output_mode.as_str();
    let mut send_stream = settings.native_enabled() && mode == "source_live";
    if send_stream {
        let diag = inner.diagnostics.lock().await;
        if diag.stream_output_active == Some(false) {
            send_stream = false;
        }
    }
    let mirror_debug = settings.debug_text_input_enabled()
        && mode == "source_live"
        && settings.debug_send_partials;
    if !send_stream && !mirror_debug {
        return Ok(());
    }
    if send_stream && !settings.send_partials {
        send_stream = false;
    }
    if !send_stream && !mirror_debug {
        return Ok(());
    }
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return Ok(());
    }
    let previous = inner.last_partial_text.lock().await.clone();
    let elapsed_ms = inner
        .last_partial_sent
        .lock()
        .await
        .map(|instant| instant.elapsed().as_millis() as u64);
    if normalized == previous {
        return Ok(());
    }
    if should_throttle_partial_update(
        &previous,
        &normalized,
        elapsed_ms,
        settings.partial_throttle_ms,
        settings.min_partial_delta_chars,
    ) {
        inner.log.partial_throttled(normalized.chars().count(), elapsed_ms);
        return Ok(());
    }
    *inner.last_partial_text.lock().await = normalized.clone();
    *inner.last_partial_sent.lock().await = Some(Instant::now());
    send_text(
        inner,
        &normalized,
        &settings,
        send_stream,
        mirror_debug,
        !settings.avoid_duplicate_text,
        false,
    )
    .await
}

async fn handle_source_final(inner: Arc<Inner>, text: &str) -> Result<(), String> {
    let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
    let mode = settings.output_mode.as_str();
    let send_stream = settings.native_enabled() && SOURCE_EVENT_OUTPUT_MODES.contains(&mode);
    let mirror_debug =
        settings.debug_text_input_enabled() && SOURCE_EVENT_OUTPUT_MODES.contains(&mode);
    if !send_stream && !mirror_debug {
        return Ok(());
    }
    let normalized = normalize_text(text);
    if normalized.is_empty() {
        return Ok(());
    }
    *inner.last_partial_text.lock().await = String::new();
    *inner.last_partial_sent.lock().await = None;
    *inner.last_payload_signature.lock().unwrap_or_else(|e| e.into_inner()) = None;
    schedule_final_send(inner, normalized, send_stream, mirror_debug).await?;
    Ok(())
}

async fn handle_payload(inner: Arc<Inner>, payload: SubtitlePayloadEvent) -> Result<(), String> {
    let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
    let mode = settings.output_mode.as_str();
    let send_stream = settings.native_enabled()
        && !matches!(mode, "disabled" | "source_live" | "source_final_only");
    let mirror_debug = settings.debug_text_input_enabled();
    if !send_stream && !mirror_debug {
        return Ok(());
    }
    if !payload.completed_block_visible {
        return Ok(());
    }
    let mut selected = select_payload_text(&payload, mode);
    // SST parity: non-translation payload modes fall back to the first visible line.
    // With the current output_mode enum, send_stream here is only true for translation_*
    // and first_visible_line, so this branch is inactive today but kept for forward compatibility.
    if send_stream && !mode.starts_with("translation_") && mode != "first_visible_line" {
        selected = select_first_visible_text(&payload);
    }
    let normalized = normalize_text(&selected);
    if normalized.is_empty() {
        return Ok(());
    }
    let signature = (payload.sequence, mode.to_string(), normalized.clone());
    if settings.avoid_duplicate_text {
        let last = inner
            .last_payload_signature
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if last.as_ref() == Some(&signature) {
            inner.log.send_skipped(
                "payload_dedup",
                json!({
                    "sequence": payload.sequence,
                    "output_mode": mode,
                }),
            );
            return Ok(());
        }
    }
    inner.log.payload_routed(
        payload.sequence,
        mode,
        normalized.chars().count(),
    );
    *inner
        .last_payload_signature
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = Some(signature);
    schedule_final_send(inner, normalized, send_stream, mirror_debug).await?;
    Ok(())
}

async fn send_text(
    inner: Arc<Inner>,
    text: &str,
    settings: &ObsCaptionSettings,
    send_stream_caption: bool,
    mirror_debug_text: bool,
    force: bool,
    schedule_clear_after: bool,
) -> Result<(), String> {
    let normalized = normalize_text(text);
    if !send_stream_caption && !mirror_debug_text {
        return Ok(());
    }

    let had_active = inner.client.lock().await.is_some();
    if !had_active && !wait_for_connection(&inner, settings, 3).await {
        {
            let mut diag = inner.diagnostics.lock().await;
            diag.last_send_used_active_connection = false;
            diag.last_send_waited_for_connection = true;
            if diag.last_error.is_none() {
                diag.last_error = Some(error_codes::error::NOT_CONNECTED.into());
            }
        }
        inner.log.send_skipped(
            "not_connected",
            json!({ "text_len": normalized.chars().count() }),
        );
        return Ok(());
    }

    let used_active_connection = had_active;
    let waited_for_connection = !had_active;
    {
        let mut diag = inner.diagnostics.lock().await;
        diag.last_send_used_active_connection = used_active_connection;
        diag.last_send_waited_for_connection = waited_for_connection;
    }

    let mut client_guard = inner.client.lock().await;
    let Some(client) = client_guard.as_mut() else {
        return Ok(());
    };

    let mut should_send_caption = send_stream_caption;
    let mut should_send_debug =
        mirror_debug_text && !settings.debug_input_name.trim().is_empty();

    if settings.avoid_duplicate_text && !force {
        let diag = inner.diagnostics.lock().await;
        if should_send_caption && diag.last_caption_text.as_deref() == Some(normalized.as_str()) {
            should_send_caption = false;
        }
        if should_send_debug
            && diag.last_debug_text.as_deref() == Some(normalized.as_str())
            && diag.last_debug_input_name.as_deref() == Some(settings.debug_input_name.as_str())
        {
            should_send_debug = false;
        }
    }

    if !should_send_caption && !should_send_debug {
        inner.log.send_skipped(
            "dedup",
            json!({
                "text_len": normalized.chars().count(),
                "send_stream": send_stream_caption,
                "mirror_debug": mirror_debug_text,
            }),
        );
        return Ok(());
    }

    if should_send_debug {
        client
            .send_request(
                "SetInputSettings",
                json!({
                    "inputName": settings.debug_input_name,
                    "inputSettings": { "text": normalized },
                    "overlay": true
                }),
            )
            .await
            .map_err(|err| format!("OBS debug mirror failed: {err}"))?;
        let mut diag = inner.diagnostics.lock().await;
        diag.last_debug_text = Some(normalized.clone());
        diag.last_debug_input_name = Some(settings.debug_input_name.clone());
        if !should_send_caption {
            inner.log.debug_mirror_sent(normalized.chars().count());
        }
    }

    if should_send_caption {
        match client
            .send_request("SendStreamCaption", json!({ "captionText": normalized }))
            .await
        {
            Ok(_) => {
                {
                    let mut diag = inner.diagnostics.lock().await;
                    diag.last_caption_text = Some(normalized.clone());
                    diag.last_caption_sent_at_utc = Some(utc_now_iso());
                    diag.stream_output_active = Some(true);
                    diag.stream_output_reconnecting = Some(false);
                    diag.native_caption_status = Some(native_status::STREAM_DELIVERED.into());
                    diag.last_error = None;
                }
                inner.log.caption_sent(
                    normalized.chars().count(),
                    true,
                    should_send_debug,
                    used_active_connection,
                    waited_for_connection,
                );
                set_connection_state(&inner, ConnectionState::Connected, None).await;
            }
            Err(ObsClientError::RequestFailed {
                code: Some(501), ..
            }) => {
                let mut diag = inner.diagnostics.lock().await;
                diag.stream_output_active = Some(false);
                diag.stream_output_reconnecting = Some(false);
                diag.native_caption_status = Some(native_status::STREAM_NOT_RUNNING.into());
                diag.last_error = Some(error_codes::error::STREAM_NOT_RUNNING.into());
                drop(diag);
                inner.log.stream_output_inactive();
                set_connection_state(&inner, ConnectionState::Connected, None).await;
                drop(client_guard);
                if mirror_debug_text
                    && schedule_clear_after
                    && !normalized.is_empty()
                {
                    enqueue_clear(
                        inner,
                        false,
                        true,
                        settings.clear_after_ms,
                    );
                }
                return Ok(());
            }
            Err(err) => {
                let code = error_codes::error::SEND_FAILED;
                let detail = err.to_string();
                {
                    let mut diag = inner.diagnostics.lock().await;
                    diag.last_error = Some(code.into());
                }
                inner.log.caption_send_failed(&detail);
                set_connection_state(&inner, ConnectionState::Error, Some(code)).await;
                drop(client_guard);
                if let Some(client) = inner.client.lock().await.take() {
                    client.close().await;
                }
                ensure_connection_task(inner.clone()).await;
                return Err(code.into());
            }
        }
    }

    drop(client_guard);
    if schedule_clear_after && !normalized.is_empty() {
        enqueue_clear(
            inner,
            should_send_caption,
            should_send_debug,
            settings.clear_after_ms,
        );
    }

    Ok(())
}

fn bump_delayed_generation(inner: &Inner) {
    inner.delayed_generation.fetch_add(1, Ordering::SeqCst);
}

fn bump_clear_generation(inner: &Inner) {
    inner.clear_generation.fetch_add(1, Ordering::SeqCst);
}

fn payload_will_supersede_caption(
    inner: &Inner,
    settings: &ObsCaptionSettings,
    payload: &SubtitlePayloadEvent,
) -> bool {
    let mode = settings.output_mode.as_str();
    let send_stream = settings.native_enabled()
        && !matches!(mode, "disabled" | "source_live" | "source_final_only");
    let mirror_debug = settings.debug_text_input_enabled();
    if !send_stream && !mirror_debug {
        return false;
    }
    if !payload.completed_block_visible {
        return false;
    }
    let mut selected = select_payload_text(payload, mode);
    if send_stream && !mode.starts_with("translation_") && mode != "first_visible_line" {
        selected = select_first_visible_text(payload);
    }
    let normalized = normalize_text(&selected);
    if normalized.is_empty() {
        return false;
    }
    if settings.avoid_duplicate_text {
        let signature = (payload.sequence, mode.to_string(), normalized);
        let last = inner
            .last_payload_signature
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if last.as_ref() == Some(&signature) {
            return false;
        }
    }
    true
}

async fn schedule_final_send(
    inner: Arc<Inner>,
    text: String,
    send_stream: bool,
    mirror_debug: bool,
) -> Result<(), String> {
    let settings = ObsCaptionSettings::from_config(&(inner.config_getter)());
    let delay_ms = settings.final_replace_delay_ms;
    if delay_ms == 0 {
        return send_text(
            inner,
            &text,
            &settings,
            send_stream,
            mirror_debug,
            false,
            true,
        )
        .await;
    }
    let generation = inner.delayed_generation.load(Ordering::SeqCst);
    enqueue_item(
        &inner,
        QueueItem::DelayedSend {
            text,
            send_stream,
            mirror_debug,
            delay_ms,
            generation,
        },
    );
    Ok(())
}

fn enqueue_clear(inner: Arc<Inner>, send_stream: bool, mirror_debug: bool, delay_ms: u64) {
    if delay_ms == 0 {
        return;
    }
    bump_clear_generation(&inner);
    let generation = inner.clear_generation.load(Ordering::SeqCst);
    enqueue_item(
        &inner,
        QueueItem::DelayedClear {
            send_stream,
            mirror_debug,
            delay_ms,
            generation,
        },
    );
}

fn enqueue_item(inner: &Inner, item: QueueItem) {
    let worker_running = inner
        .worker_task
        .try_lock()
        .ok()
        .and_then(|worker| worker.as_ref().map(|task| !task.is_finished()))
        .unwrap_or(false);
    if !worker_running {
        return;
    }
    {
        let mut queue = inner.queue.lock().expect("obs queue lock");
        if queue.len() >= QUEUE_MAX_SIZE {
            queue.pop_front();
        }
        queue.push_back(item);
    }
    inner.queue_notify.notify_one();
}

fn drain_queue(inner: &Inner) {
    inner.queue.lock().expect("obs queue lock").clear();
}

fn drop_queued_partials(inner: &Inner) {
    let mut queue = inner.queue.lock().expect("obs queue lock");
    queue.retain(|item| !matches!(item, QueueItem::SourcePartial(_)));
}

async fn wait_for_connection(
    inner: &Arc<Inner>,
    settings: &ObsCaptionSettings,
    timeout_secs: u64,
) -> bool {
    if inner.client.lock().await.is_some() {
        return true;
    }
    if !settings.should_connect() {
        return false;
    }
    *inner.desired_connection.lock().await = true;
    ensure_connection_task(inner.clone()).await;
    let notified = inner.connected_notify.notified();
    tokio::pin!(notified);
    tokio::time::timeout(Duration::from_secs(timeout_secs), &mut notified)
        .await
        .is_ok()
        && inner.client.lock().await.is_some()
}

async fn set_connection_state(
    inner: &Inner,
    state: ConnectionState,
    error: Option<&str>,
) {
    let prev = {
        let mut diag = inner.diagnostics.lock().await;
        let prev = diag.connection_state;
        diag.connection_state = state;
        prev
    };
    if prev != state {
        inner.log.connection_state_changed(state, error);
    }
}

fn utc_now_iso() -> String {
    Utc::now().to_rfc3339()
}
