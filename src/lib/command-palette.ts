import type { NavTarget } from "./navigation";
import { isMoreTab, tabToNavDestination } from "./navigation";

export interface CommandPaletteItem {
  id: string;
  labelKey: string;
  groupKey: string;
  keywords?: string[];
  shortcut?: string;
  run: () => void | Promise<void>;
}

export interface CommandPaletteHandlers {
  navigate: (target: NavTarget) => void;
  start: () => void | Promise<void>;
  stop: () => void | Promise<void>;
  save: () => void | Promise<void>;
  toggleTheme: () => void;
  toggleLayout: () => void;
  exportDiagnostics: () => void | Promise<void>;
  onError?: (message: string) => void;
}

export function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return true;
  return el.isContentEditable;
}

const TAB_COMMANDS: Array<{ id: NavTarget; labelKey: string; keywords: string[] }> = [
  { id: "live", labelKey: "command_palette.tab.live", keywords: ["transcript", "overview", "recognition", "partial"] },
  { id: "translation", labelKey: "command_palette.tab.translation", keywords: ["translate", "deepl", "provider"] },
  { id: "subtitles", labelKey: "command_palette.tab.subtitles", keywords: ["output", "overlay"] },
  { id: "style", labelKey: "command_palette.tab.style", keywords: ["font", "preset"] },
  { id: "theme", labelKey: "command_palette.tab.theme", keywords: ["color", "palette", "dark", "light"] },
  { id: "obs", labelKey: "command_palette.tab.obs", keywords: ["browser", "source", "stream"] },
  { id: "modules", labelKey: "command_palette.tab.modules", keywords: ["tts", "speech", "twitch", "module"] },
  { id: "replacement", labelKey: "command_palette.tab.replacement", keywords: ["word", "replace"] },
  { id: "tools", labelKey: "command_palette.tab.tools", keywords: ["export", "diagnostics", "profile"] },
  { id: "settings", labelKey: "command_palette.tab.settings", keywords: ["config", "asr", "browser"] },
  { id: "help", labelKey: "command_palette.tab.help", keywords: ["docs", "wiki"] },
];

export function buildCommandPaletteItems(handlers: CommandPaletteHandlers): CommandPaletteItem[] {
  const items: CommandPaletteItem[] = [
    {
      id: "start",
      labelKey: "command_palette.start",
      groupKey: "command_palette.group.runtime",
      keywords: ["play", "recognition", "asr"],
      shortcut: "Ctrl+Enter",
      run: handlers.start,
    },
    {
      id: "stop",
      labelKey: "command_palette.stop",
      groupKey: "command_palette.group.runtime",
      keywords: ["pause", "end"],
      run: handlers.stop,
    },
    {
      id: "save",
      labelKey: "command_palette.save",
      groupKey: "command_palette.group.settings",
      keywords: ["persist", "config"],
      shortcut: "Ctrl+S",
      run: handlers.save,
    },
    {
      id: "toggle-theme",
      labelKey: "command_palette.toggle_theme",
      groupKey: "command_palette.group.settings",
      keywords: ["dark", "light", "mode"],
      run: handlers.toggleTheme,
    },
    {
      id: "toggle-layout",
      labelKey: "command_palette.toggle_layout",
      groupKey: "command_palette.group.settings",
      keywords: ["compact", "standard", "sidebar"],
      run: handlers.toggleLayout,
    },
    {
      id: "export-diagnostics",
      labelKey: "command_palette.export_diagnostics",
      groupKey: "command_palette.group.tools",
      keywords: ["zip", "logs", "debug"],
      run: handlers.exportDiagnostics,
    },
  ];

  for (const tab of TAB_COMMANDS) {
    items.push({
      id: `tab-${tab.id}`,
      labelKey: tab.labelKey,
      groupKey: "command_palette.group.navigation",
      keywords: tab.keywords,
      run: () => handlers.navigate(tab.id),
    });
  }

  return items;
}

export function filterCommandItems(
  items: CommandPaletteItem[],
  query: string,
  t: (key: string) => string,
): CommandPaletteItem[] {
  const q = query.trim().toLowerCase();
  if (!q) return items;
  return items.filter((item) => {
    const label = t(item.labelKey).toLowerCase();
    if (label.includes(q) || item.id.includes(q)) return true;
    return (item.keywords || []).some((kw) => kw.includes(q));
  });
}
