<script lang="ts">
  import { locale, t } from "../i18n";
  import type { ConfigPayload } from "../types";

  export let config: ConfigPayload;
  export let onChange: (next: ConfigPayload) => void;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  $: browser = (config.asr?.browser || {}) as Record<string, unknown>;
  $: realtime = (config.asr?.realtime || {}) as Record<string, unknown>;

  function patchBrowser(partial: Record<string, unknown>) {
    onChange({
      ...config,
      asr: {
        ...(config.asr || {}),
        mode: "browser_google",
        browser: { ...browser, ...partial },
      },
    });
  }

  function patchRealtime(partial: Record<string, unknown>) {
    onChange({
      ...config,
      asr: {
        ...(config.asr || {}),
        mode: "browser_google",
        realtime: { ...realtime, ...partial },
      },
    });
  }

  function browserInt(key: string, fallback: number): number {
    const value = Number(browser[key]);
    return Number.isFinite(value) ? value : fallback;
  }

  function realtimeInt(key: string, fallback: number): number {
    const value = Number(realtime[key]);
    return Number.isFinite(value) ? value : fallback;
  }
</script>

<div class="stack web-speech-advanced-root">
  <p class="muted dashboard-prose-hint">{tr("settings.webspeech.advanced.hint")}</p>

  <div class="section-heading">
    <p class="eyebrow">{tr("settings.webspeech.advanced.section.final_thresholds")}</p>
  </div>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.force_final_min_chars")}</span>
      <input
        class="control"
        type="number"
        min="1"
        max="256"
        step="1"
        value={browserInt("force_final_min_chars", 3)}
        on:input={(e) =>
          patchBrowser({ force_final_min_chars: Number((e.currentTarget as HTMLInputElement).value) })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.force_final_min_stable_ms")}</span>
      <input
        class="control"
        type="number"
        min="0"
        max="60000"
        step="50"
        value={browserInt("force_final_min_stable_ms", 700)}
        on:input={(e) =>
          patchBrowser({
            force_final_min_stable_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
  </div>

  <div class="section-heading" style="margin-top: 12px;">
    <p class="eyebrow">{tr("settings.webspeech.advanced.section.restart")}</p>
  </div>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.minimum_reconnect_interval_ms")}</span>
      <input
        class="control"
        type="number"
        min="100"
        max="60000"
        step="50"
        value={browserInt("minimum_reconnect_interval_ms", 500)}
        on:input={(e) =>
          patchBrowser({
            minimum_reconnect_interval_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.normal_restart_delay_ms")}</span>
      <input
        class="control"
        type="number"
        min="0"
        max="60000"
        step="50"
        value={browserInt("normal_restart_delay_ms", 350)}
        on:input={(e) =>
          patchBrowser({
            normal_restart_delay_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.no_speech_restart_delay_ms")}</span>
      <input
        class="control"
        type="number"
        min="0"
        max="60000"
        step="50"
        value={browserInt("no_speech_restart_delay_ms", 350)}
        on:input={(e) =>
          patchBrowser({
            no_speech_restart_delay_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.stuck_stopping_timeout_ms")}</span>
      <input
        class="control"
        type="number"
        min="500"
        max="30000"
        step="100"
        value={browserInt("stuck_stopping_timeout_ms", 2500)}
        on:input={(e) =>
          patchBrowser({
            stuck_stopping_timeout_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
  </div>

  <div class="section-heading" style="margin-top: 12px;">
    <p class="eyebrow">{tr("settings.webspeech.advanced.section.network")}</p>
  </div>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.network_reconnect_initial_ms")}</span>
      <input
        class="control"
        type="number"
        min="100"
        max="120000"
        step="100"
        value={browserInt("network_reconnect_initial_ms", 1000)}
        on:input={(e) =>
          patchBrowser({
            network_reconnect_initial_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.network_reconnect_max_ms")}</span>
      <input
        class="control"
        type="number"
        min="100"
        max="300000"
        step="500"
        value={browserInt("network_reconnect_max_ms", 30000)}
        on:input={(e) =>
          patchBrowser({
            network_reconnect_max_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
  </div>

  <div class="section-heading" style="margin-top: 12px;">
    <p class="eyebrow">{tr("settings.webspeech.advanced.section.session")}</p>
  </div>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.max_browser_session_age_ms")}</span>
      <input
        class="control"
        type="number"
        min="10000"
        max="3600000"
        step="1000"
        value={browserInt("max_browser_session_age_ms", 180000)}
        on:input={(e) =>
          patchBrowser({
            max_browser_session_age_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.prepare_cycle_before_ms")}</span>
      <input
        class="control"
        type="number"
        min="0"
        max="600000"
        step="1000"
        value={browserInt("prepare_cycle_before_ms", 15000)}
        on:input={(e) =>
          patchBrowser({
            prepare_cycle_before_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
  </div>

  <div class="section-heading" style="margin-top: 12px;">
    <p class="eyebrow">{tr("settings.webspeech.advanced.section.partial")}</p>
  </div>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.partial_min_delta_chars")}</span>
      <input
        class="control"
        type="number"
        min="0"
        max="256"
        step="1"
        value={realtimeInt("partial_min_delta_chars", 0)}
        on:input={(e) =>
          patchRealtime({
            partial_min_delta_chars: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("settings.webspeech.advanced.partial_coalescing_ms")}</span>
      <input
        class="control"
        type="number"
        min="0"
        max="10000"
        step="50"
        value={realtimeInt("partial_coalescing_ms", 0)}
        on:input={(e) =>
          patchRealtime({
            partial_coalescing_ms: Number((e.currentTarget as HTMLInputElement).value),
          })}
      />
    </label>
  </div>
  <p class="muted dashboard-prose-hint">{tr("settings.webspeech.advanced.partial.note")}</p>
</div>

<style>
  .web-speech-advanced-root {
    margin-top: 12px;
  }
</style>
