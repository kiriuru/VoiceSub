<script lang="ts">
  import { locale, t } from "../i18n";
  import BottomNav from "./BottomNav.svelte";
  import RuntimeBar from "./RuntimeBar.svelte";
  import RuntimeMiniStrip from "./RuntimeMiniStrip.svelte";
  import OverviewSection from "./OverviewSection.svelte";
  import DashboardPanels from "./DashboardPanels.svelte";
  import MoreSettingsHub from "./MoreSettingsHub.svelte";
  import SubtitlesSettingsHub from "./SubtitlesSettingsHub.svelte";
  import ModulesPanel from "../panels/ModulesPanel.svelte";
  import PanelTopNavLayout from "./PanelTopNavLayout.svelte";
  import ScrollToTopFab from "./ScrollToTopFab.svelte";
  import {
    HELP_PANEL_SECTIONS,
    TRANSLATION_PANEL_SECTIONS,
    SUBTITLES_PANEL_SECTIONS,
  } from "../panel-sections";
  import {
    isMoreTab,
    navDestinationTitleKey,
    shouldShowMoreHub,
    shouldShowSubtitlesHub,
    tabTitleKey,
    type NavDestinationId,
  } from "../navigation";
  import type {
    CompactPaneId,
    ConfigPayload,
    DiagnosticsSnapshot,
    FontCatalog,
    RuntimeStatus,
    StylePresetCatalog,
    TabId,
    TranscriptState,
    TranslationResultState,
  } from "../types";
  import ArrowLeft from "lucide-svelte/icons/arrow-left";

  export let compactNav: CompactPaneId = "live";
  export let moreHubOpen = true;
  export let subtitlesHubOpen = false;
  export let activeTab: TabId;
  export let version: string;
  export let config: ConfigPayload;
  export let runtime: RuntimeStatus;
  export let wsConnected = false;
  export let busy: boolean;
  export let transcript: TranscriptState;
  export let overlayPayload: Record<string, unknown> | null;
  export let subtitleStylePresets: StylePresetCatalog;
  export let diagnostics: DiagnosticsSnapshot;
  export let fontCatalog: FontCatalog | null;
  export let translationResults: TranslationResultState | null;
  export let overlayUrl: string;

  export let onSelectNav: (dest: NavDestinationId) => void;
  export let onSelectMoreTab: (tab: TabId) => void;
  export let onSelectSubtitlesTab: (tab: TabId) => void;
  export let onActiveTabChange: (tab: TabId) => void = () => {};
  export let onOpenMoreHub: () => void;
  export let onStart: () => void;
  export let onStop: () => void;
  export let onSave: () => void;
  export let onOpenCommandPalette: () => void;
  export let onConfigChange: (next: ConfigPayload) => void;
  export let onConfigLoad: (next: ConfigPayload) => void;
  export let onFontCatalogChange: (catalog: FontCatalog) => void;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
  $: showMoreHub = shouldShowMoreHub(compactNav, moreHubOpen);
  $: showSubtitlesHub = shouldShowSubtitlesHub(compactNav, subtitlesHubOpen);
  $: running = Boolean(runtime.running || runtime.is_running);
  $: screenTitle = showMoreHub
    ? tr("nav.more")
    : showSubtitlesHub
      ? tr("nav.subtitles")
      : compactNav === "more" && isMoreTab(activeTab)
        ? tr(tabTitleKey(activeTab))
        : compactNav === "subtitles"
          ? tr(navDestinationTitleKey("subtitles"))
          : tr(navDestinationTitleKey(compactNav));
  $: showBack = compactNav === "more" && !showMoreHub;

  $: translationSections = TRANSLATION_PANEL_SECTIONS.filter(
    (section) => section.id !== "translation-section-results" || translationResults,
  );
  $: useTranslationTopNav = compactNav === "translation";
  $: useSubtitlesTopNav = compactNav === "subtitles" && !showSubtitlesHub;
  $: useHelpTopNav = compactNav === "more" && !showMoreHub && activeTab === "help";
  $: topNavSections = useTranslationTopNav
    ? translationSections
    : useSubtitlesTopNav
      ? SUBTITLES_PANEL_SECTIONS
      : useHelpTopNav
        ? HELP_PANEL_SECTIONS
        : [];
  $: usePanelTopNav = useTranslationTopNav || useSubtitlesTopNav || useHelpTopNav;
  $: panelNavMode = (useSubtitlesTopNav ? "tabs" : "scroll") as "scroll" | "tabs";
  $: panelNavAriaKey = useHelpTopNav ? "help.jump.aria" : "nav.section.jump";

  function handleSubtitlesTabChange(tab: TabId) {
    onActiveTabChange(tab);
    compactScrollEl?.scrollTo({ top: 0, behavior: "auto" });
  }

  function handleBack() {
    onOpenMoreHub();
  }

  let compactScrollEl: HTMLElement | null = null;
  $: scrollToTopBottom =
    compactNav === "live"
      ? "calc(var(--bottom-nav-height) + var(--fab-size) + 16px)"
      : "calc(var(--bottom-nav-height) + 12px)";
