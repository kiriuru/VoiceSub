<script lang="ts">
  import { locale, t } from "../i18n";
  import { buildProviderOptionGroups } from "../translation-helpers";

  export let value = "";
  export let onChange: (next: string) => void = () => {};

  $: loc = $locale;
  $: groups = buildProviderOptionGroups();
</script>

<select
  class="control provider-select"
  {value}
  on:change={(e) => onChange((e.currentTarget as HTMLSelectElement).value)}
>
  {#each groups as group}
    <optgroup label={t(group.labelKey, undefined, loc)}>
      {#each group.providers as provider}
        <option value={provider.id}>{provider.label}</option>
      {/each}
    </optgroup>
  {/each}
</select>

<style>
  .provider-select {
    width: 100%;
    max-width: 100%;
    min-width: 0;
    box-sizing: border-box;
  }
</style>
