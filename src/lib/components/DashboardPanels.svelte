<script lang="ts">

  import TranslationPanel from "../panels/TranslationPanel.svelte";
  import SubtitlesPanel from "../panels/SubtitlesPanel.svelte";
  import StylePanel from "../panels/StylePanel.svelte";
  import ThemePanel from "../panels/ThemePanel.svelte";
  import ObsPanel from "../panels/ObsPanel.svelte";
  import ReplacementPanel from "../panels/ReplacementPanel.svelte";
  import ToolsPanel from "../panels/ToolsPanel.svelte";
  import SettingsPanel from "../panels/SettingsPanel.svelte";
  import HelpPanel from "../panels/HelpPanel.svelte";
  import type {
    ConfigPayload,
    DiagnosticsSnapshot,
    FontCatalog,
    RuntimeStatus,
    StylePresetCatalog,
    TabId,
    TranslationResultState,
  } from "../types";
  export let activeTab: TabId;
  export let config: ConfigPayload;
  export let diagnostics: DiagnosticsSnapshot;
  export let subtitleStylePresets: StylePresetCatalog;
  export let fontCatalog: FontCatalog | null;
  export let overlayUrl: string;
  export let layoutMode: "standard" | "compact" = "standard";
  export let runtime: RuntimeStatus | null = null;
  export let overlayPayload: Record<string, unknown> | null = null;
  export let onChange: (next: ConfigPayload) => void;
  export let onConfigLoad: (next: ConfigPayload) => void;
  export let onFontCatalogChange: (catalog: FontCatalog) => void;
  export let translationResults: TranslationResultState | null = null;

</script>

<div class="tab-panel active panel-bento-root" data-tab-panel={activeTab}>
  {#if activeTab === "translation"}

    <TranslationPanel {config} {translationResults} {layoutMode} onChange={onChange} />
  {:else if activeTab === "subtitles"}

    <SubtitlesPanel {config} onChange={onChange} />
  {:else if activeTab === "style"}

    <StylePanel
      {config}
      {subtitleStylePresets}
      {fontCatalog}
      {runtime}
      {overlayPayload}
      onChange={onChange}
    />
  {:else if activeTab === "theme"}

    <ThemePanel {config} {fontCatalog} onChange={onChange} />
  {:else if activeTab === "obs"}

    <ObsPanel
      {overlayUrl}
      {config}
      obsDiagnostics={diagnostics.obs}
      onChange={onChange}
    />
  {:else if activeTab === "replacement"}

    <ReplacementPanel {config} onChange={onChange} />
  {:else if activeTab === "tools"}

    <ToolsPanel {config} {diagnostics} {onChange} onConfigLoad={onConfigLoad} />
  {:else if activeTab === "settings"}

    <SettingsPanel {config} {fontCatalog} onChange={onChange} onFontCatalogChange={onFontCatalogChange} />
  {:else if activeTab === "help"}

    <HelpPanel />
  {/if}

</div>
