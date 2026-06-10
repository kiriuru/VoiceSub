<script lang="ts">
  import { onMount } from "svelte";
  import { locale, t } from "../i18n";
  import type { CompactPaneId, TabId } from "../types";
  import Radio from "lucide-svelte/icons/radio";
  import Languages from "lucide-svelte/icons/languages";
  import Subtitles from "lucide-svelte/icons/subtitles";
  import Palette from "lucide-svelte/icons/palette";
  import SunMoon from "lucide-svelte/icons/sun-moon";
  import MonitorPlay from "lucide-svelte/icons/monitor-play";
  import Replace from "lucide-svelte/icons/replace";
  import Wrench from "lucide-svelte/icons/wrench";
  import Settings from "lucide-svelte/icons/settings";
  import CircleHelp from "lucide-svelte/icons/circle-help";

  export let activeTab: TabId = "translation";
  export let onSelect: (tab: TabId) => void = () => {};
  export let variant: "standard" | "compact" = "standard";

  export let activePane: CompactPaneId = "live";
  export let onSelectPane: ((pane: CompactPaneId) => void) | undefined = undefined;

  let scrollEl: HTMLDivElement | null = null;
  let canScrollLeft = false;
  let canScrollRight = false;

  $: loc = $locale;
  function tr(key: string) {
    loc;
    return t(key);
  }

  const settingsTabs: Array<{ id: TabId; labelKey: string; icon: typeof Languages }> = [
    { id: "translation", labelKey: "tab.translation", icon: Languages },
    { id: "subtitles", labelKey: "tab.subtitles", icon: Subtitles },
    { id: "style", labelKey: "tab.style", icon: Palette },
    { id: "theme", labelKey: "tab.theme", icon: SunMoon },
    { id: "obs", labelKey: "tab.obs", icon: MonitorPlay },
    { id: "replacement", labelKey: "tab.replacement", icon: Replace },
    { id: "tools", labelKey: "tab.tools", icon: Wrench },
    { id: "settings", labelKey: "tab.settings", icon: Settings },
    { id: "help", labelKey: "tab.help", icon: CircleHelp },
  ];

  const compactNavItems: Array<{ id: CompactPaneId; labelKey: string; icon: typeof Languages }> = [
    { id: "live", labelKey: "tab.live", icon: Radio },
    ...settingsTabs,
  ];

  function updateScrollHints() {
    const el = scrollEl;
    if (!el) {
      canScrollLeft = false;
      canScrollRight = false;
      return;
    }
    const maxScroll = el.scrollWidth - el.clientWidth;
    canScrollLeft = el.scrollLeft > 2;
    canScrollRight = maxScroll - el.scrollLeft > 2;
  }

  function scrollActiveTabIntoView() {
    const el = scrollEl;
    if (!el) return;
    const active = el.querySelector<HTMLElement>(".compact-bottom-tab.is-active");
    active?.scrollIntoView({ inline: "nearest", block: "nearest", behavior: "smooth" });
    requestAnimationFrame(updateScrollHints);
  }

  function handleWheel(event: WheelEvent, el: HTMLDivElement | null = scrollEl) {
    if (!el || el.scrollWidth <= el.clientWidth) return;
    if (Math.abs(event.deltaY) <= Math.abs(event.deltaX)) return;
    event.preventDefault();
    el.scrollLeft += event.deltaY;
    updateScrollHints();
  }

  function horizontalWheelScroll(node: HTMLDivElement) {
    const handler = (event: WheelEvent) => handleWheel(event, node);
    node.addEventListener("wheel", handler, { passive: false });
    return {
      destroy() {
        node.removeEventListener("wheel", handler);
      },
    };
  }

  function scrollByPage(direction: -1 | 1) {
    const el = scrollEl;
    if (!el) return;
    el.scrollBy({ left: direction * Math.max(120, el.clientWidth * 0.55), behavior: "smooth" });
    window.setTimeout(updateScrollHints, 220);
  }

  $: activePane, scrollEl, scrollActiveTabIntoView();

  onMount(() => {
    updateScrollHints();
    const onResize = () => updateScrollHints();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });
</script>

{#if variant === "compact"}
  <nav
    class="compact-bottom-nav"
    class:can-scroll-left={canScrollLeft}
    class:can-scroll-right={canScrollRight}
    aria-label="Dashboard sections"
  >
    {#if canScrollLeft}
      <button
        type="button"
        class="compact-bottom-nav-edge compact-bottom-nav-edge--left"
        aria-label={tr("compact.nav.scroll_left")}
        on:click={() => scrollByPage(-1)}
      >
        ‹
      </button>
    {/if}
    <div
      class="compact-bottom-nav-scroll"
      bind:this={scrollEl}
      use:horizontalWheelScroll
      on:scroll={updateScrollHints}
    >
      <div class="compact-bottom-nav-track">
        {#each compactNavItems as tab}
          <button
            type="button"
            class="compact-bottom-tab"
            class:is-active={activePane === tab.id}
            title={tr(tab.labelKey)}
            on:click={() => onSelectPane?.(tab.id)}
          >
            <span class="compact-bottom-tab-icon">
              <svelte:component this={tab.icon} size={18} strokeWidth={1.75} />
            </span>
            <span class="compact-bottom-tab-label">{tr(tab.labelKey)}</span>
          </button>
        {/each}
      </div>
    </div>
    {#if canScrollRight}
      <button
        type="button"
        class="compact-bottom-nav-edge compact-bottom-nav-edge--right"
        aria-label={tr("compact.nav.scroll_right")}
        on:click={() => scrollByPage(1)}
      >
        ›
      </button>
    {/if}
  </nav>
{:else}
  <nav class="tab-bar tab-bar-standard" aria-label="Dashboard sections">
    {#each settingsTabs as tab}
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === tab.id}
        on:click={() => onSelect(tab.id)}
      >
        <svelte:component this={tab.icon} size={16} strokeWidth={1.75} />
        <span class="tab-label">{tr(tab.labelKey)}</span>
      </button>
    {/each}
  </nav>
{/if}
