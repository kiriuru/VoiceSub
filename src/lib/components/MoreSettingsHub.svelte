<script lang="ts">
  import { locale, t } from "../i18n";
  import type { TabId } from "../types";
  import { MORE_SETTINGS_GROUPS } from "../navigation";
  import ChevronRight from "lucide-svelte/icons/chevron-right";

  export let onSelect: (tab: TabId) => void = () => {};

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);
</script>

<div class="more-settings-hub">
  <p class="muted muted--flush">{tr("nav.more.hint")}</p>

  {#each MORE_SETTINGS_GROUPS as group}
    <section class="more-settings-group" aria-labelledby="group-{group.groupKey}">
      <h2 class="more-settings-group__title" id="group-{group.groupKey}">
        {tr(group.groupKey)}
      </h2>
      <ul class="more-settings-list">
        {#each group.items as item}
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
    </section>
  {/each}
</div>
