import { describe, expect, it } from "vitest";
import { clampPopoverPosition } from "./popover-position";

describe("clampPopoverPosition", () => {
  const popover = { top: 0, left: 0, width: 300, height: 120 };

  it("anchors below trigger when there is room", () => {
    const trigger = { top: 40, left: 80, width: 24, height: 24 };
    expect(clampPopoverPosition(trigger, popover, { viewportWidth: 800, viewportHeight: 600 })).toEqual({
      top: 70,
      left: 80,
    });
  });

  it("flips left when popover would overflow the right edge", () => {
    const trigger = { top: 40, left: 720, width: 24, height: 24 };
    expect(clampPopoverPosition(trigger, popover, { viewportWidth: 800, viewportHeight: 600 })).toEqual({
      top: 70,
      left: 444,
    });
  });

  it("opens above trigger when there is no room below", () => {
    const trigger = { top: 500, left: 100, width: 24, height: 24 };
    expect(clampPopoverPosition(trigger, popover, { viewportWidth: 800, viewportHeight: 600 })).toEqual({
      top: 374,
      left: 100,
    });
  });
});
