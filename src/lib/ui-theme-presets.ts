export interface UiThemePreset {
  id: string;
  labelKey: string;
  theme: "dark" | "light" | null;
  palette: Record<string, string> | null;
}

export const UI_THEME_PRESETS: UiThemePreset[] = [
  { id: "custom", labelKey: "style.ui_theme.custom", theme: null, palette: null },
  {
    id: "ocean",
    labelKey: "style.ui_theme.ocean",
    theme: "dark",
    palette: { accent: "#6cc7ff", accent_secondary: "#4fe3ff", accent_tertiary: "#7ce3ad" },
  },
  {
    id: "neon",
    labelKey: "style.ui_theme.neon",
    theme: "dark",
    palette: { accent: "#8bddff", accent_secondary: "#ff6ce6", accent_tertiary: "#ffd166" },
  },
  {
    id: "sunset",
    labelKey: "style.ui_theme.sunset",
    theme: "dark",
    palette: { accent: "#ffb703", accent_secondary: "#ff5c7a", accent_tertiary: "#6cc7ff" },
  },
  {
    id: "paper",
    labelKey: "style.ui_theme.paper",
    theme: "light",
    palette: { accent: "#2563eb", accent_secondary: "#db2777", accent_tertiary: "#059669" },
  },
  {
    id: "aurora",
    labelKey: "style.ui_theme.aurora",
    theme: "dark",
    palette: { accent: "#7C6AFF", accent_secondary: "#FF6B9D", accent_tertiary: "#5EC4A0" },
  },
  {
    id: "anime",
    labelKey: "style.ui_theme.anime",
    theme: "dark",
    palette: { accent: "#FF4FB4", accent_secondary: "#1FD6FF", accent_tertiary: "#FFC95E" },
  },
];
