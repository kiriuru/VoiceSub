<script lang="ts">
  import {
    connectTwitchChat,
    disconnectTwitchChat,
    fetchTwitchStatus,
    setTtsChannelAudioDevice,
    updateTwitchSettings,
  } from "../lib/tts-ipc";
  import { isNativePlaybackMode } from "../lib/audio-player";
  import type { AudioOutputDevice, TtsPlaybackMode } from "../lib/types";
  import { twitchOAuthRedirectUri } from "../lib/twitch-oauth";
  import {
    fetchPendingOAuthToken,
    openTwitchOAuthInSystemBrowser,
  } from "../lib/twitch-oauth-flow";
  import { defaultTwitchSettings } from "../lib/twitch-defaults";
  import {
    TWITCH_MAX_CHANNELS,
    channelsFromRows,
    resolveChannelRows,
  } from "../lib/twitch-channels";
  import ReplacementPairEditor from "./ReplacementPairEditor.svelte";
  import { locale, t, getLocale } from "../../src/lib/i18n";
  import type { LocaleCode } from "../../src/lib/types";
  import { formatSpeechVolume, formatPlaybackRate } from "../lib/playback-format";
  import {
    prependTwitchChatLog,
    type TwitchChatLogEntry,
  } from "../lib/twitch-chat-log";
  import { ttsTrace, ttsTraceText } from "../lib/tts-trace";
  import { clampPopoverPosition, rectFromElement } from "../lib/popover-position";
  import { tick } from "svelte";
  import type {
    TwitchPauseStyle,
    TwitchChatMessage,
    TwitchConnectionStatus,
    TwitchReplacement,
    TwitchTtsSettings,
  } from "../lib/types";

  interface Props {
    twitch: TwitchTtsSettings;
    moduleEnabled: boolean;
    moduleSpeechRate?: number;
    moduleSpeechVolume?: number;
    playbackMode?: TtsPlaybackMode;
    audioOutputs?: AudioOutputDevice[];
    onTwitchConfigSaved?: (twitch: TwitchTtsSettings) => void;
  }

  let {
    twitch = $bindable(),
    moduleEnabled,
    moduleSpeechRate = 1,
    moduleSpeechVolume = 1,
    playbackMode = "native",
    audioOutputs = [],
    onTwitchConfigSaved,
  }: Props = $props();

  const twitchRateOverride = $derived((twitch.speech_rate ?? 0) > 0);
  const twitchVolumeOverride = $derived(
    typeof twitch.speech_volume === "number" && twitch.speech_volume >= 0,
  );

  const nativePlayback = $derived(isNativePlaybackMode(playbackMode));

  let status = $state<TwitchConnectionStatus>({
    state: "disconnected",
    channel: "",
    channels: [],
    message: "",
  });
  let error = $state("");
  let busy = $state(false);
  let settingsSaved = $state(false);
  let settingsSaving = $state(false);
  let chatLog = $state<TwitchChatLogEntry[]>([]);
  let chatLogSeq = 0;
  let ignoreUsersText = $state("");
  let stripSymbolsText = $state("");
  let enabledLanguagesText = $state("");
  let ignoreUsersDirty = $state(false);
  let stripSymbolsDirty = $state(false);
  let enabledLanguagesDirty = $state(false);
  let ignoreUsersInput: HTMLInputElement | null = null;
  let stripSymbolsInput: HTMLInputElement | null = null;
  let channelRows = $state<string[]>([""]);
  let channelRowsDirty = $state(false);

  const nickPairs = $derived(twitch.nick_replacements ?? []);

  let currentLocale = $state<LocaleCode>(getLocale());

  $effect(() => {
    const unsubscribe = locale.subscribe((code) => {
      currentLocale = code;
    });
    return unsubscribe;
  });

  function tr(key: string, vars?: Record<string, string | number>) {
    return t(key, vars, currentLocale);
  }
  let settingsTimer: ReturnType<typeof setTimeout> | null = null;
  let settingsSavedTimer: ReturnType<typeof setTimeout> | null = null;
  let persistTail: Promise<void> = Promise.resolve();
  let oauthPollTimer: ReturnType<typeof setInterval> | null = null;
  let showOAuthToken = $state(false);
  let oauthNotice = $state("");
  let nickHelpOpen = $state(false);
  let nickHelpPositioned = $state(false);
  let nickHelpTriggerEl = $state<HTMLButtonElement | null>(null);
  let nickHelpPopoverEl = $state<HTMLDivElement | null>(null);
  let nickHelpPos = $state({ top: 0, left: 0 });
  const oauthRedirectUri = twitchOAuthRedirectUri();

  const canAddChannel = $derived(channelRows.length < TWITCH_MAX_CHANNELS);
  const hasConnectCredentials = $derived.by(() => {
    const { channels } = channelsFromRows(channelRows);
    return (
      channels.length > 0 &&
      twitch.nick.trim().length > 0 &&
      twitch.oauth_token.trim().length > 0
    );
  });
  const isConnected = $derived(status.state === "connected");
  const isConnecting = $derived(status.state === "connecting");
  const canDisconnect = $derived(isConnected || isConnecting);
  const connectDisabledReason = $derived.by(() => {
    if (busy || isConnecting || isConnected) return "";
    if (!moduleEnabled) return tr("tts.twitch.connect_need_module");
    const { channels } = channelsFromRows(channelRows);
    if (channels.length === 0) return tr("tts.twitch.connect_need_channel");
    if (!twitch.nick.trim()) return tr("tts.twitch.connect_need_nick");
    if (!twitch.oauth_token.trim()) return tr("tts.twitch.connect_need_oauth");
    return "";
  });

  $effect(() => {
    if (!channelRowsDirty) {
      const rows = resolveChannelRows(twitch);
      channelRows = rows.length > 0 ? rows : [""];
    }
  });
  const channelBadgeLabel = $derived.by(() => {
    const configured = channelsFromRows(channelRows).channels;
    const joined =
      status.channels?.length ??
      (status.channel ? status.channel.split(", ").filter((entry) => entry.trim()).length : 0);

    if (configured.length <= 1 && joined <= 1) {
      const single =
        status.channels?.[0] ??
        (status.channel || (configured[0] ? `#${configured[0]}` : ""));
      return single;
    }

    const total = configured.length || joined || TWITCH_MAX_CHANNELS;
    const count = joined > 0 ? joined : configured.length;
    return tr("tts.twitch.channels_badge", { joined: count, total });
  });

  const showChannelBadge = $derived(
    (isConnected || isConnecting) &&
      (channelBadgeLabel.trim().length > 0 || channelsFromRows(channelRows).channels.length > 0),
  );

  const twitchAudioOutputs = $derived.by(() => {
    const deviceId = twitch.audio_output_device_id || "";
    if (!deviceId) return audioOutputs;
    if (audioOutputs.some((entry) => entry.id === deviceId)) {
      return audioOutputs;
    }
    return [
      ...audioOutputs,
      {
        id: deviceId,
        label: twitch.audio_output_device_label || tr("tts.module.saved_output"),
        is_default: false,
      },
    ];
  });

  $effect(() => {
    const users = (twitch.ignore_users || []).join(", ");
    if (!ignoreUsersDirty && document.activeElement !== ignoreUsersInput) {
      ignoreUsersText = users;
    }
    const symbols = (twitch.strip_symbols ?? []).join(", ");
    if (!stripSymbolsDirty && document.activeElement !== stripSymbolsInput) {
      stripSymbolsText = symbols;
    }
    const langs = (twitch.enabled_languages ?? []).join(", ");
    if (!enabledLanguagesDirty) {
      enabledLanguagesText = langs;
    }
  });

  export function handleConnectionUpdate(next: TwitchConnectionStatus) {
    status = next;
    ttsTrace("twitch", "connection_update", {
      state: next.state,
      channel: next.channel,
      channels: next.channels?.length ?? 0,
      message: next.message,
    });
  }

  export function recordChatMessage(message: TwitchChatMessage) {
    chatLog = prependTwitchChatLog(
      chatLog,
      message as TwitchChatMessage & Record<string, unknown>,
      `chat-${chatLogSeq++}`,
    );
    ttsTraceText("twitch", "chat_message", message.text, {
      id: message.id,
      user: message.user,
      speakable: message.speakable ?? true,
      lang: message.language,
    });
  }

  async function refreshStatus() {
    try {
      status = await fetchTwitchStatus();
      ttsTrace("twitch", "status_refresh", {
        state: status.state,
        channel: status.channel,
      });
    } catch (err) {
      ttsTrace("twitch", "status_refresh_error", {
        message: err instanceof Error ? err.message : String(err),
      });
    }
  }

  function enqueuePersist() {
    persistTail = persistTail
      .then(() => persistSettings())
      .catch(() => {
        // persistSettings records UI error state
      });
  }

  function queueSave() {
    if (settingsTimer) clearTimeout(settingsTimer);
    settingsTimer = setTimeout(() => {
      settingsTimer = null;
      enqueuePersist();
    }, 400);
  }

  function saveNow() {
    if (settingsTimer) {
      clearTimeout(settingsTimer);
      settingsTimer = null;
    }
    enqueuePersist();
  }

  function flushPendingSave() {
    if (!settingsTimer) return;
    clearTimeout(settingsTimer);
    settingsTimer = null;
    enqueuePersist();
  }

  function markSettingsSaved() {
    settingsSaved = true;
    if (settingsSavedTimer) clearTimeout(settingsSavedTimer);
    settingsSavedTimer = setTimeout(() => {
      settingsSavedTimer = null;
      settingsSaved = false;
    }, 2500);
  }

  function stopOAuthPoll() {
    if (oauthPollTimer) {
      clearInterval(oauthPollTimer);
      oauthPollTimer = null;
    }
  }

  async function pollOAuthFromBrowser() {
    try {
      const token = await fetchPendingOAuthToken();
      if (!token) return;
      stopOAuthPoll();
      twitch = { ...twitch, oauth_token: token };
      oauthNotice = tr("tts.twitch.oauth_received");
      ttsTrace("twitch", "oauth_implicit_ok", { source: "system_browser" });
      saveNow();
      error = "";
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("twitch", "oauth_poll_error", { message });
    }
  }

  function startOAuthPoll() {
    stopOAuthPoll();
    oauthNotice = tr("tts.twitch.oauth_waiting");
    void pollOAuthFromBrowser();
    oauthPollTimer = setInterval(() => {
      void pollOAuthFromBrowser();
    }, 1500);
  }

  async function handleGetOAuthToken() {
    error = "";
    oauthNotice = "";
    stopOAuthPoll();
    try {
      ttsTrace("twitch", "oauth_implicit_start", {
        redirect_uri: oauthRedirectUri,
      });
      await openTwitchOAuthInSystemBrowser(twitch.oauth_client_id);
      startOAuthPoll();
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      error = message.startsWith("tts.") ? tr(message) : message;
    }
  }

  function toggleOAuthTokenVisibility() {
    showOAuthToken = !showOAuthToken;
  }

  async function toggleNickHelp(event: MouseEvent) {
    event.stopPropagation();
    if (nickHelpOpen) {
      nickHelpOpen = false;
      nickHelpPositioned = false;
      return;
    }
    nickHelpPositioned = false;
    nickHelpOpen = true;
    await tick();
    if (nickHelpTriggerEl && nickHelpPopoverEl) {
      nickHelpPos = clampPopoverPosition(
        rectFromElement(nickHelpTriggerEl),
        rectFromElement(nickHelpPopoverEl),
        {
          viewportWidth: window.innerWidth,
          viewportHeight: window.innerHeight,
        },
      );
      nickHelpPositioned = true;
    }
  }

  function closeNickHelp() {
    nickHelpOpen = false;
    nickHelpPositioned = false;
  }

  function updateChannelRow(index: number, value: string) {
    channelRowsDirty = true;
    channelRows = channelRows.map((row, rowIndex) =>
      rowIndex === index ? value : row,
    );
    queueSave();
  }

  function addChannelRow() {
    if (!canAddChannel) return;
    channelRowsDirty = true;
    channelRows = [...channelRows, ""];
  }

  function removeChannelRow(index: number) {
    channelRowsDirty = true;
    if (channelRows.length <= 1) {
      channelRows = [""];
      queueSave();
      return;
    }
    channelRows = channelRows.filter((_, rowIndex) => rowIndex !== index);
    queueSave();
  }

  function updateNickReplacements(pairs: TwitchReplacement[]) {
    twitch = { ...twitch, nick_replacements: pairs };
    saveNow();
  }

  function emoteSources() {
    const defaults = defaultTwitchSettings().emote_sources!;
    return { ...defaults, ...twitch.emote_sources };
  }

  async function persistSettings() {
    settingsSaving = true;
    try {
      const ignore_users = ignoreUsersText
        .split(/[,\n;]/)
        .map((entry) => entry.trim())
        .filter(Boolean);
      const strip_symbols = stripSymbolsText
        .split(/[,\n;]/)
        .map((entry) => entry.trim())
        .filter((entry) => Boolean(entry) && entry !== "_");
      const enabled_languages = enabledLanguagesText
        .split(/[,;\s]+/)
        .map((entry) => entry.trim().toLowerCase())
        .filter(Boolean);
      const { channels, channel } = channelsFromRows(channelRows);
      const next = {
        ...twitch,
        channels,
        channel,
        ignore_users,
        strip_symbols,
        enabled_languages,
        nick_replacements: [...(twitch.nick_replacements ?? [])],
        emote_sources: emoteSources(),
      };
      twitch = next;
      const saved = await updateTwitchSettings(next);
      twitch = saved.twitch ?? next;
      channelRowsDirty = false;
      channelRows = resolveChannelRows(twitch);
      if (channelRows.length === 0) channelRows = [""];
      ignoreUsersDirty = false;
      stripSymbolsDirty = false;
      enabledLanguagesDirty = false;
      onTwitchConfigSaved?.(twitch);
      markSettingsSaved();
      ttsTrace("twitch", "settings_saved", {
        channels: next.channels?.length ?? 0,
        enabled: next.enabled,
        lang: next.language,
        ignore_users: next.ignore_users.length,
        strip_symbols: next.strip_symbols.length,
        strip_links: next.strip_links ?? true,
      });
      error = "";
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("twitch", "settings_save_error", { message });
      error = message;
    } finally {
      settingsSaving = false;
    }
  }

  async function handleConnect() {
    busy = true;
    error = "";
    ttsTrace("twitch", "connect_click", {
      channels: channelsFromRows(channelRows).channels.length,
    });
    try {
      if (!twitch.enabled) {
        twitch = { ...twitch, enabled: true };
      }
      await persistSettings();
      status = await connectTwitchChat();
      ttsTrace("twitch", "connect_ok", {
        state: status.state,
        channel: status.channel,
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("twitch", "connect_error", { message });
      error = message;
      await refreshStatus();
    } finally {
      busy = false;
    }
  }

  async function handleTwitchAudioDeviceChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    const deviceId = target.value;
    const device = audioOutputs.find((entry) => entry.id === deviceId);
    try {
      const saved = await setTtsChannelAudioDevice(
        "twitch",
        deviceId,
        device?.label || "",
      );
      twitch = saved.twitch ?? twitch;
      onTwitchConfigSaved?.(twitch);
      error = "";
      ttsTrace("twitch", "audio_device", {
        device_id: deviceId || "default",
      });
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function handleDisconnect() {
    busy = true;
    error = "";
    ttsTrace("twitch", "disconnect_click", {});
    try {
      await disconnectTwitchChat();
      status = { state: "disconnected", channel: "", channels: [], message: "" };
      ttsTrace("twitch", "disconnect_ok", {});
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      ttsTrace("twitch", "disconnect_error", { message });
      error = message;
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    void refreshStatus();
    return () => {
      flushPendingSave();
      stopOAuthPoll();
      if (settingsSavedTimer) clearTimeout(settingsSavedTimer);
    };
  });
</script>

<section class="surface-card bento-tile panel-padding stack">
  <div class="section-heading section-heading--stacked">
    <p class="eyebrow">{tr("tts.twitch.eyebrow")}</p>
    <h2>{tr("tts.twitch.title")}</h2>
  </div>

  <div class="tts-status-badges">
    <span class="badge" class:active={isConnected}>
      {tr("tts.twitch.irc")}: {status.state}
    </span>
    {#if showChannelBadge}
      <span class="badge" class:active={isConnected}>{channelBadgeLabel}</span>
    {/if}
    {#if settingsSaving}
      <span class="badge">{tr("tts.twitch.settings_saving")}</span>
    {:else if settingsSaved}
      <span class="badge active">{tr("tts.twitch.settings_saved")}</span>
    {/if}
    {#if error || status.message}
      <span class="badge err">{error || status.message}</span>
    {/if}
  </div>

  <label class="checkbox-row stack-field--full">
    <input
      type="checkbox"
      checked={twitch.enabled}
      onchange={(e) => {
        twitch = { ...twitch, enabled: (e.currentTarget as HTMLInputElement).checked };
        saveNow();
      }}
    />
    <span>{tr("tts.twitch.enable")}</span>
  </label>

  <div class="tts-twitch-connect">
    <div class="tts-twitch-connect__head">
      <h3>{tr("tts.twitch.connection_title")}</h3>
      <p class="muted">{tr("tts.twitch.channels_hint", { max: TWITCH_MAX_CHANNELS })}</p>
    </div>

    <div class="tts-twitch-channels">
      <span class="tts-twitch-channels__label">{tr("tts.twitch.channels")}</span>
      <div class="tts-twitch-channel-list">
        {#each channelRows as row, index (index)}
          <div class="tts-twitch-channel-row">
            <span class="tts-twitch-channel-row__prefix">#</span>
            <input
              class="control"
              placeholder={tr("tts.twitch.channel_placeholder")}
              value={row}
              disabled={isConnected || isConnecting}
              oninput={(e) =>
                updateChannelRow(index, (e.currentTarget as HTMLInputElement).value)}
            />
            <button
              type="button"
              class="btn btn-ghost btn-sm tts-twitch-channel-row__remove"
              title={tr("tts.twitch.channel_remove")}
              disabled={isConnected || isConnecting || (channelRows.length <= 1 && !row.trim())}
              onclick={() => removeChannelRow(index)}
            >
              ×
            </button>
          </div>
        {/each}
      </div>
      {#if canAddChannel}
        <button
          type="button"
          class="btn btn-ghost btn-sm tts-twitch-channel-add"
          disabled={isConnected || isConnecting}
          onclick={addChannelRow}
        >
          {tr("tts.twitch.channel_add")}
        </button>
      {/if}
    </div>

    <label class="stack-field">
      <div class="stack-field__label-row">
        <span>{tr("tts.twitch.nick")}</span>
        <span class="tts-telemetry-help">
          <button
            type="button"
            class="tts-telemetry-help-trigger"
            bind:this={nickHelpTriggerEl}
            aria-label={tr("tts.twitch.nick_help_trigger")}
            aria-expanded={nickHelpOpen}
            onclick={toggleNickHelp}
          >
            ?
          </button>
        </span>
      </div>
      <input
        class="control"
        placeholder={tr("tts.twitch.nick_placeholder")}
        value={twitch.nick}
        disabled={isConnected || isConnecting}
        oninput={(e) => {
          twitch = { ...twitch, nick: (e.currentTarget as HTMLInputElement).value };
          queueSave();
        }}
      />
    </label>

    <div class="stack-field stack-field--full">
      <span>{tr("tts.twitch.oauth_token")}</span>
      <div class="tts-oauth-token-row">
        <input
          class="control"
          type={showOAuthToken ? "text" : "password"}
          autocomplete="off"
          placeholder={tr("tts.twitch.oauth_placeholder")}
          value={twitch.oauth_token}
          disabled={isConnected || isConnecting}
          oninput={(e) => {
            twitch = { ...twitch, oauth_token: (e.currentTarget as HTMLInputElement).value };
            queueSave();
          }}
        />
        <button
          type="button"
          class="btn btn-ghost btn-sm"
          disabled={isConnected || isConnecting}
          onclick={toggleOAuthTokenVisibility}
        >
          {showOAuthToken ? tr("tts.twitch.oauth_hide") : tr("tts.twitch.oauth_show")}
        </button>
      </div>
      <button
        type="button"
        class="btn btn-primary tts-twitch-oauth-btn"
        disabled={isConnected || isConnecting}
        onclick={handleGetOAuthToken}
      >
        {tr("tts.twitch.oauth_get")}
      </button>
      {#if oauthNotice}
        <p class="muted tts-oauth-notice">{oauthNotice}</p>
      {/if}
      <p class="muted tts-oauth-hint">
        {tr("tts.twitch.oauth_hint", { uri: oauthRedirectUri })}
      </p>
    </div>

    <div class="tts-twitch-connect__actions">
      <button
        type="button"
        class="btn btn-primary"
        disabled={busy || isConnecting || isConnected || !hasConnectCredentials || !moduleEnabled}
        onclick={() => void handleConnect()}
      >
        {isConnecting ? tr("tts.twitch.connecting") : tr("tts.twitch.connect")}
      </button>
      <button
        type="button"
        class="btn btn-ghost"
        disabled={busy || !canDisconnect}
        onclick={() => void handleDisconnect()}
      >
        {tr("tts.twitch.disconnect")}
      </button>
    </div>
    {#if connectDisabledReason}
      <p class="muted tts-twitch-connect__hint">{connectDisabledReason}</p>
    {/if}
  </div>

  <div class="tts-settings-grid">
    <label class="stack-field stack-field--full">
      <span>{tr("tts.twitch.audio_output")}</span>
      <select
        class="control"
        value={twitch.audio_output_device_id || ""}
        onchange={(e) => void handleTwitchAudioDeviceChange(e)}
      >
        {#each twitchAudioOutputs as device (device.id || "default")}
          <option value={device.id}>{device.label}</option>
        {/each}
      </select>
    </label>

    <label class="stack-field">
      <span>{tr("tts.twitch.fallback_lang")}</span>
      <input
        class="control"
        placeholder="en"
        value={twitch.language}
        oninput={(e) => {
          twitch = { ...twitch, language: (e.currentTarget as HTMLInputElement).value };
          queueSave();
        }}
      />
    </label>

    <label class="stack-field">
      <span>{tr("tts.twitch.min_chars")}</span>
      <input
        class="control"
        type="number"
        min="1"
        max="32"
        value={twitch.min_chars}
        onchange={(e) => {
          twitch = {
            ...twitch,
            min_chars: Number((e.currentTarget as HTMLInputElement).value) || 1,
          };
          saveNow();
        }}
      />
    </label>

    <label class="checkbox-row">
      <input
        type="checkbox"
        checked={twitch.include_username}
        onchange={(e) => {
          twitch = {
            ...twitch,
            include_username: (e.currentTarget as HTMLInputElement).checked,
          };
          saveNow();
        }}
      />
      <span>{tr("tts.twitch.include_username")}</span>
    </label>

    <label class="checkbox-row">
      <input
        type="checkbox"
        checked={twitch.block_commands}
        onchange={(e) => {
          twitch = {
            ...twitch,
            block_commands: (e.currentTarget as HTMLInputElement).checked,
          };
          saveNow();
        }}
      />
      <span>{tr("tts.twitch.block_commands")}</span>
    </label>

    <label class="stack-field stack-field--full">
      <span>{tr("tts.twitch.ignore_users")}</span>
      <input
        class="control"
        bind:this={ignoreUsersInput}
        placeholder={tr("tts.twitch.ignore_users_placeholder")}
        value={ignoreUsersText}
        oninput={(e) => {
          ignoreUsersDirty = true;
          ignoreUsersText = (e.currentTarget as HTMLInputElement).value;
          queueSave();
        }}
        onblur={() => flushPendingSave()}
      />
    </label>

    <label class="checkbox-row">
      <input
        type="checkbox"
        checked={twitch.replace_underscore_with_space ?? true}
        onchange={(e) => {
          twitch = {
            ...twitch,
            replace_underscore_with_space: (e.currentTarget as HTMLInputElement).checked,
          };
          saveNow();
        }}
      />
      <span>{tr("tts.twitch.replace_underscore")}</span>
    </label>

    <label class="stack-field stack-field--full">
      <span>{tr("tts.twitch.strip_symbols")}</span>
      <input
        class="control"
        bind:this={stripSymbolsInput}
        placeholder={tr("tts.twitch.strip_symbols_placeholder")}
        value={stripSymbolsText}
        oninput={(e) => {
          stripSymbolsDirty = true;
          stripSymbolsText = (e.currentTarget as HTMLInputElement).value;
          queueSave();
        }}
        onblur={() => flushPendingSave()}
      />
      <span class="muted">{tr("tts.twitch.strip_symbols_hint")}</span>
    </label>

    <details class="tts-twitch-advanced stack-field--full">
      <summary>{tr("tts.twitch.advanced")}</summary>
      <div class="tts-twitch-advanced__body">
        {#if !nativePlayback}
          <label class="stack-field stack-field--range">
            <label class="checkbox-row">
              <input
                type="checkbox"
                checked={twitchRateOverride}
                onchange={(e) => {
                  const checked = (e.currentTarget as HTMLInputElement).checked;
                  twitch = {
                    ...twitch,
                    speech_rate: checked ? moduleSpeechRate : 0,
                  };
                  saveNow();
                }}
              />
              <span>{tr("tts.twitch.override_rate")}</span>
            </label>
            {#if twitchRateOverride}
              <span class="stack-field__label-row">
                <span>{tr("tts.speech.rate")}</span>
                <output class="stack-field__value" for="tts-twitch-rate">
                  {formatPlaybackRate(twitch.speech_rate ?? moduleSpeechRate)}
                </output>
              </span>
              <input
                id="tts-twitch-rate"
                type="range"
                min="0.5"
                max="2"
                step="0.05"
                value={twitch.speech_rate ?? moduleSpeechRate}
                oninput={(e) => {
                  twitch = {
                    ...twitch,
                    speech_rate: Number((e.currentTarget as HTMLInputElement).value) || 0.5,
                  };
                }}
                onchange={() => saveNow()}
              />
            {:else}
              <span class="muted">
                {tr("tts.twitch.inherit_rate", {
                  rate: formatPlaybackRate(moduleSpeechRate),
                })}
              </span>
            {/if}
          </label>
        {/if}

        <label class="stack-field stack-field--range">
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={twitchVolumeOverride}
              onchange={(e) => {
                const checked = (e.currentTarget as HTMLInputElement).checked;
                twitch = {
                  ...twitch,
                  speech_volume: checked ? moduleSpeechVolume : -1,
                };
                saveNow();
              }}
            />
            <span>{tr("tts.twitch.override_volume")}</span>
          </label>
          {#if twitchVolumeOverride}
            <span class="stack-field__label-row">
              <span>{tr("tts.speech.volume")}</span>
              <output class="stack-field__value" for="tts-twitch-volume">
                {formatSpeechVolume(twitch.speech_volume ?? moduleSpeechVolume)}
              </output>
            </span>
            <input
              id="tts-twitch-volume"
              type="range"
              min="0"
              max="1.5"
              step="0.05"
                value={twitch.speech_volume ?? moduleSpeechVolume}
                oninput={(e) => {
                  twitch = {
                    ...twitch,
                    speech_volume: Number((e.currentTarget as HTMLInputElement).value),
                  };
                }}
                onchange={() => saveNow()}
              />
          {:else}
            <span class="muted">
              {tr("tts.twitch.inherit_volume", {
                volume: formatSpeechVolume(moduleSpeechVolume),
              })}
            </span>
          {/if}
        </label>

        <label class="stack-field">
          <span>{tr("tts.twitch.max_queue")}</span>
          <input
            class="control"
            type="number"
            min="0"
            max="32"
            value={twitch.max_queue_items ?? 0}
            onchange={(e) => {
              twitch = {
                ...twitch,
                max_queue_items: Number((e.currentTarget as HTMLInputElement).value) || 0,
              };
              saveNow();
            }}
          />
          <span class="muted">{tr("tts.twitch.max_queue_hint")}</span>
        </label>

        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={twitch.strip_emotes ?? true}
            onchange={(e) => {
              twitch = {
                ...twitch,
                strip_emotes: (e.currentTarget as HTMLInputElement).checked,
              };
              saveNow();
            }}
          />
          <span>{tr("tts.twitch.strip_emotes")}</span>
        </label>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={twitch.strip_emoji ?? true}
            onchange={(e) => {
              twitch = {
                ...twitch,
                strip_emoji: (e.currentTarget as HTMLInputElement).checked,
              };
              saveNow();
            }}
          />
          <span>{tr("tts.twitch.strip_emoji")}</span>
        </label>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={twitch.strip_links ?? true}
            onchange={(e) => {
              twitch = {
                ...twitch,
                strip_links: (e.currentTarget as HTMLInputElement).checked,
              };
              saveNow();
            }}
          />
          <span>{tr("tts.twitch.strip_links")}</span>
        </label>
        <div class="tts-twitch-emote-sources">
          <span class="muted">{tr("tts.twitch.emote_sources")}</span>
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={emoteSources().twitch}
              onchange={(e) => {
                twitch = {
                  ...twitch,
                  emote_sources: {
                    ...emoteSources(),
                    twitch: (e.currentTarget as HTMLInputElement).checked,
                  },
                };
                saveNow();
              }}
            />
            <span>{tr("tts.twitch.emote_twitch")}</span>
          </label>
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={emoteSources().bttv}
              onchange={(e) => {
                twitch = {
                  ...twitch,
                  emote_sources: {
                    ...emoteSources(),
                    bttv: (e.currentTarget as HTMLInputElement).checked,
                  },
                };
                saveNow();
              }}
            />
            <span>{tr("tts.twitch.emote_bttv")}</span>
          </label>
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={emoteSources().seventv}
              onchange={(e) => {
                twitch = {
                  ...twitch,
                  emote_sources: {
                    ...emoteSources(),
                    seventv: (e.currentTarget as HTMLInputElement).checked,
                  },
                };
                saveNow();
              }}
            />
            <span>{tr("tts.twitch.emote_7tv")}</span>
          </label>
        </div>
        <label class="checkbox-row">
          <input
            type="checkbox"
            checked={twitch.detect_language ?? true}
            onchange={(e) => {
              twitch = {
                ...twitch,
                detect_language: (e.currentTarget as HTMLInputElement).checked,
              };
              saveNow();
            }}
          />
          <span>{tr("tts.twitch.detect_language")}</span>
        </label>
        <label class="stack-field">
          <span>{tr("tts.twitch.lang_min_chars")}</span>
          <input
            class="control"
            type="number"
            min="1"
            max="32"
            value={twitch.lang_min_chars ?? 4}
            onchange={(e) => {
              twitch = {
                ...twitch,
                lang_min_chars: Number((e.currentTarget as HTMLInputElement).value) || 4,
              };
              saveNow();
            }}
          />
        </label>
        <label class="stack-field stack-field--full">
          <span>{tr("tts.twitch.allowed_languages")}</span>
          <input
            class="control"
            placeholder={tr("tts.twitch.allowed_languages_placeholder")}
            value={enabledLanguagesText}
            oninput={(e) => {
              enabledLanguagesDirty = true;
              enabledLanguagesText = (e.currentTarget as HTMLInputElement).value;
              queueSave();
            }}
            onblur={() => flushPendingSave()}
          />
        </label>
        <label class="stack-field">
          <span>{tr("tts.twitch.max_chars")}</span>
          <input
            class="control"
            type="number"
            min="16"
            max="2000"
            value={twitch.max_chars}
            onchange={(e) => {
              twitch = {
                ...twitch,
                max_chars: Number((e.currentTarget as HTMLInputElement).value) || 200,
              };
              saveNow();
            }}
          />
          <span class="muted">{tr("tts.twitch.max_chars_hint")}</span>
        </label>
        <label class="stack-field">
          <span>{tr("tts.twitch.pause_style")}</span>
          <select
            class="control"
            value={twitch.pause_style ?? "period"}
            onchange={(e) => {
              twitch = {
                ...twitch,
                pause_style: (e.currentTarget as HTMLSelectElement).value as TwitchPauseStyle,
              };
              saveNow();
            }}
          >
            <option value="period">{tr("tts.twitch.pause.period")}</option>
            <option value="comma">{tr("tts.twitch.pause.comma")}</option>
            <option value="dash">{tr("tts.twitch.pause.dash")}</option>
            <option value="ellipsis">{tr("tts.twitch.pause.ellipsis")}</option>
          </select>
          <span class="muted">{tr("tts.twitch.pause_style_hint")}</span>
        </label>
        <label class="stack-field stack-field--full">
          <span>{tr("tts.twitch.speak_template")}</span>
          <input
            class="control"
            placeholder={tr("tts.twitch.speak_template_placeholder")}
            value={twitch.speak_template ?? "{nick}{pause}{text}"}
            oninput={(e) => {
              twitch = {
                ...twitch,
                speak_template: (e.currentTarget as HTMLInputElement).value,
              };
              queueSave();
            }}
          />
          <span class="muted">{tr("tts.twitch.speak_template_hint")}</span>
        </label>
        <label class="checkbox-row stack-field--full">
          <input
            type="checkbox"
            checked={twitch.include_builtin_profanity !== false}
            onchange={(e) => {
              twitch = {
                ...twitch,
                include_builtin_profanity: (e.currentTarget as HTMLInputElement).checked,
              };
              saveNow();
            }}
          />
          <span>{tr("tts.twitch.profanity_builtin")}</span>
        </label>
        <ReplacementPairEditor
          title={tr("tts.twitch.nick_replacements")}
          wordLabel={tr("tts.twitch.word_from")}
          replaceLabel={tr("tts.twitch.word_to")}
          wordPlaceholder={tr("tts.twitch.nick_from_placeholder")}
          replacePlaceholder={tr("tts.twitch.nick_to_placeholder")}
          addLabel={tr("tts.twitch.word_add")}
          removeLabel={tr("tts.twitch.word_remove_selected")}
          emptyLabel={tr("tts.twitch.nick_list_empty")}
          pairs={nickPairs}
          onChange={updateNickReplacements}
        />
      </div>
    </details>
  </div>

  {#if chatLog.length}
    <ul class="transcript-box tts-activity-log">
      {#each chatLog as line (line.logKey)}
        <li>
          <strong>{line.display_name}</strong>
          {#if line.channel}
            <span class="muted">{line.channel}</span>
          {/if}
          : {line.text}
          {#if line.speakable !== false}
            <span class="muted">
              → [{line.language}] {line.speak_text}
              {#if line.clean_text && line.clean_text !== line.text}
                ({tr("tts.twitch.clean")}: {line.clean_text})
              {/if}
            </span>
          {:else}
            <span class="muted"> {tr("tts.twitch.filtered")}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {:else}
    <p class="muted">{tr("tts.twitch.chat_empty")}</p>
  {/if}

  {#if nickHelpOpen}
    <button
      type="button"
      class="tts-telemetry-help-backdrop"
      aria-label={tr("tts.twitch.nick_help_close")}
      tabindex="-1"
      onclick={closeNickHelp}
    ></button>
    <div
      class="tts-telemetry-help-popover"
      class:tts-telemetry-help-popover--pending={!nickHelpPositioned}
      role="dialog"
      aria-modal="true"
      aria-labelledby="tts-twitch-nick-help-title"
      tabindex="-1"
      bind:this={nickHelpPopoverEl}
      style:top="{nickHelpPos.top}px"
      style:left="{nickHelpPos.left}px"
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => {
        event.stopPropagation();
        if (event.key === "Escape") closeNickHelp();
      }}
    >
      <p id="tts-twitch-nick-help-title" class="tts-telemetry-help-popover__title">
        {tr("tts.twitch.nick_help_title")}
      </p>
      <p>{tr("tts.twitch.nick_help_intro")}</p>
      <p>{tr("tts.twitch.nick_help_token")}</p>
    </div>
  {/if}
</section>
