<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { get } from "svelte/store";
  import { locale, t, setLocale } from "./lib/i18n";
  import { UI_LOCALES } from "./lib/constants";
  import {
    downloadDiagnostics,
    fetchObsUrl,
    fetchRuntimeStatus,
    checkUpdates,
    openExternalUrl,
    fetchVersion,
    postClientLog,
    loadSettings,
    saveSettings,
    startRuntime,
    stopRuntime,
  } from "./lib/api";
  import CommandPalette from "./lib/components/CommandPalette.svelte";
  import SaveSnackbar from "./lib/components/SaveSnackbar.svelte";
  import UpdateBanner from "./lib/components/UpdateBanner.svelte";
  import CompactShell from "./lib/components/CompactShell.svelte";
  import StandardShell from "./lib/components/StandardShell.svelte";
  import { applyDashboardWindowSize } from "./lib/compact-window";
  import type { CommandPaletteHandlers } from "./lib/command-palette";
  import { isMoreTab, tabToNavDestination } from "./lib/navigation";
  import type { NavDestinationId, NavTarget } from "./lib/navigation";
  import { applyUiPaletteToDocument } from "./lib/ui-theme-css";
  import { publishUiConfigSync } from "./lib/ui-config-sync";
  import { isUpdateBannerDismissedForVersion, shouldShowUpdateBanner } from "./lib/update-banner-state";
  import { getRestartRequiredReasons } from "./lib/config-restart";
  import {
    formatTranslationConfigError,
    getTranslationConfigErrors,
  } from "./lib/translation-helpers";
  import { normalizeConfigPayload } from "./lib/config-normalize";
  import { mergeFontCatalogPreservingSystem } from "./lib/font-catalog";
  import { mergeStylePresetCatalog } from "./lib/style-presets";
  import { appStore, handleWsEvent, patchApp } from "./lib/stores/app";
  import { initLoopbackApiToken } from "./lib/loopback-api";
  import { startRuntimeEventChannel } from "./lib/runtime-events";
  import { EventsSocket } from "./lib/ws";
  import type { ConfigPayload, LocaleCode, TabId, VersionInfo } from "./lib/types";

  const UPDATE_BANNER_DISMISS_KEY = "voicesub:update-banner-dismissed";

  let snapshot = get(appStore);
  const unsubscribe = appStore.subscribe((value) => {
    snapshot = value;
  });

  $: loc = $locale;
  $: isCompact = snapshot.config.ui?.layout === "compact";
  $: tr = (key: string) => t(key, undefined, loc);
  $: overlayUrl = snapshot.overlayUrl || snapshot.runtime.overlay?.overlay_url || "";

  let socketUnlisten: (() => void) | null = null;
  let eventsSocket: EventsSocket | null = null;
  let compactNav: NavDestinationId = "live";
  let standardNav: NavDestinationId = "live";
  let moreHubOpen = false;
  let subtitlesHubOpen = false;
  let commandPaletteOpen = false;
  /** Last config persisted to disk — baseline for restart-required diff on save. */
  let lastSavedConfig: ConfigPayload | null = null;

  const commandPaletteHandlers: CommandPaletteHandlers = {
    navigate: (target) => navigateTo(target),
    start: () => handleStart(),
    stop: () => handleStop(),
    save: () => handleSave(),
    toggleTheme: () => {
      const ui = snapshot.config.ui || {};
      const nextTheme = ui.theme === "light" ? "dark" : "light";
      updateConfig({ ...snapshot.config, ui: { ...ui, theme: nextTheme } });
    },
    toggleLayout: () => {
      const ui = snapshot.config.ui || {};
      const nextLayout = ui.layout === "compact" ? "standard" : "compact";
      if (nextLayout === "compact") {
        compactNav = "live";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      }
      updateConfig({ ...snapshot.config, ui: { ...ui, layout: nextLayout } });
    },
    exportDiagnostics: () => downloadDiagnostics(),
    onError: (message) => patchApp({ saveStatus: { tone: "error", message } }),
  };

  function applyUiFromConfig(config: ConfigPayload) {
    const uiLang = config.ui?.language;
    if (uiLang && UI_LOCALES.some((l) => l.code === uiLang)) {
      setLocale(uiLang as (typeof UI_LOCALES)[number]["code"]);
    }
    const theme = config.ui?.theme === "light" ? "light" : "dark";
    document.documentElement.dataset.uiTheme = theme;
    const compact = config.ui?.layout === "compact";
    document.body.classList.toggle("voicesub-layout-compact", compact);
    void applyDashboardWindowSize(compact);
    if (!compact) {
      document.body.classList.remove("compact-nav-expanded");
    }
    const palette = config.ui?.palette;
    if (palette) {
      applyUiPaletteToDocument(palette);
    }
    publishUiConfigSync(config);
  }

  function readDismissedVersion(): string {
    try {
      return sessionStorage.getItem(UPDATE_BANNER_DISMISS_KEY) || "";
    } catch {
      return "";
    }
  }

  function dismissUpdateBanner(latestVersion: string) {
    try {
      sessionStorage.setItem(UPDATE_BANNER_DISMISS_KEY, latestVersion);
    } catch {
      // ignore storage errors
    }
    patchApp({ updateBannerDismissed: true });
  }

  function applyVersionInfo(versionInfo: VersionInfo) {
    const currentVersion =
      versionInfo.current_version || versionInfo.version || snapshot.version;
    const latest = versionInfo.sync?.latest_known_version || "";
    const dismissedFor = readDismissedVersion();
    patchApp({
      version: currentVersion,
      versionInfo,
      updateBannerDismissed: isUpdateBannerDismissedForVersion(latest, dismissedFor),
    });
  }

  async function refreshUpdateCheck() {
    try {
      const versionInfo = await checkUpdates();
      applyVersionInfo(versionInfo);
    } catch (err) {
      void postClientLog("dashboard", "update check failed", {
        error: err instanceof Error ? err.message : String(err),
      });
      try {
        const versionInfo = await fetchVersion();
        applyVersionInfo(versionInfo);
      } catch {
        // backend may still be starting
      }
    }
  }

  async function bootstrap() {
    try {
      await initLoopbackApiToken();
      const [settings, version, obs, runtime] = await Promise.all([
        loadSettings(),
        fetchVersion(),
        fetchObsUrl(),
        fetchRuntimeStatus(),
      ]);
      const loadedConfig = normalizeConfigPayload(settings.payload);
      lastSavedConfig = structuredClone(loadedConfig);
      applyVersionInfo(version);
      patchApp({
        config: loadedConfig,
        overlayUrl: obs.overlay_url,
        runtime,
        subtitleStylePresets: settings.subtitle_style_presets || {},
        fontCatalog: settings.font_catalog
          ? mergeFontCatalogPreservingSystem(settings.font_catalog, snapshot.fontCatalog)
          : snapshot.fontCatalog,
        saveStatus: { tone: "default" },
      });
      applyUiFromConfig(loadedConfig);
      await refreshUpdateCheck();
    } catch (err) {
      patchApp({
        saveStatus: {
          tone: "error",
          message: err instanceof Error ? err.message : String(err),
        },
      });
    }
  }

  async function persistUiLanguage(lang: LocaleCode) {
    const inMemoryConfig = normalizeConfigPayload({
      ...snapshot.config,
      ui: { ...(snapshot.config.ui || {}), language: lang },
    });
    setLocale(lang);
    patchApp({ config: inMemoryConfig });
    applyUiFromConfig(inMemoryConfig);

    const baseline = lastSavedConfig ?? inMemoryConfig;
    const persistPayload = normalizeConfigPayload({
      ...baseline,
      ui: { ...(baseline.ui || {}), language: lang },
    });

    try {
      const res = await saveSettings(persistPayload);
      if (res.ok) {
        const saved = res.payload ? normalizeConfigPayload(res.payload) : persistPayload;
        lastSavedConfig = structuredClone(saved);
      }
    } catch {
      // keep in-memory locale; user can retry via Save
    }
  }

  function handleUiLanguageChange(lang: LocaleCode) {
    void persistUiLanguage(lang);
  }

  async function handleSave() {
    const validationErrors = getTranslationConfigErrors(snapshot.config);
    if (validationErrors.length > 0) {
      patchApp({
        saveStatus: {
          tone: "error",
          message: formatTranslationConfigError(validationErrors[0] ?? "", (key, vars) =>
            t(key, vars, loc),
          ),
        },
      });
      return;
    }

    patchApp({ busy: true, saveStatus: { tone: "busy" } });
    try {
      const res = await saveSettings(normalizeConfigPayload(snapshot.config));
      const nextConfig = res.payload ? normalizeConfigPayload(res.payload) : snapshot.config;
      const restartReasonKeys = getRestartRequiredReasons(
        lastSavedConfig ?? nextConfig,
        nextConfig,
      );
      patchApp({
        busy: false,
        config: nextConfig,
        subtitleStylePresets: res.subtitle_style_presets || snapshot.subtitleStylePresets,
        fontCatalog: res.font_catalog
          ? mergeFontCatalogPreservingSystem(res.font_catalog, snapshot.fontCatalog)
          : snapshot.fontCatalog,
        saveStatus: res.ok
          ? {
              tone: restartReasonKeys.length ? "warn" : "success",
              liveApplied: Boolean(res.live_applied),
              restartReasonKeys,
            }
          : {
              tone: "error",
              message: res.message || t("common.error", undefined, loc),
            },
      });
      applyUiFromConfig(nextConfig);
      if (res.ok) {
        lastSavedConfig = structuredClone(nextConfig);
      }
    } catch (err) {
      patchApp({
        busy: false,
        saveStatus: {
          tone: "error",
          message: err instanceof Error ? err.message : String(err),
        },
      });
    }
  }

  async function handleStart() {
    patchApp({ busy: true });
    try {
      const res = await startRuntime(normalizeConfigPayload(snapshot.config));
      patchApp({ runtime: res.runtime, busy: false });
    } catch (err) {
      patchApp({
        busy: false,
        saveStatus: { tone: "error", message: err instanceof Error ? err.message : String(err) },
      });
    }
  }

  async function handleStop() {
    patchApp({ busy: true });
    try {
      const res = await stopRuntime();
      patchApp({
        runtime: res.runtime,
        busy: false,
        transcript: { partial: "", finals: [] },
        translation: { current: null, history: [] },
        overlayPayload: null,
      });
    } catch (err) {
      patchApp({
        busy: false,
        saveStatus: { tone: "error", message: err instanceof Error ? err.message : String(err) },
      });
    }
  }

  function updateConfig(next: ConfigPayload) {
    const wasCompact = snapshot.config.ui?.layout === "compact";
    const nextCompact = next.ui?.layout === "compact";
    if (nextCompact && !wasCompact) {
      compactNav = "live";
      moreHubOpen = false;
      subtitlesHubOpen = false;
    }
    patchApp({
      config: next,
      subtitleStylePresets: mergeStylePresetCatalog(
        snapshot.subtitleStylePresets,
        next.subtitle_style as Record<string, unknown> | undefined,
      ),
    });
    applyUiFromConfig(next);
  }

  function loadConfigFromTools(next: ConfigPayload) {
    const normalized = normalizeConfigPayload(next);
    patchApp({
      config: normalized,
      subtitleStylePresets: mergeStylePresetCatalog(snapshot.subtitleStylePresets, normalized.subtitle_style),
    });
    applyUiFromConfig(normalized);
  }

  function updateFontCatalog(catalog: import("./lib/types").FontCatalog) {
    patchApp({ fontCatalog: catalog });
  }

  function patchActiveTab(tab: TabId) {
    patchApp({ activeTab: tab });
  }

  function navigateTo(target: NavTarget) {
    if (target === "live") {
      if (isCompact) {
        compactNav = "live";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      } else {
        standardNav = "live";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      }
      return;
    }

    if (target === "modules") {
      if (isCompact) {
        compactNav = "modules";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      } else {
        standardNav = "modules";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      }
      return;
    }

    const tab = target as TabId;
    if (tab === "style" || tab === "subtitles") {
      if (isCompact) {
        compactNav = "subtitles";
        subtitlesHubOpen = false;
      } else {
        standardNav = "subtitles";
        subtitlesHubOpen = false;
      }
      moreHubOpen = false;
      patchActiveTab(tab);
      return;
    }

    if (isMoreTab(tab)) {
      if (isCompact) {
        compactNav = "more";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      } else {
        standardNav = "more";
        moreHubOpen = false;
        subtitlesHubOpen = false;
      }
      patchActiveTab(tab);
      return;
    }

    const dest = tabToNavDestination(tab);
    if (isCompact) {
      compactNav = dest;
      moreHubOpen = false;
      subtitlesHubOpen = false;
    } else {
      standardNav = dest;
      moreHubOpen = false;
      subtitlesHubOpen = false;
    }
    patchActiveTab(tab);
  }

  function selectStandardNav(dest: NavDestinationId) {
    standardNav = dest;
    if (dest === "live") {
      moreHubOpen = false;
      subtitlesHubOpen = false;
      return;
    }
    if (dest === "more") {
      moreHubOpen = true;
      subtitlesHubOpen = false;
      return;
    }
    if (dest === "subtitles") {
      moreHubOpen = false;
      subtitlesHubOpen = false;
      patchActiveTab("subtitles");
      return;
    }
    if (dest === "modules") {
      moreHubOpen = false;
      subtitlesHubOpen = false;
      return;
    }
    moreHubOpen = false;
    subtitlesHubOpen = false;
    if (dest === "translation") patchActiveTab("translation");
    else if (dest === "obs") patchActiveTab("obs");
  }

  function selectCompactNav(dest: NavDestinationId) {
    compactNav = dest;
    if (dest === "live") {
      moreHubOpen = false;
      subtitlesHubOpen = false;
      return;
    }
    if (dest === "more") {
      moreHubOpen = true;
      subtitlesHubOpen = false;
      return;
    }
    if (dest === "subtitles") {
      moreHubOpen = false;
      subtitlesHubOpen = false;
      patchActiveTab("subtitles");
      return;
    }
    if (dest === "modules") {
      moreHubOpen = false;
      subtitlesHubOpen = false;
      return;
    }
    moreHubOpen = false;
    subtitlesHubOpen = false;
    if (dest === "translation") patchActiveTab("translation");
    else if (dest === "obs") patchActiveTab("obs");
  }

  function selectMoreTab(tab: TabId) {
    moreHubOpen = false;
    if (tab === "style") {
      if (isCompact) compactNav = "subtitles";
      else standardNav = "subtitles";
      subtitlesHubOpen = false;
      patchActiveTab("style");
      return;
    }
    subtitlesHubOpen = false;
    if (isCompact) compactNav = "more";
    else standardNav = "more";
    patchActiveTab(tab);
  }

  function selectSubtitlesTab(tab: TabId) {
    subtitlesHubOpen = false;
    if (isCompact) compactNav = "subtitles";
    else standardNav = "subtitles";
    patchActiveTab(tab);
  }

  function openMoreHub() {
    moreHubOpen = true;
  }

  function openSubtitlesHub() {
    subtitlesHubOpen = true;
  }

  function dismissSaveSnackbar() {
    if (snapshot.saveStatus.tone !== "default") {
      patchApp({ saveStatus: { tone: "default" } });
    }
  }

  onMount(() => {
    void bootstrap();
    void startRuntimeEventChannel(() => patchApp({ wsConnected: true })).then((unlisten) => {
      if (unlisten) {
        socketUnlisten = unlisten;
        return;
      }
      eventsSocket = new EventsSocket(handleWsEvent, (status) => {
        patchApp({ wsConnected: status === "connected" });
      });
      eventsSocket.connect();
      socketUnlisten = () => {
        eventsSocket?.disconnect();
        eventsSocket = null;
      };
    });

    const poll = window.setInterval(async () => {
      try {
        const runtime = await fetchRuntimeStatus();
        patchApp({ runtime });
      } catch {
        // server may be restarting
      }
    }, 4000);

    return () => {
      window.clearInterval(poll);
    };
  });

  onDestroy(() => {
    socketUnlisten?.();
    eventsSocket = null;
    unsubscribe();
    document.body.classList.remove("voicesub-layout-compact", "compact-nav-expanded");
  });
