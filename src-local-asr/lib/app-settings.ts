import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { normalizeConfigPayload } from "../../src/lib/config-normalize";
import { apiFetch } from "./loopback-api-client";
import { initLoopbackApiToken } from "../../src/lib/loopback-api";
import { setLocale, getLocale } from "../../src/lib/i18n";
import { normalizeRuntimeEventMessage } from "../../src/lib/runtime-events";
import type { ConfigPayload, LocaleCode } from "../../src/lib/types";
import {
  applyUiColorSchemeToDocument,
  applyUiFontToDocument,
  applyUiPaletteToDocument,
} from "../../src/lib/ui-theme-css";
import {
  subscribeUiConfigSync,
  subscribeUiLocaleSync,
  UI_CONFIG_WS_EVENT,
  uiConfigFromWsPayload,
} from "../../src/lib/ui-config-sync";

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
  applyUiColorSchemeToDocument(theme);
  if (ui.palette) {
    applyUiPaletteToDocument(ui.palette);
  }
  applyUiFontToDocument(ui.font_family);
}

/** Live dashboard theme/locale/font sync without touching ASR settings. */
export function applyDashboardUiPresentation(raw: ConfigPayload): void {
  const config = normalizeConfigPayload(raw);
  applyUiThemeFromConfig(config);
  applyUiLocaleFromConfig(config);
}

async function subscribeUiConfigRuntimeEvent(
  handler: (payload: ConfigPayload) => void,
): Promise<() => void> {
  try {
    const unlisten: UnlistenFn = await listen("runtime-event", (event) => {
      const message = normalizeRuntimeEventMessage(event.payload);
      if (!message || message.type !== UI_CONFIG_WS_EVENT) return;
      const partial = uiConfigFromWsPayload(message.payload);
      if (partial) handler(partial);
    });
    return unlisten;
  } catch {
    return () => {};
  }
}

export async function bootstrapLocalAsrFromSettings(): Promise<() => void> {
  await initLoopbackApiToken();
  try {
    const response = await apiFetch("/api/settings/load");
    if (response.ok) {
      const body = (await response.json()) as { payload?: ConfigPayload };
      if (body.payload && typeof body.payload === "object") {
        const config = normalizeConfigPayload(body.payload);
        applyUiThemeFromConfig(config);
        applyUiLocaleFromConfig(config);
      }
    }
  } catch {
    // Module still works with browser locale / defaults.
  }
  const unsubs = [
    // Tauri IPC already delivers ui_config_sync; skip /ws/events so overlay/runtime
    // frames do not flood this module webview (CPU + retained WS buffers).
    subscribeUiConfigSync(
      (partial) => {
        applyDashboardUiPresentation(partial);
      },
      { enableWebSocket: false },
    ),
    subscribeUiLocaleSync((code) => {
      if (getLocale() === code) return;
      if (code === "en" || code === "ru" || code === "ja" || code === "ko" || code === "zh") {
        setLocale(code);
      }
    }),
    await subscribeUiConfigRuntimeEvent((partial) => {
      applyDashboardUiPresentation(partial);
    }),
  ];
  return () => {
    for (const unsub of unsubs) unsub();
  };
}
