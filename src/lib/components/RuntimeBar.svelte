<script lang="ts">
  import RuntimeStatusStrip from "./RuntimeStatusStrip.svelte";
  import RuntimeDetailsSheet from "./RuntimeDetailsSheet.svelte";
  import type { RuntimeStatus } from "../types";

  export let runtime: RuntimeStatus;
  export let obsDiagnostics: Record<string, unknown> | undefined = undefined;
  export let wsConnected = false;
  export let busy = false;
  export let variant: "standard" | "compact" = "standard";
  export let onStart: () => void = () => {};
  export let onStop: () => void = () => {};

  let detailsOpen = false;
</script>

<RuntimeStatusStrip
  {runtime}
  {obsDiagnostics}
  {wsConnected}
  {busy}
  {variant}
  {onStart}
  {onStop}
  onOpenDetails={() => {
    detailsOpen = true;
  }}
/>

<RuntimeDetailsSheet
  bind:open={detailsOpen}
  {runtime}
  {obsDiagnostics}
  {wsConnected}
/>
