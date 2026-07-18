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
  import ToolsResultDialog from "../components/ToolsResultDialog.svelte";
  import { containsRedactedPlaceholders } from "../config-redacted";
  import { normalizeConfigPayload } from "../config-normalize";
  import { formatObsCcRuntimeStatus } from "../obs-status-i18n";
  import { isValidProfileName, normalizeProfileName } from "../profile-name";
  import { redactObject } from "../redaction";
  import type { ConfigPayload, DiagnosticsSnapshot, StylePresetCatalog } from "../types";

  export let config: ConfigPayload;
  export let diagnostics: DiagnosticsSnapshot;
  export let onChange: (next: ConfigPayload) => void;
  export let onConfigLoad: (next: ConfigPayload, presets?: StylePresetCatalog) => void;

  const CONFIG_EXPORT_FILENAME = "voicesub-config.json";

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
  let busy = false;
  let resultOpen = false;
  let resultTone: "success" | "error" = "success";
  let resultMessage = "";

  function showResult(tone: "success" | "error", message: string) {
    resultTone = tone;
    resultMessage = message;
    resultOpen = true;
  }

  function setSuccess(message: string) {
    showResult("success", message);
  }

  function setFailure(err: unknown, fallback?: string) {
    const message = err instanceof Error ? err.message : fallback || String(err);
    showResult("error", message);
  }

  function requireValidProfileName(): string | null {
    const name = normalizeProfileName(profileName);
    if (!name || !isValidProfileName(name)) {
      setFailure(tr("tools.profiles.invalid_name"));
      return null;
    }
    return name;
  }

  function profileExists(name: string): boolean {
    const needle = name.toLowerCase();
    return profiles.some((entry) => entry.toLowerCase() === needle);
  }

  /** @returns false when the list could not be refreshed (error already announced when requested). */
  async function refreshLists(options?: { announce?: boolean }): Promise<boolean> {
    const announce = options?.announce !== false;
    try {
      const profilesRes = await listProfiles();
      profiles = profilesRes.profiles || [];
      return true;
    } catch (err) {
      if (announce) setFailure(err);
      return false;
    }
  }

  onMount(() => {
    void refreshLists({ announce: false });
  });

  async function handleLoadProfile() {
    const name = requireValidProfileName();
    if (!name) return;
    const confirmKey =
      name === "default" ? "tools.profiles.default_load_confirm" : "tools.profiles.load_confirm";
    if (!globalThis.confirm(tr(confirmKey, { name }))) return;
    busy = true;
    try {
      const res = await loadProfile(name);
      onConfigLoad(normalizeConfigPayload(res.payload));
      setSuccess(tr("tools.result.profile_load", { name }));
    } catch (err) {
      setFailure(err);
    } finally {
      busy = false;
    }
  }

  async function handleSaveProfile() {
    const name = requireValidProfileName();
    if (!name) return;
    if (profileExists(name)) {
      if (!globalThis.confirm(tr("tools.profiles.save_overwrite_confirm", { name }))) return;
    }
    busy = true;
    try {
      const res = await saveProfile(name, config);
      if (!(await refreshLists())) return;
      const path = res.saved_to || `user-data/profiles/${name}.json`;
      setSuccess(tr("tools.result.profile_save", { name, path }));
    } catch (err) {
      setFailure(err);
    } finally {
      busy = false;
    }
  }

  async function handleDeleteProfile() {
    const name = requireValidProfileName();
    if (!name) return;
    if (name === "default") {
      setFailure(tr("tools.profiles.cannot_delete_default"));
      return;
    }
    if (!globalThis.confirm(tr("tools.profiles.delete_confirm", { name }))) return;
    busy = true;
    try {
      const res = await deleteProfile(name);
      if (!(await refreshLists())) return;
      if (res.deleted) {
        setSuccess(tr("tools.result.profile_delete", { name }));
      } else {
        setFailure(`${name} — ${tr("common.error")}`);
      }
    } catch (err) {
      setFailure(err);
    } finally {
      busy = false;
    }
  }

  async function handleExportDiagnostics() {
    busy = true;
    try {
      const { filename } = await downloadDiagnostics();
      setSuccess(tr("tools.result.export_diagnostics", { filename }));
    } catch (err) {
      setFailure(err, tr("tools.runtime.export_failed"));
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
  $: asrDiag = (diagnostics.asr || {}) as Record<string, unknown>;
  $: browserWorker = (asrDiag.browser_worker || {}) as Record<string, unknown>;
  $: localModule = (asrDiag.local_module || {}) as Record<string, unknown>;
  $: asrProvider = String(asrDiag.provider || asrDiag.active_mode || "n/a");
  $: workerConnected = browserWorker.worker_connected === true;
  $: localReady = localModule.ready === true;
  $: localPhase = String(localModule.phase || localModule.message || "—");

  function handleExportConfig() {
    const blob = new Blob([JSON.stringify(redactObject(config), null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = CONFIG_EXPORT_FILENAME;
    anchor.click();
    URL.revokeObjectURL(url);
    setSuccess(tr("tools.result.export_config", { filename: CONFIG_EXPORT_FILENAME }));
  }

  async function handleImportConfig(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    busy = true;
    try {
      const text = await file.text();
      const parsed = JSON.parse(text) as ConfigPayload;
      if (containsRedactedPlaceholders(parsed)) {
        if (!globalThis.confirm(tr("tools.config.import_redacted_confirm"))) {
          return;
        }
      }
      const payload = normalizeConfigPayload(parsed);
      onConfigLoad(payload);
      setSuccess(tr("tools.result.import_config", { filename: file.name }));
    } catch (err) {
      setFailure(err);
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
      disabled={busy}
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
    <label class="btn btn-sm tools-file-btn" class:is-disabled={busy}>
      {tr("tools.config.import")}
      <input
        type="file"
        accept="application/json,.json"
        hidden
        disabled={busy}
        on:change={handleImportConfig}
      />
    </label>
  </div>
  <p class="muted">{tr("tools.config.save_hint")}</p>
  </article>

  <article class="surface-card panel-padding bento-tile bento-span-full stack">
  <div class="section-heading section-heading--stacked">
    <p class="eyebrow">{tr("tools.runtime.eyebrow")}</p>
    <h3>{tr("tools.runtime.title")}</h3>
  </div>

  <p class="muted mono-block">
    ASR: {asrProvider}
    · worker: {workerConnected ? tr("common.connected") : tr("common.disconnected")}
    · Local ASR: {localReady ? tr("tools.runtime.local_asr.ready") : tr("tools.runtime.local_asr.not_ready")} ({localPhase})
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
  </p>
  <label class="tools-logging-toggle stack-gap-sm">
    <input
      type="checkbox"
      checked={fullLoggingEnabled}
      disabled={busy}
      on:change={(e) => setFullLoggingEnabled((e.currentTarget as HTMLInputElement).checked)}
    />
    <span>{tr("tools.runtime.full_logging")}</span>
  </label>
  <p class="muted">{tr("tools.runtime.full_logging.hint")}</p>
  <p class="muted">{tr("tools.runtime.logs_location")}</p>
  <p class="muted mono-block">{formatObsCcRuntimeStatus(diagnostics.obs, tr)}</p>
  <p class="muted">{tr("tools.runtime.note")}</p>
  </article>
  </div>
</section>

<ToolsResultDialog bind:open={resultOpen} tone={resultTone} message={resultMessage} />

<style>
  .mono-block {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 13px;
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

  .tools-file-btn.is-disabled {
    opacity: 0.55;
    pointer-events: none;
    cursor: not-allowed;
  }
</style>
