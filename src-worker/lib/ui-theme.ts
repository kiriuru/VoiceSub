function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Number(value) || 0));
}

function hexToRgbTriplet(hex: string): string | null {
  const raw = String(hex || "")
    .trim()
    .replace("#", "");
  if (/^[0-9a-fA-F]{3}$/.test(raw)) {
    const r = Number.parseInt(raw[0] + raw[0], 16);
    const g = Number.parseInt(raw[1] + raw[1], 16);
    const b = Number.parseInt(raw[2] + raw[2], 16);
    return `${clampByte(r)} ${clampByte(g)} ${clampByte(b)}`;
  }
  if (/^[0-9a-fA-F]{6}$/.test(raw)) {
    const r = Number.parseInt(raw.slice(0, 2), 16);
    const g = Number.parseInt(raw.slice(2, 4), 16);
    const b = Number.parseInt(raw.slice(4, 6), 16);
    return `${clampByte(r)} ${clampByte(g)} ${clampByte(b)}`;
  }
  return null;
}

function safeThemeConfig(configPayload: Record<string, unknown> | null | undefined) {
  const ui =
    configPayload?.ui && typeof configPayload.ui === "object"
      ? (configPayload.ui as Record<string, unknown>)
      : {};
  const palette =
    ui.palette && typeof ui.palette === "object" ? (ui.palette as Record<string, string>) : {};
  return {
    theme: ui.theme === "light" ? ("light" as const) : ("dark" as const),
    palette: {
      accent: String(palette.accent || "#6cc7ff"),
      accent_secondary: String(palette.accent_secondary || "#ff6ce6"),
      accent_tertiary: String(palette.accent_tertiary || "#7ce3ad"),
    },
  };
}

export function applyUiThemeFromConfigPayload(
  configPayload: Record<string, unknown> | null | undefined,
  targetDocument: Document = document
) {
  const resolved = safeThemeConfig(configPayload || {});
  const root = targetDocument?.documentElement;
  if (!root) {
    return resolved;
  }

  root.dataset.uiTheme = resolved.theme;

  const accentRgb = hexToRgbTriplet(resolved.palette.accent) || "108 199 255";
  const accent2Rgb = hexToRgbTriplet(resolved.palette.accent_secondary) || "255 108 230";
  const accent3Rgb = hexToRgbTriplet(resolved.palette.accent_tertiary) || "124 227 173";

  root.style.setProperty("--ui-accent", resolved.palette.accent);
  root.style.setProperty("--ui-accent-secondary", resolved.palette.accent_secondary);
  root.style.setProperty("--ui-accent-tertiary", resolved.palette.accent_tertiary);
  root.style.setProperty("--ui-accent-rgb", accentRgb);
  root.style.setProperty("--ui-accent-secondary-rgb", accent2Rgb);
  root.style.setProperty("--ui-accent-tertiary-rgb", accent3Rgb);
  root.style.setProperty("--bg", resolved.theme === "light" ? "#f6f8ff" : "#09111b");
  root.style.setProperty("--bg-top", resolved.theme === "light" ? "#ffffff" : "#0b1422");
  root.style.setProperty(
    "--panel",
    resolved.theme === "light" ? "rgba(255, 255, 255, 0.82)" : "rgba(14, 24, 40, 0.84)"
  );
  root.style.setProperty(
    "--panel-strong",
    resolved.theme === "light" ? "rgba(255, 255, 255, 0.92)" : "rgba(7, 14, 24, 0.82)"
  );
  root.style.setProperty(
    "--line",
    resolved.theme === "light" ? "rgba(24, 44, 82, 0.14)" : "rgba(160, 193, 255, 0.14)"
  );
  root.style.setProperty(
    "--line-strong",
    resolved.theme === "light" ? "rgba(24, 44, 82, 0.22)" : "rgba(160, 193, 255, 0.24)"
  );
  root.style.setProperty("--text", resolved.theme === "light" ? "#0b1422" : "#f5f7fb");
  root.style.setProperty("--muted", resolved.theme === "light" ? "#3a4a66" : "#9cb0d0");
  root.style.setProperty("--accent", resolved.palette.accent);
  root.style.setProperty("--accent-strong", resolved.palette.accent_secondary);

  try {
    root.style.setProperty("color-scheme", resolved.theme);
  } catch {
    // ignore
  }

  return resolved;
}

export async function autoLoadAndApplyUiTheme(targetDocument: Document = document) {
  try {
    const response = await fetch("/api/settings/load");
    const data = await response.json().catch(() => null);
    const payload = data?.payload || null;
    if (payload && typeof payload === "object") {
      return applyUiThemeFromConfigPayload(payload as Record<string, unknown>, targetDocument);
    }
  } catch {
    // best-effort
  }
  return applyUiThemeFromConfigPayload({}, targetDocument);
}
