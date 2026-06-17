import type { TabId } from "./types";

export interface PanelSection {
  id: string;
  labelKey: string;
  /** When set, scroll-spy can sync `activeTab` while scrolling. */
  tabId?: TabId;
}

export function panelSectionIdForTab(sections: PanelSection[], tab: TabId): string | undefined {
  return sections.find((section) => section.tabId === tab)?.id;
}

export function panelSectionTabForId(sections: PanelSection[], id: string): TabId | undefined {
  return sections.find((section) => section.id === id)?.tabId;
}

export const TRANSLATION_PANEL_SECTIONS: PanelSection[] = [
  { id: "translation-section-lines", labelKey: "translation.lines.title" },
  { id: "translation-section-provider", labelKey: "translation.provider_settings.title" },
  { id: "translation-section-results", labelKey: "nav.translation.section.results" },
];

export const SUBTITLES_PANEL_SECTIONS: PanelSection[] = [
  { id: "subtitles-section-display", labelKey: "tab.subtitles", tabId: "subtitles" },
  { id: "subtitles-section-style", labelKey: "tab.style", tabId: "style" },
];

/** Anchor ids in ObsPanel (section nav removed; ids kept for deep links / tests). */
export const OBS_PANEL_SECTIONS: PanelSection[] = [
  { id: "obs-section-overlay", labelKey: "nav.obs.section.overlay" },
  { id: "obs-section-captions", labelKey: "nav.obs.section.captions" },
  { id: "obs-section-status", labelKey: "nav.obs.section.status" },
];
