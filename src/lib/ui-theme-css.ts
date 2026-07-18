/** Apply dashboard UI accent palette / font CSS variables to document root. */

export const UI_THEME_STORAGE_KEY = "voicesub.ui.theme";

export function readStoredUiTheme(): "dark" | "light" | null {
  try {
    const value = localStorage.getItem(UI_THEME_STORAGE_KEY);
    if (value === "light" || value === "dark") {
      return value;
    }
  } catch {
    // ignore
  }
  return null;
}

function persistUiTheme(theme: "dark" | "light"): void {
  try {
    localStorage.setItem(UI_THEME_STORAGE_KEY, theme);
  } catch {
    // ignore quota / private mode
  }
}

/** Keep CSS `color-scheme` aligned with `data-ui-theme` (dialogs, form controls). */
export function applyUiColorSchemeToDocument(
  theme: "dark" | "light",
  targetDocument: Document = document,
): void {
  const root = targetDocument?.documentElement;
  if (!root) return;
  root.dataset.uiTheme = theme;
  try {
    root.style.setProperty("color-scheme", theme);
  } catch {
    // ignore
  }
  if (targetDocument === document) {
    persistUiTheme(theme);
  }
}

export function hexToRgbTriplet(hex: string): string {
  const clean = hex.replace("#", "");
  if (clean.length !== 6) return "108 199 255";
  const r = parseInt(clean.slice(0, 2), 16);
  const g = parseInt(clean.slice(2, 4), 16);
  const b = parseInt(clean.slice(4, 6), 16);
  return `${r} ${g} ${b}`;
}

export function applyUiPaletteToDocument(palette: Record<string, string | undefined>): void {
  if (palette.accent) {
    document.documentElement.style.setProperty("--ui-accent-rgb", hexToRgbTriplet(palette.accent));
  }
  if (palette.accent_secondary) {
    document.documentElement.style.setProperty(
      "--ui-accent-secondary-rgb",
      hexToRgbTriplet(palette.accent_secondary),
    );
  }
  if (palette.accent_tertiary) {
    document.documentElement.style.setProperty(
      "--ui-accent-tertiary-rgb",
      hexToRgbTriplet(palette.accent_tertiary),
    );
  }
}

/** Set `--font-ui` from config; empty clears the override so tokens.css default applies. */
export function applyUiFontToDocument(
  fontFamily?: string | null,
  targetDocument: Document = document,
): void {
  const root = targetDocument?.documentElement;
  if (!root) return;
  const value = String(fontFamily || "").trim();
  if (value) {
    root.style.setProperty("--font-ui", value);
  } else {
    root.style.removeProperty("--font-ui");
  }
}
