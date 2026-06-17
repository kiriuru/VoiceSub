import { describe, expect, it } from "vitest";
import {
  SCROLL_TO_TOP_THRESHOLD_PX,
  isScrollableElement,
  shouldShowScrollToTop,
} from "./scroll-to-top";

describe("scroll-to-top", () => {
  it("shows control only after threshold", () => {
    expect(shouldShowScrollToTop(0)).toBe(false);
    expect(shouldShowScrollToTop(SCROLL_TO_TOP_THRESHOLD_PX)).toBe(false);
    expect(shouldShowScrollToTop(SCROLL_TO_TOP_THRESHOLD_PX + 1)).toBe(true);
  });

  it("detects scrollable panes by overflow height", () => {
    expect(isScrollableElement(null)).toBe(false);
    expect(
      isScrollableElement({
        scrollHeight: 400,
        clientHeight: 200,
      } as HTMLElement),
    ).toBe(true);
    expect(
      isScrollableElement({
        scrollHeight: 200,
        clientHeight: 200,
      } as HTMLElement),
    ).toBe(false);
  });
});
