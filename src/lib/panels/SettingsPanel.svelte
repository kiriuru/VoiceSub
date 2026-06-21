<script lang="ts">
  import { onMount } from "svelte";
  import { locale, t } from "../i18n";
  import WebSpeechAdvancedSettings from "../components/WebSpeechAdvancedSettings.svelte";
  import { fontOptions, refreshSystemFonts } from "../font-catalog";
  import type { ConfigPayload, FontCatalog } from "../types";

  export let config: ConfigPayload;
  export let fontCatalog: FontCatalog | null;
  export let onChange: (next: ConfigPayload) => void;
  export let onFontCatalogChange: (catalog: FontCatalog) => void;

  let systemFontCount = 0;
  let refreshingFonts = false;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
  $: ui = config.ui || {};
  $: translation = config.translation || {};
  $: projectFontCount = fontCatalog?.project_local?.length || 0;
  $: fallbackFontCount = fontCatalog?.fallback?.length || 0;
  $: totalFonts = fontOptions(fontCatalog).length;

  onMount(() => {
    systemFontCount = fontCatalog?.system?.length || 0;
  });

  async function handleRefreshSystemFonts() {
    refreshingFonts = true;
    try {
      const system = await refreshSystemFonts();
      systemFontCount = system.length;
      if (fontCatalog) {
        onFontCatalogChange({ ...fontCatalog, system });
      }
    } finally {
      refreshingFonts = false;
    }
  }

  function patchTranslation(partial: Record<string, unknown>) {
    onChange({ ...config, translation: { ...translation, ...partial } });
  }
</script>

<section class="surface-card panel-padding stack">
  <div class="section-heading">
    <div>
      <p class="eyebrow">{tr("settings.eyebrow")}</p>
      <h2>{tr("settings.title")}</h2>
    </div>
  </div>

  <label class="stack-field">
    <span>{tr("ui.layout.label")}</span>
    <select
      class="control"
      value={String(ui.layout || "standard")}
      on:change={(e) =>
        onChange({ ...config, ui: { ...ui, layout: (e.currentTarget as HTMLSelectElement).value } })}
    >
      <option value="standard">{tr("ui.layout.standard")}</option>
      <option value="compact">{tr("ui.layout.compact")}</option>
    </select>
  </label>

  <div class="section-heading section-heading--spaced">
    <div>
      <p class="eyebrow">{tr("translation.dispatcher.eyebrow")}</p>
      <h3>{tr("translation.dispatcher.title")}</h3>
    </div>
  </div>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("translation.dispatcher.timeout_ms")}</span>
      <input
        class="control"
        type="number"
        min="1000"
        max="60000"
        step="500"
        value={Number(translation.timeout_ms ?? 10000)}
        on:input={(e) =>
          patchTranslation({ timeout_ms: Number((e.currentTarget as HTMLInputElement).value) })}
      />
    </label>
    <label class="stack-field">
      <span>{tr("translation.dispatcher.queue_max_size")}</span>
      <input
        class="control"
        type="number"
        min="1"
        max="64"
        step="1"
        value={Number(translation.queue_max_size ?? 8)}
        on:input={(e) =>
          patchTranslation({ queue_max_size: Number((e.currentTarget as HTMLInputElement).value) })}
      />
    </label>
  </div>

  <label class="stack-field">
    <span>{tr("translation.dispatcher.max_concurrent_jobs")}</span>
    <input
      class="control"
      type="number"
      min="1"
      max="8"
      step="1"
      value={Number(translation.max_concurrent_jobs ?? 2)}
      on:input={(e) =>
        patchTranslation({ max_concurrent_jobs: Number((e.currentTarget as HTMLInputElement).value) })}
    />
  </label>

  <p class="muted dashboard-prose-hint">{tr("translation.dispatcher.note")}</p>

  <div class="section-heading section-heading--stacked section-heading--spaced">
    <p class="eyebrow">{tr("settings.fonts.eyebrow")}</p>
    <h3>{tr("settings.fonts.title")}</h3>
  </div>

  <p class="muted mono-block">
    {fontCatalog?.project_fonts_dir || "—"}
  </p>

  <p class="muted">
    Project: {projectFontCount} · System: {systemFontCount} · Fallback: {fallbackFontCount} · Total: {totalFonts}
  </p>

  <div class="url-row">
    <button type="button" class="btn" disabled={refreshingFonts} on:click={handleRefreshSystemFonts}>
      {tr("common.refresh")}
    </button>
  </div>

  <p class="muted">{tr("settings.fonts.hint")}</p>
  <p class="muted">{tr("settings.note")}</p>

  <details class="web-speech-advanced-spoiler compact-keep-hints">
    <summary class="spoiler-summary">
      <span class="spoiler-title">{tr("settings.webspeech.advanced.summary")}</span>
      <span class="muted spoiler-hint">{tr("settings.webspeech.advanced.summary_hint")}</span>
    </summary>
    <WebSpeechAdvancedSettings {config} onChange={onChange} />
  </details>
</section>

<style>
  .mono-block {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 13px;
    word-break: break-all;
  }

  .web-speech-advanced-spoiler {
    margin-top: 16px;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
  }

  .spoiler-summary {
    cursor: pointer;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 8px 0;
  }

  .spoiler-summary::-webkit-details-marker {
    display: none;
  }

  .spoiler-title {
    font-weight: 600;
    font-size: 15px;
  }

  .spoiler-hint {
    font-size: 13px;
  }
</style>
