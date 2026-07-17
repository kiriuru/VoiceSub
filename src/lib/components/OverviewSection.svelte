<script lang="ts">

  import SubtitleOutputPreview from "./SubtitleOutputPreview.svelte";

  import { locale, t } from "../i18n";

  import { BROWSER_RECOGNITION_LANGUAGES } from "../constants";

  import {

    ASR_MODE_BROWSER,

    ASR_MODE_LOCAL_PARAKEET,

    isLocalAsrMode,

    localAsrReadyFromRuntime,

    normalizeAsrMode,

  } from "../asr-mode";

  import type {

    ConfigPayload,

    RuntimeStatus,

    StylePresetCatalog,

    TranscriptState,

  } from "../types";



  let className = "";

  export { className as class };



  export let transcript: TranscriptState;

  export let overlayPayload: Record<string, unknown> | null = null;

  export let config: ConfigPayload | null = null;

  export let runtime: RuntimeStatus | null = null;

  export let subtitleStylePresets: StylePresetCatalog = {};

  export let onConfigChange: ((next: ConfigPayload) => void) | undefined = undefined;



  $: loc = $locale;

  $: tr = (key: string) => t(key, undefined, loc);



  $: browser = (config?.asr?.browser || {}) as Record<string, unknown>;

  $: configuredMode = normalizeAsrMode(config?.asr?.mode);

  $: localModuleReady = localAsrReadyFromRuntime(runtime);

  $: asrMode =

    configuredMode === ASR_MODE_LOCAL_PARAKEET && !localModuleReady

      ? ASR_MODE_BROWSER

      : configuredMode;

  $: latestFinal = transcript.finals[0] || "";

  $: partialText = transcript.partial?.trim()

    ? transcript.partial

    : tr("asr.transcript.waiting_partial");

  $: finalText = latestFinal || tr("asr.transcript.no_finals");

  $: localConfiguredButNotReady =

    configuredMode === ASR_MODE_LOCAL_PARAKEET && !localModuleReady;



  function patchAsr(partial: { mode?: string; browser?: Record<string, unknown> }) {

    if (!config || !onConfigChange) return;

    onConfigChange({

      ...config,

      asr: {

        ...(config.asr || {}),

        mode: partial.mode ?? configuredMode,

        browser: partial.browser ? { ...browser, ...partial.browser } : browser,

      },

    });

  }



  function patchBrowser(partial: Record<string, unknown>) {

    patchAsr({ browser: partial });

  }



  function handleModeChange(nextMode: string) {

    if (nextMode === ASR_MODE_LOCAL_PARAKEET && !localModuleReady) return;

    patchAsr({ mode: normalizeAsrMode(nextMode) });

  }

</script>



