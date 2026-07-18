<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { locale, t } from "../i18n";
  import { buildPreviewPayload } from "../preview-payload";
  import type { ConfigPayload, RuntimeStatus, StylePresetCatalog } from "../types";

  export let config: ConfigPayload;
  export let runtime: RuntimeStatus | null = null;
  export let overlayPayload: Record<string, unknown> | null = null;
  export let subtitleStylePresets: StylePresetCatalog = {};
  export let showHeading = true;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  $: previewPayload = buildPreviewPayload({
    config,
    runtime,
    overlayPayload,
    subtitleStylePresets,
    locale: loc,
  });

  let previewEl: HTMLDivElement | null = null;

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

    const result = renderer.render(previewEl, previewPayload, {
      surface: "dashboard",
    }) as { empty?: boolean } | undefined;
    // Match OBS overlay contract: empty render must tear down fast-path state
    // or the last subtitle frame / min-height shell can stick in the preview.
    if (result?.empty) {
      renderer.disposeRenderContainer?.(previewEl);
    }
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

<article class="subtitle-output-preview">
  {#if showHeading}
    <div class="section-heading">
      <div>
        <h2>{tr("overview.preview.title")}</h2>
      </div>
    </div>
  {/if}
  <div class="preview-surface" bind:this={previewEl}></div>
</article>

<style>
  .subtitle-output-preview {
    display: grid;
    gap: var(--space-3);
  }

  .subtitle-output-preview .section-heading {
    margin-bottom: 0;
  }

  .subtitle-output-preview .section-heading h2 {
    margin: 0;
    font-size: 16px;
  }
</style>
