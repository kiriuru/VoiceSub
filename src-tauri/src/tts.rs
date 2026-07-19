use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use tracing::{debug, info, warn};

use voicesub_audio::{CHANNEL_SPEECH, CHANNEL_TWITCH, PlaybackHub};
use voicesub_tts::{
    ChannelEnqueueResult, TTS_WINDOW_LABEL, TtsConfig, TtsModuleService, TtsSpeechPipeline,
    TtsSpeechSettings, bind_window_process, build_tts_module_url, tts_webview_data_dir,
    validate_twitch_oauth_url,
};
use voicesub_twitch::TwitchConnectionStatus;
use voicesub_twitch::TwitchTtsSettings;

use crate::webview_memory::{self, SharedWebviewMemoryManager};

/// Stop native playback and unblock Rust speech queues after the TTS WebView closes mid-utterance.
pub fn recover_tts_after_window_closed(state: &TtsState) {
    info!(
        target: "voicesub.tts",
        "recovering speech queues after TTS module window closed"
    );
    state.pipeline.set_window_open(false);
    let _ = state.playback.stop_channel(CHANNEL_SPEECH);
    let _ = state.playback.stop_channel(CHANNEL_TWITCH);
    state.service.queue_force_idle_all();
}

fn tts_module_should_stay_open(service: &TtsModuleService) -> bool {
    service
        .load_config()
        .map(|cfg| cfg.enabled)
        .unwrap_or(false)
}

/// Re-open the TTS module window after an unexpected close while the module stays enabled.
pub fn schedule_tts_window_reopen(app: AppHandle, state: &TtsState) {
    if !tts_module_should_stay_open(&state.service) {
        return;
    }
    if app.get_webview_window(TTS_WINDOW_LABEL).is_some() {
        return;
    }
    info!(
        target: "voicesub.tts.ipc",
        "scheduling tts module window reopen after unexpected close"
    );
    let state = TtsState {
        service: state.service.clone(),
        bind_addr: state.bind_addr,
        playback: state.playback.clone(),
        pipeline: state.pipeline.clone(),
    };
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if app.get_webview_window(TTS_WINDOW_LABEL).is_some() {
            return;
        }
        if !tts_module_should_stay_open(&state.service) {
            return;
        }
        let memory = app.state::<SharedWebviewMemoryManager>();
        if let Err(err) = open_tts_window(app.clone(), state, memory.inner()).await {
            warn!(
                target: "voicesub.tts.ipc",
                error = %err,
                "tts module window reopen failed"
            );
        }
    });
}

const TTS_HEALTH_POLL_SECS: u64 = 45;

/// Rust-side watchdog: reopen or recycle the TTS WebView when the module is enabled but the
/// frontend heartbeat goes stale (WS/renderer wedged while the shell process stays alive).
pub fn start_tts_health_watchdog(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(TTS_HEALTH_POLL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let memory = app.state::<SharedWebviewMemoryManager>();
            let should_watch = memory
                .lock()
                .map(|guard| guard.tts_should_be_active())
                .unwrap_or(false);
            if !should_watch {
                continue;
            }
            if !tts_module_should_stay_open(&app.state::<TtsState>().service) {
                continue;
            }

            let window_missing = app.get_webview_window(TTS_WINDOW_LABEL).is_none();
            let heartbeat_stale = memory
                .lock()
                .map(|guard| guard.tts_heartbeat_stale())
                .unwrap_or(true);

            if window_missing {
                let user_dismissed = memory
                    .lock()
                    .map(|guard| guard.user_dismissed_tts_window())
                    .unwrap_or(false);
                if user_dismissed {
                    continue;
                }
                let state = app.state::<TtsState>().inner().clone();
                schedule_tts_window_reopen(app.clone(), &state);
                continue;
            }

            if !heartbeat_stale {
                continue;
            }

            warn!(
                target: "voicesub.tts.ipc",
                "tts webview heartbeat stale; recycling module window"
            );
            let state = app.state::<TtsState>().inner().clone();
            recover_tts_after_window_closed(&state);
            if let Some(window) = app.get_webview_window(TTS_WINDOW_LABEL) {
                let _ = window.close();
            }
            if let Ok(mut guard) = memory.lock() {
                guard.set_tts_visible(false);
                guard.set_tts_focused(false);
            }
            webview_memory::refresh_main_shell_only(&app, memory.inner());
            schedule_tts_window_reopen(app.clone(), &state);
        }
    });
}

/// Close the module window when the desktop shell shuts down (same lifecycle as browser worker).
pub fn close_tts_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(TTS_WINDOW_LABEL) else {
        return;
    };
    info!(
        target: "voicesub.tts.ipc",
        "closing tts module window on app shutdown"
    );
    let _ = window.destroy();
}

