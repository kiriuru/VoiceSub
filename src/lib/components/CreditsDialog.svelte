<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale, t } from "../i18n";
  import { openExternalUrl } from "../api";
  import X from "lucide-svelte/icons/x";

  export let open = false;
  export let version = "0.6.0";

  const GITHUB_URL = "https://github.com/kiriuru/VoiceSub";

  let dialogEl: HTMLDialogElement | null = null;

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string | number>) => t(key, vars, loc);

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

  async function openGithub() {
    try {
      await openExternalUrl(GITHUB_URL);
    } catch {
      // ignore
    }
  }

  onDestroy(() => {
    dialogEl?.close();
  });
</script>

<dialog
  class="credits-dialog"
  bind:this={dialogEl}
  aria-labelledby="credits-dialog-title"
  on:close={onDialogClose}
  on:click={(e) => {
    if (e.target === dialogEl) close();
  }}
>
  <div class="credits-dialog__panel">
    <header class="credits-dialog__header">
      <h2 id="credits-dialog-title">{tr("credits.title")}</h2>
      <button type="button" class="top-app-bar__icon-btn" aria-label={tr("common.close")} on:click={close}>
        <X size={20} strokeWidth={1.75} />
      </button>
    </header>

    <div class="credits-dialog__body stack">
      <section class="credits-dialog__block">
        <h3>{tr("credits.author.heading")}</h3>
        <p class="credits-dialog__name">{tr("credits.author.name")}</p>
        <p class="credits-dialog__text">{tr("credits.author.role")}</p>
        <p class="credits-dialog__text">{tr("credits.author.note")}</p>
      </section>

      <section class="credits-dialog__block">
        <h3>{tr("credits.product.heading")}</h3>
        <p class="credits-dialog__name">
          VoiceSub <span class="credits-dialog__version">v{version}</span>
        </p>
        <p class="credits-dialog__text">{tr("credits.product.tagline")}</p>
        <p class="credits-dialog__text">{tr("credits.product.stack")}</p>
      </section>

      <div class="credits-dialog__actions">
        <button type="button" class="btn btn-primary" on:click={() => void openGithub()}>
          {tr("credits.github")}
        </button>
        <button type="button" class="btn btn-ghost" on:click={close}>
          {tr("common.close")}
        </button>
      </div>
    </div>
  </div>
</dialog>

<style>
  .credits-dialog {
    border: 0;
    padding: 0;
    background: transparent;
    max-width: min(440px, calc(100vw - 32px));
    width: 100%;
    color: var(--text-primary);
  }

  .credits-dialog::backdrop {
    background: rgba(0, 0, 0, 0.62);
  }

  .credits-dialog__panel {
    padding: var(--space-5);
    border: 1px solid var(--glass-border-strong, var(--glass-border));
    border-radius: var(--radius-lg);
    /* Opaque enough that body text stays readable on dark theme. */
    background: color-mix(in srgb, var(--bg-app, #080a12) 88%, #1c1e2a);
    color: var(--text-primary);
    box-shadow: var(--glass-shadow, 0 8px 32px rgba(0, 0, 0, 0.35));
  }

  :global(html[data-ui-theme="light"]) .credits-dialog__panel {
    background: color-mix(in srgb, #ffffff 92%, var(--bg-app, #f6f8fc));
  }

  .credits-dialog__header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-3);
    margin-bottom: var(--space-4);
  }

  .credits-dialog__header h2 {
    margin: 0;
    font-size: 1.2rem;
    font-weight: 650;
    color: var(--text-primary);
  }

  .credits-dialog__body {
    gap: var(--space-5);
  }

  .credits-dialog__block h3 {
    margin: 0 0 var(--space-2);
    font-size: 0.78rem;
    font-weight: 650;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--text-primary) 72%, var(--text-secondary));
  }

  .credits-dialog__name {
    margin: 0 0 var(--space-1);
    font-size: 1.08rem;
    font-weight: 650;
    color: var(--text-primary);
  }

  .credits-dialog__version {
    font-weight: 500;
    color: color-mix(in srgb, var(--text-primary) 78%, var(--text-secondary));
  }

  .credits-dialog__text {
    margin: 0 0 var(--space-1);
    font-size: 0.95rem;
    line-height: 1.5;
    color: color-mix(in srgb, var(--text-primary) 88%, var(--text-secondary));
  }

  .credits-dialog__text:last-child {
    margin-bottom: 0;
  }

  .credits-dialog__actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
</style>
