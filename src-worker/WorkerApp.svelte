<script lang="ts">

  import "./worker.css";

  import { locale } from "../src/lib/i18n/index";

  import type { WorkerUiStore } from "./lib/stores/worker-ui.svelte";

  import type { WorkerControllerActions } from "./lib/worker/worker-controller";



  interface Props {

    ui: WorkerUiStore;

    actions: WorkerControllerActions;

  }



  let { ui, actions }: Props = $props();



  $effect(() => {

    void $locale;

    ui.onLocaleChanged();

  });

</script>



<main class="worker-layout">

  <header class="worker-header panel">

    <h1>{ui.tr("worker.title")}</h1>

    <div class="notice" class:notice--alert={ui.documentHidden}>

      <strong>{ui.tr("worker.warning.title")}</strong>

      <p>{ui.visibilityWarning}</p>

    </div>

  </header>



  <section class="panel status-strip" aria-label={ui.tr("worker.status.title")}>

    <div class="status-chip">

      <span class="status-chip__label">{ui.tr("worker.status.label")}</span>

      <span class="status-chip__value">{ui.workerStatus}</span>

    </div>

    <div class="status-chip">

      <span class="status-chip__label">{ui.tr("worker.on_sound")}</span>

      <span class="status-chip__value" class:status-chip__value--active={ui.onSound}>{ui.onSoundLabel}</span>

    </div>

    <div class="status-chip">

      <span class="status-chip__label">{ui.tr("worker.metric.websocket")}</span>

      <span

        class="status-chip__value"

        class:status-chip__value--active={ui.websocketReady}

        class:status-chip__value--inactive={!ui.websocketReady}

      >

        <span class="status-dot" class:status-dot--on={ui.websocketReady} aria-hidden="true"></span>

        {ui.socketStatus}

      </span>

    </div>

  </section>



  <section class="panel action-bar">

    <button class="primary action-bar__start" type="button" onclick={() => void actions.onStart()}>

      {ui.tr("worker.start")}

    </button>

    <button class="secondary" type="button" onclick={() => actions.onStop()}>{ui.tr("common.stop")}</button>

    <button class="secondary" type="button" disabled={ui.saveDisabled} onclick={() => void actions.onSave()}>

      {ui.tr("common.save")}

    </button>

    {#if ui.settingsSaveStatus}

      <p

        class="action-bar__feedback"

        class:action-bar__feedback--error={ui.settingsSaveIsError}

        role="status"

      >

        {ui.settingsSaveStatus}

      </p>

    {/if}

  </section>



  <div class="worker-body">

    <section class="panel settings-panel">

      <h2>{ui.tr("worker.settings.title")}</h2>



      <div class="settings-group">

        <div class="info-row">

          <span class="info-row__label">{ui.tr("worker.configured_language")}</span>

          <span class="info-row__value">{ui.configuredLanguage}</span>

        </div>

        <p class="settings-note">{ui.tr("worker.microphone.note")}</p>

      </div>



      <div class="settings-group settings-toggles">

        <label class="toggle">

          <input type="checkbox" bind:checked={ui.interimResults} onchange={() => void actions.onInterimChange()} />

          <span>{ui.tr("worker.interim")}</span>

        </label>

        <label class="toggle">

          <input type="checkbox" bind:checked={ui.continuousResults} onchange={() => void actions.onContinuousChange()} />

          <span>{ui.tr("worker.continuous")}</span>

        </label>

        <label class="toggle">

          <input

            type="checkbox"

            bind:checked={ui.forceFinalization}

            onchange={() => void actions.onForceFinalizationChange()}

          />

          <span>{ui.tr("worker.force_finalization")}</span>

        </label>

      </div>



      <details class="settings-advanced">

        <summary>{ui.tr("worker.advanced.title")}</summary>

        <div class="settings-advanced__body">

          <p class="settings-note">{ui.tr("worker.force_finalization.note")}</p>

          <label class="stack-field">

            <span>{ui.tr("worker.force_finalization_timeout_ms")}</span>

            <input

              class="control"

              type="number"

              min="300"

              max="15000"

              step="100"

              bind:value={ui.forceFinalizationTimeoutMs}

              disabled={ui.forceFinalizationTimeoutDisabled}

              onchange={() => void actions.onForceFinalizationTimeoutChange()}

            />

          </label>

          <p class="settings-note">{ui.tr("worker.force_finalization_timeout_ms.note")}</p>

        </div>

      </details>

    </section>



    <section class="output-panel">

      <div class="panel output-card">

        <h2>{ui.tr("worker.partial.title")}</h2>

        <textarea readonly value={ui.partialText} aria-label={ui.tr("worker.partial.title")}></textarea>

      </div>

      <div class="panel output-card">

        <h2>{ui.tr("worker.final.title")}</h2>

        <textarea readonly value={ui.finalText} aria-label={ui.tr("worker.final.title")}></textarea>

      </div>

    </section>

  </div>

</main>



<footer class="app-footer">

  <span>Powered by Kiriuru</span>

</footer>



<style>

  :global(#app) {

    display: contents;

  }

</style>

