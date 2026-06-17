import { describe, expect, it } from "vitest";
import {
  OBS_PANEL_SECTIONS,
  SUBTITLES_PANEL_SECTIONS,
  TRANSLATION_PANEL_SECTIONS,
  panelSectionIdForTab,
  panelSectionTabForId,
} from "./panel-sections";

describe("panel-sections", () => {
  it("defines translation anchors for top navigation", () => {
    expect(TRANSLATION_PANEL_SECTIONS.map((s) => s.id)).toEqual([
      "translation-section-lines",
      "translation-section-provider",
      "translation-section-results",
    ]);
  });

  it("defines OBS section anchors", () => {
    expect(OBS_PANEL_SECTIONS.map((s) => s.id)).toEqual([
      "obs-section-overlay",
      "obs-section-captions",
      "obs-section-status",
    ]);
  });

  it("defines subtitles anchors with tab links", () => {
    expect(SUBTITLES_PANEL_SECTIONS.map((s) => s.id)).toEqual([
      "subtitles-section-display",
      "subtitles-section-style",
    ]);
    expect(panelSectionIdForTab(SUBTITLES_PANEL_SECTIONS, "style")).toBe("subtitles-section-style");
    expect(panelSectionTabForId(SUBTITLES_PANEL_SECTIONS, "subtitles-section-display")).toBe("subtitles");
  });

  it("maps tab-linked sections when tabId is set", () => {
    expect(panelSectionTabForId(TRANSLATION_PANEL_SECTIONS, "translation-section-lines")).toBeUndefined();
    expect(panelSectionIdForTab(TRANSLATION_PANEL_SECTIONS, "translation")).toBeUndefined();
  });
});
