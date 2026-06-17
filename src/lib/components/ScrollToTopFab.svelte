<script lang="ts">
  import { onDestroy } from "svelte";
  import ArrowUp from "lucide-svelte/icons/arrow-up";
  import { locale, t } from "../i18n";
  import { readScrollTop, scrollPaneToTop, shouldShowScrollToTop } from "../scroll-to-top";

  export let scrollRoot: HTMLElement | null = null;
  /** Extra bottom inset (e.g. above compact start FAB or bottom nav). */
  export let bottom = "var(--space-4)";

  let visible = false;
  let detach: (() => void) | null = null;

  $: loc = $locale;
  $: tr = (key: string) => t(key, undefined, loc);

  $: bindScrollTargets(scrollRoot);

  function updateVisibility() {
    visible = shouldShowScrollToTop(readScrollTop(scrollRoot));
  }

  function bindScrollTargets(root: HTMLElement | null) {
    detach?.();
    detach = null;
    visible = false;
    if (!root) return;

    const onScroll = () => updateVisibility();
    root.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("scroll", onScroll, { passive: true });
    window.addEventListener("resize", onScroll, { passive: true });
    updateVisibility();

    detach = () => {
      root.removeEventListener("scroll", onScroll);
      window.removeEventListener("scroll", onScroll);
      window.removeEventListener("resize", onScroll);
    };
  }

  function scrollToTop() {
    scrollPaneToTop(scrollRoot);
    requestAnimationFrame(updateVisibility);
  }

  onDestroy(() => {
    detach?.();
    detach = null;
  });
</script>

<button
  type="button"
  class="scroll-to-top-fab"
  class:is-visible={visible}
  style:bottom
  aria-label={tr("common.scroll_to_top")}
  title={tr("common.scroll_to_top")}
  on:click={scrollToTop}
>
  <ArrowUp size={20} strokeWidth={2} aria-hidden="true" />
</button>
