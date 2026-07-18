<script lang="ts">
  import { onMount } from "svelte";

  export let text: string;
  export let triggerLabel: string;

  let open = false;
  let root: HTMLSpanElement | undefined;
  const tipId = `field-help-${Math.random().toString(36).slice(2, 9)}`;

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

  function onDocumentKeydown(event: KeyboardEvent) {
    if (!open) return;
    if (event.key === "Escape") {
      event.preventDefault();
      open = false;
    }
  }

  onMount(() => {
    document.addEventListener("click", onDocumentClick);
    document.addEventListener("keydown", onDocumentKeydown);
    return () => {
      document.removeEventListener("click", onDocumentClick);
      document.removeEventListener("keydown", onDocumentKeydown);
    };
  });
</script>

<span class="field-help" bind:this={root}>
  <button
    type="button"
    class="field-help-trigger"
    aria-label={triggerLabel}
    aria-expanded={open}
    aria-controls={tipId}
    aria-describedby={open ? tipId : undefined}
    title={text}
    on:click={toggle}
  >
    !
  </button>
  {#if open}
    <span id={tipId} class="field-help-popover" role="tooltip">{text}</span>
  {/if}
</span>
