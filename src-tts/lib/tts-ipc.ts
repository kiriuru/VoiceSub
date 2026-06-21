import { invoke } from "@tauri-apps/api/core";
import type { SpeechChannel } from "./audio-player";
import { apiFetch } from "./loopback-api-client";
import type { ResourceTelemetry } from "./resource-telemetry";
import type {
  AudioOutputDevice,
  PythonTtsStatus,
  SpeechQueueItem,
  TtsConfig,
  TtsPlaybackMode,
  TtsProvider,
  TtsSpeechSettings,
  TwitchConnectionStatus,
  TwitchTtsSettings,
} from "./types";

export async function loadTtsConfig(): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_get_config");
}

export async function setTtsProvider(provider: TtsProvider): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_set_provider", { provider });
}

export async function fetchPythonTtsStatus(): Promise<PythonTtsStatus> {
  const response = await apiFetch("/api/tts/python/status");
  if (!response.ok) {
    throw new Error(`python status HTTP ${response.status}`);
  }
  return (await response.json()) as PythonTtsStatus;
}

export async function setTtsEnabled(enabled: boolean): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_set_enabled", { enabled });
}

export async function setTtsAudioDevice(
  deviceId: string,
  deviceLabel?: string,
): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_set_audio_device", { deviceId, deviceLabel });
}

export async function setTtsChannelAudioDevice(
  channel: "speech" | "twitch",
  deviceId: string,
  deviceLabel?: string,
): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_set_channel_audio_device", {
    channel,
    deviceId,
    deviceLabel,
  });
}

export async function setTtsPlaybackMode(
  mode: TtsPlaybackMode,
): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_set_playback_mode", { mode });
}

export async function updateVoiceSettings(
  speechRate: number,
  speechVolume: number,
): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_update_voice_settings", {
    speechRate,
    speechVolume,
  });
}

export async function listRustOutputDevices(): Promise<AudioOutputDevice[]> {
  return invoke<AudioOutputDevice[]>("tts_list_output_devices");
}

export type TtsAudioRoutingMode = "browser" | "winapi";

export async function fetchAudioRoutingMode(): Promise<TtsAudioRoutingMode> {
  return invoke<TtsAudioRoutingMode>("tts_get_audio_routing");
}

/** Apply saved output route to the TTS WebView process (WinAPI mode). */
export async function bindTtsWindowAudio(): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_bind_window_audio");
}

export async function updateSpeechSettings(speech: TtsSpeechSettings): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_update_speech_settings", { speech });
}

export async function resetSubtitlePlanner(): Promise<void> {
  return invoke("tts_reset_subtitle_planner");
}

export type ChannelEnqueueResult = {
  queue_len: number;
  dropped_ids: string[];
};

export async function speakSample(text: string, lang: string): Promise<ChannelEnqueueResult> {
  const result = await invoke<ChannelEnqueueResult>("tts_speak_sample", { text, lang });
  return {
    queue_len: result.queue_len,
    dropped_ids: result.dropped_ids ?? [],
  };
}

export type { SpeechChannel };

export async function channelClear(channel: SpeechChannel): Promise<void> {
  return invoke("tts_channel_clear", { channel });
}

export async function channelForceIdle(channel: SpeechChannel): Promise<void> {
  return invoke("tts_channel_force_idle", { channel });
}

/** Unblock Rust queues left in `Speaking` after an abrupt TTS window close. */
export async function recoverStuckSpeechQueues(): Promise<void> {
  await Promise.all([channelForceIdle("speech"), channelForceIdle("twitch")]);
}

export async function fetchResourceTelemetry(): Promise<ResourceTelemetry> {
  return invoke<ResourceTelemetry>("tts_get_resource_telemetry");
}

export async function fetchTwitchStatus(): Promise<TwitchConnectionStatus> {
  return invoke<TwitchConnectionStatus>("tts_twitch_get_status");
}

export async function connectTwitchChat(): Promise<TwitchConnectionStatus> {
  return invoke<TwitchConnectionStatus>("tts_twitch_connect");
}

export async function disconnectTwitchChat(): Promise<void> {
  return invoke("tts_twitch_disconnect");
}

export async function updateTwitchSettings(
  twitch: TwitchTtsSettings,
): Promise<TtsConfig> {
  return invoke<TtsConfig>("tts_update_twitch_settings", { twitch });
}

export async function reportTtsWebviewActivity(
  runtimeActive: boolean,
  ttsEnabled: boolean,
  enginesBusy: boolean,
): Promise<void> {
  return invoke("tts_report_webview_activity", {
    runtimeActive,
    ttsEnabled,
    enginesBusy,
  });
}
