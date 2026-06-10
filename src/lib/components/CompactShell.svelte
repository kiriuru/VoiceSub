<script lang="ts">
  import { locale, t } from "../i18n";
  import { UI_LOCALES } from "../constants";
  import { formatSaveStatusDisplay, type SaveStatusState } from "../save-status";
  import RuntimeBar from "./RuntimeBar.svelte";
  import TabNav from "./TabNav.svelte";
  import OverviewSection from "./OverviewSection.svelte";
  import DashboardPanels from "./DashboardPanels.svelte";
  import type {
    CompactPaneId,
    ConfigPayload,
    DiagnosticsSnapshot,
    FontCatalog,
    LocaleCode,
    RuntimeStatus,
    StylePresetCatalog,
    TranscriptState,
    TranslationResultState,
  } from "../types";

  export let compactPane: CompactPaneId;
  export let version: string;
  export let config: ConfigPayload;
  export let runtime: RuntimeStatus;
  export let wsConnected: boolean;
  export let busy: boolean;
  export let saveStatus: SaveStatusState;
  export let transcript: TranscriptState;
  export let overlayPayload: Record<string, unknown> | null;
  export let subtitleStylePresets: StylePresetCatalog;
  export let diagnostics: DiagnosticsSnapshot;
  export let fontCatalog: FontCatalog | null;
  export let translationResults: TranslationResultState | null;
  export let overlayUrl: string;

  export let onSelectPane: (pane: CompactPaneId) => void;
  export let onStart: () => void;
  export let onStop: () => void;
  export let onSave: () => void;
  export let onOpenCommandPalette: () => void;
  export let onConfigChange: (next: ConfigPayload) => void;
  export let onLanguageChange: (lang: LocaleCode) => void;
  export let onConfigLoad: (next: ConfigPayload) => void;
  export let onFontCatalogChange: (catalog: FontCatalog) => void;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
  $: saveStatusText = formatSaveStatusDisplay(saveStatus, runtime, loc);
</script>

<div class="compact-phone-shell">
  <header class="compact-top-bar">
    <div class="compact-top-row">
      <div class="compact-brand">
        <span class="compact-brand-title">VoiceSub</span>
        <span class="compact-brand-version muted">v{version}</span>
      </div>
      <div class="compact-top-actions">
        <button
          type="button"
          class="btn btn-ghost compact-icon-btn"
          title={tr("app.chrome.search")}
          aria-label={tr("app.chrome.search")}
          on:click={onOpenCommandPalette}
        >
          ⌕
        </button>
        <button class="btn compact-save-btn" disabled={busy} on:click={onSave}>{tr("common.save")}</button>
      </div>
    </div>

    <label class="compact-locale-field stack-field">
      <span class="sr-only">{tr("language.label")}</span>
      <select
        class="control"
        value={$locale}
        on:change={(e) => {
          onLanguageChange((e.currentTarget as HTMLSelectElement).value as LocaleCode);
        }}
      >
        {#each UI_LOCALES as item}
          <option value={item.code}>{tr(item.labelKey)}</option>
        {/each}
      </select>
    </label>

    <RuntimeBar
      {runtime}
      obsDiagnostics={diagnostics.obs}
      {wsConnected}
      {busy}
      onStart={onStart}
      onStop={onStop}
    />

    <p
      class="muted compact-save-status save-status"
      class:success={saveStatus.tone === "success"}
      class:warn={saveStatus.tone === "warn"}
      class:error={saveStatus.tone === "error"}
      class:busy={saveStatus.tone === "busy"}
    >
      {saveStatusText}
    </p>
  </header>

  <main class="compact-content-scroll">
    {#if compactPane === "live"}
      <OverviewSection
        class="compact-overview"
        {transcript}
        {overlayPayload}
        {config}
        {runtime}
        {subtitleStylePresets}
        onConfigChange={onConfigChange}
      />
    {:else}
      <DashboardPanels
        activeTab={compactPane}
        {config}
        {diagnostics}
        {subtitleStylePresets}
        {fontCatalog}
        {translationResults}
        {overlayUrl}
        onChange={onConfigChange}
        onConfigLoad={onConfigLoad}
        onFontCatalogChange={onFontCatalogChange}
      />
    {/if}
  </main>

  <TabNav variant="compact" activePane={compactPane} onSelectPane={onSelectPane} />
</div>

<style>
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
