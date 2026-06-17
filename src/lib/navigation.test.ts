import { describe, expect, it } from "vitest";
import {
  MORE_TAB_IDS,
  SUBTITLES_TAB_IDS,
  isMoreTab,
  isSubtitlesTab,
  shouldShowMoreHub,
  shouldShowSubtitlesHub,
  tabToNavDestination,
  navDestinationTitleKey,
} from "./navigation";
import type { TabId } from "./types";

describe("navigation", () => {
  it("maps content tabs to M3 primary destinations", () => {
    expect(tabToNavDestination("translation")).toBe("translation");
    expect(tabToNavDestination("subtitles")).toBe("subtitles");
    expect(tabToNavDestination("style")).toBe("subtitles");
    expect(tabToNavDestination("obs")).toBe("obs");
    expect(tabToNavDestination("theme")).toBe("more");
    expect(tabToNavDestination("tools")).toBe("more");
  });

  it("maps modules destination title", () => {
    expect(navDestinationTitleKey("modules")).toBe("nav.modules");
  });

  it("classifies More hub tabs", () => {
    for (const tab of MORE_TAB_IDS) {
      expect(isMoreTab(tab)).toBe(true);
    }
    expect(isMoreTab("translation" as TabId)).toBe(false);
    expect(isMoreTab("style" as TabId)).toBe(false);
  });

  it("classifies Subtitles hub tabs", () => {
    for (const tab of SUBTITLES_TAB_IDS) {
      expect(isSubtitlesTab(tab)).toBe(true);
    }
    expect(isSubtitlesTab("theme" as TabId)).toBe(false);
  });

  it("shows More hub only when More is selected and hub is open", () => {
    expect(shouldShowMoreHub("more", true)).toBe(true);
    expect(shouldShowMoreHub("more", false)).toBe(false);
    expect(shouldShowMoreHub("translation", true)).toBe(false);
  });

  it("shows Subtitles hub only when Subtitles is selected and hub is open", () => {
    expect(shouldShowSubtitlesHub("subtitles", true)).toBe(true);
    expect(shouldShowSubtitlesHub("subtitles", false)).toBe(false);
    expect(shouldShowSubtitlesHub("translation", true)).toBe(false);
  });
});
