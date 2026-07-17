<script lang="ts">
  import { onMount } from "svelte";
  import { locale, t } from "../i18n";
  import { openLocalAsrModule, openTtsModule } from "../api";
  import { apiFetch } from "../loopback-api-client";
  import { appStore } from "../stores/app";
  import {
    formatLocalAsrModelLabel,
    isLocalAsrCudaProvider,
    readLocalModuleBadgeSnapshot,
    mergeLocalModuleBadgeSnapshots,
  } from "../local-asr-labels";

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string | number>) => t(key, vars, loc);

  let ttsBusy = false;
  let asrBusy = false;
  let ttsStatus = "";
  let asrStatus = "";
  let ttsError = "";
  let asrError = "";

  let apiSnapshot = readLocalModuleBadgeSnapshot(null);

  $: runtimeSnapshot = readLocalModuleBadgeSnapshot(
    ($appStore.runtime?.asr?.local_module as Record<string, unknown> | undefined) ?? null,
  );
  $: asrSnapshot = mergeLocalModuleBadgeSnapshots(runtimeSnapshot, apiSnapshot);
  $: asrReady = Boolean(asrSnapshot?.ready);
  $: asrPhase = asrSnapshot?.phase ?? "";
  $: asrUsesCuda = isLocalAsrCudaProvider(asrSnapshot?.executionProvider);
  $: asrModelLabel = formatLocalAsrModelLabel(
    tr,
    asrSnapshot?.activeModelFamily,
    asrSnapshot?.activeModelVariant,
  );

  onMount(() => {
    void refreshAsrBadge();
  });

  async function refreshAsrBadge() {
    try {
      const res = await apiFetch("/api/asr/local/status");
      if (!res.ok) return;
      const body = (await res.json()) as { status?: Record<string, unknown> };
      apiSnapshot = readLocalModuleBadgeSnapshot(body.status ?? null);
    } catch {
      // dashboard may run before token bootstrap
    }
  }

  function badgeLabel(): string {
    if (!asrReady) return tr("tools.local_asr.badge.setup");
    return asrUsesCuda
      ? tr("tools.local_asr.badge.cuda_ready")
      : tr("tools.local_asr.badge.cpu_ready");
  }

  async function handleOpenTts() {
    ttsBusy = true;
    ttsStatus = "";
    ttsError = "";
    try {
      await openTtsModule();
      ttsStatus = tr("tools.tts.opened");
    } catch (err) {
      ttsError = err instanceof Error ? err.message : String(err);
    } finally {
      ttsBusy = false;
    }
  }

  async function handleOpenLocalAsr() {
    asrBusy = true;
    asrStatus = "";
    asrError = "";
    try {
      await openLocalAsrModule();
      asrStatus = tr("tools.local_asr.opened");
      await refreshAsrBadge();
    } catch (err) {
      asrError = err instanceof Error ? err.message : String(err);
    } finally {
      asrBusy = false;
    }
  }

  function phaseLabel(phase: string): string {
    if (!phase) return "";
    return tr(`tools.local_asr.phase.${phase}`);
  }
</script>

<section class="modules-layout stack">
  <div class="section-heading">
    <div>
      <p class="eyebrow">{tr("nav.modules")}</p>
      <h2>{tr("nav.modules.title")}</h2>
      <p class="muted">{tr("nav.modules.hint")}</p>
    </div>
  </div>

  <article class="surface-card panel-padding modules-card stack">
    <div class="section-heading section-heading--stacked">
      <p class="eyebrow">{tr("tools.tts.eyebrow")}</p>
      <h3>{tr("tools.tts.title")}</h3>
    </div>
    <p class="muted">{tr("tools.tts.description")}</p>
    <div class="modules-action-row">
      <button class="btn btn-sm" disabled={ttsBusy} on:click={handleOpenTts}>{tr("tools.tts.open")}</button>
    </div>
    {#if ttsStatus}
      <p class="muted modules-status">{ttsStatus}</p>
    {/if}
    {#if ttsError}
      <p class="modules-error" role="alert">{ttsError}</p>
    {/if}
  </article>

  <article class="surface-card panel-padding modules-card stack">
    <div class="section-heading section-heading--stacked">
      <p class="eyebrow">{tr("tools.local_asr.eyebrow")}</p>
      <h3>{tr("tools.local_asr.title")}</h3>
    </div>
    <p class="muted">{tr("tools.local_asr.description")}</p>
    <p
      class="modules-badge"
      class:modules-badge--ready={asrReady}
      class:modules-badge--cuda={asrReady && asrUsesCuda}
    >
      {badgeLabel()}
      {#if asrReady}
        <span class="muted"> · {asrModelLabel}</span>
      {/if}
      {#if asrPhase}
        <span class="muted"> · {phaseLabel(asrPhase)}</span>
      {/if}
    </p>
    <div class="modules-action-row">
      <button class="btn btn-sm" disabled={asrBusy} on:click={handleOpenLocalAsr}>
        {tr("tools.local_asr.open")}
      </button>
    </div>
    {#if asrStatus}
      <p class="muted modules-status">{asrStatus}</p>
    {/if}
    {#if asrError}
      <p class="modules-error" role="alert">{asrError}</p>
    {/if}
  </article>
</section>

<style>
  .modules-layout {
    gap: var(--space-3);
  }

  .modules-card {
    max-width: 560px;
  }

  .modules-action-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .modules-status {
    margin: 0;
    font-size: 13px;
  }

  .modules-error {
    margin: 0;
    color: var(--danger);
    font-size: 13px;
  }

  .modules-badge {
    margin: 0;
    font-size: 13px;
    color: var(--muted-fg, #888);
  }

  .modules-badge--ready {
    color: var(--success, #3d9970);
  }

  .modules-badge--cuda {
    color: color-mix(in srgb, var(--success, #3d9970) 72%, #5b9bd5);
  }
</style>