<div class="overview-layout overview-section-root {className}">

  <section class="surface-card panel-padding overview-main-card">

    <div class="overview-main-header">

      <div>

        <p class="eyebrow">{tr("overview.recognition.eyebrow")}</p>

        <h2>{tr("overview.transcript.title")}</h2>

      </div>

    </div>



    <div class="overview-main-body">

      <div class="overview-transcript-panel">

        <div class="overview-transcript-cols">

          <div class="overview-transcript-col">

            <p class="muted field-label">{tr("overview.partial")}</p>

            <pre

              class="transcript-box transcript-box--live"

              class:transcript-partial={Boolean(transcript.partial)}

            >

              {partialText}

            </pre>

          </div>



          <div class="overview-transcript-col">

            <p class="muted field-label">{tr("overview.final")}</p>

            <pre class="transcript-box transcript-box--live">{finalText}</pre>

          </div>

        </div>

      </div>



      <aside class="overview-recognition-panel">

        <h3 class="overview-recognition-title">{tr("overview.recognition.title")}</h3>



        {#if config && onConfigChange}

          <label class="stack-field">

            <span class="label">{tr("overview.recognition.mode.label")}</span>

            <select

              class="control"

              value={asrMode}

              on:change={(e) => handleModeChange((e.currentTarget as HTMLSelectElement).value)}

            >

              <option value={ASR_MODE_BROWSER}>{tr("overview.recognition.mode.browser_google")}</option>

              {#if localModuleReady}

                <option value={ASR_MODE_LOCAL_PARAKEET}>

                  {tr("overview.recognition.mode.local_asr")}

                </option>

              {/if}

            </select>

          </label>



          {#if localConfiguredButNotReady}

            <p class="muted recognition-note">{tr("overview.recognition.local_not_ready")}</p>

          {/if}



          {#if !isLocalAsrMode(asrMode)}

            <label class="stack-field">

              <span class="label">{tr("overview.recognition.language")}</span>

              <select

                class="control"

                value={String(browser.recognition_language || "en-US")}

                on:change={(e) =>

                  patchBrowser({ recognition_language: (e.currentTarget as HTMLSelectElement).value })}

              >

                {#each BROWSER_RECOGNITION_LANGUAGES as lang}

                  <option value={lang.code}>{lang.label}</option>

                {/each}

              </select>

            </label>

          {/if}

        {:else}

          <p class="muted">

            {tr(isLocalAsrMode(asrMode) ? "overview.recognition.mode.local_asr" : "overview.recognition.mode.browser_google")}

          </p>

        {/if}

      </aside>

    </div>

  </section>



  {#if config}

    <section class="surface-card panel-padding overview-preview-card">

      <SubtitleOutputPreview {config} {runtime} {overlayPayload} {subtitleStylePresets} />

    </section>

  {/if}

</div>



<style>

  .overview-layout {

    display: grid;

    gap: var(--space-3);

  }



  .overview-main-header h2 {

    margin: 0;

    font-size: 18px;

    font-weight: 600;

    letter-spacing: -0.02em;

  }



  .overview-main-body {

    display: grid;

    grid-template-columns: minmax(0, 1fr) minmax(220px, 272px);

    gap: var(--space-4);

    align-items: start;

    margin-top: var(--space-3);

  }



  .overview-transcript-cols {

    display: grid;

    grid-template-columns: repeat(2, minmax(0, 1fr));

    gap: var(--space-3);

  }



  .overview-transcript-col {

    min-width: 0;

  }



  .transcript-box--live {

    min-height: 120px;

    max-height: 220px;

    margin: 0;

    padding: var(--space-3);

    border-radius: var(--radius-md);

    border: 1px solid var(--glass-border);

    background: var(--glass-inset);

    font-size: 13px;

    line-height: 1.5;

    white-space: pre-wrap;

    word-break: break-word;

    overflow: auto;

  }



  .overview-recognition-panel {

    display: grid;

    gap: var(--space-3);

    padding-left: var(--space-4);

    border-left: 1px solid color-mix(in srgb, var(--glass-border) 72%, transparent);

  }



  .overview-recognition-title {

    margin: 0;

    font-size: 14px;

    font-weight: 600;

  }



  .overview-recognition-panel .stack-field {

    gap: 4px;

  }



  .overview-recognition-panel .control {

    min-height: 36px;

    font-size: 13px;

  }



  .field-label {

    margin: 0 0 var(--space-2);

    font-size: 12px;

    font-weight: 500;

    letter-spacing: 0.02em;

    text-transform: uppercase;

    color: var(--text-tertiary);

  }



  .recognition-note {

    margin: 0;

    font-size: 13px;

    line-height: 1.45;

  }



  .overview-preview-card {

    min-height: 140px;

  }



  @media (max-width: 960px) {

    .overview-main-body {

      grid-template-columns: 1fr;

    }



    .overview-recognition-panel {

      padding-left: 0;

      padding-top: var(--space-4);

      border-left: 0;

      border-top: 1px solid color-mix(in srgb, var(--glass-border) 72%, transparent);

    }



    .overview-transcript-cols {

      grid-template-columns: 1fr;

    }

  }

</style>

