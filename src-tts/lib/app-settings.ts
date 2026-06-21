import { normalizeConfigPayload } from "../../src/lib/config-normalize";
import { apiFetch } from "./loopback-api-client";
import { getLineCards } from "../../src/lib/translation-helpers";
import { setLocale } from "../../src/lib/i18n";
import type { ConfigPayload, LocaleCode } from "../../src/lib/types";
import { applyUiPaletteToDocument } from "../../src/lib/ui-theme-css";
import type { AppSpeechContext, AppTranslationLine } from "./translation-lines";

function applyUiLocaleFromConfig(config: ConfigPayload): void {
  const uiLang = String(config.ui?.language || "")
    .trim()
    .slice(0, 2)
    .toLowerCase();
  if (uiLang === "en" || uiLang === "ru" || uiLang === "ja" || uiLang === "ko" || uiLang === "zh") {
    setLocale(uiLang as LocaleCode);
  }
}

export type SettingsLoadResponse = {
  ok?: boolean;
  payload?: ConfigPayload;
};

function normalizeLang(lang: string | undefined | null): string {
  const trimmed = String(lang || "")
    .trim()
    .toLowerCase();
  if (!trimmed || trimmed === "auto") return "";
  return trimmed.split("-")[0]?.split("_")[0] || "";
}

/** Recognition language used for source TTS (ru from ru-RU, etc.). */
export function resolveRecognitionLang(config: ConfigPayload): string {
  const browser = config.asr?.browser as Record<string, unknown> | undefined;
  const recognition = normalizeLang(String(browser?.recognition_language || ""));
  if (recognition) return recognition;

  const sourceLang = normalizeLang(config.source_lang);
  if (sourceLang) return sourceLang;

  return "en";
}

function toAppTranslationLine(line: {
  slot_id: string;
  enabled?: boolean;
  target_lang?: string;
  label?: string;
}): AppTranslationLine {
  return {
    slot_id: String(line.slot_id || "").trim().toLowerCase(),
    enabled: line.enabled === true,
    target_lang: normalizeLang(line.target_lang) || "en",
    label: line.label,
  };
}

export function readFullLoggingEnabled(raw: ConfigPayload): boolean {
  const config = normalizeConfigPayload(raw);
  return config.logging?.full_enabled === true;
}

export function buildSpeechContextFromConfig(raw: ConfigPayload): AppSpeechContext {
  const config = normalizeConfigPayload(raw);
  const translation = config.translation || {};
  const activeLines = getLineCards(config)
    .filter((line) => line.enabled)
    .map(toAppTranslationLine);

  return {
    translationEnabled: translation.enabled === true,
    sourceLang: resolveRecognitionLang(config),
    lines: activeLines,
  };
}

export function applyUiThemeFromConfig(config: ConfigPayload): void {
  const ui = config.ui || {};
  const theme = ui.theme === "light" ? "light" : "dark";
  document.documentElement.dataset.uiTheme = theme;
  if (ui.palette) {
    applyUiPaletteToDocument(ui.palette);
  }
}

/** Live dashboard theme/locale sync without touching speech/translation settings. */
export function applyDashboardUiPresentation(raw: ConfigPayload): void {
  const config = normalizeConfigPayload(raw);
  applyUiThemeFromConfig(config);
  applyUiLocaleFromConfig(config);
}

export async function fetchSettingsPayload(): Promise<ConfigPayload> {
  const response = await apiFetch("/api/settings/load");
  if (!response.ok) {
    throw new Error(`settings load HTTP ${response.status}`);
  }
  const body = (await response.json()) as SettingsLoadResponse;
  if (!body.payload || typeof body.payload !== "object") {
    throw new Error("settings load: missing payload");
  }
  return body.payload;
}

export async function bootstrapTtsFromSettings(): Promise<AppSpeechContext> {
  const payload = await fetchSettingsPayload();
  applyUiThemeFromConfig(payload);
  applyUiLocaleFromConfig(payload);
  return buildSpeechContextFromConfig(payload);
}
