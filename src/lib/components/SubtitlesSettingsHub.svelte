<script lang="ts">
  import { locale, t } from "../i18n";
  import { SUBTITLES_SETTINGS_ITEMS } from "../navigation";
  import type { TabId } from "../types";
  import ChevronRight from "lucide-svelte/icons/chevron-right";

  export let onSelect: (tab: TabId) => void = () => {};

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
</script>

<div class="more-settings-hub subtitles-settings-hub">
  <p class="muted" style="margin: 0;">{tr("nav.subtitles.hint")}</p>

  <ul class="more-settings-list">
    {#each SUBTITLES_SETTINGS_ITEMS as item}
      <li>
        <button type="button" class="more-settings-item" on:click={() => onSelect(item.id)}>
          <span class="more-settings-item__body">
            <span class="more-settings-item__label">{tr(item.labelKey)}</span>
            <span class="more-settings-item__desc">{tr(item.descriptionKey)}</span>
          </span>
          <span class="more-settings-item__chevron" aria-hidden="true">
            <ChevronRight size={18} strokeWidth={1.75} />
          </span>
        </button>
      </li>
    {/each}
  </ul>
</div>
