export type TtsTab = "speech" | "twitch";

/** `browser_google` = Rust HTTP proxy; `python_stdlib` = urllib script in bin/modules/tts. */
export type TtsProvider = "browser_google" | "python_stdlib";

export interface PythonTtsStatus {
  available: boolean;
  kind: string;
  command: string;
  version: string;
  script_found: boolean;
  embedded_found: boolean;
  script_path: string;
  embedded_path: string;
  runtime_dir: string;
  build_hint: string;
}

export type WsConnectionStatus = "connecting" | "connected" | "disconnected";

export interface TtsSpeechSettings {
  speak_source: boolean;
  speak_translations: boolean;
  /** Empty = all active translation lines when speak_translations is true. */
  translation_slots?: string[];
  min_chars: number;
  max_queue_items: number;
}

export type TwitchConnectionState =
  | "disconnected"
  | "connecting"
  | "connected"
  | "error";

export type TwitchPauseStyle = "comma" | "period" | "dash" | "ellipsis";

export interface TwitchReplacement {
  from: string;
  to: string;
}

export interface TwitchEmoteSources {
  twitch: boolean;
  bttv: boolean;
  seventv: boolean;
}

export type TtsPlaybackMode = "native" | "sonic";

export interface TwitchTtsSettings {
  enabled: boolean;
  /** Legacy first-channel field; kept in sync with `channels[0]`. */
  channel: string;
  /** Up to 5 channel logins (without `#`). */
  channels?: string[];
  nick: string;
  oauth_token: string;
  /** Twitch app Client ID for built-in OAuth (implicit grant, localhost redirect). */
  oauth_client_id?: string;
  speak_chat: boolean;
  include_username: boolean;
  /** Fallback TTS language when auto-detect is off. */
  language: string;
  min_chars: number;
  max_chars: number;
  block_commands: boolean;
  ignore_users: string[];
  /** Symbol tokens not spoken in chat TTS. Empty = read all symbols. */
  strip_symbols?: string[];
  strip_emotes?: boolean;
  strip_emoji?: boolean;
  strip_links?: boolean;
  emote_sources?: TwitchEmoteSources;
  detect_language?: boolean;
  lang_min_chars?: number;
  /** ISO 639-1 codes; empty = all languages. */
  enabled_languages?: string[];
  nick_replacements?: TwitchReplacement[];
  /** Builtin profanity filter for Twitch chat (separate from main-app ASR). */
  include_builtin_profanity?: boolean;
  pause_style?: TwitchPauseStyle;
  /** `{nick}` and `{text}` placeholders. */
  speak_template?: string;
  audio_output_device_id?: string;
  audio_output_device_label?: string;
  /** > 0 overrides root speech_rate */
  speech_rate?: number;
  /** >= 0 overrides root speech_volume */
  speech_volume?: number;
  max_queue_items?: number;
}

export interface TwitchConnectionStatus {
  state: TwitchConnectionState;
  /** Comma-separated `#channel` labels. */
  channel: string;
  channels?: string[];
  message: string;
}

export interface TwitchChatMessage {
  id: string;
  user: string;
  display_name: string;
  text: string;
  speak_text: string;
  clean_text?: string;
  spoken_nick?: string;
  channel: string;
  language: string;
  is_mod: boolean;
  is_subscriber: boolean;
  speakable?: boolean;
}

export interface TtsConfig {
  enabled: boolean;
  tts_provider?: TtsProvider;
  playback_mode?: TtsPlaybackMode;
  audio_output_device_id: string;
  audio_output_device_label?: string;
  speech_rate: number;
  speech_volume: number;
  speech: TtsSpeechSettings;
  twitch?: TwitchTtsSettings;
}

export interface AudioOutputDevice {
  id: string;
  label: string;
  is_default?: boolean;
}

export interface SpeechQueueItem {
  id: string;
  text: string;
  source: string;
  lang?: string;
}

export interface RuntimeStatus {
  running?: boolean;
  is_running?: boolean;
  phase?: string;
  status?: string;
}

export interface WsMessage {
  type: string;
  payload?: Record<string, unknown>;
}
