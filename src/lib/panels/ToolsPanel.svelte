<script lang="ts">
  import { onMount } from "svelte";
  import { locale, t } from "../i18n";
  import {
    deleteProfile,
    downloadDiagnostics,
    listProfiles,
    loadProfile,
    saveProfile,
  } from "../api";
  import { normalizeConfigPayload } from "../config-normalize";
  import { formatObsCcRuntimeStatus } from "../obs-status-i18n";
  import { redactObject } from "../redaction";
  import type { ConfigPayload, DiagnosticsSnapshot, StylePresetCatalog } from "../types";

  export let config: ConfigPayload;
  export let diagnostics: DiagnosticsSnapshot;
  export let onChange: (next: ConfigPayload) => void;
  export let onConfigLoad: (next: ConfigPayload, presets?: StylePresetCatalog) => void;

  $: fullLoggingEnabled = config.logging?.full_enabled === true;

  function setFullLoggingEnabled(enabled: boolean) {
    onChange({
      ...config,
      logging: {
        ...(config.logging || {}),
        full_enabled: enabled,
      },
    });
  }

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string>) => t(key, vars, loc);

  let profiles: string[] = [];
  let profileName = "";
  let status = "";
  let error = "";
  let busy = false;

  async function refreshLists() {
    try {
      const profilesRes = await listProfiles();
      profiles = profilesRes.profiles || [];
      error = "";
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  onMount(() => {
    void refreshLists();
  });

  async function handleLoadProfile() {
    const name = profileName.trim();
    if (!name) return;
    busy = true;
    status = "";
    try {
      const res = await loadProfile(name);
      onConfigLoad(normalizeConfigPayload(res.payload));
      status = `${tr("tools.profiles.load")}: ${name}`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function handleSaveProfile() {
    const name = profileName.trim();
    if (!name) return;
    busy = true;
    status = "";
    try {
      await saveProfile(name, config);
      await refreshLists();
      status = `${tr("tools.profiles.save")}: ${name}`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function handleDeleteProfile() {
    const name = profileName.trim();
    if (!name) return;
    const confirmMessage = tr("tools.profiles.delete_confirm").replace("{name}", name);
    if (!globalThis.confirm(confirmMessage)) return;
    busy = true;
    status = "";
    try {
      const res = await deleteProfile(name);
      await refreshLists();
      status = res.deleted ? `${tr("tools.profiles.delete")}: ${name}` : `${name} — ${tr("common.error")}`;
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
  }

  async function handleExportDiagnostics() {
    busy = true;
    status = "";
    try {
      await downloadDiagnostics();
      status = tr("tools.config.export_diagnostics");
    } catch (err) {
      error = err instanceof Error ? err.message : tr("tools.runtime.export_failed");
    } finally {
      busy = false;
    }
  }
  function formatMetric(value: unknown): string {
    if (value === null || value === undefined || value === "") return "—";
    const n = Number(value);
    if (Number.isFinite(n)) return `${Math.round(n)} ms`;
    return String(value);
  }

  function metricValue(
    key: string,
    metrics: Record<string, unknown>,
    translation: Record<string, unknown>,
  ): unknown {
    if (metrics[key] !== undefined && metrics[key] !== null) return metrics[key];
    if (translation[key] !== undefined && translation[key] !== null) return translation[key];
    return undefined;
  }

  function metricLabel(value: unknown): string {
    if (value === null || value === undefined || value === "") return "—";
    return String(value);
  }

  $: translationMetrics = (diagnostics.metrics || {}) as Record<string, unknown>;
  $: translationDiag = (diagnostics.translation || {}) as Record<string, unknown>;

  function handleExportConfig() {
    const blob = new Blob([JSON.stringify(redactObject(config), null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = "voicesub-config.json";
    anchor.click();
    URL.revokeObjectURL(url);
    status = tr("tools.config.export");
  }

  async function handleImportConfig(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    busy = true;
    status = "";
    try {
      const text = await file.text();
      const payload = normalizeConfigPayload(JSON.parse(text) as ConfigPayload);
      onConfigLoad(payload);
      status = tr("tools.config.import");
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
      input.value = "";
    }
  }
</script>

<section class="tools-layout bento-root stack">
  <div class="section-heading bento-span-full">
    <div>
      <p class="eyebrow">{tr("tab.tools")}</p>
      <h2>{tr("help.tools.title")}</h2>
    </div>
  </div>

  <div class="tools-bento bento-grid">
  <article class="surface-card panel-padding bento-tile stack">
  <div class="section-heading section-heading--stacked">
    <p class="eyebrow">{tr("tools.profiles.eyebrow")}</p>
    <h3>{tr("tools.profiles.title")}</h3>
  </div>

  <label class="stack-field profiles-name-field">
    <span>{tr("tools.profiles.name_label")}</span>
    <input
      class="control"
      placeholder={tr("tools.profiles.placeholder")}
      bind:value={profileName}
      list="profile-names"
    />
    <datalist id="profile-names">
      {#each profiles as name}
        <option value={name}></option>
      {/each}
    </datalist>
  </label>
  <div class="profiles-actions">
    <button class="btn btn-sm" disabled={busy} on:click={handleLoadProfile}>{tr("tools.profiles.load")}</button>
    <button class="btn btn-sm" disabled={busy} on:click={handleSaveProfile}>{tr("tools.profiles.save")}</button>
    <button class="btn btn-sm" disabled={busy} on:click={handleDeleteProfile}>{tr("tools.profiles.delete")}</button>
  </div>
  </article>

  <article class="surface-card panel-padding bento-tile stack">
  <div class="section-heading section-heading--stacked">
    <p class="eyebrow">{tr("tools.config.eyebrow")}</p>
    <h3>{tr("tools.config.title")}</h3>
  </div>

  <div class="tools-action-row">
    <button class="btn btn-sm" disabled={busy} on:click={handleExportDiagnostics}>
      {tr("tools.config.export_diagnostics")}
    </button>
    <button class="btn btn-sm" disabled={busy} on:click={handleExportConfig}>
      {tr("tools.config.export")}
    </button>
    <label class="btn btn-sm tools-file-btn">
      {tr("tools.config.import")}
      <input type="file" accept="application/json,.json" hidden on:change={handleImportConfig} />
    </label>
  </div>
  </article>

  <article class="surface-card panel-padding bento-tile bento-span-full stack">
  <div class="section-heading section-heading--stacked">
    <p class="eyebrow">{tr("tools.runtime.eyebrow")}</p>
    <h3>{tr("tools.runtime.title")}</h3>
  </div>

  <p class="muted mono-block">
    ASR: {String(diagnostics.asr?.provider || "n/a")}
    · worker: {diagnostics.asr?.browser_worker && (diagnostics.asr.browser_worker as Record<string, unknown>).worker_connected ? tr("common.connected") : tr("common.disconnected")}
  </p>
  <p class="muted mono-block">
    Translation: {String(
      diagnostics.metrics?.translation_last_provider ||
        diagnostics.translation?.translation_last_provider ||
        "idle",
    )}
    · queue: {String(diagnostics.metrics?.translation_queue_depth ?? 0)}
    · queue latency: {formatMetric(diagnostics.metrics?.translation_queue_latency_ms)}
    · provider latency: {formatMetric(diagnostics.metrics?.translation_provider_latency_ms)}
    · cancelled: {String(diagnostics.metrics?.translation_jobs_cancelled ?? 0)}
  </p>
  <p class="muted mono-block">
    {tr("tools.runtime.dispatcher.reason")}: {metricLabel(metricValue("translation_last_runtime_reason", translationMetrics, translationDiag))}
    · {tr("tools.runtime.dispatcher.stale_dropped")}: {String(metricValue("translation_stale_results_dropped", translationMetrics, translationDiag) ?? 0)}
    · {tr("tools.runtime.dispatcher.provider_skipped")}: {String(metricValue("translation_provider_skipped_before_call", translationMetrics, translationDiag) ?? 0)}
    · {tr("tools.runtime.dispatcher.timeout")}: {formatMetric(metricValue("translation_last_timeout_ms", translationMetrics, translationDiag))}
    · {tr("tools.runtime.dispatcher.last_slot")}: {metricLabel(metricValue("translation_last_slot_id", translationMetrics, translationDiag))}/{metricLabel(metricValue("translation_last_target_lang", translationMetrics, translationDiag))}
  </p>
  <p class="muted mono-block">
    asr partial {formatMetric(diagnostics.metrics?.asr_partial_ms)}
    · asr final {formatMetric(diagnostics.metrics?.asr_final_ms)}
    · partials {String(diagnostics.metrics?.partial_updates_emitted ?? 0)}
    · finals {String(diagnostics.metrics?.finals_emitted ?? 0)}
    · suppressed {String(diagnostics.metrics?.suppressed_partial_updates ?? 0)}
    · browser rx {String(diagnostics.metrics?.browser_transcripts_received ?? 0)}
    · stale dropped {String(diagnostics.metrics?.browser_transcript_stale_dropped ?? 0)}
  </p>
  <p class="muted mono-block">
    WS connections: {String(diagnostics.metrics?.ws_events_connections_active ?? 0)}
    · jobs started: {String(diagnostics.metrics?.translation_jobs_started ?? 0)}
    · stale dropped: {String(diagnostics.metrics?.translation_stale_results_dropped ?? 0)}
  </p>
  <label class="tools-logging-toggle stack-gap-sm">
    <input
      type="checkbox"
      checked={fullLoggingEnabled}
      on:change={(e) => setFullLoggingEnabled((e.currentTarget as HTMLInputElement).checked)}
    />
    <span>{tr("tools.runtime.full_logging")}</span>
  </label>
  <p class="muted">{tr("tools.runtime.full_logging.hint")}</p>
  <p class="muted mono-block">{formatObsCcRuntimeStatus(diagnostics.obs, tr)}</p>
  <p class="muted">{tr("tools.runtime.note")}</p>
  </article>
  </div>

  {#if status || error}
    <div class="tools-feedback bento-span-full">
      {#if status}
        <p class="muted save-status success">{status}</p>
      {/if}
      {#if error}
        <p class="muted save-status error">{error}</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .mono-block {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 13px;
  }

  .tools-feedback {
    margin-top: var(--space-1);
  }

  .tools-logging-toggle {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    cursor: pointer;
  }

  .stack-gap-sm {
    gap: 0.35rem;
  }

  .profiles-name-field .control {
    width: 100%;
    max-width: 280px;
    min-height: 40px;
  }

  .profiles-actions,
  .tools-action-row {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: stretch;
  }

  .profiles-actions .btn,
  .tools-action-row .btn {
    flex: 1 1 calc(50% - var(--space-2));
    min-width: 9rem;
  }

  .tools-file-btn {
    cursor: pointer;
    margin: 0;
  }
</style>
