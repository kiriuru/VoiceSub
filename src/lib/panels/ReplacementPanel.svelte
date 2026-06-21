<script lang="ts">
  import { locale, t } from "../i18n";
  import type { ConfigPayload } from "../types";

  export let config: ConfigPayload;
  export let onChange: (next: ConfigPayload) => void;

  const MAX_PAIRS = 100;

  let newSource = "";
  let newTarget = "";
  let selected = new Set<number>();

  $: loc = $locale;
  $: tr = (key: string, vars?: Record<string, string | number>) => t(key, vars, loc);

  type SourceTextReplacementBlock = {
    enabled?: boolean;
    include_builtin?: boolean;
    include_builtin_profanity?: boolean;
    case_insensitive?: boolean;
    whole_words?: boolean;
    whole_word_only?: boolean;
    pairs?: Array<{ source?: string; target?: string }>;
  };

  $: block = (config.source_text_replacement || {}) as SourceTextReplacementBlock;
  $: pairs = Array.isArray(block.pairs) ? block.pairs : [];

  function patch(partial: Partial<SourceTextReplacementBlock>) {
    const nextBlock: SourceTextReplacementBlock = { ...block, ...partial };
    if ("include_builtin" in partial) {
      nextBlock.include_builtin_profanity = partial.include_builtin;
    }
    if ("include_builtin_profanity" in partial) {
      nextBlock.include_builtin = partial.include_builtin_profanity;
    }
    if ("whole_words" in partial) {
      nextBlock.whole_word_only = partial.whole_words;
    }
    if ("whole_word_only" in partial) {
      nextBlock.whole_words = partial.whole_word_only;
    }
    onChange({
      ...config,
      source_text_replacement: nextBlock,
    });
  }

  function addPair() {
    const source = newSource.trim().slice(0, 240);
    const target = newTarget.trim().slice(0, 240);
    if (!source || pairs.length >= MAX_PAIRS) return;
    patch({ pairs: [...pairs, { source, target }] });
    newSource = "";
    newTarget = "";
  }

  function removeSelected() {
    const next = pairs.filter((_, index) => !selected.has(index));
    patch({ pairs: next });
    selected = new Set();
  }

  function toggleSelect(index: number, checked: boolean) {
    const next = new Set(selected);
    if (checked) next.add(index);
    else next.delete(index);
    selected = next;
  }
</script>

<section class="surface-card panel-padding stack">
  <div class="section-heading">
    <div>
      <p class="eyebrow">{tr("tools.source_replacement.eyebrow")}</p>
      <h2>{tr("tools.source_replacement.title")}</h2>
    </div>
  </div>

  <p class="muted">{tr("tools.source_replacement.intro")}</p>

  <label class="checkbox-row">
    <input
      type="checkbox"
      checked={block.enabled === true}
      on:change={(e) => patch({ enabled: (e.currentTarget as HTMLInputElement).checked })}
    />
    <span>{tr("tools.source_replacement.enable")}</span>
  </label>

  <label class="checkbox-row">
    <input
      type="checkbox"
      checked={block.include_builtin !== false && block.include_builtin_profanity !== false}
      on:change={(e) =>
        patch({
          include_builtin: (e.currentTarget as HTMLInputElement).checked,
          include_builtin_profanity: (e.currentTarget as HTMLInputElement).checked,
        })}
    />
    <span>{tr("tools.source_replacement.builtin")}</span>
  </label>

  <label class="checkbox-row">
    <input
      type="checkbox"
      checked={block.case_insensitive !== false}
      on:change={(e) => patch({ case_insensitive: (e.currentTarget as HTMLInputElement).checked })}
    />
    <span>{tr("tools.source_replacement.case_insensitive")}</span>
  </label>

  <label class="checkbox-row">
    <input
      type="checkbox"
      checked={block.whole_words !== false && block.whole_word_only !== false}
      on:change={(e) =>
        patch({
          whole_words: (e.currentTarget as HTMLInputElement).checked,
          whole_word_only: (e.currentTarget as HTMLInputElement).checked,
        })}
    />
    <span>{tr("tools.source_replacement.whole_words")}</span>
  </label>

  <div class="grid-2">
    <label class="stack-field">
      <span>{tr("tools.source_replacement.word")}</span>
      <input class="control" bind:value={newSource} />
    </label>
    <label class="stack-field">
      <span>{tr("tools.source_replacement.replace_with")}</span>
      <input class="control" bind:value={newTarget} />
    </label>
  </div>

  <div class="url-row">
    <button type="button" class="btn btn-sm" disabled={!newSource.trim() || pairs.length >= MAX_PAIRS} on:click={addPair}>
      {tr("tools.source_replacement.add")}
    </button>
    <button type="button" class="btn btn-sm btn-ghost" disabled={selected.size === 0} on:click={removeSelected}>
      {tr("tools.source_replacement.remove_selected")}
    </button>
  </div>

  {#if pairs.length === 0}
    <p class="muted">{tr("tools.source_replacement.list_empty")}</p>
  {:else}
    <ul class="pair-list">
      {#each pairs as pair, index}
        <li class="pair-row">
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={selected.has(index)}
              on:change={(e) => toggleSelect(index, (e.currentTarget as HTMLInputElement).checked)}
            />
            <span><strong>{pair.source}</strong> <span class="muted">→</span> {pair.target || "—"}</span>
          </label>
        </li>
      {/each}
    </ul>
  {/if}
</section>
