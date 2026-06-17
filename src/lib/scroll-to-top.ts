/** Minimum scroll offset before the back-to-top control appears. */
export const SCROLL_TO_TOP_THRESHOLD_PX = 48;

export function isScrollableElement(element: HTMLElement | null | undefined): boolean {
  if (!element) return false;
  return element.scrollHeight > element.clientHeight + 1;
}

export function resolveScrollContainer(scrollRoot: HTMLElement | null | undefined): HTMLElement | null {
  if (isScrollableElement(scrollRoot ?? null)) {
    return scrollRoot ?? null;
  }
  return null;
}

export function readScrollTop(scrollRoot: HTMLElement | null | undefined): number {
  const container = resolveScrollContainer(scrollRoot);
  if (container) return container.scrollTop;
  return window.scrollY || document.documentElement.scrollTop || 0;
}

export function shouldShowScrollToTop(scrollTop: number): boolean {
  return scrollTop > SCROLL_TO_TOP_THRESHOLD_PX;
}

export function scrollPaneToTop(scrollRoot: HTMLElement | null | undefined): void {
  const container = resolveScrollContainer(scrollRoot);
  if (container) {
    container.scrollTo({ top: 0, behavior: "smooth" });
    return;
  }
  window.scrollTo({ top: 0, behavior: "smooth" });
}
