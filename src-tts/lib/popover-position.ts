export interface PopoverRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

export interface PopoverPositionOptions {
  gap?: number;
  margin?: number;
  viewportWidth?: number;
  viewportHeight?: number;
}

/** Keep a fixed popover inside the viewport, preferring below the trigger. */
export function clampPopoverPosition(
  trigger: PopoverRect,
  popover: PopoverRect,
  options: PopoverPositionOptions = {},
): { top: number; left: number } {
  const gap = options.gap ?? 6;
  const margin = options.margin ?? 12;
  const viewportWidth = options.viewportWidth ?? 0;
  const viewportHeight = options.viewportHeight ?? 0;

  let left = trigger.left;
  let top = trigger.top + trigger.height + gap;

  if (left + popover.width > viewportWidth - margin) {
    left = trigger.left + trigger.width - popover.width;
  }
  left = Math.max(margin, Math.min(left, viewportWidth - popover.width - margin));

  if (top + popover.height > viewportHeight - margin) {
    top = trigger.top - popover.height - gap;
  }
  top = Math.max(margin, Math.min(top, viewportHeight - popover.height - margin));

  return { top, left };
}

export function rectFromElement(el: HTMLElement): PopoverRect {
  const rect = el.getBoundingClientRect();
  return {
    top: rect.top,
    left: rect.left,
    width: rect.width,
    height: rect.height,
  };
}
