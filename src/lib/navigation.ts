import type { TabId } from "./types";

/** M3 primary destinations (3–7 items per Material navigation rail guidance). */
export type NavDestinationId = "live" | "translation" | "subtitles" | "obs" | "modules" | "more";

export type CompactPaneId = NavDestinationId;

/** Command palette and deep links may target a primary destination or a settings tab. */
export type NavTarget = NavDestinationId | TabId;

export const PRIMARY_NAV_DESTINATIONS: NavDestinationId[] = [
  "live",
  "translation",
  "subtitles",
  "obs",
  "modules",
  "more",
];

/** Secondary settings screens opened from the More hub (Android Settings subscreens). */
export const MORE_TAB_IDS: TabId[] = [
  "theme",
  "replacement",
  "tools",
  "settings",
  "help",
];

/** Subtitles hub sub-screens (display + style). */
export const SUBTITLES_TAB_IDS: TabId[] = ["subtitles", "style"];

export function isMoreTab(tab: TabId): boolean {
  return MORE_TAB_IDS.includes(tab);
}

export function isSubtitlesTab(tab: TabId): boolean {
  return SUBTITLES_TAB_IDS.includes(tab);
}

export function tabToNavDestination(tab: TabId): NavDestinationId {
  if (tab === "translation") return "translation";
  if (tab === "subtitles" || tab === "style") return "subtitles";
  if (tab === "obs") return "obs";
  return "more";
}

export function tabTitleKey(tab: TabId): string {
  return `tab.${tab}`;
}

export function navDestinationTitleKey(dest: NavDestinationId): string {
  switch (dest) {
    case "live":
      return "nav.live";
    case "translation":
      return "tab.translation";
    case "subtitles":
      return "nav.subtitles";
    case "obs":
      return "tab.obs";
    case "modules":
      return "nav.modules";
    case "more":
      return "nav.more";
    default:
      return "nav.live";
  }
}

export function shouldShowMoreHub(nav: NavDestinationId, moreHubOpen: boolean): boolean {
  return nav === "more" && moreHubOpen;
}

export function shouldShowSubtitlesHub(nav: NavDestinationId, subtitlesHubOpen: boolean): boolean {
  return nav === "subtitles" && subtitlesHubOpen;
}

export interface SubtitlesSettingsItem {
  id: TabId;
  labelKey: string;
  descriptionKey: string;
}

export const SUBTITLES_SETTINGS_ITEMS: SubtitlesSettingsItem[] = [
  {
    id: "subtitles",
    labelKey: "tab.subtitles",
    descriptionKey: "nav.subtitles.desc.display",
  },
  {
    id: "style",
    labelKey: "tab.style",
    descriptionKey: "nav.subtitles.desc.style",
  },
];

export interface MoreSettingsGroup {
  groupKey: string;
  items: Array<{ id: TabId; labelKey: string; descriptionKey: string }>;
}

export const MORE_SETTINGS_GROUPS: MoreSettingsGroup[] = [
  {
    groupKey: "nav.more.group.appearance",
    items: [{ id: "theme", labelKey: "tab.theme", descriptionKey: "nav.more.desc.theme" }],
  },
  {
    groupKey: "nav.more.group.text",
    items: [
      {
        id: "replacement",
        labelKey: "tab.replacement",
        descriptionKey: "nav.more.desc.replacement",
      },
    ],
  },
  {
    groupKey: "nav.more.group.system",
    items: [
      { id: "settings", labelKey: "tab.settings", descriptionKey: "nav.more.desc.settings" },
      { id: "tools", labelKey: "tab.tools", descriptionKey: "nav.more.desc.tools" },
    ],
  },
  {
    groupKey: "nav.more.group.about",
    items: [{ id: "help", labelKey: "tab.help", descriptionKey: "nav.more.desc.help" }],
  },
];
