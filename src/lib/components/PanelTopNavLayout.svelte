<script lang="ts">
  import type { PanelSection } from "../panel-sections";
  import type { TabId } from "../types";
  import PanelSectionNav from "./PanelSectionNav.svelte";

  export let sections: PanelSection[] = [];
  export let ariaLabelKey = "nav.section.jump";
  export let activeTab: TabId | undefined = undefined;
  export let onActiveTabChange: (tab: TabId) => void = () => {};
  export let navMode: "scroll" | "tabs" = "scroll";
</script>

<div class="panel-top-nav-layout">
  <div class="panel-top-nav-layout__nav">
    <PanelSectionNav
      {sections}
      {ariaLabelKey}
      {activeTab}
      {navMode}
      {onActiveTabChange}
    />
  </div>
  <div class="panel-top-nav-layout__content">
    <slot />
  </div>
</div>
