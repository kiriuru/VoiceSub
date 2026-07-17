<script lang="ts">
  import { onDestroy } from "svelte";
  import type { LocalAsrModuleStatus } from "../lib/local-asr-api";

  interface Props {
    open: boolean;
    status: LocalAsrModuleStatus;
    tr: (key: string, vars?: Record<string, string | number>) => string;
    mark: (ok: boolean) => string;
    markClass: (ok: boolean) => string;
    phaseLabel: (phase: string) => string;
    onClose: () => void;
  }

  let { open, status, tr, mark, markClass, phaseLabel, onClose }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);

  $effect(() => {
    if (!dialogEl) return;
    if (open && !dialogEl.open) {
      dialogEl.showModal();
    } else if (!open && dialogEl.open) {
      dialogEl.close();
    }
  });

  function onDialogClose() {
    onClose();
  }

  onDestroy(() => {
    dialogEl?.close();
  });
</script>

<dialog
  class="module-status-details-sheet"
  bind:this={dialogEl}
  aria-labelledby="local-asr-status-details-title"
  onclose={onDialogClose}
  onclick={(event) => {
    if (event.target === dialogEl) onClose();
  }}
>
  <div class="module-status-details-sheet__panel surface-card">
    <header class="module-status-details-sheet__header">
      <h2 id="local-asr-status-details-title">{tr("local_asr.status.details.title")}</h2>
      <button type="button" class="top-app-bar__icon-btn" aria-label={tr("common.close")} onclick={onClose}>
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
          <path
            fill="currentColor"
            d="M18.3 5.71a1 1 0 0 0-1.41 0L12 10.59 7.11 5.7A1 1 0 0 0 5.7 7.11L10.59 12l-4.89 4.89a1 1 0 1 0 1.41 1.42L12 13.41l4.89 4.89a1 1 0 0 0 1.42-1.41L13.41 12l4.89-4.89a1 1 0 0 0-.01-1.4Z"
          />
        </svg>
      </button>
    </header>

    <div class="module-status-details-sheet__body stack">
      <p class="status-line">
        {tr("local_asr.status.phase")}: <strong>{phaseLabel(status.phase)}</strong>
        · {tr("local_asr.status.cpu_deps")}:
        <span class={markClass(status.env.cpuDepsReady)}>{mark(status.env.cpuDepsReady)}</span>
        · {tr("local_asr.status.cuda_deps")}:
        <span class={markClass(status.env.cudaDepsReady)}>{mark(status.env.cudaDepsReady)}</span>
        · {tr("local_asr.status.ready")}:
        <span class={markClass(status.ready)}>{mark(status.ready)}</span>
      </p>

      {#if status.message}
        <p class="status-line">{status.message}</p>
      {/if}

      {#if status.lastError}
        <p class="error-line">{status.lastError}</p>
      {/if}

      <div>
        <p class="eyebrow module-status-details-sheet__eyebrow">{tr("local_asr.setup.checklist_title")}</p>
        <ul class="setup-checklist">
          <li class={markClass(status.setup.depsReady)}>
            {tr("local_asr.setup.checklist.deps")}: {mark(status.setup.depsReady)}
          </li>
          <li class={markClass(status.modelInstalled)}>
            {tr("local_asr.setup.checklist.model")}: {mark(status.modelInstalled)}
          </li>
          <li class={markClass(status.setup.micTestPassed)}>
            {tr("local_asr.setup.checklist.mic_test")}: {mark(status.setup.micTestPassed)}
          </li>
          <li class={markClass(status.setup.parakeetFinalReceived)}>
            {tr("local_asr.setup.checklist.parakeet_final")}: {mark(status.setup.parakeetFinalReceived)}
          </li>
        </ul>
      </div>
    </div>
  </div>
</dialog>
