<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { locale, t } from "../i18n";
  import { BROWSER_RECOGNITION_LANGUAGES } from "../constants";
  import { buildPreviewPayload } from "../preview-payload";
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
  $: asrMode = String(config?.asr?.mode || "browser_google");
  $: latestFinal = transcript.finals[0] || "";
  $: partialText = transcript.partial?.trim()
    ? transcript.partial
    : tr("asr.transcript.waiting_partial");
  $: finalText = latestFinal || tr("asr.transcript.no_finals");

  $: previewPayload =
    config &&
    buildPreviewPayload({
      config,
      runtime,
      overlayPayload,
      subtitleStylePresets,
      locale: loc,
    });

  let previewEl: HTMLDivElement | null = null;

  function patchBrowser(partial: Record<string, unknown>) {
    if (!config || !onConfigChange) return;
    onConfigChange({
      ...config,
      asr: {
        ...(config.asr || {}),
        mode: "browser_google",
        browser: { ...browser, ...partial },
      },
    });
  }

  function renderPreview() {
    if (!previewEl) return;
    const renderer = (window as Window & {
      SubtitleStyleRenderer?: {
        render: (el: HTMLElement, payload: unknown, opts?: unknown) => unknown;
        disposeRenderContainer?: (el: HTMLElement) => void;
      };
    }).SubtitleStyleRenderer;

    if (!renderer) {
      previewEl.innerHTML = `<p class="muted">${tr("overlay.preview.renderer_unavailable")}</p>`;
      return;
    }

    if (!previewPayload) {
      renderer.disposeRenderContainer?.(previewEl);
      previewEl.innerHTML = `<p class="muted">${tr("overlay.preview.waiting")}</p>`;
      return;
    }

    renderer.render(previewEl, previewPayload, { surface: "dashboard" });
  }

  $: previewPayload, previewEl, loc, config, renderPreview();

  onMount(() => {
    renderPreview();
  });

  onDestroy(() => {
    const renderer = (window as Window & {
      SubtitleStyleRenderer?: { disposeRenderContainer?: (el: HTMLElement) => void };
    }).SubtitleStyleRenderer;
    if (previewEl) renderer?.disposeRenderContainer?.(previewEl);
  });
</script>

<section class="glass-panel panel-padding overview-grid overview-section-root {className}">
  <div class="stack overview-bento-main">
    <article>
      <div class="section-heading">
        <div>
          <p class="eyebrow">{tr("overview.recognition.eyebrow")}</p>
          <h2>{tr("overview.transcript.title")}</h2>
        </div>
      </div>

      <p class="muted field-label">{tr("overview.partial")}</p>
      <pre class="transcript-box transcript-box--compact" class:transcript-partial={Boolean(transcript.partial)}>
        {partialText}
      </pre>

      <p class="muted field-label">{tr("overview.final")}</p>
      <pre class="transcript-box transcript-box--compact">{finalText}</pre>
    </article>

    <article class="overview-preview-article">
      <div class="section-heading">
        <div>
          <p class="eyebrow">{tr("overview.snapshot.eyebrow")}</p>
          <h2>{tr("overview.preview.title")}</h2>
        </div>
      </div>
      <div class="preview-surface" bind:this={previewEl}></div>
    </article>
  </div>

  <aside class="stack overview-recognition-aside">
    <article class="glass-panel glass-panel-elevated panel-padding recognition-card">
      <h2>{tr("overview.recognition.title")}</h2>

      {#if asrMode === "browser_google"}
        <details class="recognition-hint">
          <summary>{tr("overview.recognition.hint.summary")}</summary>
          <p class="muted">{tr("overview.recognition.hint.browser_google.warning")}</p>
        </details>
      {/if}

      {#if config && onConfigChange}
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

        <label class="stack-field">
          <span class="label">{tr("overview.recognition.worker_browser")}</span>
          <select
            class="control"
            value={String(browser.worker_launch_browser || "auto")}
            on:change={(e) =>
              patchBrowser({ worker_launch_browser: (e.currentTarget as HTMLSelectElement).value })}
          >
            <option value="auto">{tr("overview.recognition.worker_browser.auto")}</option>
            <option value="google_chrome">{tr("overview.recognition.worker_browser.chrome")}</option>
          </select>
        </label>
      {:else}
        <p class="muted">{tr("overview.recognition.mode.browser_google")}</p>
      {/if}
    </article>
  </aside>
</section>

<style>
  .recognition-card {
    padding: var(--space-4);
  }

  .recognition-card h2 {
    margin-top: 0;
    font-size: 18px;
  }

  .recognition-card .stack-field {
    gap: 4px;
  }

  .recognition-card .control {
    min-height: 38px;
    font-size: 13px;
  }

  .field-label {
    margin-bottom: var(--space-2);
    font-size: 13px;
  }

  .transcript-box--compact {
    min-height: 52px;
    max-height: 120px;
  }

  .recognition-hint {
    margin-bottom: var(--space-3);
  }

  .recognition-hint summary {
    cursor: pointer;
    color: var(--text-secondary);
    font-size: 13px;
    margin-bottom: var(--space-2);
  }

  .recognition-hint p {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
  }
</style>
