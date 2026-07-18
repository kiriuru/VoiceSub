<script lang="ts">
  import { onDestroy } from "svelte";
  import { openLocalUrl } from "../api";
  import { locale, t } from "../i18n";
  import { obsStatusMessage } from "../diagnostics";
  import { formatObsNativeCaptionStatus } from "../obs-status-i18n";
  import type { ConfigPayload } from "../types";

  export let overlayUrl: string;
  export let config: ConfigPayload;
  export let obsDiagnostics: Record<string, unknown> | undefined = undefined;
  export let onChange: (next: ConfigPayload) => void;

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string>) => t(key, vars, loc);

  $: obs = (config.obs_closed_captions || {}) as {
    enabled?: boolean;
    output_mode?: string;
    connection?: { host?: string; port?: number; password?: string };
    debug_mirror?: { enabled?: boolean; input_name?: string; send_partials?: boolean };
    timing?: {
      send_partials?: boolean;
      partial_throttle_ms?: number;
      min_partial_delta_chars?: number;
      final_replace_delay_ms?: number;
      clear_after_ms?: number;
      avoid_duplicate_text?: boolean;
    };
  };

  let copied = false;
  let showPassword = false;
  let copyFlashTimer: ReturnType<typeof setTimeout> | null = null;

  $: statusText = obsStatusMessage(obs.enabled === true, obsDiagnostics, tr);

  onDestroy(() => {
    if (copyFlashTimer !== null) {
      clearTimeout(copyFlashTimer);
      copyFlashTimer = null;
    }
  });

  const outputModes = [
    "disabled",
    "source_live",
    "source_final_only",
    "translation_1",
    "translation_2",
    "translation_3",
    "translation_4",
    "translation_5",
    "first_visible_line",
  ] as const;

  function patchObs(partial: Record<string, unknown>) {
    onChange({
      ...config,
      obs_closed_captions: { ...obs, ...partial },
    });
  }

  function patchNested(section: string, partial: Record<string, unknown>) {
    onChange({
      ...config,
      obs_closed_captions: {
        ...obs,
        [section]: { ...(obs as Record<string, Record<string, unknown>>)[section], ...partial },
      },
    });
  }

  async function copyUrl() {
    if (!overlayUrl) return;
    try {
      await navigator.clipboard.writeText(overlayUrl);
      copied = true;
      if (copyFlashTimer !== null) clearTimeout(copyFlashTimer);
      copyFlashTimer = setTimeout(() => {
        copied = false;
        copyFlashTimer = null;
      }, 1200);
    } catch {
      // ignore
    }
  }

  async function openOverlay() {
    if (overlayUrl) await openLocalUrl(overlayUrl);
  }
</script>

