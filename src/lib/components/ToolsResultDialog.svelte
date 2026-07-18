<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale, t } from "../i18n";

  export let open = false;
  export let title = "";
  export let message = "";
  export let tone: "success" | "error" = "success";

  let dialogEl: HTMLDialogElement | null = null;
  /** Prevent on:close → bind:open → reactive close re-entrancy. */
  let syncing = false;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
  $: dialogTitle = title || (tone === "error" ? tr("tools.result.title.error") : tr("tools.result.title.success"));

  $: syncDialog(open, dialogEl);

  function syncDialog(isOpen: boolean, el: HTMLDialogElement | null) {
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
  }

  function close() {
    open = false;
  }

  function onDialogClose() {
    if (syncing) return;
    open = false;
  }

  onDestroy(() => {
    // Do not call close() here — it can re-enter bind:open during teardown.
    dialogEl = null;
  });
</script>

<dialog
  class="tools-result-dialog"
  class:is-error={tone === "error"}
  bind:this={dialogEl}
  aria-labelledby="tools-result-title"
  aria-describedby="tools-result-message"
  on:close={onDialogClose}
  on:click={(e) => {
    if (e.target === dialogEl) close();
  }}
>
  <div class="tools-result-dialog__panel">
    <header class="tools-result-dialog__header">
      <h2 id="tools-result-title">{dialogTitle}</h2>
    </header>
    <p id="tools-result-message" class="tools-result-dialog__message">{message}</p>
    <footer class="tools-result-dialog__actions">
      <button type="button" class="btn btn-primary" on:click={close}>{tr("tools.result.ok")}</button>
    </footer>
  </div>
</dialog>

<style>
  .tools-result-dialog {
    border: none;
    padding: 0;
    background: transparent;
    max-width: min(28rem, calc(100vw - 2rem));
  }

  .tools-result-dialog__panel {
    background: var(--surface-elevated, var(--surface-card, #1c1f26));
    color: var(--text-primary, inherit);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.12));
    border-radius: 12px;
    padding: 1.1rem 1.2rem 1rem;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.35);
  }

  .tools-result-dialog__header h2 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 650;
  }

  .tools-result-dialog.is-error .tools-result-dialog__header h2 {
    color: var(--danger, #ff7b7b);
  }

  .tools-result-dialog:not(.is-error) .tools-result-dialog__header h2 {
    color: var(--success, #7ce3ad);
  }

  .tools-result-dialog__message {
    margin: 0.75rem 0 1rem;
    white-space: pre-line;
    line-height: 1.45;
    color: var(--text-secondary, inherit);
    font-size: 0.95rem;
  }

  .tools-result-dialog__actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
