<script lang="ts">

  import { locale, t } from "../i18n";

  import type { TranslationResultState } from "../types";



  export let results: TranslationResultState;

  export let showPanel = true;

  export let onShowPanelChange: ((enabled: boolean) => void) | undefined = undefined;



  $: loc = $locale;

  $: tr = (key: string, vars?: Record<string, string | number>) => t(key, vars, loc);



  $: entry = results.current;



  function setShowPanel(enabled: boolean) {

    onShowPanelChange?.(enabled);

  }

</script>



<article class="translation-results stack">

  <div class="section-heading translation-results-heading">

    <div>

      <p class="eyebrow">{tr("translation.latest.eyebrow")}</p>

      <h2>{tr("translation.latest.title")}</h2>

    </div>

    {#if onShowPanelChange}

      <label class="translation-results-toggle checkbox-row">

        <input

          type="checkbox"

          checked={showPanel}

          on:change={(e) => setShowPanel((e.currentTarget as HTMLInputElement).checked)}

        />

        <span>{tr("translation.latest.show")}</span>

      </label>

    {/if}

  </div>



  {#if !showPanel}

    <p class="muted">{tr("translation.latest.hidden")}</p>

  {:else if !entry}

    <p class="muted">{tr("translation.result.empty")}</p>

  {:else}

    <p class="muted">{tr("translation.segment", { sequence: entry.sequence })}</p>

    <pre class="transcript-box">{entry.source_text || "—"}</pre>



    {#if entry.statusMessage}

      <p class="translation-status-message" class:error={entry.translations.some((line) => line.success === false)}>

        {entry.statusMessage}

      </p>

    {/if}



    {#if entry.translations.length === 0}
      {#if !entry.statusMessage}
        <p class="muted">{tr("translation.result.disabled")}</p>
      {/if}
    {:else}

      <ul class="result-lines">

        {#each entry.translations as line}

          <li class="result-line" class:failed={line.success === false}>

            <div class="result-line-head">

              <strong>{line.label || line.slot_id || line.target_lang}</strong>

              {#if line.provider}

                <span class="result-line-provider">{line.provider}</span>

              {/if}

              {#if line.cached}<span class="badge ok">{tr("translation.result.cached")}</span>{/if}

            </div>

            {#if line.success === false}

              <p class="translation-error">{line.error || tr("translation.result.failed")}</p>

            {:else if line.text}

              <p>{line.text}</p>

            {:else}

              <p class="muted">—</p>

            {/if}

          </li>

        {/each}

      </ul>

    {/if}

  {/if}

</article>



<style>

  .translation-results-heading {

    align-items: start;

    gap: var(--space-3);

    flex-wrap: wrap;

  }



  .translation-results-toggle {

    margin-left: auto;

    flex-shrink: 0;

    white-space: nowrap;

  }



  :global(body.voicesub-layout-compact) .translation-results-heading {

    flex-wrap: wrap;

  }



  :global(body.voicesub-layout-compact) .translation-results-toggle {

    margin-left: 0;

    width: 100%;

  }



  .translation-results {

    min-width: 0;

    width: 100%;

  }



  .result-line {

    min-width: 0;

  }



  .result-line p {

    margin: 0;

    overflow-wrap: anywhere;

    word-break: break-word;

    line-height: 1.5;

  }



  .translation-results .transcript-box {

    width: 100%;

    max-width: 100%;

    box-sizing: border-box;

  }



  .translation-status-message {

    margin: 0;

    padding: var(--space-2) var(--space-3);

    border-radius: var(--radius-sm);

    border: 1px solid var(--line-subtle);

    background: var(--bg-control);

    font-size: 13px;

    line-height: 1.45;

    color: var(--text-secondary);

  }



  .translation-status-message.error {

    border-color: color-mix(in srgb, var(--danger) 35%, transparent);

    color: var(--danger);

  }



  .translation-error {

    margin: 0;

    color: var(--danger);

    font-size: 14px;

    line-height: 1.45;

    overflow-wrap: anywhere;

  }



  .result-line-provider {

    font-size: 12px;

    font-weight: 500;

    color: var(--text-secondary);

  }

</style>

