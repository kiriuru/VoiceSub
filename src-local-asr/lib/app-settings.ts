import { normalizeConfigPayload } from "../../src/lib/config-normalize";
import { apiFetch } from "./loopback-api-client";
import { initLoopbackApiToken } from "../../src/lib/loopback-api";
import { setLocale } from "../../src/lib/i18n";
import type { ConfigPayload, LocaleCode } from "../../src/lib/types";
import { applyUiPaletteToDocument } from "../../src/lib/ui-theme-css";
import { subscribeUiLocaleSync } from "../../src/lib/ui-config-sync";

function applyUiLocaleFromConfig(config: ConfigPayload): void {
  const uiLang = String(config.ui?.language || "")
    .trim()
    .slice(0, 2)
    .toLowerCase();
  if (uiLang === "en" || uiLang === "ru" || uiLang === "ja" || uiLang === "ko" || uiLang === "zh") {
    setLocale(uiLang as LocaleCode);
  }
}

export function applyUiThemeFromConfig(config: ConfigPayload): void {
  const ui = config.ui || {};
  const theme = ui.theme === "light" ? "light" : "dark";
  document.documentElement.dataset.uiTheme = theme;
  if (ui.palette) {
    applyUiPaletteToDocument(ui.palette);
  }
}

export async function bootstrapLocalAsrFromSettings(): Promise<void> {
  await initLoopbackApiToken();
  try {
    const response = await apiFetch("/api/settings/load");
    if (!response.ok) return;
    const body = (await response.json()) as { payload?: ConfigPayload };
    if (!body.payload || typeof body.payload !== "object") return;
    const config = normalizeConfigPayload(body.payload);
    applyUiThemeFromConfig(config);
    applyUiLocaleFromConfig(config);
  } catch {
    // Module still works with browser locale / defaults.
  }
  subscribeUiLocaleSync((code) => {
    if (code === "en" || code === "ru" || code === "ja" || code === "ko" || code === "zh") {
      setLocale(code);
    }
  });
}
