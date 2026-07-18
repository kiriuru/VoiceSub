<script lang="ts">
  import { locale, t } from "../i18n";
  import type { NavDestinationId } from "../navigation";
  import { PRIMARY_NAV_DESTINATIONS } from "../navigation";
  import Radio from "lucide-svelte/icons/radio";
  import Languages from "lucide-svelte/icons/languages";
  import Subtitles from "lucide-svelte/icons/subtitles";
  import MonitorPlay from "lucide-svelte/icons/monitor-play";
  import Blocks from "lucide-svelte/icons/blocks";
  import LayoutGrid from "lucide-svelte/icons/layout-grid";

  export let active: NavDestinationId = "live";
  export let onSelect: (dest: NavDestinationId) => void = () => {};

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  const items: Array<{
    id: NavDestinationId;
    labelKey: string;
    icon: typeof Radio;
  }> = [
    { id: "live", labelKey: "nav.live", icon: Radio },
    { id: "translation", labelKey: "tab.translation", icon: Languages },
    { id: "subtitles", labelKey: "nav.subtitles", icon: Subtitles },
    { id: "obs", labelKey: "tab.obs", icon: MonitorPlay },
    { id: "modules", labelKey: "nav.modules", icon: Blocks },
    { id: "more", labelKey: "nav.more", icon: LayoutGrid },
  ];

  $: ordered = PRIMARY_NAV_DESTINATIONS.map(
    (id) => items.find((item) => item.id === id)!,
  );
</script>

<nav class="compact-bottom-nav--primary" aria-label={tr("nav.bottom.label")}>
  {#each ordered as item}
    <button
      type="button"
      class="compact-bottom-tab--primary"
      class:is-active={active === item.id}
      title={tr(item.labelKey)}
      aria-current={active === item.id ? "page" : undefined}
      on:click={() => onSelect(item.id)}
    >
      <span class="compact-bottom-tab-icon" aria-hidden="true">
        <svelte:component this={item.icon} size={25} strokeWidth={1.75} />
      </span>
      <span class="compact-bottom-tab-label">{tr(item.labelKey)}</span>
    </button>
  {/each}
</nav>
