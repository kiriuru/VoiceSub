<script lang="ts">
  import { onMount } from "svelte";

  export let text: string;
  export let triggerLabel: string;

  let open = false;
  let root: HTMLSpanElement | undefined;

  function toggle(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    open = !open;
  }

  function onDocumentClick(event: MouseEvent) {
    if (!open || !root) return;
    if (!root.contains(event.target as Node)) {
      open = false;
    }
  }

  onMount(() => {
    document.addEventListener("click", onDocumentClick);
    return () => document.removeEventListener("click", onDocumentClick);
  });
</script>

<span class="field-help" bind:this={root}>
  <button
    type="button"
    class="field-help-trigger"
    aria-label={triggerLabel}
    aria-expanded={open}
    title={text}
    on:click={toggle}
  >
    !
  </button>
  {#if open}
    <span class="field-help-popover" role="tooltip">{text}</span>
  {/if}
</span>
