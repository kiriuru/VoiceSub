<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale, t } from "../i18n";
  import type { RuntimeStatus } from "../types";
  import type { SaveStatusState } from "../save-status";
  import { formatSaveStatusDisplay, saveSnackbarDismissMs, shouldAutoDismissSaveTone } from "../save-status";

  export let saveStatus: SaveStatusState = { tone: "default" };
  export let runtime: RuntimeStatus | undefined = undefined;
  export let onDismiss: () => void = () => {};

  let visible = false;
  let message = "";
  let tone: SaveStatusState["tone"] = "default";
  let hideTimer: ReturnType<typeof setTimeout> | null = null;
  let lastSignature = "";

  $: loc = $locale;
  $: signature = JSON.stringify(saveStatus);
  $: if (signature !== lastSignature) {
    lastSignature = signature;
    applySaveStatus(saveStatus);
  }

  function applySaveStatus(state: SaveStatusState) {
    if (state.tone === "default") {
      clearHideTimer();
      visible = false;
      return;
    }

    tone = state.tone;
    message = formatSaveStatusDisplay(state, runtime, loc);
    visible = Boolean(message);

    clearHideTimer();
    if (shouldAutoDismissSaveTone(state.tone)) {
      hideTimer = setTimeout(() => {
        visible = false;
        onDismiss();
        hideTimer = null;
      }, saveSnackbarDismissMs(state.tone));
    }
  }

  function clearHideTimer() {
    if (hideTimer) {
      clearTimeout(hideTimer);
      hideTimer = null;
    }
  }

  onDestroy(clearHideTimer);
</script>

{#if visible && message}
  <div class="snackbar-host" role="presentation">
    <div
      class="snackbar"
      class:success={tone === "success"}
      class:warn={tone === "warn"}
      class:error={tone === "error"}
      class:busy={tone === "busy"}
      role="status"
      aria-live="polite"
    >
      {message}
    </div>
  </div>
{/if}