<div class="obs-layout bento-root stack">
  <section
    id="obs-section-overlay"
    class="surface-card panel-padding bento-tile bento-span-full stack panel-section-anchor"
  >
    <div class="section-heading">
      <div>
        <p class="eyebrow">{tr("tab.obs")}</p>
        <h2>{tr("overview.overlay.title")}</h2>
      </div>
    </div>
    <p class="muted">{tr("obs.overlay.instructions")}</p>
    <div class="url-row">
      <input class="control" readonly value={overlayUrl || tr("common.loading")} />
      <button type="button" class="btn btn-sm" class:copy-flash={copied} on:click={copyUrl}>
        {copied ? tr("obs.overlay.copied") : tr("obs.overlay.copy")}
      </button>
      <button type="button" class="btn btn-sm btn-ghost" on:click={openOverlay}>{tr("common.open_overlay")}</button>
    </div>
  </section>

  <section class="surface-card panel-padding stack obs-bento-main bento-grid bento-tile">
    <article id="obs-section-captions" class="stack panel-section-anchor">
      <div class="section-heading">
        <div>
          <p class="eyebrow">{tr("obs.eyebrow")}</p>
          <h2>{tr("obs.title")}</h2>
        </div>
      </div>

      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={obs.enabled === true}
          on:change={(e) => patchObs({ enabled: (e.currentTarget as HTMLInputElement).checked })}
        />
        <span>{tr("obs.enable")}</span>
      </label>

      <div class="grid-2">
        <label class="stack-field">
          <span>{tr("obs.host")}</span>
          <input
            class="control"
            value={obs.connection?.host || "127.0.0.1"}
            on:input={(e) => patchNested("connection", { host: (e.currentTarget as HTMLInputElement).value })}
          />
        </label>
        <label class="stack-field">
          <span>{tr("obs.port")}</span>
          <input
            class="control"
            type="number"
            min="1"
            max="65535"
            value={obs.connection?.port ?? 4455}
            on:input={(e) => patchNested("connection", { port: Number((e.currentTarget as HTMLInputElement).value) })}
          />
        </label>
      </div>

      <div class="url-row">
        <label class="stack-field" style="flex:1;">
          <span>{tr("obs.password")}</span>
          <input
            class="control"
            type={showPassword ? "text" : "password"}
            value={obs.connection?.password || ""}
            on:input={(e) => patchNested("connection", { password: (e.currentTarget as HTMLInputElement).value })}
          />
        </label>
        <button type="button" class="btn btn-sm btn-ghost" on:click={() => (showPassword = !showPassword)}>
          {showPassword ? tr("security.hide") : tr("security.show")}
        </button>
      </div>

      <label class="stack-field">
        <span>{tr("obs.output_mode")}</span>
        <select
          class="control"
          value={obs.output_mode || "disabled"}
          on:change={(e) => patchObs({ output_mode: (e.currentTarget as HTMLSelectElement).value })}
        >
          {#each outputModes as mode}
            <option value={mode}>{tr(`obs.output.${mode}`)}</option>
          {/each}
        </select>
      </label>

      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={obs.debug_mirror?.enabled === true}
          on:change={(e) => patchNested("debug_mirror", { enabled: (e.currentTarget as HTMLInputElement).checked })}
        />
        <span>{tr("obs.debug.enable")}</span>
      </label>

      <label class="stack-field">
        <span>{tr("obs.debug.input")}</span>
        <input
          class="control"
          value={obs.debug_mirror?.input_name || "CC_DEBUG"}
          on:input={(e) => patchNested("debug_mirror", { input_name: (e.currentTarget as HTMLInputElement).value })}
        />
      </label>

      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={obs.debug_mirror?.send_partials !== false}
          on:change={(e) =>
            patchNested("debug_mirror", { send_partials: (e.currentTarget as HTMLInputElement).checked })}
        />
        <span>{tr("obs.debug.send_partials")}</span>
      </label>

      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={obs.timing?.send_partials !== false}
          on:change={(e) => patchNested("timing", { send_partials: (e.currentTarget as HTMLInputElement).checked })}
        />
        <span>{tr("obs.source_live_partials")}</span>
      </label>

      <div class="grid-2">
        <label class="stack-field">
          <span>{tr("obs.partial_throttle")}</span>
          <input
            class="control"
            type="number"
            min="0"
            value={obs.timing?.partial_throttle_ms ?? 140}
            on:input={(e) =>
              patchNested("timing", { partial_throttle_ms: Number((e.currentTarget as HTMLInputElement).value) })}
          />
        </label>
        <label class="stack-field">
          <span>{tr("obs.min_partial_delta")}</span>
          <input
            class="control"
            type="number"
            min="0"
            value={obs.timing?.min_partial_delta_chars ?? 1}
            on:input={(e) =>
              patchNested("timing", { min_partial_delta_chars: Number((e.currentTarget as HTMLInputElement).value) })}
          />
        </label>
        <label class="stack-field">
          <span>{tr("obs.final_delay")}</span>
          <input
            class="control"
            type="number"
            min="0"
            value={obs.timing?.final_replace_delay_ms ?? 0}
            on:input={(e) =>
              patchNested("timing", { final_replace_delay_ms: Number((e.currentTarget as HTMLInputElement).value) })}
          />
        </label>
        <label class="stack-field">
          <span>{tr("obs.clear_after")}</span>
          <input
            class="control"
            type="number"
            min="0"
            value={obs.timing?.clear_after_ms ?? 2500}
            on:input={(e) =>
              patchNested("timing", { clear_after_ms: Number((e.currentTarget as HTMLInputElement).value) })}
          />
        </label>
      </div>

      <label class="checkbox-row">
        <input
          type="checkbox"
          checked={obs.timing?.avoid_duplicate_text !== false}
          on:change={(e) =>
            patchNested("timing", { avoid_duplicate_text: (e.currentTarget as HTMLInputElement).checked })}
        />
        <span>{tr("obs.avoid_duplicates")}</span>
      </label>

      <p class="muted">{tr("obs.note.native")}</p>
    </article>

    <article id="obs-section-status" class="stack panel-section-anchor">
      <div class="section-heading">
        <div>
          <p class="eyebrow">{tr("obs.state.eyebrow")}</p>
          <h2>{tr("obs.state.title")}</h2>
        </div>
      </div>
      <p class="muted">{statusText}</p>
      {#if obsDiagnostics?.connection_state}
        <p class="muted panel-note">
          {String(obsDiagnostics.connection_state)} · {obsDiagnostics.host || obs.connection?.host}:{obsDiagnostics.port ?? obs.connection?.port}
        </p>
      {/if}
      {#if obsDiagnostics?.native_caption_status}
        <p class="muted panel-note">
          {formatObsNativeCaptionStatus(String(obsDiagnostics.native_caption_status), tr)}
        </p>
      {/if}
      {#if obsDiagnostics?.stream_output_active !== undefined && obsDiagnostics?.stream_output_active !== null}
        <p class="muted panel-note">
          {tr("obs.stream.status", {
            state: obsDiagnostics.stream_output_active ? tr("common.on") : tr("common.off"),
          })}
          {#if obsDiagnostics.stream_output_reconnecting}
            · {tr("obs.stream.reconnecting")}
          {/if}
        </p>
      {/if}
      {#if obsDiagnostics?.obs_studio_version || obsDiagnostics?.obs_websocket_version}
        <p class="muted panel-note">
          {#if obsDiagnostics.obs_studio_version}OBS {String(obsDiagnostics.obs_studio_version)}{/if}
          {#if obsDiagnostics.obs_websocket_version} · ws {String(obsDiagnostics.obs_websocket_version)}{/if}
        </p>
      {/if}
      <p class="muted">{tr("obs.state.note")}</p>
    </article>
  </section>
</div>

