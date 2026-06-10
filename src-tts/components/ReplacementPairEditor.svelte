<script lang="ts">
  import type { TwitchReplacement } from "../lib/types";

  interface Props {
    title: string;
    wordLabel: string;
    replaceLabel: string;
    wordPlaceholder: string;
    replacePlaceholder: string;
    addLabel: string;
    removeLabel: string;
    emptyLabel: string;
    pairs: TwitchReplacement[];
    maxPairs?: number;
    onChange: (pairs: TwitchReplacement[]) => void;
  }

  let {
    title,
    wordLabel,
    replaceLabel,
    wordPlaceholder,
    replacePlaceholder,
    addLabel,
    removeLabel,
    emptyLabel,
    pairs,
    maxPairs = 100,
    onChange,
  }: Props = $props();

  let newFrom = $state("");
  let newTo = $state("");
  let selected = $state(new Set<number>());

  function addPair() {
    const from = newFrom.trim().slice(0, 240);
    const to = newTo.trim().slice(0, 240);
    if (!from || pairs.length >= maxPairs) return;
    const next = [...pairs];
    const existing = next.findIndex((entry) => entry.from.toLowerCase() === from.toLowerCase());
    if (existing >= 0) {
      next[existing] = { from, to };
    } else {
      next.push({ from, to });
    }
    onChange(next);
    newFrom = "";
    newTo = "";
  }

  function removeSelected() {
    onChange(pairs.filter((_, index) => !selected.has(index)));
    selected = new Set();
  }

  function toggleSelect(index: number, checked: boolean) {
    const next = new Set(selected);
    if (checked) next.add(index);
    else next.delete(index);
    selected = next;
  }
</script>

<div class="stack-field stack-field--full tts-replacement-pairs">
  <span>{title}</span>
  <div class="tts-settings-grid">
    <label class="stack-field">
      <span>{wordLabel}</span>
      <input
        class="control"
        placeholder={wordPlaceholder}
        value={newFrom}
        oninput={(e) => {
          newFrom = (e.currentTarget as HTMLInputElement).value;
        }}
      />
    </label>
    <label class="stack-field">
      <span>{replaceLabel}</span>
      <input
        class="control"
        placeholder={replacePlaceholder}
        value={newTo}
        oninput={(e) => {
          newTo = (e.currentTarget as HTMLInputElement).value;
        }}
      />
    </label>
  </div>
  <div class="tts-inline-actions">
    <button
      type="button"
      class="btn btn-sm"
      disabled={!newFrom.trim() || pairs.length >= maxPairs}
      onclick={addPair}
    >
      {addLabel}
    </button>
    <button
      type="button"
      class="btn btn-sm btn-ghost"
      disabled={selected.size === 0}
      onclick={removeSelected}
    >
      {removeLabel}
    </button>
  </div>
  {#if pairs.length === 0}
    <p class="muted">{emptyLabel}</p>
  {:else}
    <ul class="transcript-box tts-word-pair-list">
      {#each pairs as pair, index (pair.from + index)}
        <li>
          <label class="checkbox-row">
            <input
              type="checkbox"
              checked={selected.has(index)}
              onchange={(e) =>
                toggleSelect(index, (e.currentTarget as HTMLInputElement).checked)}
            />
            <span><strong>{pair.from}</strong> <span class="muted">→</span> {pair.to || "—"}</span>
          </label>
        </li>
      {/each}
    </ul>
  {/if}
</div>
