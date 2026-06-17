import { normalizeConfigPayload } from "../../src/lib/config-normalize";
import { applyUiPaletteToDocument } from "../../src/lib/ui-theme-css";
import { apiFetch } from "./loopback-api-client";

export function applyUiThemeFromConfigPayload(
  configPayload: Record<string, unknown> | null | undefined,
  targetDocument: Document = document,
) {
  const config = normalizeConfigPayload((configPayload || {}) as import("../../src/lib/types").ConfigPayload);
  const root = targetDocument?.documentElement;
  if (!root) return config.ui;

  const theme = config.ui?.theme === "light" ? "light" : "dark";
  root.dataset.uiTheme = theme;
  if (config.ui?.palette) {
    applyUiPaletteToDocument(config.ui.palette);
  }

  try {
    root.style.setProperty("color-scheme", theme);
  } catch {
    // ignore
  }

  return config.ui;
}

export async function autoLoadAndApplyUiTheme(targetDocument: Document = document) {
  try {
    const response = await apiFetch("/api/settings/load");
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
