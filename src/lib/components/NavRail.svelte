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
  import brandAvatar from "../../assets/brand-avatar.png";
  import brandAvatarHover from "../../assets/brand-avatar-hover.png";
  import CreditsDialog from "./CreditsDialog.svelte";

  export let active: NavDestinationId = "live";
  export let version = "0.6.0";
  export let onSelect: (dest: NavDestinationId) => void = () => {};

  let creditsOpen = false;

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

  function openCredits() {
    creditsOpen = true;
  }
</script>

<nav class="nav-rail" aria-label={tr("nav.rail.label")}>
  <div class="nav-rail__brand">
    <span class="nav-rail__title">VoiceSub</span>
  </div>

  <div class="nav-rail__items">
    {#each ordered as item}
      <button
        type="button"
        class="nav-rail-item"
        class:is-active={active === item.id}
        aria-current={active === item.id ? "page" : undefined}
        on:click={() => onSelect(item.id)}
      >
        <span class="nav-rail-item__icon" aria-hidden="true">
          <svelte:component this={item.icon} size={23} strokeWidth={1.75} />
        </span>
        <span class="nav-rail-item__label">{tr(item.labelKey)}</span>
      </button>
    {/each}
  </div>

  <div class="nav-rail__avatar">
    <button
      type="button"
      class="nav-rail__avatar-frame"
      aria-label={tr("credits.open")}
      title={tr("credits.open")}
      on:click={openCredits}
    >
      <img class="nav-rail__avatar-img nav-rail__avatar-img--base" src={brandAvatar} alt="" />
      <img class="nav-rail__avatar-img nav-rail__avatar-img--hover" src={brandAvatarHover} alt="" />
    </button>
  </div>
</nav>

<CreditsDialog bind:open={creditsOpen} {version} />
