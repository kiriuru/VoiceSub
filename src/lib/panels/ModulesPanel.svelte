<script lang="ts">
  import { locale, t } from "../i18n";
  import { openTtsModule } from "../api";

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  let busy = false;
  let status = "";
  let error = "";

  async function handleOpenTts() {
    busy = true;
    status = "";
    error = "";
    try {
      await openTtsModule();
      status = tr("tools.tts.opened");
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      busy = false;
    }
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
      <button class="btn btn-sm" disabled={busy} on:click={handleOpenTts}>{tr("tools.tts.open")}</button>
    </div>
    {#if status}
      <p class="muted modules-status">{status}</p>
    {/if}
    {#if error}
      <p class="modules-error" role="alert">{error}</p>
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
</style>