#[derive(Clone)]
pub struct TtsState {
    pub service: Arc<TtsModuleService>,
    pub bind_addr: std::net::SocketAddr,
    pub playback: Arc<PlaybackHub>,
    pub pipeline: Arc<TtsSpeechPipeline>,
}

pub fn sync_playback_devices(state: &TtsState) {
    let Ok(config) = state.service.load_config() else {
        return;
    };
    let speech_label = TtsModuleService::device_label_for_channel(&config, CHANNEL_SPEECH);
    let twitch_label = TtsModuleService::device_label_for_channel(&config, CHANNEL_TWITCH);
    let _ = state
        .playback
        .set_device_label(CHANNEL_SPEECH, speech_label);
    let _ = state
        .playback
        .set_device_label(CHANNEL_TWITCH, twitch_label);
    state.pipeline.sync_enabled_from_config();
}

#[tauri::command]

pub fn tts_get_config(state: State<'_, TtsState>) -> Result<TtsConfig, String> {
    debug!(target: "voicesub.tts.ipc", "tts_get_config");

    state.service.load_config().map_err(|e| e.to_string())
}

#[tauri::command]

pub fn tts_set_provider(state: State<'_, TtsState>, provider: String) -> Result<TtsConfig, String> {
    info!(target: "voicesub.tts.ipc", provider = %provider, "tts_set_provider");
    state
        .service
        .set_tts_provider(&provider)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tts_set_enabled(
    app: AppHandle,
    state: State<'_, TtsState>,
    memory: State<'_, SharedWebviewMemoryManager>,
    enabled: bool,
) -> Result<TtsConfig, String> {
    info!(target: "voicesub.tts.ipc", enabled, "tts_set_enabled");

    let config = state
        .service
        .set_enabled(enabled)
        .map_err(|e| e.to_string())?;
    state.pipeline.sync_enabled_from_config();
    if let Ok(mut guard) = memory.lock() {
        guard.set_tts_enabled(enabled);
    }
    webview_memory::refresh_from_state(&app, memory.inner());
    Ok(config)
}

#[tauri::command]

pub fn tts_set_audio_device(
    state: State<'_, TtsState>,
    device_id: String,
    device_label: Option<String>,
) -> Result<TtsConfig, String> {
    info!(
        target: "voicesub.tts.ipc",
        device_id = if device_id.is_empty() { "default" } else { device_id.as_str() },
        device_label = device_label.as_deref().unwrap_or(""),
        "tts_set_audio_device"
    );

    if !state.service.validate_device_id(&device_id) {
        warn!(target: "voicesub.tts.ipc", device_id = %device_id, "unknown audio device rejected");
        return Err(format!("unknown audio device: {device_id}"));
    }

    let config = state
        .service
        .set_audio_device(&device_id, device_label.as_deref())
        .map_err(|e| e.to_string())?;
    let label = TtsModuleService::device_label_for_channel(&config, CHANNEL_SPEECH);
    state
        .playback
        .set_device_label(CHANNEL_SPEECH, label)
        .map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn tts_set_channel_audio_device(
    state: State<'_, TtsState>,
    channel: String,
    device_id: String,
    device_label: Option<String>,
) -> Result<TtsConfig, String> {
    info!(
        target: "voicesub.tts.ipc",
        channel = %channel,
        device_id = if device_id.is_empty() { "default" } else { device_id.as_str() },
        device_label = device_label.as_deref().unwrap_or(""),
        "tts_set_channel_audio_device"
    );
    if !state.service.validate_device_id(&device_id) {
        warn!(
            target: "voicesub.tts.ipc",
            channel = %channel,
            device_id = %device_id,
            "unknown audio device rejected"
        );
        return Err(format!("unknown audio device: {device_id}"));
    }

    let config = state
        .service
        .set_channel_audio_device(&channel, &device_id, device_label.as_deref())
        .map_err(|e| e.to_string())?;
    let label = TtsModuleService::device_label_for_channel(&config, channel.as_str());
    state
        .playback
        .set_device_label(channel.as_str(), label)
        .map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn tts_set_playback_mode(
    state: State<'_, TtsState>,
    mode: String,
) -> Result<TtsConfig, String> {
    info!(target: "voicesub.tts.ipc", mode = %mode, "tts_set_playback_mode");
    let config = state
        .service
        .set_playback_mode(&mode)
        .map_err(|e| e.to_string())?;
    sync_playback_devices(&state);
    Ok(config)
}

#[tauri::command]

pub fn tts_list_output_devices() -> Result<Vec<voicesub_audio::AudioOutputDevice>, String> {
    debug!(target: "voicesub.tts.ipc", "tts_list_output_devices");

    voicesub_audio::list_output_devices_on_thread().map_err(|e| e.to_string())
}

#[tauri::command]

pub fn tts_get_audio_routing() -> Result<String, String> {
    Ok(if voicesub_audio::is_per_process_routing_enabled() {
        "winapi".to_string()
    } else {
        "browser".to_string()
    })
}

#[tauri::command]

pub fn tts_bind_window_audio(
    app: AppHandle,

    state: State<'_, TtsState>,
) -> Result<TtsConfig, String> {
    let window = app
        .get_webview_window(TTS_WINDOW_LABEL)
        .ok_or_else(|| "TTS window is not open".to_string())?;

    bind_tts_window_audio(&state.service, &window)
}

#[tauri::command]

pub fn tts_update_speech_settings(
    state: State<'_, TtsState>,

    speech: TtsSpeechSettings,
) -> Result<TtsConfig, String> {
    info!(target: "voicesub.tts.ipc", "tts_update_speech_settings");

    state
        .service
        .update_speech_settings(speech)
        .map_err(|e| e.to_string())
}

#[tauri::command]

pub fn tts_update_voice_settings(
    state: State<'_, TtsState>,
    speech_rate: f32,
    speech_volume: f32,
) -> Result<TtsConfig, String> {
    info!(
        target: "voicesub.tts.ipc",
        speech_rate,
        speech_volume,
        "tts_update_voice_settings"
    );
    state
        .service
        .update_voice_settings(speech_rate, speech_volume)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tts_reset_subtitle_planner(state: State<'_, TtsState>) -> Result<(), String> {
    info!(target: "voicesub.tts.ipc", "tts_reset_subtitle_planner");

    state.service.reset_subtitle_planner();

    Ok(())
}

#[tauri::command]
pub fn tts_speak_sample(
    state: State<'_, TtsState>,
    text: String,
    lang: String,
) -> Result<ChannelEnqueueResult, String> {
    info!(
        target: "voicesub.tts.ipc",
        text_len = text.chars().count(),
        lang = %lang,
        "tts_speak_sample"
    );
    state
        .pipeline
        .enqueue_speech_test(text, lang)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tts_channel_clear(state: State<'_, TtsState>, channel: String) -> Result<(), String> {
    info!(target: "voicesub.tts.ipc", channel = %channel, "tts_channel_clear");
    state
        .service
        .queue_clear_channel(&channel)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tts_get_resource_telemetry() -> voicesub_audio::ResourceTelemetry {
    voicesub_audio::collect_resource_telemetry()
}

#[tauri::command]
pub fn tts_channel_force_idle(state: State<'_, TtsState>, channel: String) -> Result<(), String> {
    info!(target: "voicesub.tts.ipc", channel = %channel, "tts_channel_force_idle");
    state
        .service
        .queue_force_idle(&channel)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn tts_twitch_get_status(state: State<'_, TtsState>) -> Result<TwitchConnectionStatus, String> {
    let status = state.service.twitch_status();
    debug!(
        target: "voicesub.tts.ipc",
        state = ?status.state,
        channel = %status.channel,
        "tts_twitch_get_status"
    );
    Ok(status)
}

#[tauri::command]
pub fn tts_twitch_connect(state: State<'_, TtsState>) -> Result<TwitchConnectionStatus, String> {
    info!(target: "voicesub.tts.ipc", "tts_twitch_connect");
    state.service.twitch_connect().map_err(|e| {
        warn!(target: "voicesub.tts.ipc", error = %e, "tts_twitch_connect failed");
        e.to_string()
    })
}

#[tauri::command]
pub fn tts_twitch_disconnect(state: State<'_, TtsState>) -> Result<(), String> {
    info!(target: "voicesub.tts.ipc", "tts_twitch_disconnect");
    state.service.twitch_disconnect();
    Ok(())
}

#[tauri::command]
pub fn tts_update_twitch_settings(
    state: State<'_, TtsState>,
    twitch: TwitchTtsSettings,
) -> Result<TtsConfig, String> {
    info!(
        target: "voicesub.tts.ipc",
        channel = %twitch.normalized_channel(),
        enabled = twitch.enabled,
        "tts_update_twitch_settings"
    );
    let config = state.service.update_twitch_settings(twitch).map_err(|e| {
        warn!(target: "voicesub.tts.ipc", error = %e, "tts_update_twitch_settings failed");
        e.to_string()
    })?;
    let label = TtsModuleService::device_label_for_channel(&config, CHANNEL_TWITCH);
    state
        .playback
        .set_device_label(CHANNEL_TWITCH, label)
        .map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
pub fn tts_report_webview_activity(
    app: AppHandle,
    memory: State<'_, SharedWebviewMemoryManager>,
    runtime_active: bool,
    tts_enabled: bool,
    engines_busy: bool,
) -> Result<(), String> {
    debug!(
        target: "voicesub.tts.ipc",
        runtime_active,
        tts_enabled,
        engines_busy,
        "tts_report_webview_activity"
    );
    if let Ok(mut guard) = memory.lock() {
        guard.set_tts_activity(runtime_active, tts_enabled, engines_busy);
    } else {
        return Ok(());
    }
    webview_memory::sync_tts_window_visibility(&app, memory.inner());
    webview_memory::refresh_from_state(&app, memory.inner());
    Ok(())
}

#[tauri::command]
pub fn tts_open_system_url(url: String) -> Result<(), String> {
    validate_twitch_oauth_url(&url)?;
    let trimmed = url.trim();
    info!(target: "voicesub.tts.ipc", url = %trimmed, "opening twitch oauth in system browser");
    open::that(trimmed).map_err(|err| err.to_string())
}

#[tauri::command]
// Must stay async on Windows: sync commands that call WebviewWindowBuilder::build
// deadlock the UI thread (WebView2). See Tauri docs / wry#583.
#[allow(clippy::unused_async)]
pub async fn tts_open_window(
    app: AppHandle,
    state: State<'_, TtsState>,
    memory: State<'_, SharedWebviewMemoryManager>,
) -> Result<(), String> {
    open_tts_window(app, state.inner().clone(), memory.inner()).await
}

#[allow(clippy::unused_async)]
async fn open_tts_window(
    app: AppHandle,
    state: TtsState,
    memory: &SharedWebviewMemoryManager,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TTS_WINDOW_LABEL) {
        info!(target: "voicesub.tts.ipc", "tts window focus existing");

        let _ = window.show();

        let _ = window.set_focus();

        if voicesub_audio::is_per_process_routing_enabled() {
            let _ = bind_tts_window_audio(&state.service, &window);
        }

        if let Ok(mut guard) = memory.lock() {
            guard.set_user_dismissed_tts_window(false);
            guard.set_tts_visible(true);
            guard.set_tts_focused(true);
        }
        state.pipeline.set_window_open(true);
        webview_memory::refresh_from_state(&app, memory);

        return Ok(());
    }

    recover_tts_after_window_closed(&state);

    let url = build_tts_module_url(state.bind_addr);

    info!(target: "voicesub.tts.ipc", url = %url, "creating tts window");

    let parsed = url.parse::<url::Url>().map_err(|e| e.to_string())?;

    let data_dir: PathBuf = tts_webview_data_dir(state.service.config_path());
    let _ = fs::create_dir_all(&data_dir);

    let window = WebviewWindowBuilder::new(&app, TTS_WINDOW_LABEL, WebviewUrl::External(parsed))
        .title("VoiceSub TTS")
        .inner_size(720.0, 560.0)
        .min_inner_size(480.0, 420.0)
        .resizable(true)
        .data_directory(data_dir)
        .build()
        .map_err(|e| e.to_string())?;

    if voicesub_audio::is_per_process_routing_enabled() {
        let _ = bind_tts_window_audio(&state.service, &window);
    }

    if let Ok(mut guard) = memory.lock() {
        guard.set_user_dismissed_tts_window(false);
        guard.set_tts_visible(true);
        guard.set_tts_focused(true);
    }
    state.pipeline.set_window_open(true);
    webview_memory::refresh_from_state(&app, memory);

    info!(target: "voicesub.tts.ipc", "tts module window opened");

    Ok(())
}

fn bind_tts_window_audio(
    service: &TtsModuleService,
    window: &WebviewWindow,
) -> Result<TtsConfig, String> {
    let pid = window_process_id(window).ok_or_else(|| {
        warn!(target: "voicesub.tts.ipc", "unable to resolve TTS window process id");
        "unable to resolve TTS window process id".to_string()
    })?;
    debug!(target: "voicesub.tts.ipc", pid, "binding tts window audio route");
    bind_window_process(service, pid)
}

fn window_process_id(window: &WebviewWindow) -> Option<u32> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let hwnd = window.hwnd().ok()?;

        let mut pid = 0u32;

        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&raw mut pid));
        }

        if pid == 0 { None } else { Some(pid) }
    }

    #[cfg(not(windows))]
    {
        let _ = window;

        None
    }
}
