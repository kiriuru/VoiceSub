<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale, t } from "../i18n";
  import type { RuntimeStatus } from "../types";
  import { RUNTIME_STATE_PHASES, buildRuntimeConnectionChips } from "../runtime-status";
  import X from "lucide-svelte/icons/x";

  export let open = false;
  export let runtime: RuntimeStatus;
  export let obsDiagnostics: Record<string, unknown> | undefined = undefined;
  export let wsConnected = false;

  let dialogEl: HTMLDialogElement | null = null;

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string | number>) => t(key, vars, loc);
  $: chips = buildRuntimeConnectionChips(runtime, wsConnected, obsDiagnostics);

  $: if (dialogEl) {
    if (open && !dialogEl.open) {
      dialogEl.showModal();
    } else if (!open && dialogEl.open) {
      dialogEl.close();
    }
  }

  function close() {
    open = false;
  }

  function onDialogClose() {
    open = false;
  }

  onDestroy(() => {
    dialogEl?.close();
  });
</script>

<dialog
  class="runtime-details-sheet"
  bind:this={dialogEl}
  aria-labelledby="runtime-details-title"
  on:close={onDialogClose}
  on:click={(e) => {
    if (e.target === dialogEl) close();
  }}
>
  <div class="runtime-details-sheet__panel">
    <header class="runtime-details-sheet__header">
      <h2 id="runtime-details-title">{tr("runtime.details.title")}</h2>
      <button type="button" class="top-app-bar__icon-btn" aria-label={tr("common.close")} on:click={close}>
        <X size={20} strokeWidth={1.75} />
      </button>
    </header>

    <div class="runtime-details-sheet__body stack">
      <div class="badges">
        <span class="badge" class:ok={chips.wsConnected} class:warn={!chips.wsConnected}>
          {tr("runtime.chip.ws")}: {chips.wsConnected ? tr("common.connected") : tr("common.disconnected")}
        </span>
        <span class="badge" class:active={chips.running}>
          {tr("runtime.badge.runtime", { value: chips.phase })}
        </span>
        <span class="badge" class:ok={chips.workerConnected}>
          {tr("runtime.chip.worker")}: {chips.workerConnected ? tr("common.connected") : tr("common.disconnected")}
        </span>
        <span class="badge">{tr("runtime.badge.asr", { value: chips.asrMode })}</span>
        <span class="badge" class:ok={chips.obsStatus === "ready"} class:err={chips.obsStatus === "error"}>
          {tr("runtime.badge.obs_cc", { value: chips.obsLabel })}
        </span>
        {#if chips.lastError}
          <span class="badge err">{chips.lastError}</span>
        {/if}
      </div>

      <div>
        <p class="eyebrow" style="margin-bottom: 8px;">{tr("runtime.details.state_machine")}</p>
        <div class="badges">
          {#each RUNTIME_STATE_PHASES as state}
            <span class="badge state-pill" class:is-active={chips.phase === state}>
              {tr(`runtime.state.${state}`)}
            </span>
          {/each}
        </div>
      </div>
    </div>
  </div>
</dialog>

<style>
  .runtime-details-sheet {
    max-width: min(520px, calc(100vw - 32px));
    width: 100%;
    color: var(--text-primary);
  }

  .runtime-details-sheet__panel {
    padding: var(--space-5);
    margin: 0;
    border: 1px solid var(--glass-border-strong, var(--glass-border));
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-app, #080a12) 88%, #1c1e2a);
    color: var(--text-primary);
    box-shadow: var(--glass-shadow, 0 8px 32px rgba(0, 0, 0, 0.35));
  }

  :global(html[data-ui-theme="light"]) .runtime-details-sheet__panel {
    background: color-mix(in srgb, #ffffff 92%, var(--bg-app, #f6f8fc));
  }

  .runtime-details-sheet__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
  }

  .runtime-details-sheet__header h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 500;
    color: var(--text-primary);
  }

  .runtime-details-sheet__body {
    gap: var(--space-4);
  }

  .runtime-details-sheet__body :global(.eyebrow) {
    color: color-mix(in srgb, var(--text-primary) 72%, var(--text-secondary));
  }
</style>