</script>

<SaveSnackbar
  saveStatus={snapshot.saveStatus}
  runtime={snapshot.runtime}
  onDismiss={dismissSaveSnackbar}
/>

<CommandPalette bind:open={commandPaletteOpen} handlers={commandPaletteHandlers} />

<UpdateBanner
  versionInfo={snapshot.versionInfo}
  visible={shouldShowUpdateBanner(snapshot.versionInfo, snapshot.updateBannerDismissed)}
  onClose={() => {
    const latest = snapshot.versionInfo?.sync?.latest_known_version || "";
    if (latest) dismissUpdateBanner(latest);
    else patchApp({ updateBannerDismissed: true });
  }}
  onDownload={(url) => {
    void openExternalUrl(url);
  }}
/>

{#key $locale}
<main class="app-shell">
  {#if isCompact}
    <CompactShell
      compactNav={compactNav}
      {moreHubOpen}
      {subtitlesHubOpen}
      activeTab={snapshot.activeTab}
      version={snapshot.version}
      config={snapshot.config}
      runtime={snapshot.runtime}
      wsConnected={snapshot.wsConnected}
      busy={snapshot.busy}
      transcript={snapshot.transcript}
      overlayPayload={snapshot.overlayPayload}
      subtitleStylePresets={snapshot.subtitleStylePresets}
      diagnostics={snapshot.diagnostics}
      fontCatalog={snapshot.fontCatalog}
      translationResults={snapshot.translation}
      {overlayUrl}
      onSelectNav={selectCompactNav}
      onSelectMoreTab={selectMoreTab}
      onSelectSubtitlesTab={selectSubtitlesTab}
      onActiveTabChange={patchActiveTab}
      onOpenMoreHub={openMoreHub}
      onOpenSubtitlesHub={openSubtitlesHub}
      onStart={handleStart}
      onStop={handleStop}
      onSave={handleSave}
      onOpenCommandPalette={() => {
        commandPaletteOpen = true;
      }}
      onConfigChange={updateConfig}
      onConfigLoad={loadConfigFromTools}
      onFontCatalogChange={updateFontCatalog}
    />
  {:else}
    <StandardShell
      {standardNav}
      {moreHubOpen}
      {subtitlesHubOpen}
      activeTab={snapshot.activeTab}
      version={snapshot.version}
      config={snapshot.config}
      runtime={snapshot.runtime}
      wsConnected={snapshot.wsConnected}
      busy={snapshot.busy}
      transcript={snapshot.transcript}
      overlayPayload={snapshot.overlayPayload}
      subtitleStylePresets={snapshot.subtitleStylePresets}
      diagnostics={snapshot.diagnostics}
      fontCatalog={snapshot.fontCatalog}
      translationResults={snapshot.translation}
      {overlayUrl}
      localeCode={$locale}
      onSelectNav={selectStandardNav}
      onSelectMoreTab={selectMoreTab}
      onSelectSubtitlesTab={selectSubtitlesTab}
      onActiveTabChange={patchActiveTab}
      onOpenMoreHub={openMoreHub}
      onOpenSubtitlesHub={openSubtitlesHub}
      onStart={handleStart}
      onStop={handleStop}
      onSave={handleSave}
      onOpenCommandPalette={() => {
        commandPaletteOpen = true;
      }}
      onConfigChange={updateConfig}
      onConfigLoad={loadConfigFromTools}
      onFontCatalogChange={updateFontCatalog}
      onLanguageChange={handleUiLanguageChange}
    />
  {/if}
</main>
{/key}

<style>
  :global(body) {
    margin: 0;
  }
</style>
