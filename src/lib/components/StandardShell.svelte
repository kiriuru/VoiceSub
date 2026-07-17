<script lang="ts">
  import NavRail from "./NavRail.svelte";
  import TopAppBar from "./TopAppBar.svelte";
  import RuntimeBar from "./RuntimeBar.svelte";
  import OverviewSection from "./OverviewSection.svelte";
  import DashboardPanels from "./DashboardPanels.svelte";
  import MoreSettingsHub from "./MoreSettingsHub.svelte";
  import SubtitlesSettingsHub from "./SubtitlesSettingsHub.svelte";
  import ModulesPanel from "../panels/ModulesPanel.svelte";
  import PanelTopNavLayout from "./PanelTopNavLayout.svelte";
  import ScrollToTopFab from "./ScrollToTopFab.svelte";
  import { TRANSLATION_PANEL_SECTIONS, SUBTITLES_PANEL_SECTIONS } from "../panel-sections";
  import { locale, t } from "../i18n";
  import {
    isMoreTab,
    isSubtitlesTab,
    navDestinationTitleKey,
    shouldShowMoreHub,
    shouldShowSubtitlesHub,
    tabTitleKey,
    type NavDestinationId,
  } from "../navigation";
  import type {
    ConfigPayload,
    DiagnosticsSnapshot,
    FontCatalog,
    LocaleCode,
    RuntimeStatus,
    StylePresetCatalog,
    TabId,
    TranscriptState,
    TranslationResultState,
  } from "../types";

  export let version = "0.6.0";
  export let standardNav: NavDestinationId = "live";
  export let moreHubOpen = true;
  export let subtitlesHubOpen = true;
  export let activeTab: TabId;
  export let config: ConfigPayload;
  export let runtime: RuntimeStatus;
  export let wsConnected = false;
  export let busy = false;
  export let transcript: TranscriptState;
  export let overlayPayload: Record<string, unknown> | null;
  export let subtitleStylePresets: StylePresetCatalog;
  export let diagnostics: DiagnosticsSnapshot;
  export let fontCatalog: FontCatalog | null;
  export let translationResults: TranslationResultState | null;
  export let overlayUrl: string;
  export let localeCode: LocaleCode;

  export let onSelectNav: (dest: NavDestinationId) => void;
  export let onSelectMoreTab: (tab: TabId) => void;
  export let onSelectSubtitlesTab: (tab: TabId) => void;
  export let onActiveTabChange: (tab: TabId) => void = () => {};
  export let onOpenMoreHub: () => void;
  export let onOpenSubtitlesHub: () => void;
  export let onStart: () => void;
  export let onStop: () => void;
  export let onSave: () => void;
  export let onOpenCommandPalette: () => void;
  export let onConfigChange: (next: ConfigPayload) => void;
  export let onConfigLoad: (next: ConfigPayload) => void;
  export let onFontCatalogChange: (catalog: FontCatalog) => void;
  export let onLanguageChange: (lang: LocaleCode) => void;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
  $: showMoreHub = shouldShowMoreHub(standardNav, moreHubOpen);
  $: showSubtitlesHub = shouldShowSubtitlesHub(standardNav, subtitlesHubOpen);
  $: barTitle = showMoreHub
    ? tr("nav.more")
    : showSubtitlesHub
      ? tr("nav.subtitles")
      : standardNav === "more" && isMoreTab(activeTab)
        ? tr(tabTitleKey(activeTab))
        : standardNav === "subtitles" && isSubtitlesTab(activeTab)
          ? tr(tabTitleKey(activeTab))
          : tr(navDestinationTitleKey(standardNav));
  $: showBack =
    (standardNav === "more" && !showMoreHub) ||
    (standardNav === "subtitles" && !showSubtitlesHub);
  $: translationSections = TRANSLATION_PANEL_SECTIONS.filter(
    (section) => section.id !== "translation-section-results" || translationResults,
  );
  $: useTranslationTopNav = standardNav === "translation";
  $: useSubtitlesTopNav = standardNav === "subtitles" && !showSubtitlesHub;
  $: topNavSections = useTranslationTopNav
    ? translationSections
    : useSubtitlesTopNav
      ? SUBTITLES_PANEL_SECTIONS
      : [];
  $: usePanelTopNav = useTranslationTopNav || useSubtitlesTopNav;
  $: panelNavMode = (useSubtitlesTopNav ? "tabs" : "scroll") as "scroll" | "tabs";

  function handleSubtitlesTabChange(tab: TabId) {
    onActiveTabChange(tab);
    standardScrollEl?.scrollTo({ top: 0, behavior: "auto" });
  }

  function handleBack() {
    if (standardNav === "subtitles") onOpenSubtitlesHub();
    else onOpenMoreHub();
  }

  let standardScrollEl: HTMLDivElement | null = null;
</script>

<div class="app-shell standard-shell">
  <NavRail active={standardNav} onSelect={onSelectNav} />

  <div class="standard-main">
    <TopAppBar
      title={barTitle}
      {showBack}
      onBack={handleBack}
      {busy}
      {localeCode}
      onLanguageChange={onLanguageChange}
      onOpenCommandPalette={onOpenCommandPalette}
      onSave={onSave}
    />

    <div class="standard-content" bind:this={standardScrollEl}>
      <div class="standard-content-inner">
        {#if standardNav === "live"}
          <RuntimeBar
            {runtime}
            obsDiagnostics={diagnostics.obs}
            {wsConnected}
            {busy}
            {onStart}
            {onStop}
          />
          <OverviewSection
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
        {:else if standardNav === "modules"}
          <ModulesPanel />
        {:else if usePanelTopNav}
          <PanelTopNavLayout
            sections={topNavSections}
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
              layoutMode="standard"
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
            layoutMode="standard"
            {runtime}
            {overlayPayload}
            onChange={onConfigChange}
            onConfigLoad={onConfigLoad}
            onFontCatalogChange={onFontCatalogChange}
          />
        {/if}
      </div>
    </div>

    <ScrollToTopFab scrollRoot={standardScrollEl} bottom="var(--space-4)" />

    <footer class="app-footer">
      <span class="app-footer__line">
        VoiceSub <span class="app-footer__version">v{version}</span>
        <span class="app-footer__sep" aria-hidden="true">·</span>
        Powered by Kiriuru
      </span>
    </footer>
  </div>
</div>
