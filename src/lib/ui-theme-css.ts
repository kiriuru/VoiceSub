/** Apply dashboard UI accent palette CSS variables to document root. */

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
