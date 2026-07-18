<script lang="ts">
  import { onDestroy } from "svelte";
  import { locale, t } from "../i18n";
  import { panelSectionIdForTab, panelSectionTabForId, type PanelSection } from "../panel-sections";
  import type { TabId } from "../types";

  export let sections: PanelSection[] = [];
  export let ariaLabelKey = "nav.section.jump";
  export let activeTab: TabId | undefined = undefined;
  export let onActiveTabChange: (tab: TabId) => void = () => {};
  /** `scroll` — jump to in-page anchors (translation). `tabs` — switch panel content (subtitles). */
  export let navMode: "scroll" | "tabs" = "scroll";

  let scrollActiveId = "";
  let observer: IntersectionObserver | null = null;
  let tabSyncFromScroll: TabId | undefined = undefined;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  $: tabActiveId =
    navMode === "tabs" && activeTab ? panelSectionIdForTab(sections, activeTab) ?? "" : "";

  $: activeId = navMode === "tabs" ? tabActiveId : scrollActiveId;

  $: if (navMode === "scroll" && activeTab && activeTab !== tabSyncFromScroll) {
    const mapped = panelSectionIdForTab(sections, activeTab);
    if (mapped) scrollActiveId = mapped;
    tabSyncFromScroll = undefined;
  }

  function setScrollActiveSection(id: string, fromScroll = false) {
    if (!id || scrollActiveId === id) return;
    scrollActiveId = id;
    if (!fromScroll) return;

    const tabId = panelSectionTabForId(sections, id);
    if (!tabId || tabId === activeTab) return;
    tabSyncFromScroll = tabId;
    onActiveTabChange(tabId);
  }

  function scrollToSection(id: string) {
    const el = document.getElementById(id);
    if (!el) return;
    el.scrollIntoView({ behavior: "smooth", block: "start" });
    setScrollActiveSection(id, false);
    const tabId = panelSectionTabForId(sections, id);
    if (tabId && tabId !== activeTab) {
      tabSyncFromScroll = tabId;
      onActiveTabChange(tabId);
    }
  }

  function handleSectionClick(section: PanelSection) {
    if (navMode === "tabs") {
      if (section.tabId && section.tabId !== activeTab) {
        onActiveTabChange(section.tabId);
      }
      return;
    }
    scrollToSection(section.id);
  }

  function findScrollRoot(sectionId: string): Element | null {
    const el = document.getElementById(sectionId);
    if (!el) return null;
    let node: HTMLElement | null = el.parentElement;
    while (node && node !== document.documentElement) {
      if (
        node.classList.contains("standard-content") ||
        node.classList.contains("compact-content-scroll")
      ) {
        return node;
      }
      const style = getComputedStyle(node);
      const overflowY = style.overflowY;
      if (overflowY === "auto" || overflowY === "scroll" || overflowY === "overlay") {
        return node;
      }
      node = node.parentElement;
    }
    return null;
  }

  function mountObserver(nodeSections: PanelSection[]) {
    observer?.disconnect();
    observer = null;
    if (navMode !== "scroll" || !nodeSections.length) return;

    const scrollRoot = findScrollRoot(nodeSections[0].id);

    observer = new IntersectionObserver(
      (entries) => {
        const visible = entries
          .filter((entry) => entry.isIntersecting)
          .sort((a, b) => b.intersectionRatio - a.intersectionRatio);
        const top = visible[0]?.target.id;
        if (top) setScrollActiveSection(top, true);
      },
      {
        root: scrollRoot,
        rootMargin: "-20% 0px -55% 0px",
        threshold: [0.12, 0.4, 0.7],
      },
    );

    for (const section of nodeSections) {
      const el = document.getElementById(section.id);
      if (el) observer.observe(el);
    }

    if (!scrollActiveId && nodeSections[0]) {
      scrollActiveId = nodeSections[0].id;
    }
  }

  $: mountObserver(sections);

  onDestroy(() => {
    observer?.disconnect();
    observer = null;
  });
</script>

<nav class="panel-section-nav" aria-label={tr(ariaLabelKey)}>
  <ul class="panel-section-nav__list">
    {#each sections as section}
      <li>
        <button
          type="button"
          class="panel-section-nav__btn"
          class:is-active={activeId === section.id}
          aria-current={activeId === section.id ? "page" : undefined}
          on:click={() => handleSectionClick(section)}
        >
          {tr(section.labelKey)}
        </button>
      </li>
    {/each}
  </ul>
</nav>
