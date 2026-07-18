<script lang="ts">
  import { onDestroy } from "svelte";

  export type ModuleAlertTone = "error" | "warn" | "info";

  interface Props {
    open: boolean;
    tone: ModuleAlertTone;
    title: string;
    message: string;
    dismissLabel: string;
    onClose: () => void;
  }

  let { open, tone, title, message, dismissLabel, onClose }: Props = $props();

  let dialogEl = $state<HTMLDialogElement | null>(null);
  let syncing = false;

  $effect(() => {
    const el = dialogEl;
    const isOpen = open;
    if (!el || syncing) return;
    syncing = true;
    try {
      if (isOpen) {
        if (!el.open) el.showModal();
      } else if (el.open) {
        el.close();
      }
    } finally {
      // Defer clearing so a sync `close` event during el.close() still sees syncing=true.
      queueMicrotask(() => {
        syncing = false;
      });
    }
  });

  function onDialogClose() {
    if (syncing) return;
    onClose();
  }

  onDestroy(() => {
    dialogEl = null;
  });
</script>

<dialog
  class="module-alert-dialog"
  class:is-error={tone === "error"}
  class:is-warn={tone === "warn"}
  bind:this={dialogEl}
  aria-labelledby="local-asr-alert-title"
  aria-describedby="local-asr-alert-message"
  onclose={onDialogClose}
  onclick={(event) => {
    if (event.target === dialogEl) onClose();
  }}
>
  <div class="module-alert-dialog__panel">
    <header class="module-alert-dialog__header">
      <h2 id="local-asr-alert-title">{title}</h2>
    </header>
    <p id="local-asr-alert-message" class="module-alert-dialog__message">{message}</p>
    <footer class="module-alert-dialog__actions">
      <button type="button" class="btn btn-primary" onclick={onClose}>{dismissLabel}</button>
    </footer>
  </div>
</dialog>
