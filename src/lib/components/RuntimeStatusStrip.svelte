<script lang="ts">
  import { locale, t } from "../i18n";
  import type { RuntimeStatus } from "../types";
  import { buildRuntimeConnectionChips, RUNTIME_STATE_PHASES, type RuntimePhaseChip } from "../runtime-status";
  import Info from "lucide-svelte/icons/info";

  export let runtime: RuntimeStatus;
  export let obsDiagnostics: Record<string, unknown> | undefined = undefined;
  export let wsConnected = false;
  export let busy = false;
  export let variant: "standard" | "compact" = "standard";
  export let onStart: () => void = () => {};
  export let onStop: () => void = () => {};
  export let onOpenDetails: () => void = () => {};

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string | number>) => t(key, vars, loc);
  $: chips = buildRuntimeConnectionChips(runtime, wsConnected, obsDiagnostics);
  $: phaseLabel = RUNTIME_STATE_PHASES.includes(chips.phase as RuntimePhaseChip)
    ? tr(`runtime.state.${chips.phase}` as "runtime.state.idle")
    : tr("runtime.badge.runtime", { value: chips.phase });
</script>

<section class="live-hero surface-card panel-padding" aria-labelledby="live-runtime-heading">
  <div class="live-hero__main">
    <div class="live-hero__kpi">
      <p class="live-hero__eyebrow">{tr("nav.live")}</p>
      <h2 id="live-runtime-heading" class="live-hero__phase" class:is-error={chips.phase === "error"}>
        {phaseLabel}
      </h2>
      <p class="muted live-hero__hint">
        {chips.running ? tr("runtime.strip.running") : tr("runtime.strip.stopped")}
      </p>
    </div>

    <div class="live-hero__chips" role="list" aria-label={tr("runtime.strip.connections")}>
      <span
        class="filter-chip"
        class:ok={chips.wsConnected}
        class:warn={!chips.wsConnected}
        role="listitem"
      >
        {tr("runtime.chip.ws")}: {chips.wsConnected ? tr("common.connected") : tr("common.disconnected")}
      </span>
      <span
        class="filter-chip"
        class:ok={chips.workerConnected}
        class:warn={!chips.workerConnected}
        role="listitem"
      >
        {tr("runtime.chip.worker")}: {chips.workerConnected ? tr("common.connected") : tr("common.disconnected")}
      </span>
      <span
        class="filter-chip"
        class:ok={chips.obsStatus === "ready"}
        class:err={chips.obsStatus === "error"}
        role="listitem"
      >
        {tr("runtime.chip.obs")}: {chips.obsLabel}
      </span>
    </div>
  </div>

  <div class="live-hero__actions">
    <button type="button" class="btn btn-ghost btn-sm live-hero__details" on:click={onOpenDetails}>
      <Info size={16} strokeWidth={1.75} />
      {tr("runtime.details.open")}
    </button>

    {#if variant === "standard"}
      <button class="btn btn-primary btn-sm" disabled={busy || chips.running} on:click={onStart}>
        {busy && !chips.running ? tr("common.starting") : tr("common.start")}
      </button>
      <button class="btn btn-ghost btn-sm" disabled={busy || !chips.running} on:click={onStop}>
        {tr("common.stop")}
      </button>
    {/if}
  </div>
</section>

<style>
  .live-hero {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
  }

  .live-hero__main {
    display: grid;
    gap: var(--space-2);
    min-width: min(100%, 280px);
    flex: 1 1 auto;
  }

  .live-hero__eyebrow {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-tertiary);
  }

  .live-hero__phase {
    margin: 0;
    font-size: clamp(20px, 2.2vw, 26px);
    font-weight: 500;
    letter-spacing: -0.02em;
    line-height: 1.2;
  }

  .live-hero__phase.is-error {
    color: var(--danger);
  }

  .live-hero__hint {
    margin: 0;
    font-size: 14px;
  }

  .live-hero__chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }

  .live-hero__actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    flex-shrink: 0;
  }

  .live-hero__details {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
