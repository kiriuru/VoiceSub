<script lang="ts">
  import type { LocalAsrConfig } from "../lib/local-asr-api";

  interface Props {
    config: LocalAsrConfig;
    busy: boolean;
    ortProfilingActive: boolean;
    ortProfilingDecodeCount: number;
    ortProfilingMaxDecodes: number;
    ortProfilingStoppedBudget: boolean;
    lastOrtProfilePath: string | null;
    tr: (key: string, vars?: Record<string, string | number>) => string;
    onSave: () => void | Promise<void>;
  }

  let {
    config,
    busy,
    ortProfilingActive,
    ortProfilingDecodeCount,
    ortProfilingMaxDecodes,
    ortProfilingStoppedBudget,
    lastOrtProfilePath,
    tr,
    onSave,
  }: Props = $props();
</script>

<section class="surface-card session-options-panel">
  <div class="section-heading section-heading--stacked">
    <h2>{tr("local_asr.session.title")}</h2>
    <p class="muted muted--flush">{tr("local_asr.session.note")}</p>
  </div>

  <div class="realtime-form stack">
    <div class="realtime-form__split">
      <label class="stack-field">
        <span>{tr("local_asr.session.graph_opt")}</span>
        <select
          class="control"
          value={String(config.inference.graphOptimizationLevel)}
          disabled={busy}
          onchange={(event) => {
            config.inference.graphOptimizationLevel = Number(
              (event.currentTarget as HTMLSelectElement).value,
            );
          }}
        >
          <option value="0">{tr("local_asr.session.graph_opt.disable")}</option>
          <option value="1">{tr("local_asr.session.graph_opt.basic")}</option>
          <option value="2">{tr("local_asr.session.graph_opt.extended")}</option>
          <option value="3">{tr("local_asr.session.graph_opt.all")}</option>
        </select>
      </label>

      <label class="stack-field">
        <span>{tr("local_asr.session.intra_op")}</span>
        <input
          class="control"
          type="number"
          min="1"
          max="64"
          value={config.inference.intraOpThreads}
          disabled={busy}
          oninput={(event) => {
            config.inference.intraOpThreads = Number(
              (event.currentTarget as HTMLInputElement).value,
            );
          }}
        />
      </label>

      <label class="stack-field">
        <span>{tr("local_asr.session.inter_op")}</span>
        <input
          class="control"
          type="number"
          min="1"
          max="64"
          value={config.inference.interOpThreads}
          disabled={busy}
          oninput={(event) => {
            config.inference.interOpThreads = Number(
              (event.currentTarget as HTMLInputElement).value,
            );
          }}
        />
      </label>
    </div>

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={config.inference.parallelExecution} disabled={busy} />
      <span>{tr("local_asr.session.parallel")}</span>
    </label>
    <p class="section-note">{tr("local_asr.session.parallel.help")}</p>

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={config.inference.enableMemoryPattern} disabled={busy} />
      <span>{tr("local_asr.session.mem_pattern")}</span>
    </label>
    <p class="section-note">{tr("local_asr.session.mem_pattern.help")}</p>

    <label class="checkbox-row">
      <input type="checkbox" bind:checked={config.inference.ortProfiling} disabled={busy} />
      <span>{tr("local_asr.session.profiling")}</span>
    </label>
    <p class="section-note">{tr("local_asr.session.profiling.help")}</p>

    {#if config.inference.ortProfiling}
      <label class="stack-field">
        <span>{tr("local_asr.session.profiling_max_decodes")}</span>
        <input
          class="control"
          type="number"
          min="1"
          max="50"
          value={config.inference.ortProfilingMaxDecodes}
          disabled={busy}
          oninput={(event) => {
            config.inference.ortProfilingMaxDecodes = Number(
              (event.currentTarget as HTMLInputElement).value,
            );
          }}
        />
      </label>
      <p class="section-note">{tr("local_asr.session.profiling_max_decodes.help")}</p>
    {/if}

    {#if ortProfilingActive}
      <p class="status-line" role="status">
        {tr("local_asr.session.profiling.active", {
          count: ortProfilingDecodeCount,
          max: ortProfilingMaxDecodes || config.inference.ortProfilingMaxDecodes,
        })}
      </p>
    {/if}
    {#if ortProfilingStoppedBudget}
      <p class="status-line" role="status">{tr("local_asr.session.profiling.budget_done")}</p>
    {/if}
    {#if lastOrtProfilePath}
      <p class="status-line session-options-panel__profile">
        {tr("local_asr.session.profiling.path")}:
        <code>{lastOrtProfilePath}</code>
      </p>
    {/if}
  </div>

  <div class="row-actions">
    <button class="btn btn-primary" type="button" disabled={busy} onclick={() => void onSave()}>
      {tr("local_asr.session.save")}
    </button>
  </div>
</section>