</script>

<div class="compact-phone-shell compact-fab-anchor">
  <header class="compact-top-bar">
    <div class="compact-top-row">
      <div class="compact-brand">
        {#if showBack}
          <button
            type="button"
            class="btn btn-ghost compact-icon-btn"
            aria-label={tr("nav.back")}
            on:click={handleBack}
          >
            <ArrowLeft size={18} strokeWidth={1.75} />
          </button>
        {/if}
        <span class="compact-brand-title">{screenTitle}</span>
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
  </header>

  {#if compactNav !== "live" && !showMoreHub && !showSubtitlesHub}
    <RuntimeMiniStrip {runtime} obsDiagnostics={diagnostics.obs} {wsConnected} />
  {/if}

  <main class="compact-content-scroll" bind:this={compactScrollEl}>
    {#if compactNav === "live"}
      <RuntimeBar
        {runtime}
        obsDiagnostics={diagnostics.obs}
        {wsConnected}
        {busy}
        variant="compact"
        {onStart}
        {onStop}
      />
      <OverviewSection
        class="compact-overview"
        {transcript}
        {overlayPayload}
        {config}
        {runtime}
        {subtitleStylePresets}
        onConfigChange={onConfigChange}
      />
    {:else if showMoreHub}
      <MoreSettingsHub onSelect={onSelectMoreTab} />
    {:else if showSubtitlesHub}
      <SubtitlesSettingsHub onSelect={onSelectSubtitlesTab} />
    {:else if compactNav === "modules"}
      <ModulesPanel />
    {:else if usePanelTopNav}
      <PanelTopNavLayout
        sections={topNavSections}
        ariaLabelKey={panelNavAriaKey}
        {activeTab}
        navMode={panelNavMode}
        onActiveTabChange={useSubtitlesTopNav ? handleSubtitlesTabChange : onActiveTabChange}
      >
        <DashboardPanels
          {activeTab}
          {config}
          {diagnostics}
          {subtitleStylePresets}
          {fontCatalog}
          {translationResults}
          {overlayUrl}
          layoutMode="compact"
          {runtime}
          {overlayPayload}
          onChange={onConfigChange}
          onConfigLoad={onConfigLoad}
          onFontCatalogChange={onFontCatalogChange}
        />
      </PanelTopNavLayout>
    {:else}
      <DashboardPanels
        {activeTab}
        {config}
        {diagnostics}
        {subtitleStylePresets}
        {fontCatalog}
        {translationResults}
        {overlayUrl}
        layoutMode="compact"
        {runtime}
        {overlayPayload}
        onChange={onConfigChange}
        onConfigLoad={onConfigLoad}
        onFontCatalogChange={onFontCatalogChange}
      />
    {/if}
  </main>

  <ScrollToTopFab scrollRoot={compactScrollEl} bottom={scrollToTopBottom} />

  {#if compactNav === "live"}
    {#if running}
      <button type="button" class="compact-fab compact-fab--stop" disabled={busy} on:click={onStop}>
        {tr("common.stop")}
      </button>
    {:else}
      <button type="button" class="compact-fab" disabled={busy} on:click={onStart}>
        {busy ? tr("common.starting") : tr("common.start")}
      </button>
    {/if}
  {/if}

  <BottomNav active={compactNav} onSelect={onSelectNav} />
</div>

<style>
  .compact-brand {
    align-items: center;
  }

  .compact-brand-title {
    font-size: 15px;
    font-weight: 600;
  }
</style>

