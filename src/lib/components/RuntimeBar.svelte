<script lang="ts">
  import { locale, t } from "../i18n";
  import type { RuntimeStatus } from "../types";

  export let runtime: RuntimeStatus;
  export let obsDiagnostics: Record<string, unknown> | undefined = undefined;
  export let wsConnected = false;
  export let busy = false;
  export let onStart: () => void;
  export let onStop: () => void;

  $: loc = $locale;
  function tr(key: string, vars?: Record<string, string | number>) {
    loc;
    return t(key, vars);
  }

  $: phase = (runtime.phase || runtime.status || "idle") as string;
  $: running = Boolean(runtime.running || runtime.is_running);
  $: browserWorker = runtime.asr?.diagnostics?.browser_worker as Record<string, unknown> | undefined;
  $: workerConnected = Boolean(browserWorker?.worker_connected);
  $: obsDiag = obsDiagnostics || (runtime.obs_caption_diagnostics as Record<string, unknown> | undefined) || {};
  $: obsBadgeStatus = String(obsDiag.last_error || "").trim()
    ? "error"
    : obsDiag.enabled
      ? "ready"
      : "disabled";
  $: obsBadgeValue =
    obsBadgeStatus === "ready" ? String(obsDiag.output_mode || "ready") : obsBadgeStatus;

  const states = ["idle", "starting", "listening", "transcribing", "translating", "error"] as const;
</script>

<section class="glass-panel panel-padding runtime-shell">
  <div class="runtime-shell-main">
    <div class="stack" style="gap: 12px;">
      <div class="badges">
        <span class="badge" class:ok={wsConnected} class:warn={!wsConnected}>
          ws: {wsConnected ? tr("common.connected") : tr("common.disconnected")}
        </span>
        <span class="badge" class:active={running}>
          {tr("runtime.badge.runtime", { value: phase })}
        </span>
        <span class="badge" class:ok={workerConnected}>
          worker: {workerConnected ? tr("common.connected") : tr("common.disconnected")}
        </span>
        <span class="badge">asr: {runtime.asr?.active_mode || "browser_google"}</span>
        <span class="badge" class:ok={obsBadgeStatus === "ready"} class:err={obsBadgeStatus === "error"}>
          {tr("runtime.badge.obs_cc", { value: obsBadgeValue })}
        </span>
        {#if runtime.last_error}
          <span class="badge err">{runtime.last_error}</span>
        {/if}
      </div>
      <div class="badges">
        {#each states as state}
          <span class="badge state-pill" class:is-active={phase === state}>{tr(`runtime.state.${state}`)}</span>
        {/each}
      </div>
    </div>
    <div class="runtime-actions">
      <button class="btn btn-primary" disabled={busy || running} on:click={onStart}>
        {busy && !running ? tr("common.starting") : tr("common.start")}
      </button>
      <button class="btn btn-ghost" disabled={busy || !running} on:click={onStop}>{tr("common.stop")}</button>
    </div>
  </div>
</section>
